//! Checks `requires` conditions at each direct call.
//!
//! The pass substitutes the call's arguments into the callee's condition. A
//! condition it can decide from constants, or from facts that hold on the path
//! to the call (an enclosing `if` or `while`, a guard, an early return, or the
//! caller's own `requires`), needs no check at run time. A condition it shows
//! false is a compile error. Every other call keeps the run-time check, which
//! makes the callee return Error.

use crate::{
    Parsed,
    checker::Checked,
    diagnostic::Diagnostic,
    source::{SourceFile, Span},
    syntax::{Node, SyntaxKind as K},
    types::Type,
};
use std::collections::BTreeMap;
#[path = "state_proofs.rs"]
pub(crate) mod state;

type Key = (String, String);

/// How a call meets its callee's `requires` conditions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// The compiler proved every condition; the call skips the run-time check.
    Proved,
    /// The callee checks the condition when the call runs.
    Runtime,
}

/// A condition in a form that importing modules can use without the source.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Term {
    Int(i64),
    Nat(u64),
    Double(f64),
    Bool(bool),
    Text(String),
    Name(String),
    This,
    Field(Box<Term>, String),
    Unary(String, Box<Term>),
    Binary(String, Box<Term>, Box<Term>),
    /// Anything else, such as a call. Equal only to itself.
    Opaque(String),
    Call(crate::resolution::DeclarationId, Vec<Term>),
    Case(String, String, Vec<Term>),
}

#[derive(Clone, Debug)]
pub(crate) struct Requirement {
    pub parameters: Vec<String>,
    pub term: Term,
    pub text: String,
    /// A theorem's conclusion, available inside the function after its assumptions.
    pub fact: Option<Term>,
    pub declaration: Option<crate::resolution::DeclarationId>,
    pub defaults: BTreeMap<String, Term>,
    /// The position of a proof parameter whose claim this is. The caller must establish the claim,
    /// and it has no run-time check (Q115).
    pub proof: Option<usize>,
}

#[derive(Clone, Debug, PartialEq)]
enum Constant {
    Int(i64),
    Nat(u64),
    Double(f64),
    Bool(bool),
    Text(String),
}

impl std::fmt::Display for Constant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Int(n) => write!(f, "{n}"),
            Self::Nat(n) => write!(f, "{n}"),
            Self::Double(x) => write!(f, "{x:?}"),
            Self::Bool(b) => write!(f, "{b}"),
            Self::Text(s) => write!(f, "{s:?}"),
        }
    }
}

/// What the pass found: diagnostics, how each call is checked, and the
/// requirements this file declares, for importers.
pub(crate) type Found = (
    Vec<Diagnostic>,
    BTreeMap<Span, Status>,
    BTreeMap<Key, Vec<Requirement>>,
    BTreeMap<Key, BTreeMap<crate::resolution::DeclarationId, state::Summary>>,
    Vec<crate::proof::Dispatch>,
);

pub(crate) fn check(
    parsed: &Parsed,
    checked: &Checked,
    module: &str,
    proofs: &BTreeMap<Span, Requirement>,
) -> Found {
    let state_enabled = parsed.root.descendants().any(|node| {
        (node.kind == K::ReturnType && node.child(K::Proposition).is_some())
            || (node.kind == K::ProofWitness && parsed.source.slice(node.span).starts_with("proof"))
    }) || checked.resolved_calls.values().any(|c| {
        is_proof_type(&c.signature.result)
            && matches!(c.target, crate::resolution::Callable::Declaration { .. })
    });
    let mut pass = Pass {
        parsed,
        source: &parsed.source,
        checked,
        module,
        types: checked.expression_types.iter().cloned().collect(),
        constants: BTreeMap::new(),
        local: BTreeMap::new(),
        facts: vec![],
        diagnostics: vec![],
        statuses: BTreeMap::new(),
        proofs,
        nodes: parsed.root.descendants().map(|n| (n.span, n)).collect(),
        evidence_facts: BTreeMap::new(),
        checked_evidence: std::collections::BTreeSet::new(),
        state: state::Flow::default(),
        summaries: BTreeMap::new(),
        dispatches: vec![],
        result_claim: None,
        state_enabled,
    };
    pass.collect(&parsed.root.children, None);
    pass.collect_state();
    pass.check_function_values(&parsed.root);
    pass.items(&parsed.root.children);
    pass.check_unclaimed_evidence();
    if let Some(error) = crate::proof::dispatch_override_error(
        &pass.dispatches,
        &checked.environment.definitions,
        parsed.root.span,
    ) {
        pass.diagnostics.push(error);
    }
    let exports = pass
        .summaries
        .iter()
        .filter(|(id, _)| id.file == parsed.source.name)
        .fold(
            BTreeMap::<Key, BTreeMap<_, _>>::new(),
            |mut exports, (id, summary)| {
                exports
                    .entry(summary.key.clone())
                    .or_default()
                    .insert(id.clone(), summary.clone());
                exports
            },
        );
    (
        pass.diagnostics,
        pass.statuses,
        pass.local,
        exports,
        pass.dispatches,
    )
}

struct Pass<'a> {
    parsed: &'a Parsed,
    source: &'a SourceFile,
    checked: &'a Checked,
    module: &'a str,
    types: BTreeMap<Span, Type>,
    constants: BTreeMap<String, Term>,
    local: BTreeMap<Key, Vec<Requirement>>,
    /// Conditions known to hold at the current point.
    facts: Vec<Term>,
    diagnostics: Vec<Diagnostic>,
    statuses: BTreeMap<Span, Status>,
    proofs: &'a BTreeMap<Span, Requirement>,
    nodes: BTreeMap<Span, &'a Node>,
    evidence_facts: BTreeMap<Span, Vec<Term>>,
    checked_evidence: std::collections::BTreeSet<Span>,
    state: state::Flow,
    summaries: BTreeMap<crate::resolution::DeclarationId, state::Summary>,
    dispatches: Vec<crate::proof::Dispatch>,
    result_claim: Option<Term>,
    state_enabled: bool,
}

impl Pass<'_> {
    fn text(&self, node: &Node) -> &str {
        self.source.slice(node.span)
    }

    fn named_child(&self, node: &Node) -> Option<String> {
        node.children
            .iter()
            .find(|child| child.kind == K::Name)
            .map(|child| self.text(child).to_owned())
    }

    fn qualified(&self, name: &str) -> String {
        if self.module.is_empty() {
            name.to_owned()
        } else {
            format!("{}.{name}", self.module)
        }
    }

    fn collect(&mut self, nodes: &[Node], owner: Option<&str>) {
        for node in nodes {
            match node.kind {
                K::Const => {
                    let name = node
                        .descendants()
                        .find(|n| n.kind == K::Name && n.children.is_empty())
                        .map(|n| self.text(n).to_owned());
                    if let (Some(name), Some(value)) = (name, node.children.last()) {
                        let term = self.term(value);
                        if constant(&term).is_some() {
                            self.constants.insert(name, term);
                        }
                    }
                }
                K::Function | K::Constructor => {
                    let name = if node.kind == K::Constructor {
                        "new".to_owned()
                    } else {
                        self.named_child(node).unwrap_or_default()
                    };
                    let parameters = parameter_names(self, node);
                    for requirement in node.children.iter().filter(|c| c.kind == K::Requires) {
                        let Some(condition) = requirement.children.last() else {
                            continue;
                        };
                        if let Some(proof) = self.proofs.get(&requirement.span) {
                            self.local
                                .entry((owner.unwrap_or("").to_owned(), name.clone()))
                                .or_default()
                                .push(proof.clone());
                            continue;
                        }
                        if self
                            .checked
                            .theorem_requirements
                            .contains(&requirement.span)
                        {
                            continue;
                        }
                        let term = self.term(condition);
                        let text = self.text(condition).to_owned();
                        self.local
                            .entry((owner.unwrap_or("").to_owned(), name.clone()))
                            .or_default()
                            .push(Requirement {
                                parameters: parameters.clone(),
                                term,
                                text,
                                fact: None,
                                declaration: None,
                                defaults: BTreeMap::new(),
                                proof: None,
                            });
                    }
                    // Each proof parameter's claim must be established by every caller (Q115).
                    for (position, parameter) in node
                        .child(K::ParameterList)
                        .into_iter()
                        .flat_map(|list| list.children.iter())
                        .enumerate()
                    {
                        let Some(claim) = parameter
                            .children
                            .iter()
                            .find(|c| c.kind == K::Proposition)
                            .and_then(|proposition| proposition.children.first())
                        else {
                            continue;
                        };
                        let term = self.term(claim);
                        let text = self.text(claim).to_owned();
                        self.local
                            .entry((owner.unwrap_or("").to_owned(), name.clone()))
                            .or_default()
                            .push(Requirement {
                                parameters: parameters.clone(),
                                term,
                                text,
                                fact: None,
                                declaration: None,
                                defaults: BTreeMap::new(),
                                proof: Some(position),
                            });
                    }
                }
                K::Class | K::Trait => {
                    let name = self.qualified(&self.named_child(node).unwrap_or_default());
                    self.collect(&node.children, Some(&name));
                }
                _ => {}
            }
        }
    }

    fn term(&self, node: &Node) -> Term {
        let opaque = || Term::Opaque(format!("{}@{}", self.text(node), node.span.start));
        match node.kind {
            K::Group if node.children.len() == 1 => self.term(&node.children[0]),
            K::Literal => {
                if self.types.get(&node.span) == Some(&Type::named("Nat")) {
                    self.text(node)
                        .replace('_', "")
                        .parse()
                        .map(Term::Nat)
                        .unwrap_or_else(|_| opaque())
                } else {
                    literal(self.text(node)).unwrap_or_else(opaque)
                }
            }
            K::Name => {
                let text = self.text(node);
                if text == "Zero" && self.types.get(&node.span) == Some(&Type::named("Nat")) {
                    Term::Nat(0)
                } else if let Some(Type::Named(ty, _)) = self.types.get(&node.span)
                    && self
                        .checked
                        .environment
                        .definitions
                        .get(ty)
                        .is_some_and(|d| {
                            d.kind == K::Enum
                                && d.variants
                                    .iter()
                                    .any(|v| v.name == text && v.fields.is_empty())
                        })
                {
                    Term::Case(ty.clone(), text.into(), vec![])
                } else if text == "this" {
                    Term::This
                } else if let Some(value) = self.constants.get(text) {
                    value.clone()
                } else {
                    Term::Name(text.to_owned())
                }
            }
            K::Member if node.children.len() == 2 => Term::Field(
                Box::new(self.term(&node.children[0])),
                self.text(&node.children[1]).to_owned(),
            ),
            K::Unary => {
                let operator = node.children.iter().find(|c| c.kind == K::Operator);
                let operand = node.children.iter().find(|c| c.kind != K::Operator);
                match (operator, operand) {
                    (Some(operator), Some(operand)) => {
                        Term::Unary(self.text(operator).to_owned(), Box::new(self.term(operand)))
                    }
                    _ => opaque(),
                }
            }
            K::Binary if node.children.len() == 3 => Term::Binary(
                self.text(&node.children[1]).to_owned(),
                Box::new(self.term(&node.children[0])),
                Box::new(self.term(&node.children[2])),
            ),
            K::Call => {
                if let Some(term) = self.array_term(node) {
                    return term;
                }
                if let Some(call) = self.checked.resolved_calls.get(&node.span)
                    && !call.partial
                    && !call.signature.effects
                    && !call.signature.receiver_mutates
                    && (call
                        .signature
                        .parameters
                        .iter()
                        .all(|p| p.passing == crate::types::Passing::Borrow)
                        || matches!(
                            &call.target,
                            crate::resolution::Callable::Declaration {
                                kind: crate::resolution::DeclarationKind::Constructor,
                                ..
                            }
                        )
                        || matches!(&call.target,
                            crate::resolution::Callable::Builtin(name) if name.starts_with("record.")))
                {
                    let supplied = node
                        .descendants()
                        .map(|n| (n.span, n))
                        .collect::<BTreeMap<_, _>>();
                    let mut args = vec![];
                    for arg in &call.arguments {
                        match arg {
                            crate::resolution::Argument::Supplied {
                                expression,
                                hole: false,
                            } => args.push(self.term(supplied[expression])),
                            _ => return opaque(),
                        }
                    }
                    match &call.target {
                        crate::resolution::Callable::Declaration {
                            id,
                            kind: crate::resolution::DeclarationKind::Function,
                            ..
                        } => Term::Call(id.clone(), args),
                        // A pure method is a call on its receiver, so `s.size()` in a condition and
                        // `this.size()` in a claim are the same term once the receiver is known (Q115).
                        crate::resolution::Callable::Declaration {
                            id,
                            kind: crate::resolution::DeclarationKind::Method,
                            ..
                        } => match node
                            .children
                            .first()
                            .filter(|c| c.kind == K::Member)
                            .and_then(|c| c.children.first())
                        {
                            Some(base) => {
                                let mut all = vec![self.term(base)];
                                all.extend(args);
                                Term::Call(id.clone(), all)
                            }
                            None => opaque(),
                        },
                        crate::resolution::Callable::Declaration {
                            id,
                            kind: crate::resolution::DeclarationKind::Constructor,
                            ..
                        } => Term::Call(id.clone(), args),
                        crate::resolution::Callable::Builtin(name)
                            if let Some(record) = name.strip_prefix("record.") =>
                        {
                            let Some((owner, _)) = record.split_once('|') else {
                                return opaque();
                            };
                            Term::Case(owner.to_owned(), "new".into(), args)
                        }
                        crate::resolution::Callable::Declaration {
                            kind: crate::resolution::DeclarationKind::Variant,
                            name,
                            ..
                        } => {
                            let case = name.rsplit('.').next().unwrap_or(name);
                            match self.types.get(&node.span) {
                                Some(Type::Named(ty, _))
                                    if ty == "Nat" && case == "Succ" && args.len() == 1 =>
                                {
                                    Term::Unary("natSucc".into(), Box::new(args.remove(0)))
                                }
                                Some(Type::Named(ty, _)) => {
                                    Term::Case(ty.clone(), case.into(), args)
                                }
                                _ => opaque(),
                            }
                        }
                        _ => opaque(),
                    }
                } else {
                    opaque()
                }
            }
            _ => opaque(),
        }
    }

    fn items(&mut self, nodes: &[Node]) {
        for node in nodes {
            match node.kind {
                K::Function | K::Constructor => self.function(node),
                K::Class | K::Trait => {
                    for member in &node.children {
                        if matches!(member.kind, K::Function | K::Constructor) {
                            self.function(member);
                        }
                    }
                }
                K::Enum | K::Alias | K::Const | K::Import | K::Theorem => {}
                _ => self.walk(node),
            }
        }
    }

    fn function(&mut self, node: &Node) {
        let saved = std::mem::take(&mut self.facts);
        let state = std::mem::take(&mut self.state);
        let result_claim = self.result_claim.take();
        self.result_claim = node
            .child(K::ReturnType)
            .and_then(|r| r.child(K::Proposition))
            .and_then(|p| p.children.first())
            .map(|p| self.term(p));
        self.bind_proof_parameters(node);
        // The function's own conditions hold throughout its body.
        for requirement in node.children.iter().filter(|c| c.kind == K::Requires) {
            if let Some(proof) = self.proofs.get(&requirement.span) {
                self.facts.extend(conjuncts(proof.term.clone()));
                self.facts.extend(conjuncts(proof.fact.clone().unwrap()));
                continue;
            }
            if self
                .checked
                .theorem_requirements
                .contains(&requirement.span)
            {
                continue;
            }
            if let Some(condition) = requirement.children.last() {
                self.walk(condition);
                let term = self.term(condition);
                self.facts.extend(conjuncts(term));
            }
        }
        if let Some(body) = function_body(node) {
            self.walk(body);
            if self.types.get(&body.span) != Some(&Type::Never) {
                self.check_proof_return(body);
            }
        }
        for condition in node.children.iter().filter(|n| n.kind == K::Ensures) {
            // A postcondition is not an assumption about the function's body.
            self.facts.clear();
            for expression in &condition.children {
                self.walk(expression);
            }
        }
        self.facts = saved;
        self.state = state;
        self.result_claim = result_claim;
    }

    fn walk(&mut self, node: &Node) {
        if self.checked.theorem_evidence.contains(&node.span) {
            self.evidence_facts
                .entry(node.span)
                .or_insert_with(|| self.facts.clone());
            // The theorem call and its ordinary arguments are compile-time
            // evidence. The proof kernel checks their purity and definedness.
            return;
        }
        match node.kind {
            K::ProofWitness => self.check_witness(node),
            K::Binding => {
                for child in node.children.iter().filter(|n| n.kind != K::PatternName) {
                    self.walk(child);
                }
                self.bind_state(node);
            }
            K::Return => {
                for child in &node.children {
                    self.walk(child);
                }
                if let Some(value) = node.children.first() {
                    self.check_proof_return(value);
                }
            }
            K::Function | K::Constructor => self.function(node),
            K::Class | K::Trait => self.items(std::slice::from_ref(node)),
            K::If => {
                let Some(condition) = node.children.first() else {
                    return;
                };
                self.walk(condition);
                let term = self.term(condition);
                let mark = self.facts.len();
                let before_state = self.state.clone();
                if let Some(then) = node.children.get(1) {
                    self.facts.extend(conjuncts(term.clone()));
                    self.walk(then);
                    self.facts.truncate(mark);
                }
                let then_state = self.state.clone();
                self.state = before_state.clone();
                if let Some(otherwise) = node.children.get(2) {
                    self.facts.extend(negate(&term));
                    self.walk(otherwise);
                    self.facts.truncate(mark);
                }
                self.state = state::Flow::join(then_state, self.state.clone());
            }
            K::While => {
                let Some(condition) = node.children.first() else {
                    return;
                };
                self.forget_changed_in(node);
                self.walk(condition);
                let before_state = self.state.clone();
                let mark = self.facts.len();
                self.facts.extend(conjuncts(self.term(condition)));
                for child in &node.children[1..] {
                    self.walk(child);
                }
                self.facts.truncate(mark);
                self.state = state::Flow::join(before_state, self.state.clone());
            }
            K::For => {
                self.forget_changed_in(node);
                let before_state = self.state.clone();
                let mark = self.facts.len();
                let body = node.children.last();
                let header = &node.children[..node.children.len().saturating_sub(1)];
                // The step runs after the body, so its change cannot undo the
                // condition inside the body.
                for child in header.iter().filter(|c| c.kind != K::Assignment) {
                    self.walk(child);
                    if child.kind == K::Binary {
                        self.facts.extend(conjuncts(self.term(child)));
                    }
                }
                if let Some(body) = body {
                    self.walk(body);
                }
                for child in header.iter().filter(|c| c.kind == K::Assignment) {
                    self.walk(child);
                }
                self.facts.truncate(mark);
                self.state = state::Flow::join(before_state, self.state.clone());
            }
            K::Match => {
                if let Some(scrutinee) = node.children.first() {
                    self.walk(scrutinee);
                }
                let before_state = self.state.clone();
                let mut after_state = None;
                for arm in node.children.iter().filter(|c| c.kind == K::MatchArm) {
                    self.state = before_state.clone();
                    let mark = self.facts.len();
                    if let (Some(scrutinee), Some(pattern)) =
                        (node.children.first(), arm.children.first())
                    {
                        let value = self.term(scrutinee);
                        if self.types.get(&scrutinee.span) == Some(&Type::named("Nat"))
                            && pattern.kind == K::PatternConstructor
                        {
                            match pattern.children.first().map(|n| self.text(n)) {
                                Some("Zero") => self.facts.push(Term::Binary(
                                    "==".into(),
                                    Box::new(value),
                                    Box::new(Term::Nat(0)),
                                )),
                                Some("Succ") => self.facts.push(Term::Binary(
                                    ">".into(),
                                    Box::new(value),
                                    Box::new(Term::Nat(0)),
                                )),
                                _ => {}
                            }
                        } else if pattern.kind == K::PatternLiteral {
                            self.facts.push(Term::Binary(
                                "==".into(),
                                Box::new(value),
                                Box::new(self.term(pattern)),
                            ));
                        }
                    }
                    for child in &arm.children {
                        self.walk(child);
                        if child.kind == K::Guard
                            && let Some(condition) = child.children.last()
                        {
                            self.facts.extend(conjuncts(self.term(condition)));
                        }
                    }
                    self.facts.truncate(mark);
                    after_state = Some(match after_state {
                        None => self.state.clone(),
                        Some(before) => state::Flow::join(before, self.state.clone()),
                    });
                }
                if let Some(state) = after_state {
                    self.state = state;
                }
            }
            K::Block => {
                let before_state = self.enter_state_block();
                let mark = self.facts.len();
                for child in &node.children {
                    self.walk(child);
                    // After `if c { return }`, the rest of the block knows c is false.
                    if child.kind == K::If
                        && child.children.len() == 2
                        && diverges(&child.children[1])
                    {
                        let term = self.term(&child.children[0]);
                        self.facts.extend(negate(&term));
                    }
                }
                self.facts.truncate(mark);
                self.leave_state_block(before_state, node);
            }
            K::Lambda => {
                // A lambda runs later, after the facts around it may have changed.
                let saved = std::mem::take(&mut self.facts);
                let state = std::mem::take(&mut self.state);
                let claim = self.result_claim.take();
                for child in &node.children {
                    self.walk(child);
                }
                self.facts = saved;
                self.state = state;
                self.result_claim = claim;
            }
            K::Spawn | K::Scope | K::Using | K::Unsafe => {
                self.invalidate_state(None);
                for child in &node.children {
                    self.walk(child);
                }
                self.invalidate_state(None);
            }
            K::Assignment => {
                for child in node.children.iter().skip(1) {
                    self.walk(child);
                }
                if let Some(target) = node.children.first() {
                    let value = node.children.last().map(|n| self.snapshot_value(n));
                    self.forget(target);
                    if let Some(value) = value {
                        self.assign_state(target, value);
                    }
                    if let Some(value) = node.children.last() {
                        self.assign_evidence(target, value);
                    }
                }
            }
            K::Call => {
                for child in &node.children {
                    self.walk(child);
                    if matches!(child.kind, K::Argument | K::NamedArgument)
                        && let Some(value) = child.children.last()
                    {
                        self.capture_argument(value);
                    }
                }
                self.call(node);
                let pending = self.prepare_state_call(node);
                self.forget_mutation(node);
                self.finish_state_call(node, pending);
            }
            K::Pipe => {
                for child in &node.children {
                    self.walk(child);
                }
                self.pipe(node);
                // The pipe's implicit argument positions do not yet carry state summaries.
                self.invalidate_state(None);
            }
            K::Binary
                if node.children.len() == 3
                    && matches!(self.text(&node.children[1]), "&&" | "||") =>
            {
                self.walk(&node.children[0]);
                let before = self.state.clone();
                self.walk(&node.children[2]);
                self.state = state::Flow::join(before, self.state.clone());
            }
            _ => {
                for child in &node.children {
                    self.walk(child);
                }
            }
        }
    }

    /// Drops facts about a place that an assignment changes.
    fn forget(&mut self, target: &Node) {
        self.invalidate_state(Some(self.term(target)));
        match self.term(target) {
            Term::Name(name) => self.facts.retain(|fact| !mentions(fact, &name)),
            _ => self.facts.retain(|fact| !has_field(fact)),
        }
    }

    /// A call that changes its receiver or a mut argument may change fields.
    fn forget_mutation(&mut self, call: &Node) {
        let receiver = call
            .children
            .first()
            .filter(|c| c.kind == K::Member)
            .and_then(|c| c.children.first());
        let changes_receiver =
            receiver.is_some_and(|r| self.checked.mutated_receivers.contains(&r.span));
        let changes_argument = call.children.iter().skip(1).any(|argument| {
            argument.children.last().is_some_and(|value| {
                self.checked.argument_modes.get(&value.span) == Some(&crate::types::Passing::Mut)
            })
        });
        for value in call
            .children
            .iter()
            .skip(1)
            .filter_map(|a| a.children.last())
        {
            if self.checked.argument_modes.get(&value.span) == Some(&crate::types::Passing::Mut) {
                self.forget(value);
            }
        }
        if changes_receiver || changes_argument {
            self.facts.retain(|fact| !has_field(fact));
            if let Some(receiver) = receiver.filter(|_| changes_receiver) {
                self.invalidate_state(Some(self.term(receiver)));
            }
        } else if self
            .checked
            .resolved_calls
            .get(&call.span)
            .is_some_and(|c| c.signature.receiver_mutates)
        {
            self.invalidate_state(None);
        }
    }

    /// A loop body can run after its own changes, so facts about anything it
    /// changes do not hold inside it.
    fn forget_changed_in(&mut self, node: &Node) {
        for child in node.descendants() {
            match child.kind {
                K::Assignment => {
                    if let Some(target) = child.children.first() {
                        self.forget(target);
                    }
                }
                K::Call => self.forget_mutation(child),
                _ => {}
            }
        }
    }

    fn class_of(&self, span: Span) -> Option<String> {
        let mut ty = self.types.get(&span)?;
        while let Type::Borrow(inner) | Type::StoredView(inner) = ty {
            ty = inner;
        }
        match ty {
            Type::Named(name, _) | Type::Constructor(name, _) => Some(name.clone()),
            _ => None,
        }
    }

    fn requirements(&self, key: &Key) -> Option<Vec<Requirement>> {
        self.local
            .get(key)
            .or_else(|| {
                self.checked
                    .environment
                    .calls
                    .get(key)
                    .map(|facts| &facts.requires)
                    .filter(|requires| !requires.is_empty())
            })
            .cloned()
    }

    fn method_requirements(&self, class: &str, method: &str) -> Option<(Key, Vec<Requirement>)> {
        let mut class = class.to_owned();
        for _ in 0..64 {
            let key = (class.clone(), method.to_owned());
            if let Some(found) = self.requirements(&key) {
                return Some((key, found));
            }
            match &self.checked.environment.definitions.get(&class)?.parent {
                Some(Type::Named(parent, _)) => class = parent.clone(),
                _ => return None,
            }
        }
        None
    }

    /// Finds the requirements of a callee and the receiver they read as `this`.
    fn callee(&self, callee: &Node) -> Option<(Vec<Requirement>, Option<Term>)> {
        match callee.kind {
            K::Group if callee.children.len() == 1 => self.callee(&callee.children[0]),
            K::Member if callee.children.len() == 2 => {
                let base = &callee.children[0];
                if let Some(Type::Namespace(namespace)) = self.types.get(&base.span) {
                    let name = format!("{namespace}.{}", self.text(&callee.children[1]));
                    return self.named_callee(&name);
                }
                let class = self.class_of(base.span)?;
                let method = self.text(&callee.children[1]);
                let (_, found) = self.method_requirements(&class, method)?;
                Some((found, Some(self.term(base))))
            }
            K::Name => self.named_callee(self.text(callee)),
            _ => None,
        }
    }

    fn named_callee(&self, name: &str) -> Option<(Vec<Requirement>, Option<Term>)> {
        let environment = &self.checked.environment;
        let key = match environment.names.get(name) {
            Some(id)
                if environment
                    .definitions
                    .get(id)
                    .is_some_and(|d| d.kind == K::Class) =>
            {
                (id.clone(), "new".to_owned())
            }
            _ => {
                let qualified = environment.values.get(name).and_then(|symbol| {
                    let function = match &symbol.ty {
                        Type::Function(function) => function,
                        Type::Overloaded(functions) => functions.first()?,
                        _ => return None,
                    };
                    match &function.callable {
                        Some(crate::resolution::Callable::Declaration {
                            id,
                            name,
                            kind: crate::resolution::DeclarationKind::Function,
                            ..
                        }) if id.file != self.source.name => Some(name.clone()),
                        _ => None,
                    }
                });
                (String::new(), qualified.unwrap_or_else(|| name.to_owned()))
            }
        };
        Some((self.requirements(&key)?, None))
    }

    fn call(&mut self, node: &Node) {
        let Some(callee) = node.children.first() else {
            return;
        };
        let mut arguments = vec![];
        for argument in &node.children[1..] {
            let Some(value) = argument.children.last() else {
                continue;
            };
            if value.kind == K::Hole {
                // A partial call is checked when the resulting function is called.
                if self
                    .callee(callee)
                    .is_none_or(|(rs, _)| !rs.iter().any(|r| r.fact.is_some()))
                {
                    return;
                }
            }
            let name = (argument.kind == K::NamedArgument)
                .then(|| argument.children.first().map(|n| self.text(n).to_owned()))
                .flatten();
            arguments.push((name, self.term(value)));
        }
        self.check_call(node, callee, arguments);
    }

    fn pipe(&mut self, node: &Node) {
        let (Some(left), Some(right)) = (node.children.first(), node.children.last()) else {
            return;
        };
        let left = self.term(left);
        if right.kind == K::Call {
            let Some(callee) = right.children.first() else {
                return;
            };
            let mut arguments = vec![];
            let mut filled = false;
            for argument in &right.children[1..] {
                let Some(value) = argument.children.last() else {
                    continue;
                };
                let name = (argument.kind == K::NamedArgument)
                    .then(|| argument.children.first().map(|n| self.text(n).to_owned()))
                    .flatten();
                if value.kind == K::Hole {
                    if filled {
                        return;
                    }
                    filled = true;
                    arguments.push((name, left.clone()));
                } else {
                    arguments.push((name, self.term(value)));
                }
            }
            // The inner call was a partial call and recorded nothing; the pipe completes it.
            self.check_call(node, callee, arguments);
        } else {
            self.check_call(node, right, vec![(None, left)]);
        }
    }

    fn check_call(
        &mut self,
        outer_call: &Node,
        callee: &Node,
        arguments: Vec<(Option<String>, Term)>,
    ) {
        let span = outer_call.span;
        let Some((mut requirements, receiver)) = self.callee(callee) else {
            return;
        };
        if let Some(call) = self.checked.resolved_calls.get(&span)
            && let crate::resolution::Callable::Declaration { id, .. } = &call.target
        {
            for requirement in requirements.iter().filter(|r| {
                (r.fact.is_some() && r.declaration.as_ref() == Some(id))
                    || r.proof.is_some_and(|position| {
                        r.parameters.len() == call.signature.parameters.len()
                            && call
                                .signature
                                .parameters
                                .get(position)
                                .is_some_and(|parameter| is_proof_type(&parameter.ty))
                    })
            }) {
                let mut values = BTreeMap::new();
                for ((parameter, argument), name) in call
                    .signature
                    .parameters
                    .iter()
                    .zip(&call.arguments)
                    .zip(&requirement.parameters)
                {
                    let value = match argument {
                        crate::resolution::Argument::Supplied {
                            expression,
                            hole: false,
                        } => self.nodes.get(expression).map(|n| self.term(n)),
                        crate::resolution::Argument::Default => {
                            requirement.defaults.get(name).cloned()
                        }
                        _ => None,
                    }
                    .unwrap_or_else(|| {
                        Term::Opaque(format!(
                            "unsupplied:{}",
                            parameter.name.as_deref().unwrap_or(name)
                        ))
                    });
                    values.insert(name.clone(), value);
                }
                let condition = substitute(&requirement.term, &values, receiver.as_ref());
                let evidence = requirement.proof.and_then(|position| {
                    let crate::resolution::Argument::Supplied {
                        expression,
                        hole: false,
                    } = call.arguments.get(position)?
                    else {
                        return None;
                    };
                    let mut direct = *self.nodes.get(expression)?;
                    while direct.kind == K::Group && direct.children.len() == 1 {
                        direct = &direct.children[0];
                    }
                    self.checked
                        .theorem_evidence
                        .contains(&direct.span)
                        .then_some(direct.span)
                });
                let state_evidence = requirement
                    .proof
                    .and_then(|position| call.arguments.get(position))
                    .and_then(|arg| match arg {
                        crate::resolution::Argument::Supplied { expression, .. } => {
                            self.nodes.get(expression).copied()
                        }
                        _ => None,
                    })
                    .and_then(|argument| self.check_state_argument(argument, &condition));
                if let Some(evidence_span) = evidence {
                    if let Some(argument) = self.nodes.get(&evidence_span) {
                        self.checked_evidence.insert(evidence_span);
                        if let Err(diagnostic) = crate::proof::check_runtime_evidence(
                            self.parsed,
                            self.checked,
                            outer_call,
                            argument,
                            Some(&condition),
                            &self.facts,
                        ) {
                            self.diagnostics.push(diagnostic);
                        }
                    }
                } else if (requirement.proof.is_some() && state_evidence != Some(true))
                    || (requirement.proof.is_none() && self.prove(&condition) != Some(true))
                {
                    self.diagnostics.push(Diagnostic::new("E_PROOF_ASSUMPTION",format!("This call cannot establish the assumptions of `requires {}`.",requirement.text),span).with_hint("Establish them with a caller requirement or an if/match condition before calling; theorem assumptions have no run-time fallback."));
                }
            }
        }
        requirements.retain(|r| r.fact.is_none() && r.proof.is_none());
        if requirements.is_empty() {
            self.statuses.insert(span, Status::Proved);
            return;
        }
        let positional = arguments.iter().filter(|(name, _)| name.is_none()).count();
        let fits = |parameters: &Vec<String>| {
            parameters.len() >= positional
                && arguments
                    .iter()
                    .filter_map(|(name, _)| name.as_ref())
                    .all(|name| parameters.contains(name))
        };
        let mut lists = requirements
            .iter()
            .filter(|r| fits(&r.parameters))
            .map(|r| r.parameters.clone())
            .collect::<Vec<_>>();
        lists.dedup();
        if lists.len() != 1 {
            // Overloads with different parameters: the callee checks at run time.
            self.statuses.insert(span, Status::Runtime);
            return;
        }
        let parameters = &lists[0];
        let mut values = BTreeMap::new();
        for (index, (name, term)) in arguments.into_iter().enumerate() {
            let parameter = match name {
                Some(name) => name,
                None => match parameters.get(index) {
                    Some(parameter) => parameter.clone(),
                    None => continue,
                },
            };
            values.insert(parameter, term);
        }
        let mut status = Status::Proved;
        for requirement in requirements.iter().filter(|r| r.parameters == *parameters) {
            let condition = substitute(&requirement.term, &values, receiver.as_ref());
            match self.prove(&condition) {
                Some(true) => {}
                Some(false) => {
                    let shown = parameters
                        .iter()
                        .filter_map(|p| {
                            values
                                .get(p)
                                .and_then(constant)
                                .map(|value| format!("{p} is {value}"))
                        })
                        .collect::<Vec<_>>();
                    let detail = if shown.is_empty() {
                        "on this path the condition is always false".to_owned()
                    } else {
                        format!("here {}", shown.join(" and "))
                    };
                    self.diagnostics.push(
                        Diagnostic::new(
                            "E_CONTRACT_VIOLATED",
                            format!("This call breaks `requires {}`: {detail}.", requirement.text),
                            span,
                        )
                        .with_hint("Pass a value that meets the condition, or test it with if before the call."),
                    );
                    status = Status::Runtime;
                }
                None => status = Status::Runtime,
            }
        }
        self.statuses.insert(span, status);
    }

    /// Some(true) when the condition always holds here, Some(false) when it
    /// never does, None when only the run-time check can tell.
    fn prove(&self, condition: &Term) -> Option<bool> {
        let normalized = self.resolve_state(condition);
        if normalized != *condition
            && let Some(result) = self.prove(&normalized)
        {
            return Some(result);
        }
        if let Some(Constant::Bool(value)) = evaluate(condition) {
            return Some(value);
        }
        if let Term::Unary(op, inner) = condition
            && op == "!"
        {
            let negated = negate(inner);
            if !negated.is_empty() && negated.iter().all(|p| p != condition) {
                return negated
                    .iter()
                    .map(|p| self.prove(p))
                    .try_fold(true, |a, b| b.map(|b| a && b));
            }
        }
        if let Term::Binary(operator, left, right) = condition
            && matches!(operator.as_str(), "&&" | "||")
        {
            let (left, right) = (self.prove(left), self.prove(right));
            return match (operator.as_str(), left, right) {
                ("&&", Some(false), _) | ("&&", _, Some(false)) => Some(false),
                ("&&", Some(true), Some(true)) => Some(true),
                ("||", Some(true), _) | ("||", _, Some(true)) => Some(true),
                ("||", Some(false), Some(false)) => Some(false),
                _ => None,
            };
        }
        if self
            .facts
            .iter()
            .any(|fact| implies(&self.resolve_state(fact), condition))
        {
            return Some(true);
        }
        if negate(condition)
            .iter()
            .any(|negated| self.facts.iter().any(|fact| implies(fact, negated)))
        {
            return Some(false);
        }
        None
    }

    fn check_unclaimed_evidence(&mut self) {
        let pending = self
            .checked
            .theorem_evidence
            .iter()
            .filter(|span| !self.checked_evidence.contains(span))
            .copied()
            .collect::<Vec<_>>();
        for span in pending {
            let Some(argument) = self.nodes.get(&span) else {
                continue;
            };
            self.checked_evidence.insert(span);
            let facts = self.evidence_facts.get(&span).cloned().unwrap_or_default();
            if let Err(diagnostic) = crate::proof::check_runtime_evidence(
                self.parsed,
                self.checked,
                argument,
                argument,
                None,
                &facts,
            ) {
                self.diagnostics.push(diagnostic);
            }
        }
    }

    /// Conditional theorem obligations cannot disappear when a function's identity
    /// is erased by storing or passing it. A checked wrapper can supply the facts.
    fn check_function_values(&mut self, root: &Node) {
        let mut heads = std::collections::BTreeSet::new();
        for call in root
            .descendants()
            .filter(|n| n.kind == K::Call || n.kind == K::Pipe)
        {
            if let Some(mut head) = if call.kind == K::Pipe {
                call.children.last()
            } else {
                call.children.first()
            } {
                while head.kind == K::Group {
                    head = &head.children[0];
                }
                if matches!(head.kind, K::Name | K::Member) {
                    heads.insert(head.span);
                }
            }
        }
        let requirements = self
            .local
            .values()
            .chain(self.checked.environment.calls.values().map(|f| &f.requires))
            .flatten()
            .filter(|r| r.fact.is_some() && evaluate(&r.term) != Some(Constant::Bool(true)))
            .cloned()
            .collect::<Vec<_>>();
        for node in root
            .descendants()
            .filter(|n| matches!(n.kind, K::Name | K::Member) && !heads.contains(&n.span))
        {
            let Some(ty) = self.types.get(&node.span) else {
                continue;
            };
            let signatures = match ty {
                Type::Function(f) => std::slice::from_ref(f),
                Type::Overloaded(fs) => fs.as_slice(),
                _ => continue,
            };
            if signatures.iter().any(|f| is_proof_type(&f.result)
                && matches!(&f.callable,Some(crate::resolution::Callable::Declaration {id,..}) if self.summaries.get(id).is_some_and(|s| s.proof_result()))) {
                self.diagnostics.push(Diagnostic::new("E_PROOF_FUNCTION_VALUE","A function value cannot yet retain a proof-result claim.",node.span)
                    .with_hint("Call the proof-producing declaration directly so its claim and receiver remain known."));
            }
            if signatures.iter().any(|f|matches!(&f.callable,Some(crate::resolution::Callable::Declaration {id,..}) if requirements.iter().any(|r|r.declaration.as_ref()==Some(id)))) {
                self.diagnostics.push(Diagnostic::new("E_PROOF_FUNCTION_VALUE","A function value cannot yet retain conditional theorem requirements.",node.span).with_hint("Use a wrapper that establishes the assumptions and calls the function directly; universal theorem requirements can be passed as values."));
            }
        }
    }
}

fn parameter_names(pass: &Pass, node: &Node) -> Vec<String> {
    node.child(K::ParameterList)
        .map(|list| {
            list.children
                .iter()
                .filter_map(|p| pass.named_child(p))
                .collect()
        })
        .unwrap_or_default()
}

fn is_header(kind: K) -> bool {
    matches!(
        kind,
        K::Name
            | K::Modifier
            | K::Attribute
            | K::TypeParameters
            | K::ParameterList
            | K::ReturnType
            | K::Effects
            | K::Requires
            | K::Ensures
    )
}

fn function_body(node: &Node) -> Option<&Node> {
    node.children.last().filter(|n| {
        !is_header(n.kind)
            || (n.kind == K::Name
                && node
                    .child(K::Name)
                    .is_some_and(|header| header.span != n.span))
    })
}

fn diverges(node: &Node) -> bool {
    match node.kind {
        K::Return | K::Break | K::Continue => true,
        K::Block => node.children.iter().any(diverges),
        K::If => {
            node.children.len() == 3 && diverges(&node.children[1]) && diverges(&node.children[2])
        }
        _ => false,
    }
}

fn literal(text: &str) -> Option<Term> {
    match text {
        "true" => return Some(Term::Bool(true)),
        "false" => return Some(Term::Bool(false)),
        _ => {}
    }
    if let Some(inner) = text.strip_prefix('"').and_then(|t| t.strip_suffix('"')) {
        return (!inner.contains('\\') && !inner.contains('{'))
            .then(|| Term::Text(inner.to_owned()));
    }
    let digits = text.replace('_', "");
    if let Some(hex) = digits
        .strip_prefix("0x")
        .or_else(|| digits.strip_prefix("0X"))
    {
        return i64::from_str_radix(hex, 16).ok().map(Term::Int);
    }
    if digits.contains('.') || digits.contains('e') || digits.contains('E') {
        return digits.parse().ok().map(Term::Double);
    }
    digits.parse().ok().map(Term::Int)
}

fn constant(term: &Term) -> Option<Constant> {
    evaluate(term)
}

fn evaluate(term: &Term) -> Option<Constant> {
    use Constant as C;
    Some(match term {
        Term::Int(n) => C::Int(*n),
        Term::Nat(n) => C::Nat(*n),
        Term::Double(x) => C::Double(*x),
        Term::Bool(b) => C::Bool(*b),
        Term::Text(s) => C::Text(s.clone()),
        Term::Unary(operator, operand) => match (operator.as_str(), evaluate(operand)?) {
            ("-", C::Int(n)) => C::Int(n.checked_neg()?),
            ("-", C::Double(x)) => C::Double(-x),
            ("!", C::Bool(b)) => C::Bool(!b),
            ("natSucc", C::Nat(n)) => C::Nat(n.checked_add(1)?),
            _ => return None,
        },
        Term::Binary(operator, left, right) => {
            let operator = operator.as_str();
            if matches!(operator, "&&" | "||") {
                let left = evaluate(left);
                if let Some(C::Bool(b)) = left
                    && (b == (operator == "||"))
                {
                    return Some(C::Bool(b));
                }
                let right = evaluate(right);
                if let Some(C::Bool(b)) = right
                    && (b == (operator == "||"))
                {
                    return Some(C::Bool(b));
                }
                return match (left?, right?) {
                    (C::Bool(_), C::Bool(b)) => Some(C::Bool(b)),
                    _ => None,
                };
            }
            match (evaluate(left)?, evaluate(right)?) {
                (C::Nat(a), C::Nat(b)) => match operator {
                    "+" => C::Nat(a.checked_add(b)?),
                    "-" => C::Nat(a.checked_sub(b)?),
                    op => C::Bool(match op {
                        "==" => a == b,
                        "!=" => a != b,
                        "<" => a < b,
                        "<=" => a <= b,
                        ">" => a > b,
                        ">=" => a >= b,
                        _ => return None,
                    }),
                },
                (C::Int(a), C::Int(b)) => match operator {
                    "+" => C::Int(a.checked_add(b)?),
                    "-" => C::Int(a.checked_sub(b)?),
                    "*" => C::Int(a.checked_mul(b)?),
                    "/" => C::Int(a.checked_div(b)?),
                    "%" => C::Int(a.checked_rem(b)?),
                    _ => C::Bool(compare(operator, a.cmp(&b))?),
                },
                (C::Double(a), C::Double(b)) => match operator {
                    "+" => C::Double(a + b),
                    "-" => C::Double(a - b),
                    "*" => C::Double(a * b),
                    "/" => C::Double(a / b),
                    _ => C::Bool(compare(operator, a.partial_cmp(&b)?)?),
                },
                (C::Text(a), C::Text(b)) => C::Bool(compare(operator, a.cmp(&b))?),
                (C::Bool(a), C::Bool(b)) => match operator {
                    "==" => C::Bool(a == b),
                    "!=" => C::Bool(a != b),
                    _ => return None,
                },
                _ => return None,
            }
        }
        _ => return None,
    })
}

fn compare(operator: &str, ordering: std::cmp::Ordering) -> Option<bool> {
    use std::cmp::Ordering::*;
    Some(match operator {
        "==" => ordering == Equal,
        "!=" => ordering != Equal,
        "<" => ordering == Less,
        "<=" => ordering != Greater,
        ">" => ordering == Greater,
        ">=" => ordering != Less,
        _ => return None,
    })
}

fn conjuncts(term: Term) -> Vec<Term> {
    match term {
        Term::Binary(operator, left, right) if operator == "&&" => {
            let mut found = conjuncts(*left);
            found.extend(conjuncts(*right));
            found
        }
        other => vec![other],
    }
}

fn negate(term: &Term) -> Vec<Term> {
    match term {
        Term::Binary(operator, left, right) => {
            let flipped = match operator.as_str() {
                "<" => ">=",
                "<=" => ">",
                ">" => "<=",
                ">=" => "<",
                "==" => "!=",
                "!=" => "==",
                // Not (a || b) is both not a and not b.
                "||" => {
                    let mut found = negate(left);
                    found.extend(negate(right));
                    return found;
                }
                _ => return vec![],
            };
            vec![Term::Binary(flipped.into(), left.clone(), right.clone())]
        }
        Term::Unary(operator, operand) if operator == "!" => conjuncts((**operand).clone()),
        Term::Name(_) | Term::Field(_, _) => {
            vec![Term::Unary("!".into(), Box::new(term.clone()))]
        }
        _ => vec![],
    }
}

fn substitute(term: &Term, values: &BTreeMap<String, Term>, receiver: Option<&Term>) -> Term {
    match term {
        // A name that is not a parameter belongs to the callee, so it must not
        // match a caller variable that happens to share it.
        Term::Name(name) => values
            .get(name)
            .cloned()
            .unwrap_or_else(|| Term::Opaque(format!("callee:{name}"))),
        Term::This => receiver
            .cloned()
            .unwrap_or_else(|| Term::Opaque("callee:this".into())),
        Term::Field(base, field) => {
            Term::Field(Box::new(substitute(base, values, receiver)), field.clone())
        }
        Term::Unary(operator, operand) => Term::Unary(
            operator.clone(),
            Box::new(substitute(operand, values, receiver)),
        ),
        Term::Binary(operator, left, right) => Term::Binary(
            operator.clone(),
            Box::new(substitute(left, values, receiver)),
            Box::new(substitute(right, values, receiver)),
        ),
        Term::Call(id, args) => Term::Call(
            id.clone(),
            args.iter()
                .map(|t| substitute(t, values, receiver))
                .collect(),
        ),
        Term::Case(ty, case, args) => Term::Case(
            ty.clone(),
            case.clone(),
            args.iter()
                .map(|t| substitute(t, values, receiver))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn opaque_free(term: &Term) -> bool {
    match term {
        Term::Opaque(_) => false,
        Term::Call(_, args) | Term::Case(_, _, args) => args.iter().all(opaque_free),
        Term::Field(base, _) | Term::Unary(_, base) => opaque_free(base),
        Term::Binary(_, left, right) => opaque_free(left) && opaque_free(right),
        _ => true,
    }
}

fn mentions(term: &Term, name: &str) -> bool {
    match term {
        Term::Name(found) => found == name,
        Term::Call(_, args) | Term::Case(_, _, args) => args.iter().any(|t| mentions(t, name)),
        Term::Field(base, _) | Term::Unary(_, base) => mentions(base, name),
        Term::Binary(_, left, right) => mentions(left, name) || mentions(right, name),
        _ => false,
    }
}

fn has_field(term: &Term) -> bool {
    match term {
        Term::Field(_, _) => true,
        // An ordinary pure function may read mutable fields through a borrowed
        // object parameter. Its old result is not a fact after a mutation.
        Term::Call(_, _) => true,
        Term::Case(_, _, args) => args.iter().any(has_field),
        Term::Unary(_, base) => has_field(base),
        Term::Binary(_, left, right) => has_field(left) || has_field(right),
        _ => false,
    }
}

/// A comparison of a term with a number, written with the number on the right.
fn bound(term: &Term) -> Option<(&Term, &'static str, f64)> {
    let Term::Binary(operator, left, right) = term else {
        return None;
    };
    let number = |t: &Term| match evaluate(t)? {
        Constant::Int(n) if n.unsigned_abs() < 1 << 53 => Some(n as f64),
        Constant::Nat(n) if n < 1 << 53 => Some(n as f64),
        Constant::Double(x) if x.is_finite() => Some(x),
        _ => None,
    };
    let operator = match operator.as_str() {
        "<" => "<",
        "<=" => "<=",
        ">" => ">",
        ">=" => ">=",
        "==" => "==",
        "!=" => "!=",
        _ => return None,
    };
    if let Some(value) = number(right) {
        return Some((left, operator, value));
    }
    let mirrored = match operator {
        "<" => ">",
        "<=" => ">=",
        ">" => "<",
        ">=" => "<=",
        other => other,
    };
    number(left).map(|value| (right.as_ref(), mirrored, value))
}

/// Whether knowing `fact` guarantees `goal`.
/// Whether a comparison's bound is a whole number. Kestrel compares values of one type only, so
/// a whole-number bound means a whole-number subject.
fn integral(term: &Term) -> bool {
    let Term::Binary(_, left, right) = term else {
        return false;
    };
    // The constant may stand on either side, as in `0 < x`.
    [left, right]
        .iter()
        .any(|side| matches!(evaluate(side), Some(Constant::Int(_) | Constant::Nat(_))))
}

/// Rewrites a strict bound over whole numbers as the equivalent non-strict one.
fn tighten(operator: &'static str, value: f64, integral: bool) -> (&'static str, f64) {
    match (operator, integral) {
        (">", true) => (">=", value + 1.0),
        ("<", true) => ("<=", value - 1.0),
        _ => (operator, value),
    }
}

fn implies(fact: &Term, goal: &Term) -> bool {
    if fact == goal && opaque_free(goal) {
        return true;
    }
    let (Some((x, f, a)), Some((y, g, b))) = (bound(fact), bound(goal)) else {
        return false;
    };
    if x != y || !opaque_free(x) {
        return false;
    }
    // Over whole numbers, x > a is x >= a + 1, so a fact such as `size > 0` proves `size >= 1`.
    let (f, a) = tighten(f, a, integral(fact));
    let (g, b) = tighten(g, b, integral(goal));
    // The values the fact allows, as a lower and an upper end (value, strict).
    let (low, high) = match f {
        ">" => (Some((a, true)), None),
        ">=" => (Some((a, false)), None),
        "<" => (None, Some((a, true))),
        "<=" => (None, Some((a, false))),
        "==" => (Some((a, false)), Some((a, false))),
        _ => return false,
    };
    let above = |b: f64, strict: bool| {
        low.is_some_and(|(v, s): (f64, bool)| v > b || (v == b && (s || !strict)))
    };
    let below = |b: f64, strict: bool| {
        high.is_some_and(|(v, s): (f64, bool)| v < b || (v == b && (s || !strict)))
    };
    match g {
        ">" => above(b, true),
        ">=" => above(b, false),
        "<" => below(b, true),
        "<=" => below(b, false),
        "==" => f == "==" && a == b,
        "!=" => above(b, true) || below(b, true),
        _ => false,
    }
}

/// A proof parameter's type, which has no run-time value (Q115).
fn is_proof_type(ty: &Type) -> bool {
    matches!(ty, Type::Named(name, args) if name == "Proof" && args.is_empty())
}
