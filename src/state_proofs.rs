//! Erased evidence about executable state. Summaries describe only straight-line
//! operations whose successful effects we can read; unknown code forgets state.
use super::*;
use crate::resolution::{Argument, Callable, DeclarationId, DeclarationKind};

#[derive(Clone, Debug)]
pub(crate) struct Summary {
    pub key: Key,
    parameters: Vec<String>,
    copies: Vec<bool>,
    steps: Option<Vec<Step>>,
    result: Option<Term>,
    proof: Option<Term>,
    method: bool,
    observes: bool,
}
impl Summary {
    pub(super) fn proof_result(&self) -> bool {
        self.proof.is_some()
    }
}

#[derive(Clone, Debug)]
enum Step {
    Assign(Term, Term),
    Call(DeclarationId, Option<Term>, Vec<Term>),
    Append(Term),
    Forget,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Witness {
    claim: Term,
    valid: bool,
}

#[derive(Clone, Debug, Default)]
pub(super) struct Flow {
    values: Vec<(Term, Term)>,
    aliases: BTreeMap<String, Term>,
    witnesses: BTreeMap<String, Witness>,
    calls: BTreeMap<Span, Term>,
    evidence: BTreeMap<Span, Witness>,
    trace: Vec<crate::proof::Dispatch>,
    serial: usize,
    epoch: usize,
    arguments: BTreeMap<Span, Term>,
    unknown: bool,
}

impl Flow {
    pub(super) fn join(a: Self, b: Self) -> Self {
        Self {
            values: a
                .values
                .into_iter()
                .filter(|v| b.values.contains(v))
                .collect(),
            aliases: a
                .aliases
                .into_iter()
                .filter(|(k, v)| b.aliases.get(k) == Some(v))
                .collect(),
            witnesses: a
                .witnesses
                .into_iter()
                .filter_map(|(k, mut v)| {
                    let other = b.witnesses.get(&k)?;
                    if v.claim != other.claim {
                        return None;
                    }
                    v.valid &= other.valid;
                    Some((k, v))
                })
                .collect(),
            calls: a
                .calls
                .into_iter()
                .filter(|(k, v)| b.calls.get(k) == Some(v))
                .collect(),
            evidence: a
                .evidence
                .into_iter()
                .filter(|(k, v)| b.evidence.get(k) == Some(v))
                .collect(),
            trace: a.trace.into_iter().chain(b.trace).collect(),
            serial: a.serial.max(b.serial),
            epoch: a.epoch.max(b.epoch),
            arguments: a
                .arguments
                .into_iter()
                .filter(|(k, v)| b.arguments.get(k) == Some(v))
                .collect(),
            unknown: a.unknown || b.unknown,
        }
    }
}

pub(super) struct Pending {
    writes: Vec<(Term, Term)>,
    facts: Vec<Term>,
    value: Option<Term>,
    proof: Option<Term>,
    trace: Vec<crate::proof::Dispatch>,
    serial: usize,
    unknown: bool,
}

fn field(base: Term, name: &str) -> Term {
    Term::Field(Box::new(base), name.into())
}
fn binary(op: &str, a: Term, b: Term) -> Term {
    Term::Binary(op.into(), Box::new(a), Box::new(b))
}
fn root(term: &Term) -> &Term {
    match term {
        Term::Field(base, _) => root(base),
        _ => term,
    }
}
fn reads(term: &Term, place: &Term) -> bool {
    if term == place {
        return true;
    }
    match term {
        Term::Field(base, _) | Term::Unary(_, base) => reads(base, place),
        Term::Call(_, args) | Term::Case(_, _, args) => args.iter().any(|a| reads(a, place)),
        Term::Binary(_, a, b) => reads(a, place) || reads(b, place),
        _ => false,
    }
}

impl Pass<'_> {
    pub(super) fn collect_state(&mut self) {
        for facts in self.checked.environment.calls.values() {
            self.summaries.extend(facts.state.clone());
        }
        fn collect(pass: &mut Pass<'_>, nodes: &[Node], owner: Option<&str>) {
            for node in nodes {
                if matches!(node.kind, K::Class | K::Trait) {
                    let name = pass.qualified(&pass.named_child(node).unwrap_or_default());
                    collect(pass, &node.children, Some(&name));
                } else if matches!(node.kind, K::Function | K::Constructor) {
                    let name = if node.kind == K::Constructor {
                        "new".into()
                    } else {
                        pass.named_child(node).unwrap_or_default()
                    };
                    let proof = node
                        .child(K::ReturnType)
                        .and_then(|r| r.child(K::Proposition))
                        .and_then(|p| p.children.first())
                        .map(|p| pass.term(p));
                    let body = function_body(node);
                    let mut steps = vec![];
                    let mut complete = body.is_some_and(|b| pass.state_steps(b, &mut steps));
                    let result = body.and_then(|b| {
                        let last = if b.kind == K::Block {
                            b.children.last()?
                        } else {
                            b
                        };
                        let last = if last.kind == K::Return {
                            last.children.first()?
                        } else {
                            last
                        };
                        (!matches!(last.kind, K::ProofWitness | K::Assignment | K::Binding))
                            .then(|| pass.term(last))
                    });
                    let parameters = parameter_names(pass, node);
                    fn closed(term: &Term, names: &[String]) -> bool {
                        match term {
                            Term::Name(n) => names.contains(n),
                            Term::Field(t, _) | Term::Unary(_, t) => closed(t, names),
                            Term::Binary(_, a, b) => closed(a, names) && closed(b, names),
                            Term::Call(_, args) | Term::Case(_, _, args) => {
                                args.iter().all(|a| closed(a, names))
                            }
                            _ => true,
                        }
                    }
                    complete &= result.as_ref().is_none_or(|r| closed(r, &parameters))
                        && steps.iter().all(|step| match step {
                            Step::Assign(a, b) => closed(a, &parameters) && closed(b, &parameters),
                            Step::Call(_, target, args) => {
                                target.as_ref().is_none_or(|t| closed(t, &parameters))
                                    && args.iter().all(|a| closed(a, &parameters))
                            }
                            Step::Append(t) => closed(t, &parameters),
                            Step::Forget => true,
                        });
                    // An incomplete summary does not describe every path, so its
                    // last expression is not the call's value. Keeping it would
                    // let a call with an early return certify a fall-through
                    // value the function never produces.
                    let result = if complete { result } else { None };
                    let observes = complete
                        && steps.is_empty()
                        && body.is_some_and(|b| b.kind != K::Block || b.children.len() == 1);
                    pass.summaries.insert(
                        DeclarationId {
                            file: pass.source.name.clone(),
                            span: node.span,
                        },
                        Summary {
                            key: (owner.unwrap_or("").into(), name),
                            parameters,
                            steps: complete.then_some(steps),
                            result,
                            copies: node
                                .child(K::ParameterList)
                                .map(|p| {
                                    p.children
                                        .iter()
                                        .map(|p| {
                                            pass.checked
                                                .binding_types
                                                .get(&p.span)
                                                .is_some_and(crate::types::copies)
                                        })
                                        .collect()
                                })
                                .unwrap_or_default(),
                            proof,
                            method: owner.is_some() && node.kind != K::Constructor,
                            observes,
                        },
                    );
                }
            }
        }
        collect(self, &self.parsed.root.children, None);
    }

    fn state_steps(&self, node: &Node, steps: &mut Vec<Step>) -> bool {
        match node.kind {
            K::Block => node.children.iter().all(|n| self.state_steps(n, steps)),
            K::Assignment if node.children.len() == 2 => {
                if !self.state_steps(&node.children[1], steps) {
                    return false;
                }
                steps.push(Step::Assign(
                    self.term(&node.children[0]),
                    self.term(&node.children[1]),
                ));
                true
            }
            K::Return => false, // Explicit exits need path checking, not a straight-line summary.
            K::Binding => false,
            K::If
            | K::While
            | K::For
            | K::Match
            | K::Lambda
            | K::Propagate
            | K::Using
            | K::Scope => false,
            K::ProofWitness => true,
            K::Call => {
                if node
                    .children
                    .iter()
                    .skip(1)
                    .flat_map(|n| n.descendants())
                    .any(|n| {
                        self.checked.resolved_calls.get(&n.span).is_some_and(|c| {
                            c.signature.effects
                                || c.signature.receiver_mutates
                                || c.signature
                                    .parameters
                                    .iter()
                                    .any(|p| p.passing == crate::types::Passing::Mut)
                        })
                    })
                {
                    return false;
                }
                for argument in node
                    .children
                    .iter()
                    .skip(1)
                    .filter_map(|a| a.children.last())
                {
                    if !self.state_steps(argument, steps) {
                        return false;
                    }
                }
                let Some(call) = self
                    .checked
                    .resolved_calls
                    .get(&node.span)
                    .filter(|c| !c.partial)
                else {
                    return false;
                };
                let receiver = node
                    .children
                    .first()
                    .filter(|n| n.kind == K::Member)
                    .and_then(|n| n.children.first());
                if receiver.is_some_and(|r| self.class_of(r.span).as_deref() == Some("Array")) {
                    let name = node.children[0].children.last().map(|n| self.text(n));
                    if name == Some("append") {
                        steps.push(Step::Append(self.term(receiver.unwrap())));
                        return true;
                    }
                }
                if !call.signature.receiver_mutates
                    && !call.signature.effects
                    && call
                        .signature
                        .parameters
                        .iter()
                        .all(|p| p.passing != crate::types::Passing::Mut)
                    && !matches!(
                        call.target,
                        Callable::Declaration {
                            kind: DeclarationKind::Constructor,
                            ..
                        }
                    )
                {
                    return true;
                }
                if let Callable::Declaration { id, .. } = &call.target {
                    let Some(args) = self.state_arguments(node) else {
                        return false;
                    };
                    steps.push(Step::Call(id.clone(), receiver.map(|r| self.term(r)), args));
                } else if call.signature.receiver_mutates
                    || call.signature.effects
                    || call
                        .signature
                        .parameters
                        .iter()
                        .any(|p| p.passing == crate::types::Passing::Mut)
                {
                    steps.push(Step::Forget);
                }
                true
            }
            _ => node.children.iter().all(|n| self.state_steps(n, steps)),
        }
    }

    pub(super) fn array_term(&self, node: &Node) -> Option<Term> {
        if !self.state_enabled {
            return None;
        }
        let call = self.checked.resolved_calls.get(&node.span)?;
        if !matches!(call.target, Callable::Builtin(_)) || call.partial {
            return None;
        }
        let callee = node.children.first()?;
        if callee.kind == K::Member {
            let receiver = callee.children.first()?;
            let name = self.text(callee.children.last()?);
            if self.class_of(receiver.span).as_deref() == Some("Array") && name == "length" {
                return Some(field(self.term(receiver), "#length"));
            }
            if name == "empty"
                && matches!(call.signature.result.as_ref(), Type::Named(n, _) if n == "Array")
            {
                return Some(Term::Case(
                    "#Array".into(),
                    "new".into(),
                    vec![Term::Int(0)],
                ));
            }
        } else if self.text(callee) == "Array"
            && matches!(call.signature.result.as_ref(), Type::Named(n, _) if n == "Array")
        {
            return Some(Term::Case(
                "#Array".into(),
                "new".into(),
                vec![self.term(node.children.get(1)?.children.last()?)],
            ));
        }
        None
    }

    fn state_arguments(&self, node: &Node) -> Option<Vec<Term>> {
        self.checked
            .resolved_calls
            .get(&node.span)?
            .arguments
            .iter()
            .map(|a| match a {
                Argument::Supplied {
                    expression,
                    hole: false,
                } => self
                    .state
                    .arguments
                    .get(expression)
                    .cloned()
                    .or_else(|| self.nodes.get(expression).map(|n| self.term(n))),
                _ => None,
            })
            .collect()
    }

    fn location(&self, term: &Term) -> Term {
        match term {
            Term::Name(n) => self
                .state
                .aliases
                .get(n)
                .cloned()
                .unwrap_or_else(|| term.clone()),
            Term::Field(base, name) => field(self.reference(self.location(base)), name),
            Term::Call(id, args) => Term::Call(
                id.clone(),
                args.iter()
                    .map(|a| self.reference(self.location(a)))
                    .collect(),
            ),
            Term::Binary(op, a, b) => binary(op, self.location(a), self.location(b)),
            Term::Unary(op, a) => Term::Unary(op.clone(), Box::new(self.location(a))),
            _ => term.clone(),
        }
    }

    fn reference(&self, place: Term) -> Term {
        self.state
            .values
            .iter()
            .rev()
            .find_map(|(p, v)| {
                (*p == place && matches!(v,Term::Name(n) if n.starts_with("#object:")))
                    .then(|| v.clone())
            })
            .unwrap_or(place)
    }

    fn dispatch_receiver(&self, owner: &str) -> Type {
        Type::Named(
            owner.into(),
            self.checked
                .environment
                .definitions
                .get(owner)
                .map(|d| {
                    d.parameters
                        .iter()
                        .map(|n| Type::Parameter(n.clone()))
                        .collect()
                })
                .unwrap_or_default(),
        )
    }

    pub(super) fn resolve_state(&self, term: &Term) -> Term {
        if !self.state_enabled {
            return term.clone();
        }
        self.resolve_state_at(term, 0)
    }
    fn resolve_state_at(&self, term: &Term, depth: usize) -> Term {
        if depth > 24 {
            return term.clone();
        }
        let place = self.location(term);
        if let Some((_, value)) = self.state.values.iter().rev().find(|(p, _)| *p == place)
            && *value != place
        {
            return self.resolve_state_at(value, depth + 1);
        }
        let value = match &place {
            Term::Field(base, name) => {
                let base = self.resolve_state_at(base, depth + 1);
                if name == "#length"
                    && let Term::Case(ty, _, args) = &base
                    && ty == "#Array"
                    && args.len() == 1
                {
                    return args[0].clone();
                }
                field(base, name)
            }
            Term::Call(id, args) => {
                if let Some(summary) = self.summaries.get(id).filter(|s| s.observes)
                    && let Some(result) = &summary.result
                {
                    let (receiver, args) = if summary.method {
                        (args.first(), &args[1..])
                    } else {
                        (None, &args[..])
                    };
                    let values = summary
                        .parameters
                        .iter()
                        .cloned()
                        .zip(args.iter().cloned())
                        .collect();
                    let body = substitute(result, &values, receiver);
                    if args.iter().all(|a| self.defined_state(a, depth + 1))
                        && self.defined_state(&body, depth + 1)
                    {
                        return self.resolve_state_at(&body, depth + 1);
                    }
                }
                Term::Call(
                    id.clone(),
                    args.iter()
                        .map(|a| self.resolve_state_at(a, depth + 1))
                        .collect(),
                )
            }
            Term::Binary(op, a, b) => binary(
                op,
                self.resolve_state_at(a, depth + 1),
                self.resolve_state_at(b, depth + 1),
            ),
            Term::Unary(op, a) => {
                Term::Unary(op.clone(), Box::new(self.resolve_state_at(a, depth + 1)))
            }
            _ => place,
        };
        constant(&value)
            .map(|c| match c {
                Constant::Int(n) => Term::Int(n),
                Constant::Nat(n) => Term::Nat(n),
                Constant::Bool(b) => Term::Bool(b),
                Constant::Double(n) => Term::Double(n),
                Constant::Text(t) => Term::Text(t),
            })
            .unwrap_or(value)
    }

    pub(super) fn invalidate_state(&mut self, target: Option<Term>) {
        self.state.epoch += 1;
        let place = target.map(|t| self.reference(self.location(&t)));
        if self.state_enabled {
            self.facts.retain(|fact| {
                place.as_ref().is_some_and(|p| !reads(fact, root(p))) && !has_field(fact)
            });
        }
        for witness in self
            .state
            .witnesses
            .values_mut()
            .chain(self.state.evidence.values_mut())
        {
            if place
                .as_ref()
                .is_none_or(|p| reads(&witness.claim, root(p)))
            {
                witness.valid = false;
            }
        }
        self.state
            .values
            .retain(|(p, _)| place.as_ref().is_some_and(|target| root(p) != root(target)));
    }

    pub(super) fn state_value(&self, node: &Node) -> Term {
        self.state
            .calls
            .get(&node.span)
            .cloned()
            .unwrap_or_else(|| self.resolve_state(&self.term(node)))
    }
    pub(super) fn snapshot_value(&mut self, node: &Node) -> Term {
        let value = self.state_value(node);
        self.snapshot(value)
    }
    pub(super) fn assign_state(&mut self, target: &Node, value: Term) {
        let place = self.location(&self.term(target));
        self.state.values.push((place, value));
    }
    pub(super) fn assign_evidence(&mut self, target: &Node, value: &Node) {
        if target.kind == K::Name
            && let Some(witness) = self.witness(value)
        {
            self.state
                .witnesses
                .insert(self.text(target).into(), witness);
        }
    }

    pub(super) fn bind_state(&mut self, node: &Node) {
        let Some(pattern) = node.child(K::PatternName) else {
            return;
        };
        let Some(name) = pattern.children.first() else {
            return;
        };
        let name = self.text(name).to_owned();
        let Some(value) = node.children.last() else {
            return;
        };
        if let Some(witness) = self.witness(value) {
            self.state.witnesses.insert(name.clone(), witness);
        }
        if !self.state_enabled {
            return;
        }
        if let Some(object) = self.state.calls.get(&value.span).cloned().filter(|_| {
            self.types
                .get(&value.span)
                .is_some_and(|t| !crate::types::copies(t))
        }) {
            self.state.aliases.insert(name, object);
        } else if self
            .types
            .get(&value.span)
            .is_some_and(|t| !crate::types::copies(t))
            && matches!(value.kind, K::Name | K::Member)
        {
            let value = self.location(&self.term(value));
            self.state.aliases.insert(name, value);
        } else {
            let value = self.state_value(value);
            let value = self.snapshot(value);
            let place = Term::Name(format!("#binding:{}", node.span.start));
            self.state.aliases.insert(name, place.clone());
            self.state.values.push((place, value));
        }
    }

    pub(super) fn bind_proof_parameters(&mut self, node: &Node) {
        if let Some(params) = node.child(K::ParameterList) {
            for p in &params.children {
                if let (Some(name), Some(claim)) = (
                    p.child(K::Name),
                    p.child(K::Proposition).and_then(|p| p.children.first()),
                ) {
                    self.state.witnesses.insert(
                        self.text(name).into(),
                        Witness {
                            claim: self.term(claim),
                            valid: true,
                        },
                    );
                }
            }
        }
    }

    fn witness(&self, node: &Node) -> Option<Witness> {
        match node.kind {
            K::Group | K::Return => node.children.first().and_then(|n| self.witness(n)),
            K::Block => self
                .state
                .evidence
                .get(&node.span)
                .cloned()
                .or_else(|| node.children.last().and_then(|n| self.witness(n))),
            K::Name => self.state.witnesses.get(self.text(node)).cloned(),
            K::ProofWitness => Some(Witness {
                claim: self.location(&self.term(&node.children[0])),
                valid: true,
            }),
            K::Call => self.state.evidence.get(&node.span).cloned(),
            _ => None,
        }
    }

    fn record_state_dispatches(&mut self, goal: &Term) {
        self.dispatches.extend(self.state.trace.clone());
        fn visit(pass: &mut Pass<'_>, term: &Term, depth: usize) {
            if depth > 24 {
                return;
            }
            match term {
                Term::Call(id, args) => {
                    if let Some(s) = pass.summaries.get(id).cloned() {
                        if s.method {
                            pass.dispatches.push(crate::proof::Dispatch {
                                receiver: pass.dispatch_receiver(&s.key.0),
                                method: s.key.1.clone(),
                                function: id.clone(),
                            });
                        }
                        if let Some(result) = &s.result {
                            visit(pass, result, depth + 1);
                        }
                    }
                    for arg in args {
                        visit(pass, arg, depth + 1);
                    }
                }
                Term::Field(a, _) | Term::Unary(_, a) => visit(pass, a, depth + 1),
                Term::Binary(_, a, b) => {
                    visit(pass, a, depth + 1);
                    visit(pass, b, depth + 1);
                }
                _ => {}
            }
        }
        visit(self, goal, 0);
    }

    pub(super) fn check_witness(&mut self, node: &Node) {
        let goal = self.term(&node.children[0]);
        self.record_state_dispatches(&goal);
        if self.prove(&goal) != Some(true) {
            self.diagnostics.push(Diagnostic::new("E_PROOF_UNESTABLISHED", "The current facts do not establish this proof claim.", node.span)
                .with_hint("Establish the claim with checked state changes, a condition, a proof parameter or a theorem."));
        }
    }

    pub(super) fn check_proof_return(&mut self, value: &Node) {
        let Some(goal) = self.result_claim.clone() else {
            return;
        };
        self.record_state_dispatches(&goal);
        if self.checked.theorem_evidence.contains(&value.span) {
            self.checked_evidence.insert(value.span);
            if let Err(error) = crate::proof::check_runtime_evidence(
                self.parsed,
                self.checked,
                value,
                value,
                Some(&goal),
                &self.facts,
            ) {
                self.diagnostics.push(error);
            }
            return;
        }
        let supported = self.witness(value).is_some_and(|w| {
            w.valid
                && (implies(&self.resolve_state(&w.claim), &self.resolve_state(&goal))
                    || self.resolve_state(&w.claim) == self.resolve_state(&goal))
        });
        if !supported {
            self.diagnostics.push(Diagnostic::new("E_PROOF_RESULT", "The returned evidence does not establish the declared result claim in the current state.", value.span)
                .with_hint("Return a current proof of the result claim; a mutation invalidates evidence about the old state."));
        }
    }

    pub(super) fn check_state_argument(&mut self, node: &Node, goal: &Term) -> Option<bool> {
        let witness = self.witness(node)?;
        self.record_state_dispatches(goal);
        Some(
            witness.valid
                && (self.proof_shape(&witness.claim, 0) == self.proof_shape(goal, 0)
                    || implies(
                        &self.proof_shape(&witness.claim, 0),
                        &self.proof_shape(goal, 0),
                    )),
        )
    }

    fn proof_shape(&self, term: &Term, depth: usize) -> Term {
        if !self.state_enabled {
            return term.clone();
        }
        if depth > 24 {
            return term.clone();
        }
        match self.location(term) {
            Term::Call(ref id, ref args) => {
                if let Some(s) = self.summaries.get(id).filter(|s| s.observes)
                    && let Some(result) = &s.result
                {
                    let (receiver, args) = if s.method {
                        (args.first(), &args[1..])
                    } else {
                        (None, &args[..])
                    };
                    let values = s
                        .parameters
                        .iter()
                        .cloned()
                        .zip(args.iter().cloned())
                        .collect();
                    let body = substitute(result, &values, receiver);
                    if args.iter().all(|a| self.defined_state(a, depth + 1))
                        && self.defined_state(&body, depth + 1)
                    {
                        return self.proof_shape(&body, depth + 1);
                    }
                }
                term.clone()
            }
            Term::Field(a, n) => field(self.proof_shape(&a, depth + 1), &n),
            Term::Binary(op, a, b) => binary(
                &op,
                self.proof_shape(&a, depth + 1),
                self.proof_shape(&b, depth + 1),
            ),
            Term::Unary(op, a) => Term::Unary(op, Box::new(self.proof_shape(&a, depth + 1))),
            other => other,
        }
    }

    fn defined_state(&self, term: &Term, depth: usize) -> bool {
        if depth > 24 {
            return false;
        }
        match term {
            Term::Opaque(_) => false,
            Term::Field(base, _) => self.defined_state(base, depth + 1),
            Term::Binary(op, a, b) => {
                self.defined_state(a, depth + 1)
                    && self.defined_state(b, depth + 1)
                    && (matches!(
                        op.as_str(),
                        "==" | "!=" | "<" | ">" | "<=" | ">=" | "&&" | "||"
                    ) || constant(&binary(
                        op,
                        self.resolve_state_at(a, depth + 1),
                        self.resolve_state_at(b, depth + 1),
                    ))
                    .is_some())
            }
            Term::Unary(op, a) => {
                self.defined_state(a, depth + 1)
                    && (op == "!" || constant(&self.resolve_state_at(term, depth + 1)).is_some())
            }
            Term::Call(id, args) => {
                self.summaries
                    .get(id)
                    .filter(|s| s.observes)
                    .is_some_and(|s| {
                        let (receiver, values) = if s.method {
                            (args.first(), &args[1..])
                        } else {
                            (None, &args[..])
                        };
                        let values = s
                            .parameters
                            .iter()
                            .cloned()
                            .zip(values.iter().cloned())
                            .collect();
                        args.iter().all(|a| self.defined_state(a, depth + 1))
                            && s.result.as_ref().is_some_and(|r| {
                                self.defined_state(&substitute(r, &values, receiver), depth + 1)
                            })
                    })
            }
            Term::Case(_, _, args) => args.iter().all(|a| self.defined_state(a, depth + 1)),
            _ => true,
        }
    }

    fn snapshot(&mut self, value: Term) -> Term {
        match value {
            Term::Field(_, _) | Term::Call(_, _) | Term::Name(_) if !matches!(&value,Term::Name(n) if n.starts_with("#old:") || n.starts_with("#object:")) =>
            {
                if let Some((_, old)) = self.state.values.iter().rev().find(|(p, _)| *p == value) {
                    return old.clone();
                }
                self.state.serial += 1;
                let old = Term::Name(format!("#old:{}", self.state.serial));
                self.state.values.push((value, old.clone()));
                old
            }
            Term::Binary(op, a, b) => binary(&op, self.snapshot(*a), self.snapshot(*b)),
            Term::Unary(op, a) => Term::Unary(op, Box::new(self.snapshot(*a))),
            other => other,
        }
    }

    pub(super) fn enter_state_block(&self) -> Flow {
        self.state.clone()
    }
    pub(super) fn leave_state_block(&mut self, before: Flow, node: &Node) {
        if let Some(witness) = node.children.last().and_then(|n| self.witness(n)) {
            self.state.evidence.insert(node.span, witness);
        }
        self.state.aliases = before.aliases;
        let epoch = before.epoch;
        self.state.witnesses = before
            .witnesses
            .into_iter()
            .map(|(name, mut witness)| {
                if let Some(current) = self
                    .state
                    .witnesses
                    .get(&name)
                    .filter(|w| w.claim == witness.claim)
                {
                    witness.valid &= current.valid;
                } else if self.state.epoch != epoch {
                    witness.valid = false;
                }
                (name, witness)
            })
            .collect();
    }

    pub(super) fn capture_argument(&mut self, node: &Node) {
        if !self.state_enabled {
            return;
        }
        let value = if self.types.get(&node.span).is_some_and(crate::types::copies) {
            let value = self.state_value(node);
            self.snapshot(value)
        } else {
            self.reference(
                self.state
                    .calls
                    .get(&node.span)
                    .cloned()
                    .unwrap_or_else(|| self.location(&self.term(node))),
            )
        };
        self.state.arguments.insert(node.span, value);
    }

    pub(super) fn prepare_state_call(&mut self, node: &Node) -> Option<Pending> {
        if !self.state_enabled {
            return None;
        }
        let call = self.checked.resolved_calls.get(&node.span)?;
        if call.partial {
            return None;
        }
        let (id, constructor) = match &call.target {
            Callable::Declaration { id, kind, .. } => {
                (id.clone(), *kind == DeclarationKind::Constructor)
            }
            _ => {
                let receiver = node
                    .children
                    .first()
                    .filter(|n| n.kind == K::Member)?
                    .children
                    .first()?;
                if self.class_of(receiver.span).as_deref() != Some("Array")
                    || node.children[0].children.last().map(|n| self.text(n)) != Some("append")
                {
                    return None;
                }
                let place = field(self.location(&self.term(receiver)), "#length");
                let old = self.resolve_state(&place);
                let old = self.snapshot(old);
                let value = binary("+", old, Term::Int(1));
                let mut writes = self.state.values.clone();
                writes.retain(|(p, _)| *p != place);
                writes.push((place.clone(), value));
                return Some(Pending {
                    writes,
                    facts: vec![binary(">=", place, Term::Int(1))],
                    value: None,
                    proof: None,
                    trace: vec![],
                    serial: self.state.serial,
                    unknown: false,
                });
            }
        };
        let summary = self.summaries.get(&id)?.clone();
        let receiver = if constructor {
            self.state.serial += 1;
            Some(Term::Name(format!(
                "#object:{}:{}",
                node.span.start, self.state.serial
            )))
        } else {
            node.children
                .first()
                .filter(|n| n.kind == K::Member)
                .and_then(|n| n.children.first())
                .map(|n| self.reference(self.location(&self.term(n))))
        };
        let args = self.state_arguments(node)?;
        let parameters = summary
            .parameters
            .iter()
            .cloned()
            .zip(args.iter().enumerate().map(|(i, a)| {
                if summary.copies.get(i) == Some(&true) {
                    self.resolve_state(a)
                } else {
                    self.location(a)
                }
            }))
            .collect();
        let saved = self.state.clone();
        self.state.unknown = false;
        let saved_facts = self.facts.clone();
        let mut facts = vec![];
        let mut trace = vec![];
        self.apply_summary(
            &id,
            receiver.as_ref(),
            &parameters,
            &mut facts,
            &mut trace,
            0,
        );
        let writes = self.state.values.clone();
        let serial = self.state.serial;
        let unknown = self.state.unknown;
        let value = if constructor {
            receiver.clone()
        } else if summary.proof.is_none() {
            summary
                .result
                .as_ref()
                .map(|r| self.resolve_state(&substitute(r, &parameters, receiver.as_ref())))
        } else {
            None
        };
        self.state = saved;
        self.facts = saved_facts;
        let proof = summary
            .proof
            .as_ref()
            .map(|p| substitute(p, &parameters, receiver.as_ref()));
        Some(Pending {
            writes,
            facts,
            value,
            proof,
            trace,
            serial,
            unknown,
        })
    }

    fn apply_summary(
        &mut self,
        id: &DeclarationId,
        receiver: Option<&Term>,
        parameters: &BTreeMap<String, Term>,
        facts: &mut Vec<Term>,
        trace: &mut Vec<crate::proof::Dispatch>,
        depth: usize,
    ) {
        if depth > 16 {
            self.state.values.clear();
            facts.clear();
            self.state.unknown = true;
            return;
        }
        let Some(summary) = self.summaries.get(id).cloned() else {
            self.state.values.clear();
            facts.clear();
            self.state.unknown = true;
            return;
        };
        if summary.method {
            trace.push(crate::proof::Dispatch {
                receiver: self.dispatch_receiver(&summary.key.0),
                method: summary.key.1.clone(),
                function: id.clone(),
            });
        }
        let Some(steps) = summary.steps else {
            self.state.values.clear();
            facts.clear();
            self.state.unknown = true;
            return;
        };
        for step in steps {
            match step {
                Step::Assign(target, value) => {
                    let target = self.location(&substitute(&target, parameters, receiver));
                    let value = self.resolve_state(&substitute(&value, parameters, receiver));
                    let value = self.snapshot(value);
                    self.state.values.retain(|(p, _)| !reads(p, &target));
                    facts.retain(|f| !has_field(f));
                    self.state.values.push((target, value));
                }
                Step::Append(target) => {
                    let place = field(
                        self.location(&substitute(&target, parameters, receiver)),
                        "#length",
                    );
                    let old = self.resolve_state(&place);
                    let old = self.snapshot(old);
                    let value = binary("+", old, Term::Int(1));
                    self.state.values.retain(|(p, _)| *p != place);
                    self.state.values.push((place.clone(), value));
                    facts.push(binary(">=", place, Term::Int(1)));
                }
                Step::Call(id, target, args) => {
                    let target = target.as_ref().map(|t| {
                        self.reference(self.location(&substitute(t, parameters, receiver)))
                    });
                    let Some(callee) = self.summaries.get(&id) else {
                        self.state.values.clear();
                        facts.clear();
                        self.state.unknown = true;
                        continue;
                    };
                    let values = callee
                        .parameters
                        .iter()
                        .cloned()
                        .zip(args.iter().enumerate().map(|(i, a)| {
                            let a = substitute(a, parameters, receiver);
                            if callee.copies.get(i) == Some(&true) {
                                self.resolve_state(&a)
                            } else {
                                self.location(&a)
                            }
                        }))
                        .collect();
                    self.apply_summary(&id, target.as_ref(), &values, facts, trace, depth + 1);
                }
                Step::Forget => {
                    self.state.values.clear();
                    facts.clear();
                    self.state.unknown = true;
                }
            }
        }
    }

    pub(super) fn finish_state_call(&mut self, node: &Node, pending: Option<Pending>) {
        let Some(pending) = pending else {
            return;
        };
        if pending.unknown {
            self.invalidate_state(None);
        }
        self.state.values = pending.writes;
        self.facts.extend(pending.facts);
        self.state.trace.extend(pending.trace);
        self.state.serial = self.state.serial.max(pending.serial);
        if let Some(value) = pending.value {
            self.state.calls.insert(node.span, value);
        }
        if let Some(claim) = pending.proof {
            let claim = self.location(&claim);
            self.facts.extend(conjuncts(claim.clone()));
            self.state
                .evidence
                .insert(node.span, Witness { claim, valid: true });
        }
    }
}
