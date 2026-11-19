//! Move and borrow checks, run after type checking succeeds.
//!
//! Binding a whole owned value, passing it to a `move` parameter, storing it
//! in an owned field or putting it in a collection or enum case hands it over.
//! Parameters, `this`, field reads, pattern bindings and loop items are
//! borrowed views. The pass follows each function in source order, merges the
//! moved state where branches and loops join, and then checks that no borrow
//! is returned, stored or used after its owner moved or changed.

use crate::{
    Parsed,
    checker::Checked,
    diagnostic::Diagnostic,
    source::{SourceFile, Span},
    syntax::{Node, SyntaxKind as K},
    types::{Passing, Type, copies},
};
use std::collections::{BTreeMap, BTreeSet};

/// Captures of `this` use this id, since `this` has no binding.
const THIS: usize = usize::MAX;
/// The hidden binding that owns one step of a dynamic `for` loop (Q67).
const STEP: &str = "<step>";
/// Bound repeated summary and loop analysis without accepting unfinished facts.
const ANALYSIS_WORK_LIMIT: usize = 1_000_000;
const ANALYSIS_WORK_CAP: usize = 8_000_000;

type Key = (String, String);
type Moved = BTreeMap<(usize, Vec<String>), Move>;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Loan {
    Local(usize),
    Param(String),
    /// Borrowed dependencies inside copied storage, rather than that storage's address.
    Captures(String),
    This,
    Temporary,
    Global(usize),
    /// An item lent by a traversal until its next step (Q67). The id is the
    /// cursor binding, or the hidden binding for one `for` loop step.
    Step(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Root {
    Binding(usize),
    This,
    Global(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Place {
    root: Root,
    path: Vec<String>,
    /// Reached from inside a lambda that captures the root.
    captured: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Origin {
    Owned,
    Param,
    View,
    Part(Place),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Binding {
    mutable: bool,
    readonly: bool,
    name: String,
    frame: usize,
    origin: Origin,
    loans: BTreeSet<Loan>,
    lambda: bool,
    changed_captures: BTreeSet<usize>,
    dependency_copy: bool,
    declared: Span,
    lifetime: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Move {
    span: Span,
    maybe: bool,
}

#[derive(Clone, Debug, Default)]
struct Value {
    readonly: bool,
    place: Option<Place>,
    /// Set by a binding context when the new name reads part of a place.
    part: Option<Place>,
    loans: BTreeSet<Loan>,
    lambda: bool,
    /// Enclosing bindings a saved local function may change when called.
    changed_captures: BTreeSet<usize>,
    copy: bool,
    function_copy: bool,
    dependency_copy: bool,
}

impl Value {
    fn owned(copy: bool) -> Self {
        Self {
            copy,
            ..Self::default()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ctx {
    /// Read only; the value stays where it is.
    Borrow,
    /// A new name takes the value: whole owned values move, parts become views.
    Bind,
    /// The receiver needs ownership: move parameters, owned fields, collections.
    Strict,
    /// The value leaves the function.
    Return,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Boundary {
    None,
    Lambda(usize),
    Function,
}

#[derive(Clone)]
struct Scope {
    lifetime: Span,
    names: BTreeMap<String, usize>,
    boundary: Boundary,
}

struct Frame {
    id: usize,
    key: Option<Key>,
    constructor: bool,
    moving_lambda: bool,
}

struct Event {
    owner: usize,
    through: Option<usize>,
    path: Vec<String>,
    span: Span,
    what: &'static str,
    loops: Vec<Span>,
    frame: usize,
    branches: Vec<BranchStep>,
}

/// One arm of an if or match that encloses some code. Two pieces of code in different arms of
/// the same construct never run in the same pass.
#[derive(Clone, Copy, PartialEq)]
struct BranchStep {
    node: usize,
    arm: usize,
}

#[derive(Default)]
struct LoopFlow {
    breaks: Vec<Moved>,
    continues: Vec<Moved>,
}

/// Borrow facts about the callables of one file, for importers.
pub(crate) struct Exports {
    pub parameters: BTreeMap<Key, Vec<Vec<String>>>,
    /// Parameter names, or `this`, that each result may borrow.
    pub borrows: BTreeMap<Key, BTreeSet<String>>,
    /// Return expressions that transfer storage from an owned local or parameter.
    pub owned_returns: BTreeSet<Span>,
}

pub(crate) fn check(
    parsed: &Parsed,
    checked: &Checked,
    module: &str,
) -> (Vec<Diagnostic>, Exports) {
    let _copy_definitions = crate::types::copy_definitions(&checked.environment.definitions);
    let nodes = parsed.root.descendants().count().max(1);
    let mut pass = Pass {
        source: &parsed.source,
        module,
        checked,
        types: checked.expression_types.iter().cloned().collect(),
        signatures: BTreeMap::new(),
        summaries: BTreeMap::new(),
        next: BTreeMap::new(),
        bindings: vec![],
        scopes: vec![],
        captures: vec![],
        local_changes: BTreeMap::new(),
        local_results: BTreeMap::new(),
        local_return_stack: vec![],
        captured_globals: vec![],
        frames: vec![],
        frame_count: 0,
        moved: Moved::new(),
        diverged: false,
        loops: vec![],
        uses: BTreeMap::new(),
        use_paths: BTreeMap::new(),
        returns: vec![],
        branch_path: vec![],
        events: vec![],
        loop_spans: vec![],
        tasks: vec![],
        task_results: vec![],
        diagnostics: vec![],
        owned_returns: BTreeSet::new(),
        global_captures: BTreeMap::new(),
        default_values: BTreeMap::new(),
        active_defaults: BTreeSet::new(),
        validating_defaults: false,
        foreign_globals: BTreeSet::new(),
        call_borrows: vec![],
        // A fixed-point round costs one charge per node. Larger modules need more
        // rounds before their summaries settle, so scale the budget with the node
        // count while keeping a cap that still rejects pathological call chains.
        work: ANALYSIS_WORK_LIMIT.max(nodes.saturating_mul(200).min(ANALYSIS_WORK_CAP)),
        exhausted: false,
    };
    pass.collect_signatures(&parsed.root.children, None);
    // A function returning a borrow changes what its callers borrow, so the
    // summaries are recomputed until they stop changing.
    loop {
        if !pass.charge(nodes, parsed.root.span) {
            break;
        }
        pass.run(&parsed.root);
        if pass.exhausted {
            break;
        }
        for (key, loans) in &pass.summaries {
            pass.next
                .entry(key.clone())
                .or_default()
                .extend(loans.iter().cloned());
        }
        let stable = pass.next == pass.summaries;
        pass.summaries = std::mem::take(&mut pass.next);
        if stable {
            break;
        }
    }
    let borrows = pass
        .summaries
        .iter()
        .map(|(key, loans)| {
            let names = loans
                .iter()
                .filter_map(|loan| match loan {
                    Loan::Param(name) => Some(name.clone()),
                    Loan::Captures(name) => Some(format!("$captures:{name}")),
                    Loan::This => Some("this".to_owned()),
                    _ => None,
                })
                .collect();
            (key.clone(), names)
        })
        .collect();
    let exports = Exports {
        parameters: pass.signatures.clone(),
        borrows,
        owned_returns: pass.owned_returns,
    };
    (pass.diagnostics, exports)
}

struct Pass<'a> {
    source: &'a SourceFile,
    module: &'a str,
    checked: &'a Checked,
    types: BTreeMap<Span, Type>,
    /// Parameter names of each function, method and constructor in this file.
    signatures: BTreeMap<Key, Vec<Vec<String>>>,
    /// Which parameters, or `this`, a function's result may borrow.
    summaries: BTreeMap<Key, BTreeSet<Loan>>,
    next: BTreeMap<Key, BTreeSet<Loan>>,
    bindings: Vec<Binding>,
    scopes: Vec<Scope>,
    captures: Vec<BTreeMap<usize, bool>>,
    /// Captured changes keyed by declaration, so same-named functions stay distinct.
    local_changes: BTreeMap<Span, BTreeSet<usize>>,
    local_results: BTreeMap<Span, BTreeSet<usize>>,
    local_return_stack: Vec<Span>,
    captured_globals: Vec<BTreeSet<usize>>,
    frames: Vec<Frame>,
    frame_count: usize,
    moved: Moved,
    diverged: bool,
    loops: Vec<LoopFlow>,
    uses: BTreeMap<usize, Vec<Span>>,
    /// The frame and the branch arms around each use, keyed by the use's start.
    use_paths: BTreeMap<usize, (usize, Vec<BranchStep>)>,
    /// Returns that contain no break or continue, with their frames.
    returns: Vec<(Span, usize)>,
    /// The if and match arms around the code being walked.
    branch_path: Vec<BranchStep>,
    /// Moves, reassignments and changes of owned bindings, for borrow conflicts,
    /// with the loops that enclose each one.
    events: Vec<Event>,
    loop_spans: Vec<Span>,
    /// Places the tasks of each enclosing `scope` use, innermost last.
    tasks: Vec<Vec<Task>>,
    task_results: Vec<BTreeSet<Loan>>,
    diagnostics: Vec<Diagnostic>,
    owned_returns: BTreeSet<Span>,
    global_captures: BTreeMap<crate::resolution::DeclarationId, BTreeSet<String>>,
    default_values: BTreeMap<Span, Vec<Option<Node>>>,
    active_defaults: BTreeSet<Span>,
    validating_defaults: bool,
    /// Script storage read by a callback that foreign code may retain.
    foreign_globals: BTreeSet<usize>,
    call_borrows: Vec<CallBorrow>,
    work: usize,
    exhausted: bool,
}

struct Task {
    owner: usize,
    path: Vec<String>,
    exclusive: bool,
    span: Span,
}

struct CallBorrow {
    events: std::ops::Range<usize>,
    own_events: std::ops::Range<usize>,
    capture_events: BTreeSet<usize>,
    /// A whole-variable method receiver. Its call starts after the arguments run, so
    /// an argument may change it in place, as in `c.f(c.g())` (REVIEW 1.6).
    receiver: bool,
    place: Option<Place>,
    loans: BTreeSet<Loan>,
    frame: usize,
    span: Span,
}

impl Pass<'_> {
    fn charge(&mut self, amount: usize, span: Span) -> bool {
        if self.exhausted {
            return false;
        }
        if self.work < amount {
            self.exhausted = true;
            self.error(
                span,
                "E_BORROW_LIMIT",
                "Borrow analysis exceeded its work limit before the ownership and borrowing facts stabilized.".into(),
                "Split long dependency chains into smaller modules, or simplify loops that carry many borrows.",
            );
            return false;
        }
        self.work -= amount;
        true
    }

    fn text(&self, node: &Node) -> &str {
        self.source.slice(node.span)
    }

    fn named_child(&self, node: &Node) -> Option<String> {
        node.children
            .iter()
            .find(|child| child.kind == K::Name)
            .map(|child| self.text(child).to_owned())
    }

    fn modifier(&self, node: &Node, name: &str) -> bool {
        node.children
            .iter()
            .any(|child| child.kind == K::Modifier && self.text(child) == name)
    }

    fn error(&mut self, span: Span, code: &'static str, message: String, hint: &str) {
        if !self
            .diagnostics
            .iter()
            .any(|d| d.code == code && d.span == span)
        {
            self.diagnostics
                .push(Diagnostic::new(code, message, span).with_hint(hint));
        }
    }

    fn qualified(&self, name: &str) -> String {
        if self.module.is_empty() {
            name.to_owned()
        } else {
            format!("{}.{name}", self.module)
        }
    }

    /// Parameter names of a callable declared here or in an imported module.
    fn parameters(&self, key: &Key) -> Vec<Vec<String>> {
        self.signatures
            .get(key)
            .or_else(|| {
                self.checked
                    .environment
                    .calls
                    .get(key)
                    .map(|facts| &facts.parameters)
            })
            .cloned()
            .unwrap_or_default()
    }

    /// What a callable's result may borrow, from this file or an imported module.
    fn summary(&self, key: &Key) -> Option<BTreeSet<Loan>> {
        if self.signatures.contains_key(key) {
            let mut loans = self.summaries.get(key).cloned();
            if let Some(current) = self.next.get(key) {
                loans
                    .get_or_insert_with(BTreeSet::new)
                    .extend(current.iter().cloned());
            }
            return loans;
        }
        let facts = self.checked.environment.calls.get(key)?;
        Some(
            facts
                .borrows
                .iter()
                .map(|name| {
                    if let Some(parameter) = name.strip_prefix("$captures:") {
                        Loan::Captures(parameter.to_owned())
                    } else if name == "this" {
                        Loan::This
                    } else {
                        Loan::Param(name.clone())
                    }
                })
                .collect(),
        )
    }

    fn line(&self, span: Span) -> usize {
        self.source.position(span.start).0
    }

    fn collect_signatures(&mut self, nodes: &[Node], owner: Option<&str>) {
        for node in nodes {
            match node.kind {
                K::Function | K::Constructor => {
                    let defaults = node
                        .child(K::ParameterList)
                        .map(|list| {
                            list.children
                                .iter()
                                .map(|parameter| {
                                    parameter
                                        .children
                                        .last()
                                        .filter(|value| is_expression(value.kind))
                                        .cloned()
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    self.default_values.insert(node.span, defaults);
                    let name = if node.kind == K::Constructor {
                        "new".to_owned()
                    } else {
                        self.named_child(node).unwrap_or_default()
                    };
                    let parameters = node
                        .child(K::ParameterList)
                        .map(|list| {
                            list.children
                                .iter()
                                .filter_map(|p| self.named_child(p))
                                .collect()
                        })
                        .unwrap_or_default();
                    self.signatures
                        .entry((owner.unwrap_or("").to_owned(), name))
                        .or_default()
                        .push(parameters);
                }
                K::Class | K::Trait => {
                    let name = self.qualified(&self.named_child(node).unwrap_or_default());
                    self.collect_signatures(&node.children, Some(&name));
                }
                _ => {}
            }
        }
    }

    fn run(&mut self, root: &Node) {
        self.bindings.clear();
        self.scopes.clear();
        self.captures.clear();
        self.local_changes.clear();
        self.local_results.clear();
        self.local_return_stack.clear();
        self.captured_globals.clear();
        self.frames.clear();
        self.frame_count = 0;
        self.moved.clear();
        self.diverged = false;
        self.loops.clear();
        self.uses.clear();
        self.use_paths.clear();
        self.returns.clear();
        self.branch_path.clear();
        self.events.clear();
        self.loop_spans.clear();
        self.tasks.clear();
        self.diagnostics.clear();
        self.owned_returns.clear();
        self.foreign_globals.clear();
        self.call_borrows.clear();
        self.enter_frame(None, false);
        self.scopes.push(Scope {
            lifetime: root.span,
            names: BTreeMap::new(),
            boundary: Boundary::None,
        });
        self.items(&root.children, Ctx::Borrow);
        self.scopes.pop();
        self.frames.pop();
        self.borrow_conflicts();
    }

    fn enter_frame(&mut self, key: Option<Key>, constructor: bool) {
        self.frames.push(Frame {
            id: self.frame_count,
            key,
            constructor,
            moving_lambda: false,
        });
        self.frame_count += 1;
    }

    fn frame(&self) -> &Frame {
        self.frames.last().unwrap()
    }

    fn push_scope(&mut self, boundary: Boundary, lifetime: Span) {
        self.scopes.push(Scope {
            lifetime,
            names: BTreeMap::new(),
            boundary,
        });
    }

    fn declare(&mut self, name: &str, origin: Origin, value: &Value, span: Span) {
        let loans = match &origin {
            Origin::Part(place) => self.place_loans(place),
            _ => value.loans.clone(),
        };
        let id = self.bindings.len();
        self.bindings.push(Binding {
            mutable: false,
            readonly: value.readonly,
            name: name.to_owned(),
            frame: self.frame().id,
            origin,
            loans,
            lambda: value.lambda,
            changed_captures: value.changed_captures.clone(),
            dependency_copy: value.dependency_copy,
            declared: span,
            lifetime: self.scopes.last().map(|s| s.lifetime).unwrap_or(span),
        });
        self.scopes
            .last_mut()
            .unwrap()
            .names
            .insert(name.to_owned(), id);
    }

    /// Declares a name bound by a binding context, from what that context produced.
    fn declare_bound(&mut self, name: &str, value: &Value, span: Span) {
        let origin = if let Some(part) = &value.part {
            Origin::Part(part.clone())
        } else if value.lambda || !value.loans.is_empty() {
            Origin::View
        } else {
            Origin::Owned
        };
        self.declare(name, origin, value, span);
    }

    fn callable_changes(&self, node: &Node) -> BTreeSet<usize> {
        let node = ungroup(node);
        if node.kind == K::Name {
            for scope in self.scopes.iter().rev() {
                if let Some(id) = scope.names.get(self.text(node)) {
                    return self.bindings[*id].changed_captures.clone();
                }
            }
        }
        if let Some(Type::Function(function)) = self.type_of(node.span)
            && let Some(crate::resolution::Callable::Declaration { id, .. }) = &function.callable
            && id.file == self.source.name
        {
            return self
                .local_changes
                .get(&id.span)
                .cloned()
                .unwrap_or_default();
        }
        BTreeSet::new()
    }

    fn aliased_changes(
        &self,
        node: &Node,
        aliases: &BTreeMap<String, BTreeSet<usize>>,
    ) -> BTreeSet<usize> {
        let node = ungroup(node);
        if node.kind == K::Name
            && let Some(changes) = aliases.get(self.text(node))
        {
            return changes.clone();
        }
        let mut changes = self.callable_changes(node);
        if matches!(
            node.kind,
            K::If | K::Match | K::MatchArm | K::Block | K::Call | K::Argument | K::NamedArgument
        ) {
            for child in &node.children {
                changes.extend(self.aliased_changes(child, aliases));
            }
        }
        changes
    }

    /// The enclosing variables a local function changes: assignment targets, mutating
    /// receivers and `mut` arguments rooted in a name the function does not declare,
    /// plus whatever the local functions it calls change.
    fn local_function_changes(&self, node: &Node) -> BTreeSet<usize> {
        let mut declared = BTreeSet::new();
        let mut changed = BTreeSet::new();
        let mut captured = BTreeSet::new();
        let mut aliases: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
        for part in node.descendants() {
            if let Some(roots) = self.checked.mutation_roots.get(&part.span) {
                changed.extend(roots.iter().cloned());
            }
            match part.kind {
                K::Parameter | K::PatternName | K::ForBinding => {
                    if let Some(name) = self.named_child(part) {
                        declared.insert(name);
                    }
                }
                K::Binding => {
                    if let (Some(pattern), Some(value)) =
                        (part.children.first(), part.children.last())
                    {
                        let changes = self.aliased_changes(value, &aliases);
                        for name in pattern.descendants().filter(|n| n.kind == K::Name) {
                            aliases
                                .entry(self.text(name).to_owned())
                                .or_default()
                                .extend(changes.iter().copied());
                        }
                    }
                    if let Some(pattern) = part.children.first() {
                        for name in pattern.descendants().filter(|n| n.kind == K::Name) {
                            declared.insert(self.text(name).to_owned());
                        }
                    }
                }
                K::Assignment => {
                    if let (Some(target), Some(value)) =
                        (part.children.first(), part.children.last())
                        && ungroup(target).kind == K::Name
                    {
                        let changes = self.aliased_changes(value, &aliases);
                        aliases
                            .entry(self.text(ungroup(target)).to_owned())
                            .or_default()
                            .extend(changes);
                    }
                    if let Some(root) = part
                        .children
                        .first()
                        .and_then(|p| place_root_text(p, self.source))
                    {
                        changed.insert(root.to_owned());
                    }
                }
                K::Call => {
                    if let Some(callee) = part.children.first().map(ungroup) {
                        if callee.kind == K::Member
                            && self
                                .checked
                                .mutated_receivers
                                .contains(&callee.children[0].span)
                            && let Some(root) = place_root_text(&callee.children[0], self.source)
                        {
                            changed.insert(root.to_owned());
                        }
                        captured.extend(self.aliased_changes(callee, &aliases));
                    }
                    for argument in &part.children[1..] {
                        if let Some(value) = argument.children.last() {
                            captured.extend(self.aliased_changes(value, &aliases));
                        }
                        if let Some(value) = argument.children.last()
                            && self.checked.argument_modes.get(&value.span) == Some(&Passing::Mut)
                            && let Some(root) = place_root_text(value, self.source)
                        {
                            changed.insert(root.to_owned());
                        }
                    }
                }
                _ => {}
            }
        }
        changed.retain(|name| !declared.contains(name));
        for name in changed {
            if name == "this" {
                captured.insert(THIS);
            } else if let Some(id) = self
                .scopes
                .iter()
                .rev()
                .find_map(|scope| scope.names.get(&name))
            {
                captured.insert(*id);
            }
        }
        captured
    }
    /// Whether a diagnostic already points at this span, so one mistake reports once.
    fn errors_at(&self, span: Span) -> bool {
        self.diagnostics.iter().any(|d| d.span == span)
    }
    fn resolve(&mut self, name: &str, copy: bool) -> Option<(usize, bool, bool)> {
        let mut lambdas: Vec<usize> = vec![];
        let mut function = false;
        for scope in self.scopes.iter().rev() {
            if let Some(id) = scope.names.get(name) {
                let id = *id;
                if !function {
                    for index in &lambdas {
                        let entry = self.captures[*index].entry(id).or_insert(true);
                        *entry &= copy;
                    }
                } else if !copy || !self.bindings[id].loans.is_empty() {
                    for index in &lambdas {
                        self.captured_globals[*index].insert(id);
                    }
                }
                return Some((id, !lambdas.is_empty(), function));
            }
            match scope.boundary {
                Boundary::Lambda(index) if !function => lambdas.push(index),
                Boundary::Function => function = true,
                _ => {}
            }
        }
        None
    }

    fn this_place(&mut self) -> Place {
        let mut lambdas = vec![];
        for scope in self.scopes.iter().rev() {
            match scope.boundary {
                Boundary::Lambda(index) => lambdas.push(index),
                Boundary::Function => break,
                Boundary::None => {}
            }
        }
        for index in &lambdas {
            self.captures[*index].insert(THIS, false);
        }
        Place {
            root: Root::This,
            path: vec![],
            captured: !lambdas.is_empty(),
        }
    }

    fn type_of(&self, span: Span) -> Option<&Type> {
        self.types.get(&span)
    }

    fn is_copy(&self, span: Span) -> bool {
        match self.type_of(span) {
            None => true,
            Some(ty) => {
                copies(ty)
                    || matches!(
                        ty,
                        Type::Overloaded(_)
                            | Type::Constructor(_, _)
                            | Type::Variable(_)
                            | Type::Never
                            | Type::Invalid
                    )
            }
        }
    }

    /// Copied storage can contain borrowed fields or callbacks with borrowed captures.
    fn holds_dependencies(&self, ty: &Type) -> bool {
        self.dependencies_within(ty, &mut BTreeSet::new(), 64)
    }

    fn dependencies_within(&self, ty: &Type, seen: &mut BTreeSet<String>, depth: usize) -> bool {
        if depth == 0 {
            return true;
        }
        match ty {
            Type::Function(_) | Type::Overloaded(_) | Type::Borrow(_) | Type::StoredView(_) => true,
            Type::Named(name, _)
                if matches!(name.as_str(), "ArenaRef" | "CPointer" | "CConstPointer") =>
            {
                false
            }
            Type::Named(name, args) => {
                if args
                    .iter()
                    .any(|ty| self.dependencies_within(ty, seen, depth - 1))
                {
                    return true;
                }
                if !seen.insert(name.clone()) {
                    return false;
                }
                self.checked
                    .environment
                    .definitions
                    .get(name)
                    .is_some_and(|definition| {
                        definition
                            .fields
                            .values()
                            .chain(definition.variants.iter().flat_map(|case| &case.fields))
                            .chain(definition.parent.iter())
                            .chain(definition.alias.iter())
                            .any(|ty| self.dependencies_within(ty, seen, depth - 1))
                    })
            }
            Type::Tuple(items) | Type::Union(items) => items
                .iter()
                .any(|ty| self.dependencies_within(ty, seen, depth - 1)),
            Type::Dynamic(inner)
            | Type::Indexed(inner, _)
            | Type::Lazy(inner, _)
            | Type::Cursor(inner, _)
            | Type::Guard(inner)
            | Type::Associated(inner, _) => self.dependencies_within(inner, seen, depth - 1),
            // A `T : Copy` parameter holds no borrows.
            Type::Parameter(name) | Type::Rigid(name) => !crate::types::copy_parameter(name),
            Type::Variable(_) => true,
            _ => false,
        }
    }

    /// Resolves a name or field path without reading it.
    fn place_of(&mut self, node: &Node) -> Option<Place> {
        match node.kind {
            K::Group if node.children.len() == 1 => self.place_of(&node.children[0]),
            K::Name => {
                let name = self.text(node).to_owned();
                if name == "this" {
                    return Some(self.this_place());
                }
                let copy = self.is_copy(node.span);
                let (id, captured, global) = self.resolve(&name, copy)?;
                if global {
                    return Some(Place {
                        root: Root::Global(id),
                        path: vec![],
                        captured: false,
                    });
                }
                Some(Place {
                    root: Root::Binding(id),
                    path: vec![],
                    captured,
                })
            }
            K::Member if node.children.len() == 2 => {
                let mut place = self.place_of(&node.children[0])?;
                place.path.push(self.text(&node.children[1]).to_owned());
                Some(place)
            }
            _ => None,
        }
    }

    /// Follows bindings that name part of another place to the owning place.
    fn canonical(&self, place: &Place) -> Place {
        self.canonical_within(place, 32)
    }

    fn canonical_within(&self, place: &Place, depth: usize) -> Place {
        if depth > 0
            && let Root::Binding(id) = place.root
            && let Origin::Part(inner) = &self.bindings[id].origin
        {
            let mut found = self.canonical_within(inner, depth - 1);
            found.path.extend(place.path.iter().cloned());
            found.captured |= place.captured;
            return found;
        }
        place.clone()
    }

    fn place_loans(&self, place: &Place) -> BTreeSet<Loan> {
        self.place_loans_within(place, 32)
    }

    fn place_loans_within(&self, place: &Place, depth: usize) -> BTreeSet<Loan> {
        match &place.root {
            Root::This => [Loan::This].into(),
            Root::Global(id) => [Loan::Global(*id)].into(),
            Root::Binding(id) => {
                let binding = &self.bindings[*id];
                match &binding.origin {
                    Origin::Owned => {
                        let mut loans = binding.loans.clone();
                        loans.insert(Loan::Local(*id));
                        loans
                    }
                    Origin::Param => [Loan::Param(binding.name.clone())].into(),
                    Origin::View => binding.loans.clone(),
                    Origin::Part(inner) if depth > 0 => self.place_loans_within(inner, depth - 1),
                    Origin::Part(_) => BTreeSet::new(),
                }
            }
        }
    }

    /// A method read without a call. Stored function values have no fixed callable.
    fn method_value(&self, node: &Node) -> bool {
        matches!(
            self.type_of(node.span),
            Some(Type::Function(f)) if matches!(
                f.callable,
                Some(crate::resolution::Callable::Declaration {
                    kind: crate::resolution::DeclarationKind::Method
                        | crate::resolution::DeclarationKind::TraitMethod,
                    ..
                })
            )
        )
    }

    fn callable_loans(&self, value: &Value) -> BTreeSet<Loan> {
        let Some(place) = &value.place else {
            return value.loans.clone();
        };
        let place = self.canonical(place);
        match place.root {
            Root::Binding(id) => match &self.bindings[id].origin {
                Origin::Owned | Origin::View => self.bindings[id].loans.clone(),
                Origin::Param => [Loan::Captures(self.bindings[id].name.clone())].into(),
                Origin::Part(_) => self.place_loans(&place),
            },
            Root::This => [Loan::This].into(),
            Root::Global(id) => self.bindings[id]
                .loans
                .iter()
                .map(|loan| match loan {
                    Loan::Local(owner) => Loan::Global(*owner),
                    other => other.clone(),
                })
                .collect(),
        }
    }

    fn value_loans(&self, value: &Value) -> BTreeSet<Loan> {
        if let Some(place) = &value.place {
            self.place_loans(place)
        } else if !value.loans.is_empty() || value.copy {
            value.loans.clone()
        } else {
            [Loan::Temporary].into()
        }
    }

    fn describe(&self, loans: &BTreeSet<Loan>) -> String {
        let names = loans
            .iter()
            .filter_map(|loan| match loan {
                Loan::Local(id) => Some(format!("`{}`", self.bindings[*id].name)),
                Loan::Param(name) | Loan::Captures(name) => Some(format!("`{name}`")),
                Loan::This => Some("`this`".to_owned()),
                Loan::Temporary => Some("a temporary value".to_owned()),
                Loan::Global(_) => None,
                Loan::Step(id) if self.bindings[*id].name == STEP => {
                    Some("the current loop step".to_owned())
                }
                Loan::Step(id) => {
                    Some(format!("the current step of `{}`", self.bindings[*id].name))
                }
            })
            .collect::<Vec<_>>();
        if names.is_empty() {
            "another value".to_owned()
        } else {
            names.join(" and ")
        }
    }

    /// Reads a place, reporting a use of anything already moved.
    fn check_moved(&mut self, place: &Place, span: Span) {
        let mut places = vec![place.clone()];
        let canonical = self.canonical(place);
        if canonical != *place {
            places.push(canonical);
        }
        for place in places {
            let Root::Binding(id) = place.root else {
                continue;
            };
            self.uses.entry(id).or_default().push(span);
            let frame = self.frame().id;
            let branches = self.branch_path.clone();
            self.use_paths
                .entry(span.start)
                .or_insert((frame, branches));
            let found = self
                .moved
                .iter()
                .find(|((owner, path), _)| {
                    *owner == id && (path.starts_with(&place.path) || place.path.starts_with(path))
                })
                .map(|(_, found)| *found);
            if let Some(found) = found {
                let name = self.bindings[id].name.clone();
                let line = self.line(found.span);
                let in_loop = self
                    .loop_spans
                    .iter()
                    .any(|l| l.start <= span.start && found.span.end <= l.end);
                let message = if found.span.start >= span.start && in_loop {
                    format!(
                        "`{name}` was moved at line {line} in an earlier pass of this loop, so it cannot be used here."
                    )
                } else if found.span.start >= span.start {
                    // `n.next = Some(n)` moves n while computing the value, before the target is used.
                    format!(
                        "`{name}` is moved by the value on the right, so it cannot be used as the target here."
                    )
                } else if found.maybe {
                    format!(
                        "`{name}` may have been moved at line {line}, so it cannot be used here."
                    )
                } else {
                    format!("`{name}` was moved at line {line}, so it cannot be used here.")
                };
                self.error(
                    span,
                    "E_USE_AFTER_MOVE",
                    message,
                    "Use the value before handing it over, pass it to a borrowing parameter instead of a move parameter, or assign it a new value first.",
                );
                return;
            }
        }
    }

    fn mark_moved(&mut self, place: &Place, span: Span) {
        if let Root::Binding(id) = place.root {
            self.moved
                .insert((id, place.path.clone()), Move { span, maybe: false });
        }
    }

    fn event(&mut self, place: &Place, span: Span, what: &'static str) {
        if what == "changed"
            && let Root::Binding(id) = place.root
            && self.bindings[id].readonly
        {
            self.error(
                span,
                "E_MUT_PARAMETER",
                "A returned borrow is read-only and cannot be changed through this name."
                    .to_owned(),
                "Change the owner through an explicit mut parameter instead.",
            );
        }
        if place.captured {
            return;
        }
        let through = match place.root {
            Root::Binding(id) => Some(id),
            _ => None,
        };
        let canonical = self.canonical(place);
        let places = if what == "changed" {
            self.accessed_places(&canonical)
        } else if let Root::Binding(id) = canonical.root
            && matches!(self.bindings[id].origin, Origin::Owned | Origin::Param)
        {
            vec![(id, canonical.path)]
        } else {
            vec![]
        };
        let frame = self.frame().id;
        let branches = self.branch_path.clone();
        for (owner, path) in places {
            self.events.push(Event {
                owner,
                through,
                path,
                span,
                what,
                loops: self.loop_spans.clone(),
                frame,
                branches: branches.clone(),
            });
        }
    }

    /// Changing aliases conservatively retain all of their possible owners.
    fn accessed_places(&self, place: &Place) -> Vec<(usize, Vec<String>)> {
        let place = self.canonical(place);
        let Root::Binding(id) = place.root else {
            return vec![];
        };
        let mut places = vec![(id, place.path)];
        if matches!(self.bindings[id].origin, Origin::View) {
            for loan in &self.bindings[id].loans {
                let owner = match loan {
                    Loan::Local(owner) | Loan::Global(owner) | Loan::Step(owner) => Some(*owner),
                    Loan::Param(name) | Loan::Captures(name) => {
                        self.bindings.iter().position(|binding| {
                            binding.frame == self.bindings[id].frame
                                && binding.name == *name
                                && matches!(binding.origin, Origin::Param)
                        })
                    }
                    _ => None,
                };
                if let Some(owner) = owner
                    && !places
                        .iter()
                        .any(|(id, path)| *id == owner && path.is_empty())
                {
                    places.push((owner, vec![]));
                }
            }
        }
        places
    }

    fn apply(&mut self, span: Span, value: Value, ctx: Ctx) -> Value {
        if value.copy {
            return Value::owned(true);
        }
        match ctx {
            Ctx::Borrow => value,
            Ctx::Bind => self.bind(span, value),
            Ctx::Strict => {
                self.take(span, &value);
                Value::owned(false)
            }
            Ctx::Return => {
                self.give_back(span, &value);
                Value::owned(false)
            }
        }
    }

    fn bind(&mut self, span: Span, value: Value) -> Value {
        let Some(place) = &value.place else {
            // A temporary borrowed by a new name lives as long as that name.
            return value;
        };
        if place.path.is_empty()
            && !place.captured
            && let Root::Binding(id) = place.root
        {
            match self.bindings[id].origin.clone() {
                Origin::Owned => {
                    self.take(span, &value);
                    return Value::owned(false);
                }
                Origin::View => {
                    let binding = &self.bindings[id];
                    return Value {
                        readonly: binding.readonly,
                        loans: binding.loans.clone(),
                        lambda: binding.lambda,
                        changed_captures: binding.changed_captures.clone(),
                        ..Value::default()
                    };
                }
                _ => {}
            }
        }
        Value {
            readonly: value.readonly,
            part: Some(self.canonical(place)),
            loans: self.place_loans(place),
            changed_captures: value.changed_captures,
            ..Value::default()
        }
    }

    /// Hands a value over to something that needs to own it.
    fn take(&mut self, span: Span, value: &Value) {
        let Some(place) = &value.place else {
            if value.lambda {
                self.lambda_escape(span, &value.loans);
            } else if !value.loans.is_empty() {
                let what = self.describe(&value.loans);
                self.error(
                    span,
                    "E_MOVE_BORROWED",
                    format!("This value borrows {what}, so it cannot be handed over."),
                    "Pass it to a borrowing parameter, or build an owned value instead.",
                );
            }
            return;
        };
        let canonical = self.canonical(place);
        if canonical.captured {
            self.error(
                span,
                "E_MOVE_BORROWED",
                "A lambda cannot hand over a value it captured; it may run more than once.".into(),
                "Write `move` before the lambda and use the captured value without handing it over, or pass the value as a lambda parameter.",
            );
            return;
        }
        let root_name = |pass: &Self| match canonical.root {
            Root::Binding(id) => pass.bindings[id].name.clone(),
            Root::This => "this".to_owned(),
            Root::Global(_) => "script value".to_owned(),
        };
        match canonical.root {
            Root::Binding(id) => {
                match self.bindings[id].origin.clone() {
                    Origin::Owned if !self.bindings[id].loans.is_empty() => {
                        let binding = self.bindings[id].clone();
                        let what = self.describe(&binding.loans);
                        self.error(
                            span,
                            "E_MOVE_BORROWED",
                            format!(
                                "`{}` holds a borrow of {what}, so it cannot be handed over.",
                                binding.name
                            ),
                            "Pass it to a borrowing parameter, or build an owned value instead.",
                        );
                    }
                    Origin::Owned => {
                        self.mark_moved(&canonical, span);
                        if canonical != *place {
                            self.mark_moved(place, span);
                        }
                        self.event(&canonical, span, "moved");
                    }
                    Origin::Param => {
                        let name = root_name(self);
                        self.error(
                        span,
                        "E_MOVE_BORROWED",
                        format!("`{name}` is a borrowed parameter, so this function cannot hand it over."),
                        "Declare the parameter as `move name : Type` to take ownership, or pass the value to a borrowing parameter.",
                    );
                    }
                    Origin::View => {
                        let binding = self.bindings[id].clone();
                        if binding.lambda {
                            self.lambda_escape(span, &binding.loans);
                        } else {
                            let what = self.describe(&binding.loans);
                            self.error(
                            span,
                            "E_MOVE_BORROWED",
                            format!("`{}` borrows {what}, so it cannot be handed over.", binding.name),
                            "Pass it to a borrowing parameter, or build an owned value instead.",
                        );
                        }
                    }
                    Origin::Part(_) => unreachable!(),
                }
            }
            Root::This | Root::Global(_) => {
                let name = root_name(self);
                self.error(
                    span,
                    "E_MOVE_BORROWED",
                    format!("`{name}` is borrowed here, so its value cannot be handed over."),
                    "Take ownership with a `move` parameter, or pass the value to a borrowing parameter.",
                );
            }
        }
    }

    fn lambda_escape(&mut self, span: Span, loans: &BTreeSet<Loan>) {
        self.lambda_escape_from(span, loans, false);
    }

    /// `moving` marks a `move` lambda: it already owns its captures, so what still borrows
    /// is a captured value that itself borrows, such as a function value parameter.
    fn lambda_escape_from(&mut self, span: Span, loans: &BTreeSet<Loan>, moving: bool) {
        let what = self.describe(loans);
        let hint = if moving {
            "This `move` lambda keeps a value that still borrows. Declare the borrowed parameter as `move name : Type` so the lambda can own it."
        } else {
            "Write `move` before a lambda so it owns what it captures; a method value or partial call keeps borrowing its receiver and arguments."
        };
        self.error(
            span,
            "E_LAMBDA_ESCAPE",
            format!("This function value borrows {what} and would outlive it."),
            hint,
        );
    }

    /// Checks a value that leaves the current function.
    fn give_back(&mut self, span: Span, value: &Value) {
        let (loans, lambda) = if let Some(place) = &value.place {
            let canonical = self.canonical(place);
            if canonical.captured && self.frame().moving_lambda {
                self.error(span, "E_LAMBDA_ESCAPE", "A reusable move lambda cannot hand over a noncopyable capture as its result.".into(), "Construct a fresh result on each call or return a copied field. Borrowed captured-object results are not implemented for function values yet.");
                return;
            }
            if !canonical.captured
                && !value.readonly
                && let Root::Binding(id) = canonical.root
                && matches!(self.bindings[id].origin, Origin::Owned)
            {
                // Returning an owned local hands it to the caller, with any borrows it holds.
                self.mark_moved(&canonical, span);
                let loans = self.bindings[id].loans.clone();
                if loans.is_empty() {
                    self.owned_returns.insert(span);
                    return;
                }
                (loans, false)
            } else {
                let lambda = matches!(place.root, Root::Binding(id) if self.bindings[id].lambda);
                (
                    if self.is_copy(span)
                        && self
                            .type_of(span)
                            .is_some_and(|ty| self.holds_dependencies(ty))
                    {
                        self.callable_loans(value)
                    } else {
                        self.place_loans(place)
                    },
                    lambda,
                )
            }
        } else {
            (value.loans.clone(), value.lambda)
        };
        if let Some(declaration) = self.local_return_stack.last().copied() {
            for loan in &loans {
                // A captured name resolves as `Global`: a binding of an enclosing frame.
                let owner = match loan {
                    Loan::Local(id) | Loan::Global(id) => Some(*id),
                    Loan::Param(name) => self
                        .scopes
                        .iter()
                        .rev()
                        .find_map(|scope| scope.names.get(name).copied()),
                    _ => None,
                };
                if let Some(owner) = owner
                    && self.bindings[owner].frame != self.frame().id
                {
                    self.local_results
                        .entry(declaration)
                        .or_default()
                        .insert(owner);
                }
            }
        }
        if self.frame().moving_lambda && self.captures.last().is_some_and(|captured|
            loans.iter().any(|loan| matches!(loan, Loan::Local(id) | Loan::Global(id) if captured.contains_key(id))))
        {
            self.error(span, "E_LAMBDA_ESCAPE", "This result borrows storage owned by the move lambda's captured environment.".into(), "Return an independent result. Borrowed captured-object results are not implemented for function values yet.");
            return;
        }
        if loans.is_empty() {
            return;
        }
        if lambda {
            self.lambda_escape(span, &loans);
            return;
        }
        let frame = self.frame().id;
        for loan in &loans {
            match loan {
                Loan::Local(id) if self.bindings[*id].frame == frame => {
                    let name = self.bindings[*id].name.clone();
                    self.error(
                        span,
                        "E_BORROW_ESCAPE",
                        format!("This borrows `{name}`, which belongs to this function and is freed when it returns."),
                        "Return an owned value, or take the value as a parameter so the caller owns it.",
                    );
                }
                Loan::Temporary => self.error(
                    span,
                    "E_BORROW_TEMPORARY",
                    "This borrows a temporary value that is freed before the caller can use it."
                        .into(),
                    "Return an owned value instead of a borrow of a temporary.",
                ),
                Loan::Step(_) => self.error(
                    span,
                    "E_BORROW_ESCAPE",
                    "This item is lent only until the traversal's next step, so it cannot leave the function.".into(),
                    "Return an owned value built from its fields, or collect the items with toList().",
                ),
                Loan::Param(_) | Loan::Captures(_) | Loan::This | Loan::Global(_) => {
                    if let Some(key) = self.frame().key.clone() {
                        self.next.entry(key).or_default().insert(loan.clone());
                    }
                }
                _ => {}
            }
        }
    }

    /// Checks a value stored in a `&T` field of `this`.
    fn store_borrow(&mut self, span: Span, loans: &BTreeSet<Loan>) {
        let frame = self.frame().id;
        let constructor = self.frame().constructor;
        for loan in loans {
            match loan {
                Loan::Local(id) if self.bindings[*id].frame == frame => {
                    let name = self.bindings[*id].name.clone();
                    self.error(
                        span,
                        "E_BORROW_ESCAPE",
                        format!("The object would keep a borrow of `{name}`, which is freed when this function returns."),
                        "Store a borrow of a value the caller owns, such as a constructor parameter.",
                    );
                }
                Loan::Temporary => self.error(
                    span,
                    "E_BORROW_TEMPORARY",
                    "The object would keep a borrow of a temporary value.".into(),
                    "Store a borrow of a value the caller owns, such as a constructor parameter.",
                ),
                Loan::Step(_) => self.error(
                    span,
                    "E_BORROW_ESCAPE",
                    "The object would keep an item that a traversal lends only for one step.".into(),
                    "Store a value the caller owns, or collect the items with toList().",
                ),
                Loan::Param(name) | Loan::Captures(name) if !constructor => self.error(
                    span,
                    "E_BORROW_ESCAPE",
                    format!("The object would keep a borrow of parameter `{name}` after this call returns."),
                    "Store borrows in a constructor, or take the value with `move` and store it in an owned field.",
                ),
                Loan::This if constructor => self.error(span,"E_BORROW_SELF","An object cannot store a borrow into itself because moving it would leave a dangling pointer.".to_owned(),"Store the field by ownership, or borrow an object owned by the caller."),
                Loan::Param(_) | Loan::Captures(_) | Loan::This | Loan::Global(_) => {
                    if let Some(key) = self.frame().key.clone() {
                        self.next.entry(key).or_default().insert(loan.clone());
                    }
                }
                _ => {}
            }
        }
    }

    fn items(&mut self, items: &[Node], ctx: Ctx) -> Value {
        let mut result = Value::owned(true);
        for (index, item) in items.iter().enumerate() {
            if self.exhausted {
                break;
            }
            let last = index + 1 == items.len();
            if is_statement(item.kind) {
                self.statement(item);
            } else {
                result = self.eval(item, if last { ctx } else { Ctx::Borrow });
            }
        }
        result
    }

    fn statement(&mut self, node: &Node) {
        if !self.charge(1, node.span) {
            return;
        }
        match node.kind {
            K::Binding => self.let_binding(node),
            K::FieldDeclaration => self.field_declaration(node),
            K::Assignment => self.assignment(node),
            K::For => self.for_loop(node),
            K::While => {
                let cond = node.children.first();
                if let Some(body) = node.children.last() {
                    self.loop_body(body, cond, None);
                }
            }
            K::Return => {
                if let Some(value) = node.children.iter().find(|c| is_expression(c.kind)) {
                    self.eval(value, Ctx::Return);
                }
                // A return with no break inside leaves its frame, so nothing after it in that frame runs.
                if !node
                    .descendants()
                    .any(|n| matches!(n.kind, K::Break | K::Continue))
                {
                    let frame = self.frame().id;
                    self.returns.push((node.span, frame));
                }
                self.diverged = true;
            }
            K::Break => {
                if let Some(flow) = self.loops.last_mut() {
                    flow.breaks.push(self.moved.clone());
                }
                self.diverged = true;
            }
            K::Continue => {
                if let Some(flow) = self.loops.last_mut() {
                    flow.continues.push(self.moved.clone());
                }
                self.diverged = true;
            }
            K::Function if self.frames.last().is_some_and(|frame| frame.key.is_some()) => {
                let changes = self.local_function_changes(node);
                self.local_changes.insert(node.span, changes);
                self.local_return_stack.push(node.span);
                self.function(node, None);
                self.local_return_stack.pop();
            }
            K::Function | K::Constructor => self.function(node, None),
            K::Class | K::Trait => {
                let name = self.qualified(&self.named_child(node).unwrap_or_default());
                for member in &node.children {
                    if matches!(member.kind, K::Function | K::Constructor) {
                        self.function(member, Some(&name));
                    }
                }
            }
            _ => {}
        }
    }

    fn function(&mut self, node: &Node, owner: Option<&str>) {
        // `T : Copy` parameters copy inside this body.
        let copies = node
            .child(K::TypeParameters)
            .map(|list| {
                list.children
                    .iter()
                    .filter(|parameter| {
                        parameter.children.iter().any(|bound| {
                            bound.kind == K::Bound
                                && bound
                                    .children
                                    .first()
                                    .is_some_and(|ty| self.text(ty) == "Copy")
                        })
                    })
                    .filter_map(|parameter| {
                        parameter.child(K::Name).map(|n| self.text(n).to_owned())
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut copies: BTreeSet<String> = copies;
        // A method also sees its class's `T : Copy` parameters.
        if let Some(definition) =
            owner.and_then(|owner| self.checked.environment.definitions.get(owner))
        {
            copies.extend(
                definition
                    .bounds
                    .iter()
                    .filter(|(_, bounds)| bounds.contains(&Type::named("Copy")))
                    .map(|(name, _)| name.clone()),
            );
        }
        crate::types::push_copy_parameters(copies);
        self.function_inner(node, owner);
        crate::types::pop_copy_parameters();
    }
    fn function_inner(&mut self, node: &Node, owner: Option<&str>) {
        let constructor = node.kind == K::Constructor;
        let name = if constructor {
            "new".to_owned()
        } else {
            self.named_child(node).unwrap_or_default()
        };
        let saved = (
            std::mem::take(&mut self.moved),
            self.diverged,
            std::mem::take(&mut self.loops),
        );
        let loop_spans = std::mem::take(&mut self.loop_spans);
        self.enter_frame(Some((owner.unwrap_or("").to_owned(), name)), constructor);
        self.push_scope(Boundary::Function, node.span);
        if let Some(list) = node.child(K::ParameterList) {
            for parameter in &list.children {
                if let Some(name) = self.named_child(parameter) {
                    let parameter_type = self.checked.binding_types.get(&parameter.span);
                    let origin = if self.modifier(parameter, "move")
                        && (!parameter_type.is_some_and(copies)
                            || parameter_type.is_some_and(|ty| self.holds_dependencies(ty)))
                    {
                        Origin::Owned
                    } else {
                        Origin::Param
                    };
                    self.declare(
                        &name,
                        origin,
                        &Value {
                            readonly: !self.modifier(parameter, "move")
                                && !self.modifier(parameter, "mut"),
                            dependency_copy: parameter_type
                                .is_some_and(|ty| copies(ty) && self.holds_dependencies(ty)),
                            ..Value::default()
                        },
                        parameter.span,
                    );
                }
            }
        }
        if let Some(list) = node.child(K::ParameterList) {
            let old_validation = std::mem::replace(&mut self.validating_defaults, true);
            for (index, parameter) in list.children.iter().enumerate() {
                let expression = self
                    .default_values
                    .get(&node.span)
                    .and_then(|values| values.get(index))
                    .cloned()
                    .flatten();
                if let Some(expression) = expression {
                    let earlier = list.children[..index]
                        .iter()
                        .filter_map(|p| self.named_child(p))
                        .collect::<Vec<_>>();
                    let mode = if self.modifier(parameter, "move") {
                        Passing::Move
                    } else if self.modifier(parameter, "mut") {
                        Passing::Mut
                    } else {
                        Passing::Borrow
                    };
                    self.default_value(&expression, mode, expression.span, &earlier);
                }
            }
            self.validating_defaults = old_validation;
        }
        for requirement in node.children.iter().filter(|c| c.kind == K::Requires) {
            for child in &requirement.children {
                self.eval(child, Ctx::Borrow);
            }
        }
        for condition in node.children.iter().filter(|c| c.kind == K::Ensures) {
            self.push_scope(Boundary::None, condition.span);
            self.declare(
                "result",
                Origin::Param,
                &Value {
                    readonly: true,
                    ..Value::default()
                },
                condition.span,
            );
            for expression in &condition.children {
                self.eval(expression, Ctx::Borrow);
            }
            self.scopes.pop();
        }
        if let Some(body) = node.children.last().filter(|c| is_expression(c.kind)) {
            self.eval(
                body,
                if constructor {
                    Ctx::Borrow
                } else {
                    Ctx::Return
                },
            );
        }
        self.scopes.pop();
        self.frames.pop();
        (self.moved, self.diverged, self.loops) = saved;
        self.loop_spans = loop_spans;
    }

    fn let_binding(&mut self, node: &Node) {
        let Some(position) = node.children.iter().position(|c| is_pattern(c.kind)) else {
            return;
        };
        let pattern = &node.children[position];
        let init = node.children[position + 1..]
            .iter()
            .find(|c| is_expression(c.kind));
        let simple = pattern.kind == K::PatternName;
        let mut value = match init {
            Some(init) => self.eval(init, if simple { Ctx::Bind } else { Ctx::Borrow }),
            None => Value::owned(false),
        };
        if !simple
            && let Some(init) = init
            && matches!(self.type_of(init.span),Some(Type::Named(name,_)) if matches!(name.as_str(),"List" | "Map" | "Set" | "SortedSet"))
        {
            value.readonly = true;
            value.copy = false;
            value.place = self.place_of(init);
            if value.place.is_none() && value.loans.is_empty() {
                value.loans.insert(Loan::Temporary);
            }
        }
        if let Some(otherwise) = node.child(K::Otherwise) {
            let before = self.moved.clone();
            for child in &otherwise.children {
                self.eval(child, Ctx::Borrow);
            }
            self.moved = before;
            self.diverged = false;
        }
        let first = self.bindings.len();
        self.declare_pattern(pattern, &value, simple);
        let mutable = self.modifier(node, "mut");
        for binding in &mut self.bindings[first..] {
            binding.declared = node.span;
            binding.mutable = mutable;
            if mutable && matches!(binding.origin, Origin::Part(_)) {
                binding.origin = Origin::View;
            }
        }
    }

    fn declare_pattern(&mut self, pattern: &Node, value: &Value, simple: bool) {
        let names = pattern
            .descendants()
            .filter_map(|n| match n.kind {
                K::PatternName => Some((self.text(n).to_owned(), n.span)),
                K::PatternRest => n
                    .children
                    .first()
                    .map(|name| (self.text(name).to_owned(), name.span)),
                _ => None,
            })
            .collect::<Vec<_>>();
        for (name, span) in names {
            if simple {
                self.declare_bound(&name, value, span);
            } else if let Some(place) = &value.place {
                // Stored parts name their owner directly, so alias chains never
                // grow past one step however deeply patterns nest.
                let mut part = self.canonical(place);
                part.path.push(format!("#{name}"));
                self.declare(
                    &name,
                    Origin::Part(part),
                    &Value {
                        readonly: value.readonly,
                        changed_captures: value.changed_captures.clone(),
                        ..Value::default()
                    },
                    span,
                );
            } else {
                let view = Value {
                    readonly: value.readonly,
                    loans: value.loans.clone(),
                    lambda: value.lambda,
                    changed_captures: value.changed_captures.clone(),
                    ..Value::default()
                };
                self.declare_bound(&name, &view, span);
            }
        }
    }

    /// A constructor field declaration owns its value exactly as the matching
    /// `this.field = value` would, so reuse assignment checking.
    fn field_declaration(&mut self, node: &Node) {
        let (Some(member), Some(value)) = (
            node.children.iter().find(|child| child.kind == K::Member),
            node.children.last(),
        ) else {
            return;
        };
        let assignment = Node::new(
            K::Assignment,
            node.span,
            vec![member.clone(), value.clone()],
        );
        self.assignment(&assignment);
    }

    fn assignment(&mut self, node: &Node) {
        let (Some(target), Some(value_node)) = (node.children.first(), node.children.last()) else {
            return;
        };
        let target = ungroup(target);
        if target.kind == K::Name {
            let value = self.eval(value_node, Ctx::Bind);
            let name = self.text(target).to_owned();
            let copy = self.is_copy(target.span);
            if let Some((id, false, false)) = self.resolve(&name, copy) {
                let place = Place {
                    root: Root::Binding(id),
                    path: vec![],
                    captured: false,
                };
                self.event(&place, node.span, "reassigned");
                for loan in &value.loans {
                    let too_short = match loan {
                        Loan::Local(owner) | Loan::Step(owner) => {
                            self.bindings[*owner].lifetime.end < self.bindings[id].lifetime.end
                        }
                        Loan::Temporary => self.scopes.last().is_some_and(|scope| {
                            scope.lifetime.end < self.bindings[id].lifetime.end
                        }),
                        _ => false,
                    };
                    if too_short {
                        self.error(
                            value_node.span,
                            "E_BORROW_ESCAPE",
                            "This assignment would keep a borrow after its owner is released."
                                .to_owned(),
                            "Keep the owner in the same outer block as the destination.",
                        );
                    }
                }
                let binding = &mut self.bindings[id];
                binding.readonly |= value.readonly;
                if let Some(part) = value.part {
                    binding.origin = if binding.mutable {
                        Origin::View
                    } else {
                        Origin::Part(part)
                    };
                } else if value.lambda || !value.loans.is_empty() {
                    binding.origin = Origin::View;
                }
                binding.loans.extend(value.loans);
                binding.lambda |= value.lambda;
                binding.changed_captures.extend(value.changed_captures);
                self.moved.retain(|(owner, _), _| *owner != id);
            }
            return;
        }
        if target.kind == K::Index {
            self.eval(value_node, Ctx::Strict);
            for child in &target.children {
                self.eval(child, Ctx::Borrow);
            }
            if let Some(place) = self.place_of(&target.children[0]) {
                self.event(&place, target.span, "changed");
            }
            return;
        }
        if target.kind != K::Member {
            self.eval(value_node, Ctx::Borrow);
            return;
        }
        let stored_callable = self.frame().constructor
            && self
                .type_of(target.span)
                .is_some_and(|ty| self.holds_dependencies(ty))
            && self.is_copy(target.span);
        let stored_borrow = stored_callable
            || matches!(
                self.type_of(target.span),
                Some(Type::Borrow(_) | Type::StoredView(_))
            );
        let loans = if stored_borrow {
            let value = self.eval(value_node, Ctx::Borrow);
            if stored_callable {
                self.callable_loans(&value)
            } else if value.copy {
                BTreeSet::new()
            } else {
                self.value_loans(&value)
            }
        } else {
            self.eval(value_node, Ctx::Strict);
            BTreeSet::new()
        };
        let Some(place) = self.place_of(target) else {
            self.eval(&target.children[0], Ctx::Borrow);
            return;
        };
        let mut base = place.clone();
        base.path.pop();
        self.check_moved(&base, target.span);
        let canonical = self.canonical(&place);
        if stored_borrow {
            match canonical.root {
                Root::This => self.store_borrow(value_node.span, &loans),
                Root::Binding(id) => self.bindings[id].loans.extend(loans),
                Root::Global(_) => {}
            }
        }
        if let Root::Binding(id) = canonical.root {
            let path = canonical.path.clone();
            self.moved
                .retain(|(owner, moved), _| *owner != id || !moved.starts_with(&path));
        }
        self.event(&place, node.span, "changed");
    }

    fn resolve_global_loans(&self, loans: &mut BTreeSet<Loan>) {
        let mut globals = loans
            .iter()
            .filter_map(|loan| {
                if let Loan::Global(id) = loan {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let mut seen = BTreeSet::new();
        while let Some(id) = globals.pop() {
            if !seen.insert(id) {
                continue;
            }
            for loan in &self.bindings[id].loans {
                let dependency = match loan {
                    Loan::Local(owner) | Loan::Global(owner) => Loan::Global(*owner),
                    other => other.clone(),
                };
                if let Loan::Global(owner) = dependency {
                    globals.push(owner);
                }
                loans.insert(dependency);
            }
        }
        for id in seen {
            if self.bindings[id].frame == self.frame().id {
                loans.remove(&Loan::Global(id));
                loans.insert(Loan::Local(id));
            }
        }
    }
    fn check_scope_escape(&mut self, scope: Span, value: &Value) {
        if value.copy {
            return;
        }
        let loans = self.value_loans(value);
        for loan in loans {
            if let Loan::Local(id) | Loan::Step(id) = loan
                && self.bindings[id].lifetime.start >= scope.start
                && self.bindings[id].lifetime.end <= scope.end
            {
                let name = self.bindings[id].name.clone();
                self.error(scope, "E_BORROW_ESCAPE", format!("This result borrows '{name}', but that owner's block ends here."), "Move an owned result out of the block, or declare the borrowed owner in an outer block.");
                return;
            }
        }
    }
    fn global_captures(&mut self, target: &crate::resolution::Callable) -> BTreeSet<String> {
        let crate::resolution::Callable::Declaration { id, .. } = target else {
            return BTreeSet::new();
        };
        if id.file != self.source.name {
            return BTreeSet::new();
        }
        if let Some(names) = self.global_captures.get(id) {
            return names.clone();
        }
        let mut names = BTreeSet::new();
        let mut seen = BTreeSet::new();
        let mut pending = vec![id.clone()];
        while let Some(function) = pending.pop() {
            if function.file != self.source.name || !seen.insert(function.clone()) {
                continue;
            }
            if let Some(cached) = self.global_captures.get(&function) {
                names.extend(cached.iter().cloned());
                continue;
            }
            let range = Span::new(function.span.start, 0)..Span::new(function.span.end, 0);
            for (span, reference) in self.checked.references.range(range.clone()) {
                if span.end <= function.span.end
                    && (!self.is_copy(*span)
                        || self
                            .type_of(*span)
                            .is_some_and(|ty| self.holds_dependencies(ty)))
                    && let crate::resolution::Reference::ScriptBinding(binding) = reference
                {
                    names.insert(self.source.slice(binding.span).to_owned());
                }
            }
            for (span, call) in self.checked.resolved_calls.range(range) {
                if span.end <= function.span.end
                    && let crate::resolution::Callable::Declaration { id, .. } = &call.target
                {
                    pending.push(id.clone());
                }
            }
            for (span, ty) in self
                .types
                .range(Span::new(function.span.start, 0)..Span::new(function.span.end, 0))
            {
                if span.end <= function.span.end
                    && let Type::Function(signature) = ty
                    && let Some(crate::resolution::Callable::Declaration { id, .. }) =
                        &signature.callable
                {
                    pending.push(id.clone());
                }
            }
        }
        self.global_captures.insert(id.clone(), names.clone());
        names
    }
    fn check_global_captures(&mut self, target: &crate::resolution::Callable, span: Span) {
        for name in self.global_captures(target) {
            if let Some(id) = self
                .bindings
                .iter()
                .position(|binding| binding.frame == 0 && binding.name == name)
            {
                for scope in self.scopes.iter().rev() {
                    match scope.boundary {
                        Boundary::Lambda(index) => {
                            self.captured_globals[index].insert(id);
                        }
                        Boundary::Function => break,
                        Boundary::None => {}
                    }
                }
                self.check_moved(
                    &Place {
                        root: Root::Binding(id),
                        path: vec![],
                        captured: false,
                    },
                    span,
                );
            }
        }
    }

    fn for_loop(&mut self, node: &Node) {
        self.push_scope(Boundary::None, node.span);
        let body = node.children.last();
        if let Some(binding) = node.child(K::ForBinding) {
            if let (Some(pattern), Some(collection)) =
                (binding.children.first(), binding.children.last())
            {
                let value = self.eval(collection, Ctx::Borrow);
                let mut loans = if let Some(place) = self.place_of(collection) {
                    self.place_loans(&place)
                } else if !value.loans.is_empty() {
                    value.loans.clone()
                } else {
                    [Loan::Temporary].into()
                };
                self.step_loans(collection, &mut loans);
                for name in pattern
                    .descendants()
                    .filter(|n| n.kind == K::PatternName)
                    .map(|n| (self.text(n).to_owned(), n.span))
                    .collect::<Vec<_>>()
                {
                    let view = Value {
                        readonly: true,
                        loans: loans.clone(),
                        ..Value::default()
                    };
                    self.declare(&name.0, Origin::View, &view, name.1);
                }
            }
            // The loop's cursor reads the collection before each pass, so the body may
            // not change it, even when the item goes unused (REVIEW2 1.10).
            // A persistent List or Map is iterated as a snapshot, so only a collection
            // that changes in place needs this rule.
            let iterated = binding
                .children
                .last()
                .filter(|collection| !self.type_of(collection.span).is_some_and(copies))
                .and_then(|collection| self.place_of(collection))
                .map(|place| self.canonical(&place));
            let first_event = self.events.len();
            if let Some(body) = body {
                self.loop_body(body, None, None);
            }
            if let Some(place) = iterated
                && let Root::Binding(owner) = place.root
            {
                let changed = self.events[first_event..]
                    .iter()
                    .find(|event| {
                        event.owner == owner
                            && matches!(event.what, "changed" | "reassigned" | "moved")
                            && (event.path.starts_with(&place.path)
                                || place.path.starts_with(&event.path))
                    })
                    .map(|event| event.span);
                if let Some(span) = changed
                    && !self.errors_at(span)
                {
                    let name = self.bindings[owner].name.clone();
                    self.error(
                        span,
                        "E_BORROW_CONFLICT",
                        format!("`{name}` is changed inside a loop over it."),
                        "Collect the changes and apply them after the loop, or loop over a copy.",
                    );
                }
            }
        } else {
            let parts = &node.children[..node.children.len().saturating_sub(1)];
            if let Some(init) = parts.iter().find(|c| c.kind == K::Binding) {
                self.let_binding(init);
            }
            let cond = parts
                .iter()
                .find(|c| c.kind != K::Binding && is_expression(c.kind));
            let step = parts.iter().find(|c| c.kind == K::Assignment);
            if let Some(body) = body {
                self.loop_body(body, cond, step);
            }
        }
        self.scopes.pop();
    }

    fn loop_body(&mut self, body: &Node, cond: Option<&Node>, step: Option<&Node>) {
        let start = [cond, step, Some(body)]
            .into_iter()
            .flatten()
            .map(|n| n.span.start)
            .min()
            .unwrap_or(body.span.start);
        self.loop_spans.push(Span::new(start, body.span.end));
        let entry = self.moved.clone();
        let mut head = entry.clone();
        let mut exits = vec![];
        let outer = self.bindings.len();
        while !self.exhausted {
            let bindings = self.bindings[..outer].to_vec();
            let previous = head.clone();
            self.moved = head.clone();
            self.diverged = false;
            if let Some(cond) = cond {
                self.eval(cond, Ctx::Borrow);
            }
            let after_condition = self.moved.clone();
            self.loops.push(LoopFlow::default());
            self.eval(body, Ctx::Borrow);
            let flow = self.loops.pop().unwrap();
            let mut tails = flow.continues;
            if !self.diverged {
                tails.push(self.moved.clone());
            }
            let mut tail = merge_all(tails);
            if let (Some(step), Some(state)) = (step, &tail) {
                self.moved = state.clone();
                self.diverged = false;
                self.statement(step);
                tail = Some(self.moved.clone());
            }
            exits = flow.breaks;
            exits.push(after_condition);
            head = match tail {
                Some(tail) => merge(&entry, &tail),
                None => entry.clone(),
            };
            // Body locals are new on each iteration, so they cannot flow back
            // into the next head. Outer bindings carry moves and borrow loans.
            head.retain(|(owner, _), _| *owner < outer);
            if head == previous && bindings == self.bindings[..outer] {
                break;
            }
        }
        self.moved = merge_all(exits).unwrap_or(entry);
        self.moved.retain(|(owner, _), _| *owner < outer);
        self.diverged = false;
        self.loop_spans.pop();
    }

    fn eval(&mut self, node: &Node, ctx: Ctx) -> Value {
        if self.checked.theorem_evidence.contains(&node.span) {
            return self.apply(node.span, Value::owned(true), ctx);
        }
        if self.checked.borrowed_views.contains_key(&node.span) {
            let source = self.eval_inner(node, Ctx::Borrow);
            let place = self.place_of(node);
            let mut loans = if let Some(place) = &place {
                if (self.is_copy(node.span)
                    || matches!(
                        self.type_of(node.span),
                        Some(Type::Parameter(_) | Type::Rigid(_))
                    ))
                    && matches!(place.root, Root::Binding(id) if matches!(self.bindings[id].origin, Origin::Param))
                {
                    match place.root {
                        Root::Binding(id) => [Loan::Local(id)].into(),
                        _ => unreachable!(),
                    }
                } else {
                    self.place_loans(place)
                }
            } else {
                source.loans
            };
            if loans.is_empty() {
                loans.insert(Loan::Temporary);
            }
            let mutable = self.checked.argument_modes.get(&node.span) == Some(&Passing::Mut);
            let value = Value {
                readonly: !mutable || source.readonly,
                place: if mutable { place.clone() } else { None },
                lambda: place.is_some_and(|place| place.captured),
                loans,
                ..Value::default()
            };
            return self.apply(node.span, value, ctx);
        }
        let mut value = self.eval_inner(node, ctx);
        value.function_copy =
            self.type_of(node.span).is_some_and(callable_type) && self.is_copy(node.span);
        value.dependency_copy = self.is_copy(node.span)
            && self
                .type_of(node.span)
                .is_some_and(|ty| self.holds_dependencies(ty));
        value
    }

    fn eval_inner(&mut self, node: &Node, ctx: Ctx) -> Value {
        if !self.charge(1, node.span) {
            return Value::owned(false);
        }
        // A value placed into a new box is handed over to it, as with box(x) (Q86).
        let ctx = if self.checked.autoboxed.contains_key(&node.span) {
            Ctx::Strict
        } else {
            ctx
        };
        match node.kind {
            K::Group if node.children.len() == 1 => self.eval(&node.children[0], ctx),
            K::Member if self.method_value(node) => {
                // `c.increment` keeps borrowing `c` until it is called, like a partial call.
                let receiver = self.eval(&node.children[0], Ctx::Borrow);
                let mut loans = self.value_loans(&receiver);
                if receiver.copy {
                    loans.clear();
                }
                self.resolve_global_loans(&mut loans);
                let value = Value {
                    lambda: !loans.is_empty(),
                    copy: loans.is_empty(),
                    loans,
                    ..Value::default()
                };
                self.apply(node.span, value, ctx)
            }
            K::Name | K::Member => {
                let value = self.leaf(node);
                self.apply(node.span, value, ctx)
            }
            K::Index => {
                let owner = self.eval(&node.children[0], Ctx::Borrow);
                self.eval(&node.children[1], Ctx::Borrow);
                let retains_captures = self
                    .type_of(node.span)
                    .is_some_and(|ty| self.holds_dependencies(ty));
                // A slice of a tensor, String or List is a new value.
                let tensor_copy = node.children[1].kind == K::Slice
                    || self
                        .type_of(node.children[0].span)
                        .is_some_and(crate::types::is_tensor)
                        && node.children[1]
                            .descendants()
                            .any(|part| part.kind == K::Slice);
                let value = if tensor_copy {
                    Value::owned(false)
                } else if self.is_copy(node.span) && !retains_captures {
                    Value::owned(true)
                } else {
                    // An Array slot is as writable as its Array: a `mut` receiver or
                    // parameter may change an element, and that change marks the Array.
                    Value {
                        // A stored callback is still called only through its own binding.
                        readonly: owner.readonly
                            || self.type_of(node.span).is_some_and(callable_type),
                        loans: self.value_loans(&owner),
                        ..Value::default()
                    }
                };
                self.apply(node.span, value, ctx)
            }
            K::Block => {
                self.push_scope(Boundary::None, node.span);
                let mut value = self.items(&node.children, ctx);
                if let Some(place) = &value.place {
                    let canonical = self.canonical(place);
                    if let Root::Binding(id) = canonical.root
                        && self.bindings[id].lifetime == node.span
                        && matches!(self.bindings[id].origin, Origin::Owned)
                    {
                        self.mark_moved(&canonical, node.span);
                        value = Value {
                            loans: self.bindings[id].loans.clone(),
                            changed_captures: self.bindings[id].changed_captures.clone(),
                            copy: value.copy,
                            ..Value::default()
                        };
                    }
                }
                self.check_scope_escape(node.span, &value);
                self.scopes.pop();
                value
            }
            K::If => self.if_expression(node, ctx),
            K::Match => self.match_expression(node, ctx),
            K::Call => {
                let value = self.call(node);
                self.apply(node.span, value, ctx)
            }
            K::Pipe => {
                let left = &node.children[0];
                let value = self.argument(left, false);
                let right = ungroup(node.children.last().unwrap());
                let mut children = if right.kind == K::Call {
                    right.children.clone()
                } else {
                    vec![right.clone()]
                };
                let mut filled = false;
                for argument in children.iter_mut().skip(1) {
                    if let Some(hole) = argument.children.last_mut().filter(|n| n.kind == K::Hole) {
                        *hole = left.clone();
                        filled = true;
                        break;
                    }
                }
                if !filled {
                    children.push(Node::new(K::Argument, left.span, vec![left.clone()]));
                }
                let call = Node::new(K::Call, node.span, children);
                let value = self.call_with_piped(&call, Some((left.span, value)));
                self.apply(node.span, value, ctx)
            }
            K::Lambda => {
                let value = self.lambda(node);
                self.apply(node.span, value, ctx)
            }
            K::List
                if matches!(self.type_of(node.span), Some(Type::Named(name, args))
                    if name == "List" && matches!(args.first(), Some(Type::Borrow(_)))) =>
            {
                // A list of views borrows its elements and keeps their owners' loans.
                let mut loans = BTreeSet::new();
                for child in node.children.iter().filter(|c| is_expression(c.kind)) {
                    let value = self.eval(child, Ctx::Borrow);
                    loans.extend(match self.place_of(child) {
                        Some(place) => self.place_loans(&place),
                        None if value.loans.is_empty() => [Loan::Temporary].into(),
                        None => value.loans.clone(),
                    });
                }
                let value = Value {
                    readonly: true,
                    loans,
                    copy: false,
                    ..Value::default()
                };
                self.apply(node.span, value, ctx)
            }
            K::List | K::Tuple | K::Record => {
                for child in node.children.iter().filter(|c| is_expression(c.kind)) {
                    self.eval(child, Ctx::Strict);
                }
                let value = Value::owned(self.is_copy(node.span));
                self.apply(node.span, value, ctx)
            }
            K::Using => self.using(node, ctx),
            K::Scope => {
                self.tasks.push(vec![]);
                self.task_results.push(BTreeSet::new());
                for child in node.children.iter().filter(|c| is_expression(c.kind)) {
                    self.eval(child, Ctx::Borrow);
                }
                let tasks = self.tasks.pop().unwrap_or_default();
                self.task_conflicts(node.span, &tasks);
                let loans = self.task_results.pop().unwrap_or_default();
                let value = Value {
                    readonly: !loans.is_empty(),
                    loans,
                    copy: self.is_copy(node.span),
                    ..Value::default()
                };
                self.check_scope_escape(node.span, &value);
                self.apply(node.span, value, ctx)
            }
            K::Spawn => {
                if let Some(call) = node.children.first() {
                    let result = self.eval(call, Ctx::Borrow);
                    if let Some(loans) = self.task_results.last_mut() {
                        loans.extend(result.loans);
                    }
                    let accesses = self.task_accesses(call);
                    if let Some(tasks) = self.tasks.last_mut() {
                        for (owner, path, exclusive) in accesses {
                            tasks.push(Task {
                                owner,
                                path,
                                exclusive,
                                span: node.span,
                            });
                        }
                    }
                }
                Value::owned(true)
            }
            K::Comprehension => {
                self.comprehension(node);
                let value = Value::owned(self.is_copy(node.span));
                self.apply(node.span, value, ctx)
            }
            K::Binary => {
                let short_circuit = node
                    .children
                    .iter()
                    .any(|c| c.kind == K::Operator && matches!(self.text(c), "&&" | "||"));
                let operands = node
                    .children
                    .iter()
                    .filter(|c| is_expression(c.kind))
                    .collect::<Vec<_>>();
                for (index, operand) in operands.iter().enumerate() {
                    if short_circuit && index > 0 {
                        let before = self.moved.clone();
                        self.eval(operand, Ctx::Borrow);
                        self.moved = merge(&before, &self.moved);
                    } else {
                        self.eval(operand, Ctx::Borrow);
                    }
                }
                let value = Value::owned(self.is_copy(node.span));
                self.apply(node.span, value, ctx)
            }
            K::Propagate => {
                // An Option held in a place is read through a borrow, as a match reads it (Q115), so
                // `this.left?` gives a view of the child instead of moving it out of `this`.
                let option = matches!(
                    self.type_of(node.children[0].span),
                    Some(Type::Named(name, args)) if name == "Option" && args.len() == 1
                );
                let place = if option {
                    self.place_of(&node.children[0])
                } else {
                    None
                };
                let ctx = if place.is_some() {
                    Ctx::Borrow
                } else {
                    Ctx::Bind
                };
                let inner = self.eval(&node.children[0], ctx);
                // The payload is a part of that place, so a later move of it is checked as a move
                // out of the place, which is what match does for its bindings.
                let part = match place {
                    Some(place) if !self.is_copy(node.span) => {
                        let mut part = self.canonical(&place);
                        part.path.push("#Some".to_owned());
                        Some(part)
                    }
                    _ => None,
                };
                let value = Value {
                    readonly: inner.readonly,
                    loans: if self.is_copy(node.span)
                        && !self
                            .type_of(node.span)
                            .is_some_and(|ty| self.holds_dependencies(ty))
                    {
                        BTreeSet::new()
                    } else {
                        inner.loans.clone()
                    },
                    copy: self.is_copy(node.span) && inner.loans.is_empty(),
                    part,
                    ..Value::default()
                };
                self.apply(node.span, value, ctx)
            }
            _ if is_statement(node.kind) => {
                self.statement(node);
                Value::owned(true)
            }
            _ => {
                for child in node.children.iter().filter(|c| is_expression(c.kind)) {
                    self.eval(child, Ctx::Borrow);
                }
                let value = Value::owned(self.is_copy(node.span));
                self.apply(node.span, value, ctx)
            }
        }
    }

    fn leaf(&mut self, node: &Node) -> Value {
        if let Some(place) = self.place_of(node) {
            self.check_moved(&place, node.span);
            let lambda = matches!(place.root, Root::Binding(id) if self.bindings[id].lambda);
            let captures_clear = !self
                .type_of(node.span)
                .is_some_and(|ty| self.holds_dependencies(ty))
                || self
                    .callable_loans(&Value {
                        place: Some(place.clone()),
                        ..Value::default()
                    })
                    .is_empty();
            return Value {
                readonly: matches!(place.root,Root::Binding(id) if self.bindings[id].readonly)
                    || matches!(
                        self.type_of(node.span),
                        Some(Type::Borrow(_) | Type::StoredView(_))
                    ) && !matches!(place.root,Root::Binding(id) if matches!(self.bindings[id].origin,Origin::Param) && !self.bindings[id].readonly),
                place: Some(place.clone()),
                lambda,
                changed_captures: self.callable_changes(node),
                copy: self.is_copy(node.span) && !lambda && captures_clear,
                ..Value::default()
            };
        }
        if let Some(Type::Function(signature)) = self.type_of(node.span)
            && let Some(target) = signature.callable.clone()
        {
            self.check_global_captures(&target, node.span);
            let mut loans = BTreeSet::new();
            let mut pending = self
                .global_captures(&target)
                .iter()
                .filter_map(|name| {
                    self.bindings
                        .iter()
                        .position(|binding| binding.frame == 0 && binding.name == *name)
                })
                .collect::<Vec<_>>();
            while let Some(id) = pending.pop() {
                if !loans.insert(Loan::Global(id)) {
                    continue;
                }
                for loan in &self.bindings[id].loans {
                    match loan {
                        Loan::Local(owner) | Loan::Global(owner)
                            if self.bindings[*owner].frame == 0 =>
                        {
                            pending.push(*owner)
                        }
                        _ => {
                            loans.insert(loan.clone());
                        }
                    }
                }
            }
            self.resolve_global_loans(&mut loans);
            let changed_captures = self.callable_changes(node);
            for id in &changed_captures {
                loans.insert(if *id == THIS {
                    Loan::This
                } else {
                    Loan::Local(*id)
                });
            }
            return Value {
                copy: loans.is_empty(),
                lambda: !changed_captures.is_empty(),
                changed_captures,
                loans,
                ..Value::default()
            };
        }
        if node.kind == K::Member
            && let Some(base) = node.children.first()
        {
            // A field of a temporary is owned by the expression.
            let owner = self.eval(base, Ctx::Borrow);
            if self.is_copy(node.span)
                && self
                    .type_of(node.span)
                    .is_some_and(|ty| self.holds_dependencies(ty))
            {
                let loans = self.callable_loans(&owner);
                return Value {
                    copy: loans.is_empty(),
                    loans,
                    ..Value::default()
                };
            }
        }
        Value::owned(self.is_copy(node.span))
    }

    /// A `using` name borrows its resource, so it cannot leave the block.
    fn using(&mut self, node: &Node, ctx: Ctx) -> Value {
        let (Some(name), Some(init), Some(body)) = (
            node.children.first(),
            node.children.get(1),
            node.children.last(),
        ) else {
            return Value::owned(true);
        };
        let value = self.eval(init, Ctx::Borrow);
        if value.readonly
            && matches!(self.type_of(init.span),Some(Type::Named(name,_)) if matches!(name.as_str(),"Reader" | "Writer"))
        {
            self.error(
                init.span,
                "E_MUT_PARAMETER",
                "A file using block cannot grant mutation through a read-only borrow.".into(),
                "Pass the file through a mut parameter, or open an owned file for the block.",
            );
        }
        let mut loans = self.value_loans(&value);
        if loans.is_empty() {
            loans.insert(Loan::Temporary);
        }
        self.push_scope(Boundary::None, node.span);
        let view = Value {
            loans,
            ..Value::default()
        };
        let text = self.text(name).to_owned();
        self.declare(&text, Origin::View, &view, name.span);
        let id = self.bindings.len() - 1;
        self.bindings[id].loans.insert(Loan::Local(id));
        let result = self.eval(body, ctx);
        self.check_scope_escape(node.span, &result);
        self.scopes.pop();
        result
    }

    /// The places a spawned call reads, and whether it changes them.
    fn task_accesses(&mut self, call: &Node) -> Vec<(usize, Vec<String>, bool)> {
        let call = ungroup(call);
        if call.kind != K::Call {
            return vec![];
        }
        let mut found = vec![];
        let mut add = |pass: &mut Self, node: &Node, exclusive: bool| {
            if !exclusive && pass.is_copy(node.span) {
                return;
            }
            if let Some(place) = pass.place_of(node) {
                for (id, path) in pass.accessed_places(&place) {
                    if !found.contains(&(id, path.clone(), exclusive)) {
                        found.push((id, path, exclusive));
                    }
                }
            }
        };
        if let Some(callee) = call.children.first().map(ungroup)
            && callee.kind == K::Member
        {
            let base = &callee.children[0];
            let exclusive = self.checked.mutated_receivers.contains(&base.span);
            add(self, base, exclusive);
        }
        for argument in &call.children[1..] {
            let Some(value) = argument.children.last() else {
                continue;
            };
            match self.checked.argument_modes.get(&value.span) {
                Some(Passing::Move) => {}
                Some(Passing::Mut) => add(self, value, true),
                _ => add(self, value, false),
            }
        }
        for node in call.children.first().into_iter().chain(
            call.children
                .iter()
                .skip(1)
                .filter_map(|argument| argument.children.last()),
        ) {
            for owner in self.callable_changes(node) {
                if owner != THIS && !found.contains(&(owner, vec![], true)) {
                    found.push((owner, vec![], true));
                }
            }
        }
        found
    }

    /// Tasks run at the same time as each other and as the rest of the scope.
    fn task_conflicts(&mut self, scope: Span, tasks: &[Task]) {
        let overlap = |a: &[String], b: &[String]| a.starts_with(b) || b.starts_with(a);
        let inside_task = |span: &Span| {
            tasks
                .iter()
                .any(|t| t.span.start <= span.start && span.end <= t.span.end)
        };
        for (index, task) in tasks.iter().enumerate() {
            let name = self.bindings[task.owner].name.clone();
            if self.bindings[task.owner].lifetime.end < scope.end {
                self.error(task.span, "E_TASK_LIFETIME", format!("Task borrows '{name}', but that value's block ends before the task scope joins."), "Move the value into the task, or declare its owner outside this scope.");
            }

            if tasks[..index].iter().any(|other| {
                other.owner == task.owner
                    && overlap(&other.path, &task.path)
                    && (other.exclusive || task.exclusive)
                    && other.span != task.span
            }) {
                self.error(
                    task.span,
                    "E_TASK_CONFLICT",
                    format!("Two tasks in this scope use `{name}`, and one of them changes it."),
                    "Give each task its own value, or share it through a Mutex.",
                );
                continue;
            }
            let after = |span: &Span| {
                span.start >= task.span.end && span.end <= scope.end && !inside_task(span)
            };
            let line = self.line(task.span);
            let clash = if task.exclusive {
                self.uses
                    .get(&task.owner)
                    .and_then(|uses| uses.iter().find(|u| after(u)))
                    .copied()
            } else {
                self.events
                    .iter()
                    .find(|e| {
                        e.owner == task.owner && overlap(&e.path, &task.path) && after(&e.span)
                    })
                    .map(|e| e.span)
            };
            if let Some(clash) = clash {
                let message = if task.exclusive {
                    format!(
                        "`{name}` is used here while the task started at line {line} may be changing it."
                    )
                } else {
                    format!(
                        "`{name}` changes here while the task started at line {line} may be reading it."
                    )
                };
                self.error(
                    clash,
                    "E_TASK_CONFLICT",
                    message,
                    "Wait until the scope ends, or share the value through a Mutex.",
                );
            }
        }
    }

    fn argument(&mut self, node: &Node, consuming: bool) -> Value {
        let mode = self.checked.argument_modes.get(&node.span).copied();
        let ctx = if consuming || mode == Some(Passing::Move) {
            Ctx::Strict
        } else {
            Ctx::Borrow
        };
        let mut value = self.eval(node, ctx);
        if (matches!(self.type_of(node.span), Some(Type::Named(name, _)) if matches!(name.as_str(),"List"|"Map"|"Set"|"SortedSet"))
            || self.type_of(node.span).is_some_and(callable_type) && value.loans.is_empty())
            && value.place.is_none()
        {
            value.place = self.place_of(node);
        }
        if mode == Some(Passing::Mut) && value.readonly {
            self.error(
                node.span,
                "E_MUT_PARAMETER",
                "This argument is a read-only returned borrow.".to_owned(),
                "Pass the owner to a mut parameter instead.",
            );
        }
        if mode == Some(Passing::Mut)
            && let Some(place) = &value.place
        {
            let place = place.clone();
            self.event(&place, node.span, "changed");
        }
        value
    }

    fn default_value(
        &mut self,
        expression: &Node,
        mode: Passing,
        call: Span,
        earlier: &[String],
    ) -> Value {
        if !self.active_defaults.insert(expression.span) {
            self.error(call,"E_DEFAULT_RECURSION","Evaluating these omitted defaults recursively needs the same default again.".into(),"Pass an argument that breaks the default cycle; recursive default helpers are not implemented yet.");
            return Value {
                loans: [Loan::Temporary].into(),
                ..Value::default()
            };
        }
        // Defaults use their declaration's script bindings, not the caller's locals.
        let globals = self.scopes[0].clone();
        let scopes = std::mem::replace(&mut self.scopes, vec![globals]);
        let diverged = self.diverged;
        self.push_scope(Boundary::Function, call);
        // Earlier parameters remain read-only borrows within a default.
        for name in earlier {
            self.declare(
                name,
                Origin::Param,
                &Value {
                    readonly: true,
                    ..Value::owned(true)
                },
                call,
            );
        }
        let events = self.events.len();
        let uses = self
            .uses
            .iter()
            .map(|(id, uses)| (*id, uses.len()))
            .collect::<BTreeMap<_, _>>();
        let diagnostics = self.diagnostics.len();
        let mut value = self.eval(
            expression,
            if mode == Passing::Move {
                Ctx::Strict
            } else {
                Ctx::Borrow
            },
        );
        if matches!(self.type_of(expression.span),Some(Type::Named(name,_)) if matches!(name.as_str(),"List"|"Map"|"Set"|"SortedSet"))
            && value.place.is_none()
        {
            value.place = self.place_of(expression);
        }
        if mode == Passing::Mut {
            if value.readonly {
                self.error(
                    call,
                    "E_MUT_PARAMETER",
                    "This default is a read-only borrowed value.".into(),
                    "Pass the owner explicitly to grant an exclusive borrow.",
                );
            } else if let Some(place) = &value.place {
                let place = place.clone();
                self.event(&place, call, "changed");
            }
        }
        for event in &mut self.events[events..] {
            event.span = call;
        }
        for (id, found) in &mut self.uses {
            for span in &mut found[uses.get(id).copied().unwrap_or(0)..] {
                *span = call;
            }
        }
        for diagnostic in &mut self.diagnostics[diagnostics..] {
            diagnostic.span = call;
        }
        self.scopes = scopes;
        self.diverged = diverged;
        self.active_defaults.remove(&expression.span);
        value
    }

    fn is_case(&self, name: &str) -> bool {
        name == "Some"
            || self
                .checked
                .environment
                .definitions
                .values()
                .any(|d| d.kind == K::Enum && d.variants.iter().any(|variant| variant.name == name))
    }

    fn class_of(&self, span: Span) -> Option<String> {
        let mut ty = self.type_of(span)?;
        while let Type::Borrow(inner) | Type::StoredView(inner) = ty {
            ty = inner;
        }
        match ty {
            Type::Named(name, _) => Some(name.clone()),
            _ => None,
        }
    }

    fn method_key(&self, class: &str, method: &str) -> Option<Key> {
        let mut class = class.to_owned();
        for _ in 0..64 {
            let key = (class.clone(), method.to_owned());
            if self.signatures.contains_key(&key)
                || self.checked.environment.calls.contains_key(&key)
            {
                return Some(key);
            }
            match &self.checked.environment.definitions.get(&class)?.parent {
                Some(Type::Named(parent, _)) => class = parent.clone(),
                _ => return None,
            }
        }
        None
    }

    fn call(&mut self, node: &Node) -> Value {
        self.call_with_piped(node, None)
    }
    fn call_with_piped(&mut self, node: &Node, piped: Option<(Span, Value)>) -> Value {
        let call_events = self.events.len();
        let mut call_loans = vec![];
        let Some(callee) = node.children.first() else {
            return Value::owned(true);
        };
        let callee = ungroup(callee);
        let mut changed_captures = self.callable_changes(callee);
        let mut capture_accesses = BTreeMap::from([(callee.span, changed_captures.clone())]);
        let mut capture_through = BTreeMap::new();
        if let Some(Place {
            root: Root::Binding(id),
            ..
        }) = self.place_of(callee)
        {
            capture_through.insert(callee.span, id);
        }
        if self.checked.resolved_calls.get(&node.span).is_some_and(|call| matches!(&call.target,crate::resolution::Callable::Builtin(name) if name == "std.array.generate")) {
            let mut loans = BTreeSet::new();
            for argument in node.children.iter().skip(1) {
                if let Some(value)=argument.children.last() {
                    let found = self.argument(value,false);
                    for owner in &found.changed_captures {
                        let place = Place {
                            root: if *owner == THIS { Root::This } else { Root::Binding(*owner) },
                            path: vec![],
                            captured: false,
                        };
                        let first = self.events.len();
                        self.event(&place, node.span, "changed");
                        if let Some(Place { root: Root::Binding(id), .. }) = &found.place {
                            for event in &mut self.events[first..] {
                                event.through = Some(*id);
                            }
                        }
                    }
                    if found.function_copy && self.type_of(node.span).is_some_and(|ty| self.holds_dependencies(ty)) {
                        loans.extend(self.value_loans(&found));
                    }
                }
            }
            return Value {loans, ..Value::owned(false)};
        }
        let mut receiver = None;
        let mut receiver_place = None;
        let mut callable_loans = BTreeSet::new();
        let mut key = None;
        let mut consuming = false;
        let mut stepping = None;
        // Calls whose body this pass cannot see: trait objects, generic
        // receivers and function values. Their results may borrow any argument.
        let mut opaque = false;
        match callee.kind {
            K::Member if callee.children.len() == 2 => {
                let base = &callee.children[0];
                let method = self.text(&callee.children[1]).to_owned();
                let enum_case = base.kind == K::Name
                    && self
                        .checked
                        .environment
                        .definitions
                        .get(self.text(base))
                        .is_some_and(|d| d.kind == K::Enum)
                    || self.text(base) == "Option";
                if matches!(
                    self.type_of(base.span),
                    Some(Type::Namespace(_) | Type::Constructor(_, _))
                ) {
                    // A namespace or type name is not a captured run-time receiver.
                } else if enum_case {
                    consuming = true;
                } else {
                    receiver_place = self.place_of(base);
                    let value = self.eval(base, Ctx::Borrow);
                    if self.checked.mutated_receivers.contains(&base.span) && value.readonly {
                        self.error(
                            base.span,
                            "E_MUT_PARAMETER",
                            "This receiver is a read-only returned borrow.".to_owned(),
                            "Change the owner through an explicit mut parameter instead.",
                        );
                    }
                    if self.checked.mutated_receivers.contains(&base.span)
                        && let Some(place) = &value.place
                    {
                        let place = place.clone();
                        self.event(&place, base.span, "changed");
                    } else if self.checked.mutated_receivers.contains(&base.span)
                        && ungroup(base).kind == K::Index
                        && let Some(place) = self.place_of(&ungroup(base).children[0])
                    {
                        // Changing an element changes its Array.
                        self.event(&place, base.span, "changed");
                    }
                    opaque = self.abstract_receiver(base.span);
                    if method == "next"
                        && (opaque || self.hidden_cursor(base.span))
                        && self.type_of(node.span).is_some_and(contains_borrow)
                        && let Some(place) = &receiver_place
                        && let Root::Binding(id) = self.canonical(place).root
                    {
                        stepping = Some(id);
                        // Each step ends the loan of the item before it.
                        let frame = self.frame().id;
                        let branches = self.branch_path.clone();
                        self.events.push(Event {
                            owner: id,
                            through: Some(id),
                            path: vec![],
                            span: base.span,
                            what: "advanced",
                            loops: self.loop_spans.clone(),
                            frame,
                            branches,
                        });
                    }
                    if let Some(class) = self.class_of(base.span) {
                        consuming = matches!(
                            class.as_str(),
                            "Map" | "MutMap" | "Set" | "MutSet" | "SortedSet"
                        ) && matches!(method.as_str(), "put" | "add");
                        key = self.method_key(&class, &method);
                    }
                    let loans = self.value_loans(&value);
                    if !loans.is_empty() {
                        let place = value.place.as_ref().map(|place| self.canonical(place));
                        let whole = ungroup(base).kind == K::Name
                            && place.as_ref().is_some_and(|place| place.path.is_empty());
                        call_loans.push((
                            base.span,
                            loans,
                            call_events..self.events.len(),
                            place,
                            whole,
                        ));
                    }
                    receiver = Some(value);
                }
            }
            K::Name => {
                let name = self.text(callee).to_owned();
                if self
                    .scopes
                    .iter()
                    .any(|scope| scope.names.contains_key(&name))
                {
                    let value = self.eval(callee, Ctx::Borrow);
                    if self.checked.mutated_receivers.contains(&callee.span) {
                        if value.readonly {
                            self.error(
                                callee.span,
                                "E_MUT_PARAMETER",
                                "This callable is a read-only returned borrow.".into(),
                                "Call through an owner or an explicit mut parameter.",
                            );
                        } else if let Some(place) = &value.place {
                            self.event(place, callee.span, "changed");
                        }
                    }
                    callable_loans = if value.function_copy {
                        self.callable_loans(&value)
                    } else if let Some(place) = self.place_of(callee) {
                        self.place_loans(&place)
                    } else {
                        value.loans
                    };
                    opaque = true;
                } else if name == "box"
                    || self.is_case(&name)
                    || matches!(
                        name.as_str(),
                        "Set"
                            | "MutSet"
                            | "SortedSet"
                            | "Map"
                            | "MutMap"
                            | "List"
                            | "Array"
                            | "Mutex"
                    )
                {
                    consuming = true;
                } else if let Some(id) = self.checked.environment.names.get(&name).filter(|id| {
                    self.checked
                        .environment
                        .definitions
                        .get(*id)
                        .is_some_and(|d| d.kind == K::Class)
                }) {
                    key = Some((id.clone(), "new".to_owned()));
                } else {
                    key = Some((String::new(), name));
                }
            }
            _ => {
                let value = self.eval(callee, Ctx::Borrow);
                changed_captures.extend(value.changed_captures.iter().copied());
                capture_accesses.insert(callee.span, changed_captures.clone());
                if self.checked.mutated_receivers.contains(&callee.span) && value.readonly {
                    self.error(
                        callee.span,
                        "E_MUT_PARAMETER",
                        "This callable is a read-only returned borrow.".into(),
                        "Call through an owner or an explicit mut parameter.",
                    );
                }
                callable_loans = value.loans;
                if callable_loans.is_empty() {
                    callable_loans.insert(Loan::Temporary);
                }
                opaque = true;
            }
        }
        if self
            .checked
            .resolved_calls
            .get(&node.span)
            .is_some_and(|call| match &call.target {
                crate::resolution::Callable::Declaration {
                    kind: crate::resolution::DeclarationKind::Variant,
                    ..
                } => true,
                crate::resolution::Callable::Builtin(name) => matches!(
                    name.rsplit('.').next(),
                    Some(
                        "Some"
                            | "box"
                            | "Box"
                            | "Set"
                            | "MutSet"
                            | "SortedSet"
                            | "Map"
                            | "MutMap"
                            | "List"
                            | "Array"
                            | "Mutex"
                            | "Atomic"
                    )
                ),
                _ => false,
            })
        {
            consuming = true;
        }
        if self.checked.resolved_calls.get(&node.span).is_some_and(|call| matches!(&call.target, crate::resolution::Callable::Builtin(name) if name.rsplit('.').next() == Some("Arena"))) {
            consuming = true;
            opaque = false;
            callable_loans.clear();
        }
        if let Some(call) = self.checked.resolved_calls.get(&node.span) {
            self.check_global_captures(&call.target, node.span);
        }
        if self
            .checked
            .resolved_calls
            .get(&node.span)
            .is_some_and(|call| matches!(call.target, crate::resolution::Callable::Indirect))
        {
            opaque = true;
            key = None;
            if callee.kind == K::Member {
                callable_loans = if let Some(place) = self.place_of(callee) {
                    self.place_loans(&place)
                } else if let Some(receiver) = &receiver {
                    self.value_loans(receiver)
                } else {
                    [Loan::Temporary].into()
                };
            }
        }
        if opaque && callee.kind != K::Member && !callable_loans.is_empty() {
            call_loans.push((
                callee.span,
                callable_loans.clone(),
                call_events..self.events.len(),
                None,
                false,
            ));
        }
        if let Some(call) = self.checked.resolved_calls.get(&node.span)
            && let crate::resolution::Callable::Declaration {
                id,
                owner,
                name,
                kind,
                ..
            } = &call.target
            && matches!(
                kind,
                crate::resolution::DeclarationKind::Function
                    | crate::resolution::DeclarationKind::Method
                    | crate::resolution::DeclarationKind::StaticMethod
                    | crate::resolution::DeclarationKind::Constructor
            )
        {
            key = Some((
                owner.clone().unwrap_or_default(),
                if *kind == crate::resolution::DeclarationKind::Constructor {
                    "new".into()
                } else if *kind == crate::resolution::DeclarationKind::Function
                    && id.file != self.source.name
                {
                    name.clone()
                } else {
                    name.rsplit('.').next().unwrap().to_owned()
                },
            ));
            opaque = false;
        }

        if self.checked.resolved_calls.get(&node.span).is_some_and(|call| matches!(&call.target, crate::resolution::Callable::Builtin(name) if name == "std.convert.toArray")) {
            consuming = false;
        }
        let mut arguments = vec![];
        let mut consuming_captures = vec![];
        let mut hole = false;
        let mut position = 0;
        for argument in &node.children[1..] {
            match argument.kind {
                K::ArrowArgument => {
                    for child in argument.children.iter().filter(|c| is_expression(c.kind)) {
                        self.eval(child, Ctx::Strict);
                    }
                }
                K::Argument | K::NamedArgument => {
                    let Some(value) = argument.children.last() else {
                        continue;
                    };
                    let name = (argument.kind == K::NamedArgument)
                        .then(|| argument.children.first().map(|n| self.text(n).to_owned()))
                        .flatten();
                    let index = if name.is_none() {
                        position += 1;
                        Some(position - 1)
                    } else {
                        None
                    };
                    if value.kind == K::Hole {
                        hole = true;
                        continue;
                    }
                    if (consuming
                        || self.checked.argument_modes.get(&value.span) == Some(&Passing::Move))
                        && !self.is_copy(value.span)
                    {
                        consuming_captures.push(value.span);
                    }
                    let own_events = self.events.len();
                    let found = if let Some((span, found)) = &piped
                        && *span == value.span
                    {
                        found.clone()
                    } else {
                        self.argument(value, consuming)
                    };
                    capture_accesses.insert(value.span, found.changed_captures.clone());
                    if let Some(Place {
                        root: Root::Binding(id),
                        ..
                    }) = &found.place
                    {
                        capture_through.insert(value.span, *id);
                    }
                    // Python may keep a function it is given, so the function must own
                    // what it captured (Q54).
                    if (found.lambda || found.function_copy)
                        && self.checked.python.contains_key(&node.span)
                    {
                        let mut loans = if found.function_copy {
                            self.callable_loans(&found)
                        } else {
                            self.value_loans(&found)
                        };
                        self.resolve_global_loans(&mut loans);
                        let globals = loans
                            .iter()
                            .map(|loan| match loan {
                                Loan::Global(id) | Loan::Local(id)
                                    if self.bindings[*id].frame == 0 =>
                                {
                                    Some(*id)
                                }
                                _ => None,
                            })
                            .collect::<Option<BTreeSet<_>>>();
                        if !found.lambda
                            && let Some(globals) = globals
                        {
                            self.foreign_globals.extend(globals);
                        } else if !loans.is_empty() {
                            self.lambda_escape(value.span, &loans);
                        }
                    }
                    if !consuming
                        && self.checked.argument_modes.get(&value.span) != Some(&Passing::Move)
                    {
                        let loans = if found.function_copy {
                            self.callable_loans(&found)
                        } else {
                            self.value_loans(&found)
                        };
                        if !loans.is_empty() {
                            let place = if found.function_copy {
                                None
                            } else {
                                found.place.as_ref().map(|place| self.canonical(place))
                            };
                            call_loans.push((
                                value.span,
                                loans,
                                own_events..self.events.len(),
                                place,
                                false,
                            ));
                        }
                    }
                    if !consuming
                        && (self.checked.argument_modes.get(&value.span) != Some(&Passing::Move)
                            || self.is_copy(value.span))
                    {
                        arguments.push((name, index, found));
                    }
                }
                _ => {
                    self.eval(argument, Ctx::Borrow);
                }
            }
        }
        if hole {
            for (span, loans, own_events, place, receiver) in call_loans {
                self.call_borrows.push(CallBorrow {
                    events: call_events..self.events.len(),
                    own_events,
                    capture_events: BTreeSet::new(),
                    receiver,
                    place,
                    loans,
                    frame: self.frame().id,
                    span,
                });
            }
            for span in consuming_captures {
                self.error(span, "E_PARTIAL_MOVE_CAPTURE", "This reusable partial call would hand over a captured owned value each time it runs.".into(), "Leave that argument as _, use a borrowing parameter, or make a direct call. Copyable captures such as String and List can be reused.");
            }
            // A partial call keeps its borrowed arguments until it is called.
            let mut loans = BTreeSet::new();
            for (_, _, value) in &arguments {
                loans.extend(self.value_loans(value));
                changed_captures.extend(value.changed_captures.iter().copied());
            }
            if let Some(receiver) = &receiver {
                loans.extend(self.value_loans(receiver));
            }
            self.resolve_global_loans(&mut loans);
            for id in &changed_captures {
                loans.insert(if *id == THIS {
                    Loan::This
                } else {
                    Loan::Local(*id)
                });
            }
            return Value {
                lambda: !loans.is_empty(),
                copy: loans.is_empty(),
                changed_captures,
                loans,
                ..Value::default()
            };
        }
        if let Some(call) = self.checked.resolved_calls.get(&node.span) {
            for (index, (argument, parameter)) in call
                .arguments
                .iter()
                .zip(&call.signature.parameters)
                .enumerate()
            {
                if *argument != crate::resolution::Argument::Default {
                    continue;
                }
                let expression = if self.validating_defaults {
                    None
                } else {
                    match &call.target {
                        crate::resolution::Callable::Declaration { id, .. }
                            if id.file == self.source.name =>
                        {
                            self.default_values
                                .get(&id.span)
                                .and_then(|values| values.get(index))
                                .cloned()
                                .flatten()
                        }
                        _ => None,
                    }
                };
                let value = if let Some(expression) = expression {
                    let earlier = call.signature.parameters[..index]
                        .iter()
                        .filter_map(|p| p.name.clone())
                        .collect::<Vec<_>>();
                    self.default_value(&expression, parameter.passing, node.span, &earlier)
                } else {
                    Value {
                        readonly: true,
                        copy: copies(&parameter.ty),
                        loans: [Loan::Temporary].into(),
                        ..Value::default()
                    }
                };
                if parameter.passing == Passing::Move && !copies(&parameter.ty) {
                    continue;
                }
                arguments.push((parameter.name.clone(), Some(index), value));
            }
        }
        // A callback may run inside the call. Keep its captured mutations even
        // when an alias or a function parameter hides the original declaration.
        for (_, _, value) in &arguments {
            changed_captures.extend(value.changed_captures.iter().copied());
        }
        let mut capture_events = BTreeMap::new();
        for id in &changed_captures {
            let place = Place {
                root: if *id == THIS {
                    Root::This
                } else {
                    Root::Binding(*id)
                },
                path: vec![],
                captured: false,
            };
            let first = self.events.len();
            self.event(&place, node.span, "changed");
            // The callable may change its own captures; other borrowed arguments
            // and views must still be checked against those changes.
            let through = capture_accesses.iter().find_map(|(span, captures)| {
                captures
                    .contains(id)
                    .then(|| capture_through.get(span).copied())
                    .flatten()
            });
            for event in &mut self.events[first..] {
                event.through = through;
            }
            capture_events.insert(*id, first..self.events.len());
        }
        let mut loans = BTreeSet::new();
        let mut lambda = false;
        for (span, loans, own_events, place, receiver) in call_loans {
            self.call_borrows.push(CallBorrow {
                events: call_events..self.events.len(),
                own_events,
                capture_events: capture_accesses
                    .get(&span)
                    .into_iter()
                    .flatten()
                    .filter_map(|owner| capture_events.get(owner))
                    .flat_map(|events| events.clone())
                    .collect(),
                receiver,
                place,
                loans,
                frame: self.frame().id,
                span,
            });
        }
        let returns_callable = self.type_of(node.span).is_some_and(callable_type);
        let returns_dependencies = self
            .type_of(node.span)
            .is_some_and(|ty| self.holds_dependencies(ty));
        if opaque {
            loans.extend(callable_loans);
            for (_, _, value) in &arguments {
                if !value.copy {
                    loans.extend(self.value_loans(value));
                }
            }
            if let Some(receiver) = &receiver
                && !receiver.copy
            {
                loans.extend(self.value_loans(receiver));
            }
        }
        if matches!(
            self.type_of(node.span),
            Some(Type::Lazy(..) | Type::Cursor(..))
        ) {
            // A lazy map or filter keeps the lambda it was given.
            for (_, _, value) in &arguments {
                if value.lambda || value.function_copy {
                    loans.extend(if value.function_copy {
                        self.callable_loans(value)
                    } else {
                        self.value_loans(value)
                    });
                    lambda |= value.lambda;
                }
            }
        }
        if let Some(key) = &key
            && let Some(sources) = self.summary(key)
        {
            let overloads = self.parameters(key);
            for source in sources {
                let captures = matches!(source, Loan::Captures(_));
                match source {
                    Loan::This => {
                        if let Some(receiver) = &receiver {
                            loans.extend(self.value_loans(receiver));
                        }
                    }
                    Loan::Global(id) => {
                        loans.insert(Loan::Global(id));
                    }
                    Loan::Param(parameter) | Loan::Captures(parameter) => {
                        for (name, index, value) in &arguments {
                            let matches = match (name, index) {
                                (Some(name), _) => *name == parameter,
                                (None, Some(index)) => overloads
                                    .iter()
                                    .any(|names| names.get(*index) == Some(&parameter)),
                                _ => false,
                            };
                            if matches {
                                if captures
                                    || value.function_copy
                                        && self.is_copy(node.span)
                                        && returns_dependencies
                                {
                                    loans.extend(self.callable_loans(value));
                                    if returns_callable {
                                        lambda |= value.lambda;
                                    }
                                } else if value.copy && value.place.is_none() {
                                    loans.insert(Loan::Temporary);
                                } else {
                                    loans.extend(self.value_loans(value));
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        if let Some(receiver) = &receiver
            && self.type_of(node.span).is_some_and(|ty| {
                contains_borrow(ty)
                    || self.holds_dependencies(ty)
                        && self.class_of(callee.children[0].span).as_deref() != Some("Arena")
                    || matches!(ty, Type::Guard(_))
            })
        {
            // A persistent Map's put or a Set's add makes a new collection, and an Array's pop
            // moves its last item out: the result keeps what the receiver borrows, not a loan
            // of the receiver's variable.
            let persistent_update = self.checked.resolved_calls.get(&node.span).is_some_and(|call| {
                matches!(&call.target, crate::resolution::Callable::Builtin(name)
                    if matches!(name.as_str(), "std.core.Map.put" | "std.core.Map.remove" | "std.core.Set.add" | "std.core.SortedSet.add"))
            });
            if let Some(place) = &receiver_place
                && !persistent_update
            {
                loans.extend(self.place_loans(place));
            } else {
                loans.extend(self.value_loans(receiver));
            }
        }
        // A local function's result keeps the captures its returned views came from.
        if (!self.is_copy(node.span) || returns_dependencies)
            && let Some(crate::resolution::ResolvedCall {
                target: crate::resolution::Callable::Declaration { id, .. },
                ..
            }) = self.checked.resolved_calls.get(&node.span)
            && let Some(owners) = self.local_results.get(&id.span)
        {
            for owner in owners {
                loans.extend(self.place_loans(&Place {
                    root: Root::Binding(*owner),
                    path: vec![],
                    captured: false,
                }));
            }
        }
        self.resolve_global_loans(&mut loans);
        // An Array's pop and a MutMap's remove move a stored item out. Items reach those
        // collections only through `move` parameters, which refuse a value that borrows
        // (E_LAMBDA_ESCAPE), so the item is owned and borrows nothing (Q163).
        if self
            .checked
            .resolved_calls
            .get(&node.span)
            .is_some_and(|call| {
                matches!(&call.target, crate::resolution::Callable::Builtin(name)
                if matches!(name.as_str(), "std.core.Array.pop" | "std.core.MutMap.remove"))
            })
        {
            return Value::owned(false);
        }
        // slice, pick and reversed build a new List; it shares the item cells but not the
        // receiver's storage, so it does not borrow the receiver.
        if self.checked.resolved_calls.get(&node.span).is_some_and(|call| {
            matches!(&call.target, crate::resolution::Callable::Builtin(name)
                if matches!(name.as_str(), "std.core.List.slice" | "std.core.List.pick" | "std.core.List.reversed"))
        }) {
            return Value::owned(false);
        }
        if self.is_copy(node.span) && (!returns_dependencies || loans.is_empty()) {
            return Value::owned(true);
        }
        if let Some(id) = stepping {
            loans.insert(Loan::Step(id));
        }
        Value {
            readonly: !loans.is_empty()
                && !consuming
                && !matches!(
                    self.type_of(node.span),
                    Some(Type::Guard(_) | Type::Cursor(..))
                )
                && !matches!(self.type_of(node.span), Some(Type::Lazy(_, traversal)) if traversal.mutates),
            lambda: lambda && !loans.is_empty(),
            changed_captures: if returns_dependencies {
                changed_captures
            } else {
                BTreeSet::new()
            },
            loans,
            ..Value::default()
        }
    }

    /// A cursor that `items()` returned from an abstract Iterable.
    fn hidden_cursor(&self, span: Span) -> bool {
        let mut ty = self.type_of(span);
        while let Some(Type::Borrow(inner) | Type::StoredView(inner)) = ty {
            ty = Some(inner);
        }
        matches!(ty, Some(Type::Cursor(..)))
    }

    /// A collection whose items a hidden cursor lends for one step at a time (Q67).
    fn abstract_collection(&self, span: Span) -> bool {
        let mut ty = self.type_of(span);
        loop {
            match ty {
                Some(Type::Borrow(inner) | Type::StoredView(inner)) => ty = Some(inner),
                Some(Type::Named(name, args)) if name == "Box" && args.len() == 1 => {
                    ty = Some(&args[0])
                }
                _ => break,
            }
        }
        matches!(
            ty,
            Some(Type::Dynamic(_) | Type::Parameter(_) | Type::Rigid(_) | Type::Associated(_, _))
        )
    }

    /// Adds the hidden owner of one traversal step to the items' loans.
    fn step_loans(&mut self, collection: &Node, loans: &mut BTreeSet<Loan>) {
        if self.abstract_collection(collection.span) {
            let id = self.bindings.len();
            self.declare(STEP, Origin::Owned, &Value::owned(false), collection.span);
            loans.insert(Loan::Step(id));
        }
    }

    fn abstract_receiver(&self, span: Span) -> bool {
        let mut ty = self.type_of(span);
        while let Some(Type::Borrow(inner) | Type::StoredView(inner)) = ty {
            ty = Some(inner);
        }
        matches!(
            ty,
            Some(Type::Dynamic(_) | Type::Parameter(_) | Type::Rigid(_) | Type::Associated(_, _))
        )
    }

    fn lambda(&mut self, node: &Node) -> Value {
        let take = self.modifier(node, "move");
        let changed_captures: BTreeSet<usize> = node
            .descendants()
            .filter(|part| part.kind == K::Call)
            .filter_map(|part| part.children.first())
            .flat_map(|callee| self.callable_changes(callee))
            .collect();
        let saved = (
            self.moved.clone(),
            self.diverged,
            std::mem::take(&mut self.loops),
        );
        let index = self.captures.len();
        self.captures.push(BTreeMap::new());
        self.captured_globals.push(BTreeSet::new());
        self.enter_frame(None, false);
        self.frames.last_mut().unwrap().moving_lambda = take;
        self.push_scope(Boundary::Lambda(index), node.span);
        if let Some(parameters) = node.child(K::LambdaParameters) {
            let mut names = parameters
                .children
                .iter()
                .filter_map(|p| match p.kind {
                    K::Parameter => self.named_child(p).map(|name| {
                        (
                            name,
                            p.span,
                            self.modifier(p, "move"),
                            self.modifier(p, "mut"),
                        )
                    }),
                    K::Name | K::PatternName => {
                        Some((self.text(p).to_owned(), p.span, false, false))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            if parameters.children.is_empty() {
                let text = self.text(parameters).to_owned();
                if is_identifier(&text) {
                    names.push((text, parameters.span, false, false));
                }
            }
            // A parameter may take `mut` from the type the checker expected (REVIEW 1.6).
            let inferred: Vec<bool> = match self.type_of(node.span) {
                Some(Type::Function(function)) => function
                    .parameters
                    .iter()
                    .map(|parameter| parameter.passing == Passing::Mut)
                    .collect(),
                _ => vec![],
            };
            for (index, (name, span, moving, mutable)) in names.into_iter().enumerate() {
                let mutable = mutable || inferred.get(index).copied().unwrap_or(false);
                let ty = self.checked.binding_types.get(&span);
                let origin = if moving
                    && (!ty.is_some_and(copies) || ty.is_some_and(|ty| self.holds_dependencies(ty)))
                {
                    Origin::Owned
                } else {
                    Origin::Param
                };
                self.declare(
                    &name,
                    origin,
                    &Value {
                        readonly: !moving && !mutable,
                        dependency_copy: ty
                            .is_some_and(|ty| copies(ty) && self.holds_dependencies(ty)),
                        ..Value::default()
                    },
                    span,
                );
            }
        }
        if let Some(body) = node
            .children
            .last()
            .filter(|c| is_expression(c.kind) && c.kind != K::LambdaParameters)
        {
            self.eval(body, Ctx::Return);
        }
        self.scopes.pop();
        self.frames.pop();
        let captured = self.captures.pop().unwrap_or_default();
        let mut globals = self
            .captured_globals
            .pop()
            .unwrap_or_default()
            .into_iter()
            .map(Loan::Global)
            .collect::<BTreeSet<_>>();
        self.resolve_global_loans(&mut globals);
        (self.moved, self.diverged, self.loops) = saved;
        if take {
            if !changed_captures.is_empty() {
                let loans = changed_captures
                    .iter()
                    .map(|id| {
                        if *id == THIS {
                            Loan::This
                        } else {
                            Loan::Local(*id)
                        }
                    })
                    .collect();
                self.lambda_escape(node.span, &loans);
            }
            for (id, copy) in captured {
                if copy {
                    if id != THIS
                        && (self.bindings[id].dependency_copy
                            || !self.bindings[id].loans.is_empty())
                    {
                        let value = Value {
                            place: Some(Place {
                                root: Root::Binding(id),
                                path: vec![],
                                captured: false,
                            }),
                            ..Value::default()
                        };
                        let loans = self.callable_loans(&value);
                        if !loans.is_empty() {
                            self.lambda_escape_from(node.span, &loans, true);
                        }
                    }
                    continue;
                }
                let place = if id == THIS {
                    Place {
                        root: Root::This,
                        path: vec![],
                        captured: false,
                    }
                } else {
                    if globals.contains(&Loan::Local(id)) || globals.contains(&Loan::Global(id)) {
                        self.error(node.span, "E_MOVE_BORROWED", "This move capture would invalidate a named function's reference to the original script storage.".into(), "Pass the object explicitly to the named function or keep borrowing the script object.");
                    }
                    Place {
                        root: Root::Binding(id),
                        path: vec![],
                        captured: self.bindings[id].frame != self.frame().id,
                    }
                };
                let value = Value {
                    place: Some(place),
                    ..Value::default()
                };
                self.take(node.span, &value);
            }
            return Value {
                lambda: !globals.is_empty(),
                copy: globals.is_empty(),
                loans: globals,
                ..Value::default()
            };
        }
        let mut loans = globals;
        for id in &changed_captures {
            loans.insert(if *id == THIS {
                Loan::This
            } else {
                Loan::Local(*id)
            });
        }
        for id in captured.keys() {
            if *id == THIS {
                loans.insert(Loan::This);
            } else {
                let place = Place {
                    root: Root::Binding(*id),
                    path: vec![],
                    captured: false,
                };
                loans.extend(self.place_loans(&place));
            }
        }
        self.resolve_global_loans(&mut loans);
        Value {
            lambda: !loans.is_empty(),
            copy: loans.is_empty(),
            changed_captures,
            loans,
            ..Value::default()
        }
    }

    fn branch(&mut self, node: &Node, ctx: Ctx, start: &Moved) -> (Value, Option<Moved>) {
        self.moved = start.clone();
        self.diverged = false;
        let value = self.eval(node, ctx);
        let state = (!self.diverged).then(|| self.moved.clone());
        (value, state)
    }

    fn if_expression(&mut self, node: &Node, ctx: Ctx) -> Value {
        let Some(condition) = node.children.first() else {
            return Value::owned(true);
        };
        self.eval(condition, Ctx::Borrow);
        let start = self.moved.clone();
        let mut values = vec![];
        let mut states = vec![];
        for (index, arm) in node
            .children
            .iter()
            .skip(1)
            .filter(|c| is_expression(c.kind))
            .enumerate()
        {
            self.branch_path.push(BranchStep {
                node: node.span.start,
                arm: index,
            });
            let (value, state) = self.branch(arm, ctx, &start);
            self.branch_path.pop();
            values.push(value);
            states.push(state);
        }
        if values.len() < 2 {
            states.push(Some(start.clone()));
        }
        self.join(states, start);
        merge_values(values)
    }

    fn match_expression(&mut self, node: &Node, ctx: Ctx) -> Value {
        let Some(scrutinee) = node.children.first() else {
            return Value::owned(true);
        };
        // `match (a, b) { ... }` only looks at its parts, so the tuple borrows them
        // instead of taking them (REVIEW 1.6).
        let mut value = if ungroup(scrutinee).kind == K::Tuple {
            let mut loans = BTreeSet::new();
            let mut copy = true;
            for part in ungroup(scrutinee)
                .children
                .iter()
                .filter(|c| is_expression(c.kind))
            {
                let part_value = self.eval(part, Ctx::Borrow);
                copy &= part_value.copy;
                if !part_value.copy {
                    loans.extend(self.value_loans(&part_value));
                    if part_value.loans.is_empty() && part_value.place.is_none() {
                        loans.insert(Loan::Temporary);
                    }
                }
            }
            if copy {
                Value::owned(true)
            } else {
                Value {
                    readonly: true,
                    loans,
                    ..Value::default()
                }
            }
        } else {
            self.eval(scrutinee, Ctx::Borrow)
        };
        if matches!(self.type_of(scrutinee.span),Some(Type::Named(name,_)) if matches!(name.as_str(),"List" | "Map" | "Set" | "SortedSet"))
        {
            value.readonly = true;
            value.copy = false;
            value.place = self.place_of(scrutinee);
            if value.place.is_none() && value.loans.is_empty() {
                value.loans.insert(Loan::Temporary);
            }
        }
        let start = self.moved.clone();
        let mut values = vec![];
        let mut states = vec![];
        for (index, arm) in node
            .children
            .iter()
            .filter(|c| c.kind == K::MatchArm)
            .enumerate()
        {
            self.moved = start.clone();
            self.push_scope(Boundary::None, arm.span);
            if let Some(pattern) = arm.children.first() {
                self.declare_pattern(pattern, &value, false);
            }
            if let Some(guard) = arm.child(K::Guard) {
                for child in &guard.children {
                    self.eval(child, Ctx::Borrow);
                }
            }
            let after_guard = self.moved.clone();
            if let Some(body) = arm
                .children
                .last()
                .filter(|c| c.kind != K::Guard && arm.children.len() > 1)
            {
                self.branch_path.push(BranchStep {
                    node: node.span.start,
                    arm: index,
                });
                let (found, state) = self.branch(body, ctx, &after_guard);
                self.branch_path.pop();
                values.push(found);
                states.push(state);
            }
            self.scopes.pop();
        }
        self.join(states, start);
        merge_values(values)
    }

    fn join(&mut self, states: Vec<Option<Moved>>, start: Moved) {
        let reached = states.into_iter().flatten().collect::<Vec<_>>();
        self.diverged = reached.is_empty();
        self.moved = merge_all(reached).unwrap_or(start);
    }

    fn comprehension(&mut self, node: &Node) {
        self.push_scope(Boundary::None, node.span);
        self.comprehension_clause(node);
        for clause in node
            .children
            .iter()
            .filter(|c| c.kind == K::ComprehensionFor)
        {
            self.comprehension_clause(clause);
        }
        if let Some(element) = node.children.first().filter(|c| is_expression(c.kind)) {
            self.eval(element, Ctx::Strict);
        }
        self.scopes.pop();
    }
    /// One `for pattern in collection if guard` clause of a comprehension.
    fn comprehension_clause(&mut self, node: &Node) {
        let pattern = node.children.iter().find(|c| is_pattern(c.kind));
        let pattern_index = node.children.iter().position(|c| is_pattern(c.kind));
        let collection = pattern_index.and_then(|i| node.children.get(i + 1));
        if let Some(collection) = collection {
            let value = self.eval(collection, Ctx::Borrow);
            let mut loans = if let Some(place) = self.place_of(collection) {
                self.place_loans(&place)
            } else if !value.loans.is_empty() {
                value.loans.clone()
            } else {
                [Loan::Temporary].into()
            };
            self.step_loans(collection, &mut loans);
            if let Some(pattern) = pattern {
                for (name, span) in pattern
                    .descendants()
                    .filter(|n| n.kind == K::PatternName)
                    .map(|n| (self.text(n).to_owned(), n.span))
                    .collect::<Vec<_>>()
                {
                    let view = Value {
                        readonly: true,
                        loans: loans.clone(),
                        ..Value::default()
                    };
                    self.declare(&name, Origin::View, &view, span);
                }
            }
        }
        if let Some(guard) = node.child(K::Guard) {
            for child in &guard.children {
                self.eval(child, Ctx::Borrow);
            }
        }
    }

    /// Whether code can run after an event and reach a use. A use in another arm of the same if or
    /// match cannot follow the event, and nothing after a return that holds the event can either.
    /// A use in another frame is assumed reachable.
    fn reachable(
        &self,
        event: Span,
        frame: usize,
        branches: &[BranchStep],
        use_span: Span,
    ) -> bool {
        let Some((use_frame, use_branches)) = self.use_paths.get(&use_span.start) else {
            return true;
        };
        if *use_frame != frame {
            return true;
        }
        for (event_step, use_step) in branches.iter().zip(use_branches) {
            if event_step.node != use_step.node {
                break;
            }
            if event_step.arm != use_step.arm {
                return false;
            }
        }
        !self.returns.iter().any(|(ret, ret_frame)| {
            *ret_frame == frame
                && ret.start <= event.start
                && event.end <= ret.end
                && ret.end <= use_span.start
        })
    }

    /// Reports owners that move or change while a borrow of them is still used.
    fn borrow_conflicts(&mut self) {
        if self.exhausted {
            return;
        }
        let events = std::mem::take(&mut self.events);
        for (event_index, event) in events.into_iter().enumerate() {
            let Event {
                owner,
                through,
                path,
                span,
                what,
                loops,
                frame,
                branches,
            } = event;
            if !self.charge(self.call_borrows.len(), span) {
                return;
            }
            let conflicts = self
                .call_borrows
                .iter()
                .find(|borrow| {
                    borrow.events.contains(&event_index)
                        && !borrow.own_events.contains(&event_index)
                        && !borrow.capture_events.contains(&event_index)
                        && (!borrow.receiver || what != "changed")
                        && !borrow.place.as_ref().is_some_and(|place| {
                            matches!(place.root, Root::Binding(id) if id == owner)
                                && !place.path.starts_with(&path)
                                && !path.starts_with(&place.path)
                        })
                        && (borrow.loans.contains(&Loan::Local(owner))
                            || borrow.loans.contains(&Loan::Global(owner))
                            || borrow.loans.contains(&Loan::Step(owner))
                            || matches!(self.bindings[owner].origin, Origin::Param)
                                && self.bindings[owner].frame == borrow.frame
                                && borrow
                                    .loans
                                    .iter().any(|loan| matches!(loan, Loan::Param(name) | Loan::Captures(name) if *name == self.bindings[owner].name)))
                })
                .map(|borrow| borrow.span);
            if let Some(callback) = conflicts {
                let name = &self.bindings[owner].name;
                self.error(span, "E_BORROW_CONFLICT", format!("`{name}` is {what} during a call whose argument still borrows it (line {}).", self.line(callback)), "Keep borrowed arguments and their owners alive and unchanged until the call finishes.");
            }
            if self.foreign_globals.contains(&owner) {
                let name = &self.bindings[owner].name;
                self.error(span, "E_BORROW_CONFLICT", format!("`{name}` is {what} here, but foreign code may still call a function that reads it."), "Keep script storage unchanged after exporting a function, or pass a move lambda that owns its captures.");
            }
            for id in 0..self.bindings.len() {
                if !self.charge(1, span) {
                    return;
                }
                if id == owner || Some(id) == through || self.bindings[id].declared.end > span.start
                {
                    continue;
                }
                let borrows = match &self.bindings[id].origin {
                    Origin::Part(place) => {
                        let place = self.canonical(place);
                        place.root == Root::Binding(owner)
                            && (place.path.starts_with(&path) || path.starts_with(&place.path))
                    }
                    _ => {
                        self.bindings[id].loans.contains(&Loan::Local(owner))
                            || self.bindings[id].loans.contains(&Loan::Step(owner))
                            || matches!(self.bindings[owner].origin, Origin::Param)
                                && self.bindings[id].frame == self.bindings[owner].frame
                                && self.bindings[id]
                                    .loans
                                    .iter().any(|loan| matches!(loan, Loan::Param(name) | Loan::Captures(name) if *name == self.bindings[owner].name))
                    }
                };
                if !borrows {
                    continue;
                }
                // A use later in the source runs later; so does any use inside a
                // loop around the event, if the borrow was made before that loop.
                let declared = self.bindings[id].declared;
                let work = self
                    .uses
                    .get(&id)
                    .map_or(0, Vec::len)
                    .saturating_mul(loops.len().max(1));
                if !self.charge(work, span) {
                    return;
                }
                let later = self
                    .uses
                    .get(&id)
                    .and_then(|uses| {
                        uses.iter().find(|u| {
                            u.start >= span.end && self.reachable(span, frame, &branches, **u)
                                || loops.iter().any(|l| {
                                    declared.end <= l.start && l.start <= u.start && u.end <= l.end
                                })
                        })
                    })
                    .copied();
                if let Some(later) = later {
                    let owner_name = self.bindings[owner].name.clone();
                    let borrower = self.bindings[id].name.clone();
                    let line = self.line(later);
                    self.error(
                        span,
                        "E_BORROW_CONFLICT",
                        format!("`{owner_name}` is {what} here while `{borrower}` still borrows it; `{borrower}` is used at line {line}."),
                        "Finish using the borrow first, or leave the owner unchanged until then.",
                    );
                    break;
                }
            }
        }
    }
}

fn merge(a: &Moved, b: &Moved) -> Moved {
    let mut result = Moved::new();
    for (key, found) in a.iter().chain(b.iter()) {
        let both = a.get(key).is_some() && b.get(key).is_some();
        let entry = result.entry(key.clone()).or_insert(*found);
        entry.maybe |= found.maybe || !both;
    }
    result
}

fn merge_all(states: Vec<Moved>) -> Option<Moved> {
    states.into_iter().reduce(|a, b| merge(&a, &b))
}

fn merge_values(values: Vec<Value>) -> Value {
    let mut values = values.into_iter();
    let Some(mut result) = values.next() else {
        return Value::owned(true);
    };
    for value in values {
        if result.part != value.part {
            result.part = None;
        }
        result.loans.extend(value.loans);
        result.changed_captures.extend(value.changed_captures);
        result.lambda |= value.lambda;
        result.copy &= value.copy;
    }
    result.place = None;
    result
}

fn contains_borrow(ty: &Type) -> bool {
    match ty {
        Type::Borrow(_) | Type::StoredView(_) | Type::Guard(_) => true,
        Type::Named(_, args) | Type::Tuple(args) | Type::Union(args) => {
            args.iter().any(contains_borrow)
        }
        Type::Lazy(inner, _) | Type::Cursor(inner, _) => contains_borrow(inner),
        _ => false,
    }
}

fn ungroup(mut node: &Node) -> &Node {
    while node.kind == K::Group && node.children.len() == 1 {
        node = &node.children[0];
    }
    node
}

fn is_identifier(text: &str) -> bool {
    crate::identifiers::word(text)
}

fn is_pattern(kind: K) -> bool {
    matches!(
        kind,
        K::PatternName
            | K::PatternWildcard
            | K::PatternLiteral
            | K::PatternConstructor
            | K::NamedPattern
            | K::PatternTuple
            | K::PatternList
            | K::PatternRest
    )
}

fn is_statement(kind: K) -> bool {
    matches!(
        kind,
        K::Binding
            | K::FieldDeclaration
            | K::Assignment
            | K::For
            | K::While
            | K::Return
            | K::Break
            | K::Continue
            | K::Function
            | K::Constructor
            | K::Class
            | K::Trait
            | K::Enum
            | K::Alias
            | K::Const
            | K::Import
            | K::Theorem
            | K::Axiom
            | K::Example
            | K::Package
            | K::Dependency
            | K::Attribute
    )
}

fn is_expression(kind: K) -> bool {
    !is_statement(kind)
        && !is_pattern(kind)
        && !matches!(
            kind,
            K::Modifier
                | K::Operator
                | K::TypeParameters
                | K::TypeParameter
                | K::Bound
                | K::ParameterList
                | K::Parameter
                | K::ReturnType
                | K::Effects
                | K::Requires
                | K::Ensures
                | K::TypeName
                | K::TypeApplication
                | K::IndexedType
                | K::TupleType
                | K::ShapeType
                | K::FunctionType
                | K::UnionType
                | K::BorrowType
                | K::MutableType
                | K::DynType
                | K::AssociatedType
                | K::LambdaParameters
                | K::Otherwise
                | K::Guard
                | K::MatchArm
                | K::ForBinding
                | K::FormatText
                | K::FormatSpec
                | K::Has
                | K::Variant
                | K::Proof
                | K::Induction
                | K::Simp
                | K::Proposition
                | K::Error
        )
}

fn callable_type(ty: &Type) -> bool {
    match ty {
        Type::Function(_) | Type::Overloaded(_) => true,
        Type::Borrow(inner) | Type::StoredView(inner) => callable_type(inner),
        _ => false,
    }
}

/// The variable a place expression starts from: `a` in `a.b[i].c`.
fn place_root_text<'a>(node: &Node, source: &'a SourceFile) -> Option<&'a str> {
    let node = ungroup(node);
    match node.kind {
        K::Name => Some(source.slice(node.span)),
        K::Member | K::Index => node
            .children
            .first()
            .and_then(|n| place_root_text(n, source)),
        _ => None,
    }
}
