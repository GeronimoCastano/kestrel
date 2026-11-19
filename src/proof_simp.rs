//! Bounded left-to-right rewriting with already checked equations.
use super::*;

const REWRITE_LIMIT: usize = 4096;
const DEPTH_LIMIT: usize = 128;

struct Rule {
    theorem: Theorem,
    matching_params: Vec<(String, Type)>,
    left: Term,
    right: Term,
    /// An `↔` rule rewrites propositions instead of terms.
    iff: Option<(Prop, Prop)>,
}

/// Peels the `forall` binders of a hypothesis into theorem parameters, so a
/// law such as `hIdL : forall a, op(e, a) = a` becomes a rewrite rule.
fn quantified_rule(prop: &Prop) -> Option<Theorem> {
    let mut params = vec![];
    let mut body = prop.clone();
    while let Prop::Forall {
        name,
        ty,
        body: inner,
    } = body
    {
        params.push((name, ty));
        body = *inner;
    }
    // A quantified law may carry premises: `forall a, P a -> op(e,a) = a`.
    // They become the rule's assumptions and are checked at every match.
    let mut requires = vec![];
    while let Prop::Implies(premise, conclusion) = body {
        requires.push((String::new(), *premise));
        body = *conclusion;
    }
    if !matches!(body, Prop::Eq(..) | Prop::Iff(..)) {
        return None;
    }
    Some(Theorem {
        params,
        bounds: BTreeMap::new(),
        requires,
        statement: body,
        dispatches: vec![],
        presupposes: false,
        admitted: false,
        trusted_smt: false,
    })
}

impl Kernel<'_> {
    pub(super) fn simplify(
        &mut self,
        node: &Node,
        goal: &Prop,
        ctx: &Context,
        current: &mut Current,
    ) -> Found<()> {
        if !matches!(goal, Prop::Eq(..)) {
            let rules = self.collect_rules(node, ctx, current)?;
            let mut budget = REWRITE_LIMIT;
            let mut ctx = ctx.clone();
            let mut rewritten =
                self.rewrite_prop(goal, &mut ctx, &rules, &mut budget, 0, node.span)?;
            let false_assumption = ctx.hyps.iter().any(|(_, fact)| {
                matches!(
                    self.normal(fact),
                    Prop::False | Prop::Holds(Term::Bool(false))
                )
            });
            if false_assumption
                || self.same(&rewritten, &Prop::True)
                || ctx.hyps.iter().any(|(_, fact)| self.same(fact, &rewritten))
                || matches!(&rewritten, Prop::Eq(a, b) if self.same(&Prop::Eq(a.clone(), a.clone()), &Prop::Eq(a.clone(), b.clone())))
            {
                return self.defined_prop(&rewritten, &ctx, node.span);
            }
            // Membership of a compound set is kept written in the goal. Unfold
            // its head so simp can close goals such as `a ∈ {a}` without a
            // separate tactic.
            for _ in 0..8 {
                if self.closes_set_simple(&rewritten) {
                    return self.defined_prop(&rewritten, &ctx, node.span);
                }
                let unfolded = self.unfold_head(&rewritten);
                if unfolded == rewritten {
                    break;
                }
                rewritten = unfolded;
            }
            return fail(
                node.span,
                "E_PROOF_MISMATCH",
                format!("simp leaves `{rewritten}`, which is not closed."),
                "Add a checked equality with simp [rule], or use match or induction.",
            );
        }
        let Prop::Eq(left, right) = goal else {
            unreachable!()
        };
        let (left, right) = ((*left).clone(), (*right).clone());
        let rules = self.collect_rules(node, ctx, current)?;
        let mut budget = REWRITE_LIMIT;
        let mut ctx = ctx.clone();
        let a = self.simp_term(&left, &mut ctx, &rules, &mut budget, 0, node.span)?;
        let b = self.simp_term(&right, &mut ctx, &rules, &mut budget, 0, node.span)?;
        if self.same(
            &Prop::Eq(a.clone(), a.clone()),
            &Prop::Eq(a.clone(), b.clone()),
        ) || ctx
            .hyps
            .iter()
            .any(|(_, fact)| self.same(fact, &Prop::Eq(a.clone(), b.clone())))
        {
            self.defined_prop(goal, &ctx, node.span)
        } else {
            fail(
                node.span,
                "E_PROOF_MISMATCH",
                format!("simp leaves `{a} = {b}`, whose sides do not reduce to the same term."),
                "Add a checked equality with simp [rule], or use match or induction.",
            )
        }
    }

    /// The structural closures that `simp` may use after unfolding a set
    /// membership. It mirrors the logical shape of `closes_simple` in the
    /// tactic module, restricted to propositions the kernel can decide here.
    fn closes_set_simple(&mut self, prop: &Prop) -> bool {
        match prop {
            Prop::True => true,
            Prop::False => false,
            Prop::Eq(a, b) => self.same(
                &Prop::Eq(a.clone(), a.clone()),
                &Prop::Eq(a.clone(), b.clone()),
            ),
            Prop::Holds(Term::Bool(value)) => *value,
            Prop::Holds(_) | Prop::Mem(..) | Prop::Subset { .. } | Prop::Atom(_) => false,
            Prop::And(a, b) => self.closes_set_simple(a) && self.closes_set_simple(b),
            Prop::Or(a, b) => self.closes_set_simple(a) || self.closes_set_simple(b),
            Prop::Not(inner) => matches!(self.normal(inner), Prop::False),
            Prop::Implies(premise, conclusion) => {
                matches!(self.normal(premise), Prop::False) || self.closes_set_simple(conclusion)
            }
            Prop::Iff(a, b) => self.same(a, b),
            Prop::Forall { .. } | Prop::Exists { .. } => false,
        }
    }

    fn collect_rules(
        &mut self,
        node: &Node,
        ctx: &Context,
        current: &mut Current,
    ) -> Found<Vec<Rule>> {
        // A rule starts as `(name, false, None)`. An explicit `←` flips the
        // flag and overrides the same name in the default simp set. A rule in
        // term position, such as `hAssoc(x, y, z)`, carries its instantiated
        // proposition instead of a name.
        let mut entries: Vec<(String, bool, Option<Prop>)> = self
            .simp_theorems
            .iter()
            .cloned()
            .map(|name| (name, false, None))
            .collect();
        for rule in &node.children {
            let (path, reverse) = if rule.kind == super::K::Unary
                && rule.children.len() == 2
                && matches!(self.text(&rule.children[0]), "←" | "\\larr")
            {
                (&rule.children[1], true)
            } else {
                (rule, false)
            };
            if path.kind == super::K::Call {
                let proposition = self.infer(path, ctx, current)?;
                entries.push((String::new(), reverse, Some(proposition)));
                continue;
            }
            let name = crate::lexer::path_name(self.text(path));
            if let Some(existing) = entries.iter_mut().find(|(n, _, _)| n == &name) {
                existing.1 = reverse;
            } else {
                entries.push((name, reverse, None));
            }
        }
        let mut rules = vec![];
        for (name, reverse, inline) in entries {
            let shown = if name.is_empty() { "this rule" } else { &name };
            let mut local_rule = true;
            let theorem = if let Some(proposition) = inline {
                quantified_rule(&proposition).ok_or_else(|| {
                    Diagnostic::new(
                        "E_PROOF_SIMP_RULE",
                        format!("{shown} is not an equality or an iff."),
                        node.span,
                    )
                    .with_hint("Pass a rule written with `=` or `↔`.")
                })?
            } else if let Some(theorem) = self.theorems.get(&name) {
                local_rule = false;
                if theorem.admitted {
                    return fail(
                        node.span,
                        "E_PROOF_SORRY",
                        format!("`{name}` uses `sorry`, so it cannot be a rewrite rule."),
                        "Prove the theorem or use `exact` and accept an admitted result.",
                    );
                }
                theorem.clone()
            } else if let Some((_, prop)) = ctx.hyps.iter().rev().find(|(n, _)| n == &name) {
                // A local hypothesis may be quantified: `hIdL : forall a, op(e,a)=a`
                // turns into one rule per instantiation.
                quantified_rule(prop).ok_or_else(|| {
                    Diagnostic::new(
                        "E_PROOF_SIMP_RULE",
                        format!(
                            "`{name}` is not an equality or an iff, so simp cannot rewrite with it."
                        ),
                        node.span,
                    )
                    .with_hint("Choose a theorem written with `=` or `↔`.")
                })?
            } else if let Some(statement) = self.conditional(&name, ctx, node.span)? {
                quantified_rule(&statement).ok_or_else(|| {
                    Diagnostic::new(
                        "E_PROOF_SIMP_RULE",
                        format!(
                            "`{name}` is not an equality or an iff, so simp cannot rewrite with it."
                        ),
                        node.span,
                    )
                    .with_hint("Choose an induction hypothesis written with `=` or `↔`.")
                })?
            } else {
                let (code, message) = if name == current.name {
                    (
                        "E_PROOF_RECURSION",
                        format!("simp cannot use `{name}` before its proof is checked."),
                    )
                } else if let Some(later) = self.declared(&name, current.start) {
                    if later {
                        (
                            "E_PROOF_ORDER",
                            format!("`{name}` is declared after this theorem."),
                        )
                    } else {
                        (
                            "E_PROOF_UNCHECKED",
                            format!("`{name}` did not check, so simp cannot use it."),
                        )
                    }
                } else {
                    (
                        "E_PROOF_UNKNOWN",
                        format!("`{name}` is not a checked theorem or a named equality here."),
                    )
                };
                return fail(
                    node.span,
                    code,
                    message,
                    "Use an earlier checked equality or a named equality assumption.",
                );
            };
            rules.push(self.build_rule(node, shown, theorem, reverse, local_rule)?);
        }
        Ok(rules)
    }

    /// One `Rule` from a checked equation, oriented left to right or reversed.
    fn build_rule(
        &mut self,
        node: &Node,
        name: &str,
        theorem: Theorem,
        reverse: bool,
        local_rule: bool,
    ) -> Found<Rule> {
        if let Some(error) = dispatch_override_error(
            &theorem.dispatches,
            &self.checked.environment.definitions,
            node.span,
        ) {
            return Err(error);
        }
        let matching_params = self.theorem_matching_params(&theorem);
        let statement = theorem.statement.clone();
        match &statement {
            Prop::Eq(left, right) => {
                let (mut left, mut right) = (left.clone(), right.clone());
                if reverse {
                    std::mem::swap(&mut left, &mut right);
                }
                let mut left_vars = BTreeSet::new();
                left.free(&mut left_vars);
                // Even an unused parameter needs an argument: its type may be
                // empty, in which case the theorem holds only vacuously.
                if matching_params.iter().any(|(n, _)| !left_vars.contains(n)) {
                    return fail(
                        node.span,
                        "E_PROOF_SIMP_RULE",
                        format!("The left side of `{name}` does not determine all its parameters."),
                        "Use the theorem explicitly with its arguments.",
                    );
                }
                // A local fact is already fully known, so its rule may use the
                // normal form: `Succ(k) + j` and `Succ(k + j)` are the same
                // term, and the goal is normalized before matching.
                if local_rule || matching_params != theorem.params {
                    left = self.reducer().nf(&left);
                    right = self.reducer().nf(&right);
                }
                Ok(Rule {
                    theorem,
                    matching_params,
                    left,
                    right,
                    iff: None,
                })
            }
            Prop::Iff(left, right) => {
                let (mut pattern, mut replacement) = ((**left).clone(), (**right).clone());
                if reverse {
                    std::mem::swap(&mut pattern, &mut replacement);
                }
                let mut left_vars = BTreeSet::new();
                pattern.map(&mut |term| {
                    term.free(&mut left_vars);
                    term.clone()
                });
                if matching_params.iter().any(|(n, _)| !left_vars.contains(n)) {
                    return fail(
                        node.span,
                        "E_PROOF_SIMP_RULE",
                        format!("The left side of `{name}` does not determine all its parameters."),
                        "Use the theorem explicitly with its arguments.",
                    );
                }
                Ok(Rule {
                    theorem,
                    matching_params,
                    left: Term::Bool(true),
                    right: Term::Bool(true),
                    iff: Some((pattern, replacement)),
                })
            }
            _ => fail(
                node.span,
                "E_PROOF_SIMP_RULE",
                format!("`{name}` is not an equality or an iff, so simp cannot rewrite with it."),
                "Choose a theorem written with `=` or `↔`.",
            ),
        }
    }

    pub(super) fn rewrite(
        &mut self,
        node: &Node,
        goal: &Prop,
        ctx: &Context,
        current: &mut Current,
    ) -> Found<Prop> {
        let rules = self.collect_rules(node, ctx, current)?;
        // Rewrite one occurrence at a time, and stop as soon as the goal is
        // reflexive. This reads naturally for both `rw [plusZeroR]`, where two
        // nested occurrences must change, and `rw [unionComm]`, where changing
        // the second side would undo the first.
        let mut current = goal.clone();
        let mut seen = BTreeSet::new();
        // A goal that `rfl` already closes needs no rewrite; the next tactic
        // finishes it. This also makes a redundant `rw` harmless.
        let mut changed = self.reflexive_goal(goal);
        if changed {
            return Ok(current);
        }
        for _ in 0..256 {
            if self.reflexive_goal(&current) {
                break;
            }
            let key = current.alpha_normalize();
            if !seen.insert(format!("{key:?}")) {
                break;
            }
            let Some(next) = self.rewrite_first_prop(&current, ctx, &rules, node.span)? else {
                break;
            };
            if next == current {
                break;
            }
            current = next;
            changed = true;
        }
        if !changed {
            // The goal may already be at the rule's result up to reduction.
            // That is the change `rw` used to report, so keep it.
            let normal = self.normal(goal);
            if normal != *goal {
                return Ok(normal);
            }
            return fail(
                node.span,
                "E_PROOF_MISMATCH",
                "rw did not change the goal.",
                "Use a rule whose left side occurs in the goal.",
            );
        }
        Ok(current)
    }

    /// Whether `rfl` could close this proposition.
    fn reflexive_goal(&mut self, prop: &Prop) -> bool {
        match prop {
            Prop::Eq(a, b) => self.same(
                &Prop::Eq(a.clone(), a.clone()),
                &Prop::Eq(a.clone(), b.clone()),
            ),
            Prop::Iff(a, b) => self.same(a, b),
            Prop::Holds(Term::Bool(true)) | Prop::True => true,
            _ => false,
        }
    }

    /// `rw` rewrites the first matching occurrence, left to right, and stops.
    /// Unlike `simp` it is not a fixed point: `rw [unionComm]` swaps one union
    /// instead of swapping forever.
    fn rewrite_first_prop(
        &mut self,
        prop: &Prop,
        ctx: &Context,
        rules: &[Rule],
        span: Span,
    ) -> Found<Option<Prop>> {
        for rule in rules {
            let Some((left, right)) = &rule.iff else {
                continue;
            };
            let holes = rule
                .matching_params
                .iter()
                .map(|(name, _)| name.clone())
                .collect::<BTreeSet<_>>();
            let mut map = BTreeMap::new();
            if !unify_prop(left, prop, &holes, &mut map) {
                continue;
            }
            if !self.complete_class_arguments(&rule.theorem, &mut map)
                || !self.rewrite_bounds(&rule.theorem, &map, ctx)
            {
                continue;
            }
            let requires = rule
                .theorem
                .requires
                .iter()
                .map(|(_, p)| p.map(&mut |t| t.subst(&map, &mut self.fresh)))
                .collect::<Vec<_>>();
            if requires.iter().any(|p| {
                !ctx.hyps.iter().any(|(_, h)| self.same(h, p))
                    && self.decide(p) != Some(true)
                    && !self.known_fact(p, ctx, span)
            }) {
                continue;
            }
            let mut fresh = self.fresh;
            let next = right.subst_prop(&map, &mut fresh);
            self.fresh = fresh;
            if next != *prop {
                if rule.theorem.trusted_smt {
                    self.smt_trusted.set(true);
                }
                return Ok(Some(next));
            }
        }
        let rewritten = match prop {
            Prop::Eq(a, b) => match self.rewrite_first_term(a, ctx, rules, span)? {
                Some(a) => Some(Prop::Eq(a, b.clone())),
                None => self
                    .rewrite_first_term(b, ctx, rules, span)?
                    .map(|b| Prop::Eq(a.clone(), b)),
            },
            Prop::Holds(t) => self
                .rewrite_first_term(t, ctx, rules, span)?
                .map(Prop::Holds),
            Prop::Mem(a, b) => match self.rewrite_first_term(a, ctx, rules, span)? {
                Some(a) => Some(Prop::Mem(a, b.clone())),
                None => self
                    .rewrite_first_term(b, ctx, rules, span)?
                    .map(|b| Prop::Mem(a.clone(), b)),
            },
            Prop::Subset {
                left,
                right,
                element,
            } => match self.rewrite_first_term(left, ctx, rules, span)? {
                Some(left) => Some(Prop::Subset {
                    left,
                    right: right.clone(),
                    element: element.clone(),
                }),
                None => self
                    .rewrite_first_term(right, ctx, rules, span)?
                    .map(|right| Prop::Subset {
                        left: left.clone(),
                        right,
                        element: element.clone(),
                    }),
            },
            Prop::And(a, b) => match self.rewrite_first_prop(a, ctx, rules, span)? {
                Some(a) => Some(Prop::And(Box::new(a), b.clone())),
                None => self
                    .rewrite_first_prop(b, ctx, rules, span)?
                    .map(|b| Prop::And(a.clone(), Box::new(b))),
            },
            Prop::Or(a, b) => match self.rewrite_first_prop(a, ctx, rules, span)? {
                Some(a) => Some(Prop::Or(Box::new(a), b.clone())),
                None => self
                    .rewrite_first_prop(b, ctx, rules, span)?
                    .map(|b| Prop::Or(a.clone(), Box::new(b))),
            },
            Prop::Not(p) => self
                .rewrite_first_prop(p, ctx, rules, span)?
                .map(|p| Prop::Not(Box::new(p))),
            Prop::Implies(a, b) => match self.rewrite_first_prop(a, ctx, rules, span)? {
                Some(a) => Some(Prop::Implies(Box::new(a), b.clone())),
                None => self
                    .rewrite_first_prop(b, ctx, rules, span)?
                    .map(|b| Prop::Implies(a.clone(), Box::new(b))),
            },
            Prop::Iff(a, b) => match self.rewrite_first_prop(a, ctx, rules, span)? {
                Some(a) => Some(Prop::Iff(Box::new(a), b.clone())),
                None => self
                    .rewrite_first_prop(b, ctx, rules, span)?
                    .map(|b| Prop::Iff(a.clone(), Box::new(b))),
            },
            Prop::Forall { name, ty, body } => self
                .rewrite_first_prop(body, ctx, rules, span)?
                .map(|body| Prop::Forall {
                    name: name.clone(),
                    ty: ty.clone(),
                    body: Box::new(body),
                }),
            Prop::Exists { name, ty, body } => self
                .rewrite_first_prop(body, ctx, rules, span)?
                .map(|body| Prop::Exists {
                    name: name.clone(),
                    ty: ty.clone(),
                    body: Box::new(body),
                }),
            Prop::Atom(term) => self
                .rewrite_first_term(term, ctx, rules, span)?
                .map(Prop::Atom),
            Prop::True | Prop::False => None,
        };
        Ok(rewritten)
    }

    /// The first matching rewrite inside one term, pre-order.
    fn rewrite_first_term(
        &mut self,
        term: &Term,
        ctx: &Context,
        rules: &[Rule],
        span: Span,
    ) -> Found<Option<Term>> {
        let normal = self.reducer().nf(term);
        for rule in rules {
            if rule.iff.is_some() {
                continue;
            }
            let mut map = BTreeMap::new();
            if !self.simp_match(&rule.left, &normal, &rule.matching_params, ctx, &mut map) {
                continue;
            }
            if !self.complete_class_arguments(&rule.theorem, &mut map)
                || !self.rewrite_bounds(&rule.theorem, &map, ctx)
            {
                continue;
            }
            let requires = rule
                .theorem
                .requires
                .iter()
                .map(|(_, p)| p.map(&mut |t| t.subst(&map, &mut self.fresh)))
                .collect::<Vec<_>>();
            if requires.iter().any(|p| {
                !ctx.hyps.iter().any(|(_, h)| self.same(h, p))
                    && self.decide(p) != Some(true)
                    && !self.known_fact(p, ctx, span)
            }) {
                continue;
            }
            let next = rule.right.subst(&map, &mut self.fresh);
            if next != normal {
                if rule.theorem.trusted_smt {
                    self.smt_trusted.set(true);
                }
                return Ok(Some(next));
            }
        }
        let descend = |kernel: &mut Self, args: &[Term]| -> Found<Option<Vec<Term>>> {
            for (index, arg) in args.iter().enumerate() {
                if let Some(rewritten) = kernel.rewrite_first_term(arg, ctx, rules, span)? {
                    let mut args = args.to_vec();
                    args[index] = rewritten;
                    return Ok(Some(args));
                }
            }
            Ok(None)
        };
        match normal {
            Term::Ctor { ty, case, args } => {
                Ok(descend(self, &args)?.map(|args| Term::Ctor { ty, case, args }))
            }
            Term::Call {
                function,
                name,
                args,
                dispatch,
            } => Ok(descend(self, &args)?.map(|args| Term::Call {
                function,
                name,
                args,
                dispatch,
            })),
            Term::Opaque { name, args } => {
                Ok(descend(self, &args)?.map(|args| Term::Opaque { name, args }))
            }
            Term::Op(op, args) => Ok(descend(self, &args)?.map(|args| Term::Op(op, args))),
            Term::App(function, args) => {
                if let Some(function) = self.rewrite_first_term(&function, ctx, rules, span)? {
                    return Ok(Some(Term::App(Box::new(function), args)));
                }
                Ok(descend(self, &args)?.map(|args| Term::App(function, args)))
            }
            Term::Lam(params, body) => Ok(self
                .rewrite_first_term(&body, ctx, rules, span)?
                .map(|body| Term::Lam(params, Box::new(body)))),
            Term::If(c, a, b) => {
                if let Some(c) = self.rewrite_first_term(&c, ctx, rules, span)? {
                    return Ok(Some(Term::If(Box::new(c), a, b)));
                }
                if let Some(a) = self.rewrite_first_term(&a, ctx, rules, span)? {
                    return Ok(Some(Term::If(c, Box::new(a), b)));
                }
                Ok(self
                    .rewrite_first_term(&b, ctx, rules, span)?
                    .map(|b| Term::If(c, a, Box::new(b))))
            }
            Term::Match(scrutinee, arms) => Ok(self
                .rewrite_first_term(&scrutinee, ctx, rules, span)?
                .map(|s| Term::Match(Box::new(s), arms))),
            Term::FamilyUnion { set, element } => Ok(self
                .rewrite_first_term(&set, ctx, rules, span)?
                .map(|set| Term::FamilyUnion {
                    set: Box::new(set),
                    element,
                })),
            Term::FamilyInter { set, element } => Ok(self
                .rewrite_first_term(&set, ctx, rules, span)?
                .map(|set| Term::FamilyInter {
                    set: Box::new(set),
                    element,
                })),
            Term::Finite(elements) => Ok(descend(self, &elements)?.map(Term::Finite)),
            Term::Builder {
                name,
                element,
                body,
            } => Ok(self
                .rewrite_first_prop(&body, ctx, rules, span)?
                .map(|body| Term::Builder {
                    name,
                    element,
                    body: Box::new(body),
                })),
            Term::Prop(inner) => Ok(self
                .rewrite_first_prop(&inner, ctx, rules, span)?
                .map(|inner| Term::Prop(Box::new(inner)))),
            _ => Ok(None),
        }
    }

    fn rewrite_prop(
        &mut self,
        prop: &Prop,
        ctx: &mut Context,
        rules: &[Rule],
        budget: &mut usize,
        depth: usize,
        span: Span,
    ) -> Found<Prop> {
        if depth >= DEPTH_LIMIT {
            return Self::simp_limit(span);
        }
        let mut prop = prop.clone();
        loop {
            if *budget == 0 {
                return Self::simp_limit(span);
            }
            *budget -= 1;
            let mut rewritten = None;
            for rule in rules {
                let Some((left, right)) = &rule.iff else {
                    continue;
                };
                let holes = rule
                    .matching_params
                    .iter()
                    .map(|(name, _)| name.clone())
                    .collect::<BTreeSet<_>>();
                let mut map = BTreeMap::new();
                if !unify_prop(left, &prop, &holes, &mut map) {
                    continue;
                }
                if !self.complete_class_arguments(&rule.theorem, &mut map) {
                    continue;
                }
                if !self.rewrite_bounds(&rule.theorem, &map, ctx) {
                    continue;
                }
                let requires = rule
                    .theorem
                    .requires
                    .iter()
                    .map(|(_, p)| p.map(&mut |t| t.subst(&map, &mut self.fresh)))
                    .collect::<Vec<_>>();
                if requires.iter().any(|p| {
                    !ctx.hyps.iter().any(|(_, h)| self.same(h, p))
                        && self.decide(p) != Some(true)
                        && !self.known_fact(p, ctx, span)
                }) {
                    continue;
                }
                let mut fresh = self.fresh;
                let next = right.subst_prop(&map, &mut fresh);
                self.fresh = fresh;
                if next != prop {
                    if rule.theorem.trusted_smt {
                        self.smt_trusted.set(true);
                    }
                    rewritten = Some(next);
                    break;
                }
            }
            match rewritten {
                Some(next) => prop = next,
                None => break,
            }
        }
        Ok(match &prop {
            Prop::Eq(a, b) => Prop::Eq(
                self.simp_term(a, ctx, rules, budget, depth + 1, span)?,
                self.simp_term(b, ctx, rules, budget, depth + 1, span)?,
            ),
            Prop::Holds(t) => {
                Prop::Holds(self.simp_term(t, ctx, rules, budget, depth + 1, span)?)
            }
            Prop::Mem(a, b) => Prop::Mem(
                self.simp_term(a, ctx, rules, budget, depth + 1, span)?,
                self.simp_term(b, ctx, rules, budget, depth + 1, span)?,
            ),
            Prop::Subset {
                left,
                right,
                element,
            } => Prop::Subset {
                left: self.simp_term(left, ctx, rules, budget, depth + 1, span)?,
                right: self.simp_term(right, ctx, rules, budget, depth + 1, span)?,
                element: element.clone(),
            },
            Prop::And(a, b) => Prop::And(
                Box::new(self.rewrite_prop(a, ctx, rules, budget, depth + 1, span)?),
                Box::new(self.rewrite_prop(b, ctx, rules, budget, depth + 1, span)?),
            ),
            Prop::Or(a, b) => Prop::Or(
                Box::new(self.rewrite_prop(a, ctx, rules, budget, depth + 1, span)?),
                Box::new(self.rewrite_prop(b, ctx, rules, budget, depth + 1, span)?),
            ),
            Prop::Not(p) => Prop::Not(Box::new(self.rewrite_prop(
                p,
                ctx,
                rules,
                budget,
                depth + 1,
                span,
            )?)),
            Prop::Implies(a, b) => Prop::Implies(
                Box::new(self.rewrite_prop(a, ctx, rules, budget, depth + 1, span)?),
                Box::new(self.rewrite_prop(b, ctx, rules, budget, depth + 1, span)?),
            ),
            Prop::Iff(a, b) => Prop::Iff(
                Box::new(self.rewrite_prop(a, ctx, rules, budget, depth + 1, span)?),
                Box::new(self.rewrite_prop(b, ctx, rules, budget, depth + 1, span)?),
            ),
            Prop::Forall { name, ty, body } => Prop::Forall {
                name: name.clone(),
                ty: ty.clone(),
                body: Box::new(self.rewrite_prop(body, ctx, rules, budget, depth + 1, span)?),
            },
            Prop::Exists { name, ty, body } => Prop::Exists {
                name: name.clone(),
                ty: ty.clone(),
                body: Box::new(self.rewrite_prop(body, ctx, rules, budget, depth + 1, span)?),
            },
            Prop::True => Prop::True,
            Prop::False => Prop::False,
            // An opaque predicate is a term, so equations about it can rewrite
            // its arguments even though its truth is not decided.
            Prop::Atom(term) => {
                Prop::Atom(self.simp_term(term, ctx, rules, budget, depth + 1, span)?)
            }
        })
    }

    fn simp_limit<T>(span: Span) -> Found<T> {
        fail(
            span,
            "E_PROOF_LIMIT",
            "simp reached its rewrite or nesting limit; these rules may expand terms or rewrite in a cycle.",
            "Use fewer rules, orient the equations toward smaller terms, or give an explicit proof.",
        )
    }

    fn simp_term(
        &mut self,
        input: &Term,
        ctx: &mut Context,
        rules: &[Rule],
        budget: &mut usize,
        depth: usize,
        span: Span,
    ) -> Found<Term> {
        if depth >= DEPTH_LIMIT || *budget == 0 {
            return Self::simp_limit(span);
        }
        let mut term = input.clone();
        loop {
            if *budget == 0 {
                return Self::simp_limit(span);
            }
            *budget -= 1;
            term = match term {
                Term::Ctor { ty, case, args } => Term::Ctor {
                    ty,
                    case,
                    args: self.simp_args(&args, ctx, rules, budget, depth, span)?,
                },
                Term::Call {
                    function,
                    name,
                    args,
                    dispatch,
                } => Term::Call {
                    function,
                    name,
                    args: self.simp_args(&args, ctx, rules, budget, depth, span)?,
                    dispatch,
                },
                Term::Opaque { name, args } => Term::Opaque {
                    name,
                    args: self.simp_args(&args, ctx, rules, budget, depth, span)?,
                },
                Term::App(function, args) => Term::App(
                    Box::new(self.simp_term(&function, ctx, rules, budget, depth + 1, span)?),
                    self.simp_args(&args, ctx, rules, budget, depth, span)?,
                ),
                Term::Lam(params, body) => Term::Lam(
                    params,
                    Box::new(self.simp_term(&body, ctx, rules, budget, depth + 1, span)?),
                ),
                Term::Op(op, args) => {
                    Term::Op(op, self.simp_args(&args, ctx, rules, budget, depth, span)?)
                }
                Term::FamilyUnion { set, element } => Term::FamilyUnion {
                    set: Box::new(self.simp_term(&set, ctx, rules, budget, depth + 1, span)?),
                    element,
                },
                Term::FamilyInter { set, element } => Term::FamilyInter {
                    set: Box::new(self.simp_term(&set, ctx, rules, budget, depth + 1, span)?),
                    element,
                },
                Term::Finite(elements) => {
                    Term::Finite(self.simp_args(&elements, ctx, rules, budget, depth, span)?)
                }
                Term::Builder {
                    name,
                    element,
                    body,
                } => Term::Builder {
                    name,
                    element,
                    body,
                },
                Term::If(c, a, b) => Term::If(
                    Box::new(self.simp_term(&c, ctx, rules, budget, depth + 1, span)?),
                    Box::new(self.simp_term(&a, ctx, rules, budget, depth + 1, span)?),
                    Box::new(self.simp_term(&b, ctx, rules, budget, depth + 1, span)?),
                ),
                // A stuck arm introduces binders absent from the proof context.
                // Split the scrutinee before rewriting there; an outer hypothesis
                // must never rewrite a same-spelled variable bound by the arm.
                Term::Match(s, arms) => Term::Match(
                    Box::new(self.simp_term(&s, ctx, rules, budget, depth + 1, span)?),
                    arms,
                ),
                other => other,
            };
            let mut reducer = self.reducer();
            let normal = reducer.nf(&term);
            self.fresh = reducer.fresh;
            if normal != term {
                term = normal;
                continue;
            }
            let mut rewritten = None;
            for rule in rules {
                let mut map = BTreeMap::new();
                if !self.simp_match(&rule.left, &term, &rule.matching_params, ctx, &mut map) {
                    continue;
                }
                if !self.complete_class_arguments(&rule.theorem, &mut map) {
                    continue;
                }
                if !self.rewrite_bounds(&rule.theorem, &map, ctx) {
                    continue;
                }
                let requires = rule
                    .theorem
                    .requires
                    .iter()
                    .map(|(_, p)| p.map(&mut |t| t.subst(&map, &mut self.fresh)))
                    .collect::<Vec<_>>();
                if requires.iter().any(|p| {
                    !ctx.hyps.iter().any(|(_, h)| self.same(h, p))
                        && self.decide(p) != Some(true)
                        && !self.known_fact(p, ctx, span)
                }) {
                    continue;
                }
                for (name, _) in &rule.theorem.params {
                    self.defined_term(&map[name], ctx, span)?;
                }
                let fact = rule
                    .theorem
                    .statement
                    .map(&mut |t| t.subst(&map, &mut self.fresh));
                if !ctx.hyps.iter().any(|(_, p)| p == &fact) {
                    if rule.theorem.trusted_smt {
                        self.smt_trusted.set(true);
                    }
                    ctx.hyps.push((String::new(), fact));
                }
                let next = rule.right.subst(&map, &mut self.fresh);
                if next != term {
                    if rule.theorem.trusted_smt {
                        self.smt_trusted.set(true);
                    }
                    rewritten = Some(next);
                    break;
                }
            }
            match rewritten {
                Some(next) => term = next,
                None => return Ok(term),
            }
            if *budget == 0 {
                return Self::simp_limit(span);
            }
        }
    }

    fn simp_args(
        &mut self,
        args: &[Term],
        ctx: &mut Context,
        rules: &[Rule],
        budget: &mut usize,
        depth: usize,
        span: Span,
    ) -> Found<Vec<Term>> {
        args.iter()
            .map(|a| self.simp_term(a, ctx, rules, budget, depth + 1, span))
            .collect()
    }

    pub(super) fn simp_match(
        &self,
        pattern: &Term,
        term: &Term,
        params: &[(String, Type)],
        ctx: &Context,
        map: &mut BTreeMap<String, Term>,
    ) -> bool {
        if let Term::Var(name) = pattern
            && let Some((_, ty)) = params.iter().find(|(n, _)| n == name)
        {
            if !self.fits(ty, &self.term_type(term, ctx)) {
                return false;
            }
            return match map.get(name) {
                Some(previous) => previous == term,
                None => {
                    map.insert(name.clone(), term.clone());
                    true
                }
            };
        }
        let matches = |a: &[Term], b: &[Term], map: &mut BTreeMap<String, Term>| {
            a.len() == b.len()
                && a.iter()
                    .zip(b)
                    .all(|(a, b)| self.simp_match(a, b, params, ctx, map))
        };
        match (pattern, term) {
            (
                Term::Call {
                    function: a,
                    args: xs,
                    ..
                },
                Term::Call {
                    function: b,
                    args: ys,
                    ..
                },
            ) if a == b => matches(xs, ys, map),
            (Term::Opaque { name: a, args: xs }, Term::Opaque { name: b, args: ys }) if a == b => {
                matches(xs, ys, map)
            }
            (
                Term::Ctor {
                    ty: a,
                    case: c,
                    args: xs,
                },
                Term::Ctor {
                    ty: b,
                    case: d,
                    args: ys,
                },
            ) if a == b && c == d => matches(xs, ys, map),
            (Term::Ctor { ty, case, args }, Term::Nat(n)) if ty == "Nat" => {
                match (case.as_str(), args.as_slice(), n) {
                    ("Zero", [], 0) => true,
                    ("Succ", [p], n) if *n > 0 => {
                        self.simp_match(p, &Term::Nat(n - 1), params, ctx, map)
                    }
                    _ => false,
                }
            }
            (Term::Op(a, xs), Term::Op(b, ys)) if a == b => matches(xs, ys, map),
            (Term::App(a, xs), Term::App(b, ys)) => {
                self.simp_match(a, b, params, ctx, map) && matches(xs, ys, map)
            }
            _ => pattern == term,
        }
    }
}
