//! Q78: explicit proofs preserve the checked work evaluated by their terms.
//! Bounds use exact integers; no solver is needed or trusted by this pass.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Range(i128, i128);
impl Range {
    fn intersection(self, other: Self) -> Self {
        Self(self.0.max(other.0), self.1.min(other.1))
    }
    fn union(self, other: Self) -> Self {
        Self(self.0.min(other.0), self.1.max(other.1))
    }
    fn point(n: i128) -> Self {
        Self(n, n)
    }
}

fn machine(ty: &Type) -> Option<Range> {
    match ty {
        Type::Named(n, a) if a.is_empty() => match n.as_str() {
            "Int" => Some(Range(i64::MIN.into(), i64::MAX.into())),
            "Nat" => Some(Range(0, u64::MAX.into())),
            "Bool" => Some(Range(0, 1)),
            _ => None,
        },
        _ => None,
    }
}

struct Verifier<'a, 'p> {
    kernel: &'a Kernel<'p>,
    ctx: Context,
    known: Vec<Term>,
    ranges: Vec<(Term, Range)>,
    facts: Vec<Prop>,
    /// Theorems being applied, so a conditional lemma cannot justify its own
    /// precondition by applying itself again.
    active: BTreeSet<String>,
    fuel: usize,
    /// Numbers binders renamed during case analysis, so no two arms share a name.
    next: usize,
    span: Span,
}
impl<'a, 'p> Verifier<'a, 'p> {
    fn new(kernel: &'a Kernel<'p>, ctx: &Context, span: Span) -> Self {
        let mut verifier = Self {
            kernel,
            ctx: ctx.clone(),
            known: vec![],
            ranges: vec![],
            facts: vec![],
            active: BTreeSet::new(),
            fuel: 8192,
            next: 0,
            span,
        };
        // A constructor supplied by a case split already exists as a machine
        // value. Thus Succ(k) in that arm also establishes k < UINT64_MAX.
        for (_, term, ty) in &ctx.scope.terms {
            verifier.remember(term);
            if let Some(range) = machine(ty) {
                verifier.narrow(term, range, 0);
            }
        }
        // Each assumption counts in its written form and its normal form, so a
        // bound on add(Succ(k), b) also bounds Succ(add(k, b)) and add(k, b).
        for (_, prop) in &ctx.hyps {
            let normal = kernel.reducer().nf_prop(prop);
            for fact in [prop.clone(), normal] {
                if !verifier.facts.contains(&fact) {
                    verifier.remember_prop(&fact);
                    verifier.facts.push(fact);
                }
            }
        }
        verifier.refine();
        verifier
    }

    fn error<T>(&self, term: &Term) -> Found<T> {
        fail(
            self.span,
            "E_PROOF_UNDEFINED",
            format!(
                "The proof has not established that `{term}` can be evaluated without arithmetic failure."
            ),
            "Add bounds before using this expression, split cases, or cite a checked lemma that establishes its value.",
        )
    }

    fn remember(&mut self, term: &Term) {
        if !self.known.contains(term) {
            self.known.push(term.clone());
        }
        match term {
            Term::Ctor { args, .. } | Term::Call { args, .. } => {
                args.iter().for_each(|t| self.remember(t));
            }
            Term::Op(op, args) if !matches!(op.as_str(), "&&" | "||") => {
                args.iter().for_each(|t| self.remember(t));
            }
            Term::Op(_, args) => self.remember(&args[0]),
            Term::If(c, _, _) | Term::Match(c, _) => self.remember(c),
            Term::FamilyUnion { set, .. } | Term::FamilyInter { set, .. } => self.remember(set),
            Term::Finite(elements) => elements.iter().for_each(|element| self.remember(element)),
            Term::Builder { .. } => {}
            _ => {}
        }
    }

    fn remember_prop(&mut self, prop: &Prop) {
        match prop {
            Prop::Eq(a, b) => {
                self.remember(a);
                self.remember(b);
            }
            Prop::Holds(t) => self.remember(t),
            Prop::Mem(a, b) => {
                self.remember(a);
                self.remember(b);
            }
            Prop::Subset { left, right, .. } => {
                self.remember(left);
                self.remember(right);
            }
            Prop::And(a, b) | Prop::Or(a, b) => {
                self.remember_prop(a);
                self.remember_prop(b);
            }
            Prop::Not(p) | Prop::Forall { body: p, .. } | Prop::Exists { body: p, .. } => {
                self.remember_prop(p)
            }
            Prop::Implies(a, b) => {
                self.remember_prop(a);
                self.remember_prop(b);
            }
            Prop::Iff(a, b) => {
                self.remember_prop(a);
                self.remember_prop(b);
            }
            Prop::True | Prop::False | Prop::Atom(_) => {}
        }
    }

    fn range(&self, term: &Term, depth: usize) -> Option<Range> {
        if depth > 96 {
            return None;
        }
        let r = |t| self.range(t, depth + 1);
        let base = match term {
            Term::Int(n) => Some(Range::point((*n).into())),
            Term::Nat(n) => Some(Range::point((*n).into())),
            Term::Bool(b) => Some(Range::point(i128::from(*b))),
            Term::Ctor { ty, case, args } if ty == "Nat" && case == "Succ" => {
                r(&args[0]).map(|Range(lo, hi)| Range(lo + 1, hi + 1))
            }
            Term::Op(op, args) => match (op.as_str(), args.as_slice()) {
                ("seq", [_, b]) => r(b),
                ("-", [a]) => r(a).map(|Range(lo, hi)| Range(-hi, -lo)),
                ("+" | "-" | "*", [a, b]) => {
                    let Range(al, ah) = r(a)?;
                    let Range(bl, bh) = r(b)?;
                    match op.as_str() {
                        "+" => Some(Range(al.checked_add(bl)?, ah.checked_add(bh)?)),
                        "-" if a == b => Some(Range::point(0)),
                        "-" => Some(Range(al.checked_sub(bh)?, ah.checked_sub(bl)?)),
                        _ => {
                            let products = [
                                al.checked_mul(bl)?,
                                al.checked_mul(bh)?,
                                ah.checked_mul(bl)?,
                                ah.checked_mul(bh)?,
                            ];
                            Some(Range(*products.iter().min()?, *products.iter().max()?))
                        }
                    }
                }
                _ => machine(&self.kernel.term_type(term, &self.ctx)),
            },
            Term::If(c, a, b) => match self.truth(c) {
                Some(true) => r(a),
                Some(false) => r(b),
                None => Some(r(a)?.union(r(b)?)),
            },
            _ => machine(&self.kernel.term_type(term, &self.ctx)),
        };
        let constraint = self.ranges.iter().find(|(t, _)| t == term).map(|(_, r)| *r);
        match (base, constraint) {
            (Some(a), Some(b)) => Some(a.intersection(b)),
            (a, b) => a.or(b),
        }
    }

    fn narrow(&mut self, term: &Term, range: Range, depth: usize) {
        if depth > 96 {
            return;
        }
        if let Some((_, old)) = self.ranges.iter_mut().find(|(t, _)| t == term) {
            *old = old.intersection(range);
        } else {
            self.ranges.push((term.clone(), range));
        }
        // Constraints on an existing sum or constructor constrain its inputs.
        // This is used for the witnesses introduced by case splitting.
        match term {
            Term::Ctor { ty, case, args } if ty == "Nat" && case == "Succ" => {
                self.narrow(&args[0], Range(range.0 - 1, range.1 - 1), depth + 1);
            }
            Term::Op(op, args) if args.len() == 1 && op == "-" => {
                self.narrow(&args[0], Range(-range.1, -range.0), depth + 1);
            }
            _ => {}
        }
    }

    fn compare(&mut self, op: &str, a: &Term, b: &Term) {
        let (Some(ar), Some(br)) = (self.range(a, 0), self.range(b, 0)) else {
            return;
        };
        match op {
            "==" => {
                self.narrow(a, br, 0);
                self.narrow(b, ar, 0);
            }
            "<" | "<=" => {
                let step = i128::from(op == "<");
                self.narrow(a, Range(ar.0, br.1.saturating_sub(step)), 0);
                self.narrow(b, Range(ar.0.saturating_add(step), br.1), 0);
            }
            ">" => self.compare("<", b, a),
            ">=" => self.compare("<=", b, a),
            "!=" if br.0 == br.1 => {
                if ar.0 == br.0 {
                    self.narrow(a, Range(ar.0 + 1, ar.1), 0);
                }
                if ar.1 == br.0 {
                    self.narrow(a, Range(ar.0, ar.1 - 1), 0);
                }
            }
            "!=" if ar.0 == ar.1 => self.compare("!=", b, a),
            _ => {}
        }
    }

    fn fact(&mut self, prop: &Prop, positive: bool) {
        match prop {
            Prop::Eq(a, b) if positive => self.compare("==", a, b),
            Prop::Not(p) => self.fact(p, !positive),
            Prop::And(a, b) if positive => {
                self.fact(a, true);
                self.fact(b, true);
            }
            Prop::Or(a, b) if !positive => {
                self.fact(a, false);
                self.fact(b, false);
            }
            Prop::Holds(Term::Op(op, args)) if args.len() == 2 => {
                let effective = if positive {
                    op.as_str()
                } else {
                    match op.as_str() {
                        "<" => ">=",
                        "<=" => ">",
                        ">" => "<=",
                        ">=" => "<",
                        "==" => "!=",
                        "!=" => "==",
                        _ => "",
                    }
                };
                if positive && op == "&&" || !positive && op == "||" {
                    self.fact(&Prop::Holds(args[0].clone()), positive);
                    self.fact(&Prop::Holds(args[1].clone()), positive);
                } else {
                    self.compare(effective, &args[0], &args[1]);
                }
            }
            Prop::Holds(Term::Op(op, args)) if op == "!" => {
                self.fact(&Prop::Holds(args[0].clone()), !positive)
            }
            Prop::Holds(t) => self.narrow(t, Range::point(i128::from(positive)), 0),
            _ => {}
        }
    }

    fn refine(&mut self) {
        // Bound propagation is deliberately finite and conservative. A harder
        // relation needs case splitting or an explicit smt proof.
        for _ in 0..8 {
            let previous = self.ranges.clone();
            for fact in self.facts.clone() {
                self.fact(&fact, true);
            }
            if self.ranges == previous {
                break;
            }
        }
    }

    fn truth(&self, term: &Term) -> Option<bool> {
        if let Some((_, Range(lo, hi))) = self.ranges.iter().find(|(t, _)| t == term)
            && lo == hi
            && (0..=1).contains(lo)
        {
            return Some(*lo == 1);
        }
        match term {
            Term::Bool(b) => Some(*b),
            Term::Op(op, args) if op == "!" => self.truth(&args[0]).map(|b| !b),
            Term::Op(op, args)
                if args.len() == 2
                    && matches!(op.as_str(), "<" | "<=" | ">" | ">=" | "==" | "!=") =>
            {
                let (Range(al, ah), Range(bl, bh)) =
                    (self.range(&args[0], 0)?, self.range(&args[1], 0)?);
                match op.as_str() {
                    "<" if ah < bl => Some(true),
                    "<" if al >= bh => Some(false),
                    "<=" if ah <= bl => Some(true),
                    "<=" if al > bh => Some(false),
                    ">" if al > bh => Some(true),
                    ">" if ah <= bl => Some(false),
                    ">=" if al >= bh => Some(true),
                    ">=" if ah < bl => Some(false),
                    "==" if al == ah && al == bl && bl == bh => Some(true),
                    "==" if ah < bl || bh < al => Some(false),
                    "!=" if ah < bl || bh < al => Some(true),
                    "!=" if al == ah && al == bl && bl == bh => Some(false),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn branch(&mut self, condition: &Term, positive: bool, body: &Term, depth: usize) -> Found<()> {
        if self.truth(condition) == Some(!positive) {
            return Ok(());
        }
        let mut ctx = self.ctx.clone();
        let fact = Prop::Holds(condition.clone());
        ctx.hyps.push((
            String::new(),
            if positive {
                fact
            } else {
                Prop::Not(Box::new(fact))
            },
        ));
        let mut branch = Verifier::new(self.kernel, &ctx, self.span);
        branch.fuel = self.fuel;
        branch.next = self.next;
        branch.active = self.active.clone();
        branch.term(body, depth)?;
        self.fuel = branch.fuel;
        self.next = branch.next;
        Ok(())
    }

    fn established(&self, prop: &Prop) -> bool {
        if self.kernel.reducer().decide(prop) == Some(true) {
            return true;
        }
        let normal = self.kernel.reducer().nf_prop(prop);
        if self
            .facts
            .iter()
            .any(|p| self.kernel.reducer().nf_prop(p) == normal)
        {
            return true;
        }
        match prop {
            Prop::Holds(t) => self.truth(t) == Some(true),
            Prop::And(a, b) => self.established(a) && self.established(b),
            Prop::Or(a, b) => self.established(a) || self.established(b),
            _ => false,
        }
    }

    fn lemma(&mut self, term: &Term, depth: usize) -> bool {
        for (name, theorem) in &self.kernel.theorems {
            if theorem.admitted || self.active.contains(name) {
                continue;
            }
            let Prop::Eq(left, right) = &theorem.statement else {
                continue;
            };
            for pattern in [left, right] {
                if !matches!(
                    pattern,
                    Term::Call { .. } | Term::Ctor { .. } | Term::Op(..)
                ) {
                    continue;
                }
                let mut map = BTreeMap::new();
                if !self
                    .kernel
                    .simp_match(pattern, term, &theorem.params, &self.ctx, &mut map)
                    || theorem
                        .params
                        .iter()
                        .any(|(name, _)| !map.contains_key(name))
                {
                    continue;
                }
                if !self.kernel.rewrite_bounds(theorem, &map, &self.ctx) {
                    continue;
                }
                if theorem
                    .params
                    .iter()
                    .any(|(name, _)| self.term(&map[name], depth + 1).is_err())
                {
                    continue;
                }
                let requires = theorem
                    .requires
                    .iter()
                    .map(|(_, p)| p.map(&mut |t| t.subst(&map, &mut 0)))
                    .collect::<Vec<_>>();
                self.active.insert(name.clone());
                let requires_ok = requires
                    .iter()
                    .all(|p| self.prop(p).is_ok() && self.established(p));
                self.active.remove(name);
                if !requires_ok {
                    continue;
                }
                let fact = theorem.statement.map(&mut |t| t.subst(&map, &mut 0));
                self.remember_prop(&fact);
                self.facts.push(fact);
                self.refine();
                return true;
            }
        }
        false
    }

    fn value(&self, term: &Term) -> Term {
        if let Some(Range(lo, hi)) = self.range(term, 0)
            && lo == hi
        {
            match self.kernel.term_type(term, &self.ctx) {
                Type::Named(n, _) if n == "Int" => {
                    if let Ok(n) = i64::try_from(lo) {
                        return Term::Int(n);
                    }
                }
                Type::Named(n, _) if n == "Nat" => {
                    if let Ok(n) = u64::try_from(lo) {
                        return Term::Nat(n);
                    }
                }
                _ => {}
            }
        }
        self.kernel.reducer().whnf(term)
    }

    fn term(&mut self, term: &Term, depth: usize) -> Found<()> {
        if self.fuel == 0 || depth >= 96 {
            return self.error(term);
        }
        self.fuel -= 1;
        if self.known.contains(term)
            || self.ranges.iter().any(|(_, r)| r.0 > r.1)
            || self.facts.contains(&Prop::False)
        {
            return Ok(());
        }
        if matches!(term, Term::Call { .. }) && self.lemma(term, depth) {
            return Ok(());
        }
        match term {
            Term::Var(_) | Term::Int(_) | Term::Nat(_) | Term::Bool(_) => {}
            Term::Lam(_, body) => self.term(body, depth + 1)?,
            Term::Prop(_) => {}
            Term::App(function, args) => {
                self.term(function, depth + 1)?;
                for arg in args {
                    self.term(arg, depth + 1)?;
                }
            }
            Term::Opaque { .. } => return self.error(term),
            Term::Call {
                function,
                args,
                dispatch,
                ..
            } => {
                for arg in args {
                    self.term(arg, depth + 1)?;
                }
                let total = if self.kernel.runtime_bounds.get() {
                    &self.kernel.total
                } else {
                    &self.kernel.total_math
                };
                if total.contains(function) {
                    self.remember(term);
                    return Ok(());
                }
                let Some(def) = self.kernel.functions.get(function) else {
                    return self.error(term);
                };
                let map = def
                    .params
                    .iter()
                    .cloned()
                    .zip(args.iter().cloned())
                    .collect();
                self.preconditions(term, &def.preconditions, &map)?;
                self.requirements(term, &def.requires, &map, depth)?;
                let mut body = def.body.subst(&map, &mut 0);
                if let Some(receiver) = dispatch
                    && trait_method_default(function, &self.kernel.checked.environment.definitions)
                {
                    body = specialize_trait_calls(
                        &body,
                        receiver,
                        args.first(),
                        &self.kernel.functions,
                        &self.kernel.checked.environment.definitions,
                    );
                }
                self.term(&body, depth + 1)?;
                if let Some(range) = self.range(&body, 0) {
                    self.narrow(term, range, 0);
                }
            }
            Term::If(c, a, b) => {
                self.term(c, depth + 1)?;
                self.branch(c, true, a, depth + 1)?;
                self.branch(c, false, b, depth + 1)?;
            }
            Term::Match(value, arms) => {
                self.term(value, depth + 1)?;
                let mut reducer = self.kernel.reducer();
                let value = self.value(value);
                for (index, (pat, body)) in arms.iter().enumerate() {
                    match reducer.bind(pat, &value) {
                        Some(Some(map)) => {
                            let body = body.subst(&map, &mut 0);
                            self.term(&body, depth + 1)?;
                            if let Some(range) = self.range(&body, 0) {
                                self.narrow(term, range, 0);
                            }
                            self.remember(term);
                            return Ok(());
                        }
                        Some(None) => {}
                        // A stuck scrutinee may match any later arm; check each one.
                        None => {
                            self.cases(term, &value, &arms[index..], depth)?;
                            self.remember(term);
                            return Ok(());
                        }
                    }
                }
                return self.error(term);
            }
            Term::Op(op, args) if matches!(op.as_str(), "&&" | "||") => {
                self.term(&args[0], depth + 1)?;
                self.branch(&args[0], op == "&&", &args[1], depth + 1)?;
            }
            Term::FamilyUnion { set, .. } | Term::FamilyInter { set, .. } => {
                self.term(set, depth + 1)?;
            }
            Term::Finite(elements) => {
                for element in elements {
                    self.term(element, depth + 1)?;
                }
            }
            Term::Builder { body, .. } => {
                self.prop(body)?;
            }
            Term::Ctor { args, .. } | Term::Op(_, args) => {
                for arg in args {
                    self.term(arg, depth + 1)?;
                }
                let arithmetic = matches!(term, Term::Ctor { ty, case, .. } if ty == "Nat" && case == "Succ")
                    || matches!(term, Term::Op(op, _) if matches!(op.as_str(), "+" | "-" | "*"));
                // In a proof Int is the integers and Nat the naturals, so only a Nat
                // subtraction below zero can fail (REVIEW 2.8).
                let mathematical = !self.kernel.runtime_bounds.get();
                let ty = self.kernel.term_type(term, &self.ctx);
                let partial = !mathematical
                    || matches!(&ty, Type::Named(n, a) if n == "Nat" && a.is_empty())
                        && matches!(term, Term::Op(op, args) if op == "-" && args.len() == 2);
                if arithmetic && partial {
                    let Some(actual) = self.range(term, 0) else {
                        return self.error(term);
                    };
                    let Some(allowed) = machine(&ty) else {
                        return self.error(term);
                    };
                    if actual.0 < allowed.0 || (!mathematical && actual.1 > allowed.1) {
                        return self.error(term);
                    }
                }
            }
        }
        self.remember(term);
        Ok(())
    }

    /// A call to a function with run-time `requires` conditions stands for its success
    /// value only where the proof establishes them (Q80). They then join the facts.
    fn preconditions(
        &mut self,
        call: &Term,
        preconditions: &[Prop],
        map: &BTreeMap<String, Term>,
    ) -> Found<()> {
        for condition in preconditions {
            let needed = condition.map(&mut |t| t.subst(map, &mut 0));
            if self.prop(&needed).is_err() || !self.established(&needed) {
                return fail(
                    self.span,
                    "E_PROOF_PRECONDITION",
                    format!("`{call}` requires `{needed}`, and the proof has not established it."),
                    "Add it as a requires assumption, split cases so it holds, or cite a lemma that gives it.",
                );
            }
            self.remember_prop(&needed);
            self.facts.push(needed);
        }
        if !preconditions.is_empty() {
            self.refine();
        }
        Ok(())
    }

    /// A call to a function that requires theorems is defined where each theorem's
    /// assumptions hold at the call's arguments (Q70). Each conclusion then joins the facts.
    fn requirements(
        &mut self,
        call: &Term,
        requires: &[(DeclarationId, Vec<Term>)],
        map: &BTreeMap<String, Term>,
        depth: usize,
    ) -> Found<()> {
        for (id, arguments) in requires {
            let Some(exported) = self.kernel.exported.get(id) else {
                return fail(
                    self.span,
                    "E_PROOF_UNCHECKED",
                    format!("`{call}` requires a theorem that has not checked yet."),
                    "Check the required theorem above this one.",
                );
            };
            let theorem = exported.theorem.clone();
            let instance = theorem
                .params
                .iter()
                .map(|(name, _)| name.clone())
                .zip(arguments.iter().map(|t| t.subst(map, &mut 0)))
                .collect::<BTreeMap<_, _>>();
            for argument in instance.values() {
                self.term(argument, depth + 1)?;
            }
            if !self.kernel.rewrite_bounds(&theorem, &instance, &self.ctx) {
                return fail(
                    self.span,
                    "E_PROOF_BOUND",
                    format!(
                        "Unfolding `{call}` requires type bounds that are not established here."
                    ),
                    "Add the needed trait bounds to the theorem.",
                );
            }
            for (name, needed) in &theorem.requires {
                let needed = needed.map(&mut |t| t.subst(&instance, &mut 0));
                if self.prop(&needed).is_err() || !self.established(&needed) {
                    return fail(
                        self.span,
                        "E_PROOF_ASSUMPTION",
                        format!(
                            "Unfolding `{call}` needs `{needed}` (assumption `{name}` of a theorem it requires), and nothing here establishes it."
                        ),
                        "Add it as a requires assumption, or split cases so it holds.",
                    );
                }
            }
            let fact = theorem.statement.map(&mut |t| t.subst(&instance, &mut 0));
            self.remember_prop(&fact);
            self.facts.push(fact);
        }
        if !requires.is_empty() {
            self.refine();
        }
        Ok(())
    }

    /// A match stuck on an unknown value is defined when its remaining arms cover
    /// every case and each arm's body is defined, given that the value has that
    /// arm's shape. Binders get fresh names in each arm.
    fn cases(
        &mut self,
        term: &Term,
        value: &Term,
        arms: &[(Pat, Term)],
        depth: usize,
    ) -> Found<()> {
        let ty = self.kernel.term_type(value, &self.ctx);
        let mut expanded = vec![];
        for (pat, body) in arms {
            expand_alternatives(pat, body, &mut expanded);
        }
        let arms = expanded.as_slice();
        if !self.exhaustive(&ty, arms) {
            return self.error(term);
        }
        for (pat, body) in arms {
            let mut bound = BTreeSet::new();
            pat.binders(&mut bound);
            let renames = bound
                .iter()
                .map(|b| {
                    self.next += 1;
                    (b.clone(), format!("{}~{}", shown(b), self.next))
                })
                .collect::<BTreeMap<_, _>>();
            let pat = pat.rename(&renames);
            let vars = renames
                .iter()
                .map(|(old, new)| (old.clone(), Term::Var(new.clone())))
                .collect();
            let body = body.subst(&vars, &mut 0);
            let mut ctx = self.ctx.clone();
            let Some(shape) = self.shape(&pat, &ty, &mut ctx) else {
                return self.error(term);
            };
            ctx.hyps
                .push((String::new(), Prop::Eq(value.clone(), shape)));
            let mut branch = Verifier::new(self.kernel, &ctx, self.span);
            branch.fuel = self.fuel;
            branch.next = self.next;
            branch.active = self.active.clone();
            branch.term(&body, depth + 1)?;
            self.fuel = branch.fuel;
            self.next = branch.next;
        }
        Ok(())
    }

    /// Whether the arms leave no case of `ty` unmatched.
    fn exhaustive(&self, ty: &Type, arms: &[(Pat, Term)]) -> bool {
        if arms
            .iter()
            .any(|(p, _)| matches!(p, Pat::Bind(_) | Pat::Any))
        {
            return true;
        }
        let Some(variants) = self.kernel.value_cases(ty) else {
            return false;
        };
        variants.iter().all(|(name, _)| {
            arms.iter().any(
                |(p, _)| matches!(p, Pat::Ctor(case, _) if case == name && Kernel::covers_case(p)),
            )
        })
    }

    /// The term a pattern describes, adding its binders to the scope with their types.
    fn shape(&mut self, pat: &Pat, ty: &Type, ctx: &mut Context) -> Option<Term> {
        match pat {
            Pat::Bind(name) => {
                ctx.scope = ctx.scope.with(name, Term::Var(name.clone()), ty.clone());
                Some(Term::Var(name.clone()))
            }
            Pat::Any => {
                self.next += 1;
                let name = format!("_~{}", self.next);
                ctx.scope = ctx.scope.with(&name, Term::Var(name.clone()), ty.clone());
                Some(Term::Var(name))
            }
            Pat::Lit(literal) => Some(literal.clone()),
            Pat::Range { .. } => {
                // Any value in the range is one the body runs for; a fresh value
                // proves the body defined for at least as much.
                self.next += 1;
                let name = format!("_~{}", self.next);
                ctx.scope = ctx.scope.with(&name, Term::Var(name.clone()), ty.clone());
                Some(Term::Var(name))
            }
            Pat::Alt(_) => None,
            Pat::Ctor(case, pats) => {
                let Type::Named(id, _) = ty else {
                    return None;
                };
                let types = self.kernel.case_fields(ty, case)?;
                let mut fields = vec![];
                for (sub, field) in pats.iter().zip(&types) {
                    fields.push(self.shape(sub, field, ctx)?);
                }
                if id == "Nat" && case == "Zero" {
                    return Some(Term::Nat(0));
                }
                Some(self.kernel.ctor(id, case, fields))
            }
        }
    }

    fn prop(&mut self, prop: &Prop) -> Found<()> {
        match prop {
            Prop::Eq(a, b) => {
                self.term(a, 0)?;
                self.term(b, 0)
            }
            Prop::Holds(t) => self.term(t, 0),
            Prop::Mem(a, b) => {
                self.term(a, 0)?;
                self.term(b, 0)
            }
            Prop::Subset { left, right, .. } => {
                self.term(left, 0)?;
                self.term(right, 0)
            }
            Prop::Not(p) | Prop::Forall { body: p, .. } | Prop::Exists { body: p, .. } => {
                self.prop(p)
            }
            Prop::And(a, b) | Prop::Or(a, b) | Prop::Implies(a, b) | Prop::Iff(a, b) => {
                self.prop(a)?;
                self.prop(b)
            }
            Prop::True | Prop::False | Prop::Atom(_) => Ok(()),
        }
    }
}

/// `A | B` guards one body, so the verifier reads the same body once per
/// alternative.
fn expand_alternatives(pat: &Pat, body: &Term, out: &mut Vec<(Pat, Term)>) {
    match pat {
        Pat::Alt(pats) => {
            for pat in pats {
                expand_alternatives(pat, body, out);
            }
        }
        _ => out.push((pat.clone(), body.clone())),
    }
}

impl Kernel<'_> {
    /// Whether a fact follows from the context: an assumption, a closed decision,
    /// or a bound the assumptions imply (Q70, Q81).
    pub(super) fn known_fact(&self, needed: &Prop, ctx: &Context, span: Span) -> bool {
        // A fact whose own arithmetic could fail is not known, even when reflexive.
        let mut verifier = Verifier::new(self, ctx, span);
        if verifier.prop(needed).is_err() {
            return false;
        }
        let normal = self.reducer().nf_prop(needed);
        matches!(&normal, Prop::Eq(a, b) if a == b)
            || verifier.established(needed)
            || verifier.established(&normal)
    }

    pub(super) fn with_runtime_bounds<T>(&self, check: impl FnOnce(&Self) -> T) -> T {
        let previous = self.runtime_bounds.replace(true);
        let result = check(self);
        self.runtime_bounds.set(previous);
        result
    }
    pub(super) fn defined_prop(&self, prop: &Prop, ctx: &Context, span: Span) -> Found<()> {
        Verifier::new(self, ctx, span).prop(prop)
    }
    pub(super) fn defined_term(&self, term: &Term, ctx: &Context, span: Span) -> Found<()> {
        Verifier::new(self, ctx, span).term(term, 0)
    }
    pub(super) fn defined_from(
        &self,
        goal: &Prop,
        fact: &Prop,
        ctx: &Context,
        span: Span,
    ) -> Found<()> {
        let mut ctx = ctx.clone();
        ctx.hyps.push((String::new(), fact.clone()));
        self.defined_prop(goal, &ctx, span)
    }
}
