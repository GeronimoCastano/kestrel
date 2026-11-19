use crate::{
    Parsed,
    diagnostic::Diagnostic,
    resolution::{
        Argument as ResolvedArgument, Callable, DeclarationId, DeclarationKind, Reference,
        ResolvedCall, erase_callable_identities,
    },
    source::{SourceFile, Span},
    syntax::{Node, SyntaxKind as K},
    types::{FunctionType, Index, Inference, Parameter, Passing, Type},
};
use std::collections::{BTreeMap, BTreeSet};
#[path = "associated.rs"]
mod associated_types;

#[derive(Clone, Debug)]
pub struct Checked {
    pub binding_types: BTreeMap<Span, Type>,
    pub diagnostics: Vec<Diagnostic>,
    pub expression_types: Vec<(Span, Type)>,
    /// Passing mode of each argument expression, from the selected overload.
    pub argument_modes: BTreeMap<Span, Passing>,
    /// Receiver expressions of calls to mut methods.
    pub mutated_receivers: BTreeSet<Span>,
    pub(crate) mutation_roots: BTreeMap<Span, BTreeSet<String>>,
    pub(crate) owned_returns: BTreeSet<Span>,
    /// Expressions placed into a new box because a `Box[...]` was expected,
    /// with that Box type (Q86).
    pub(crate) autoboxed: BTreeMap<Span, Type>,
    /// Int expressions that widen to BigInt where a BigInt is expected, with that BigInt type (Q125).
    pub(crate) widened: BTreeMap<Span, Type>,
    pub(crate) borrowed_views: BTreeMap<Span, Type>,
    /// How each direct call to a function with `requires` is checked.
    pub contract_checks: BTreeMap<Span, crate::contracts::Status>,
    /// Direct calls to checked theorems used as erased proof arguments.
    pub(crate) theorem_evidence: BTreeSet<Span>,
    pub(crate) state_proof_dispatches: Vec<crate::proof::Dispatch>,
    pub(crate) theorem_requirements: BTreeSet<Span>,
    pub(crate) theorem_bounds: BTreeMap<Span, BTreeMap<String, Vec<Type>>>,
    pub resolved_calls: BTreeMap<Span, ResolvedCall>,
    pub references: BTreeMap<Span, Reference>,
    /// `Type.field` expressions that name a static final field.
    pub static_references: BTreeMap<Span, (String, String)>,
    /// Every static final field, with the module and span of its initializer.
    pub static_fields: BTreeMap<(String, String), StaticFieldInfo>,
    /// Python calls and attribute reads, keyed by the call or member expression.
    pub python: BTreeMap<Span, PythonOperation>,
    pub(crate) environment: Environment,
}

/// A static final field and where its lazy initializer lives.
#[derive(Clone, Debug)]
pub struct StaticFieldInfo {
    pub ty: Type,
    pub mutable: bool,
    pub file: String,
    pub initializer: Span,
}

/// One trip across the Python boundary (Q46, Q49).
#[derive(Clone, Debug)]
pub struct PythonOperation {
    /// The `import python "name"` module, when the receiver is its local name.
    pub module: Option<String>,
    /// Otherwise the expression whose PythonValue is the receiver.
    pub receiver: Option<Span>,
    pub attribute: Option<String>,
    pub call: bool,
    /// Arguments in source order; a name marks a keyword argument.
    pub arguments: Vec<(Option<String>, Span)>,
    /// `T or Error`, checked when the call runs.
    pub result: Type,
}

impl Checked {
    pub fn is_ok(&self) -> bool {
        self.diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity == crate::diagnostic::Severity::Warning)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Symbol {
    pub ty: Type,
    pub quantified: BTreeSet<usize>,
    pub mutable: bool,
    pub visibility: Visibility,
    pub access: Option<bool>,
    pub origin: Option<Vec<String>>,
    pub reference: Option<Reference>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Visibility {
    Private,
    Package,
    Public,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Environment {
    pub values: BTreeMap<String, Symbol>,
    /// Names every program sees without an import, such as max (Q114). They are read only after
    /// the file's own values, so a declaration with the same name shadows them freely.
    pub implicit: BTreeMap<String, Symbol>,
    pub names: BTreeMap<String, String>,
    pub definitions: BTreeMap<String, Definition>,
    /// Facts the ownership and contract passes found, keyed by (class id or "", name).
    pub calls: BTreeMap<(String, String), CallFacts>,
    /// Imported C names the boundary cannot carry, with the reason.
    pub foreign: BTreeMap<String, String>,
    /// Project headers resolved while checking each import, before native lowering.
    pub c_headers: BTreeMap<String, std::path::PathBuf>,
    pub proofs: crate::proof::Definitions,
    /// Theorem names visible through imports, local or qualified, and their declarations.
    pub theorems: BTreeMap<String, crate::resolution::DeclarationId>,
}

/// What callers in other modules need to know about a function, method or constructor.
#[derive(Clone, Debug, Default)]
pub(crate) struct CallFacts {
    pub state: BTreeMap<DeclarationId, crate::contracts::state::Summary>,
    /// Parameter names of each overload.
    pub parameters: Vec<Vec<String>>,
    /// Parameter names, or `this`, that the result may borrow.
    pub borrows: BTreeSet<String>,
    pub requires: Vec<crate::contracts::Requirement>,
    /// Built from an import cycle's headers, before the bodies were checked.
    /// Facts from a checked body replace these when both reach one module.
    pub header: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct Definition {
    pub kind: K,
    pub sealed: bool,
    /// A `record` class. Records and enums compare and print by their fields
    /// unless they write `equals` or `text` themselves (REVIEW 2.2).
    pub record: bool,
    pub parameters: Vec<String>,
    pub associated_types: BTreeSet<String>,
    pub bounds: BTreeMap<String, Vec<Type>>,
    pub fields: BTreeMap<String, Type>,
    /// Fields introduced by constructor declarations. They are private, and
    /// because Kestrel constructors do not chain, a subclass cannot initialize
    /// them; the checker refuses to extend a class that has any.
    pub constructor_fields: BTreeSet<String>,
    pub static_fields: BTreeMap<String, Type>,
    pub static_vars: BTreeSet<String>,
    pub mutable_fields: BTreeSet<String>,
    pub initialized_fields: BTreeSet<String>,
    /// `invariant` clauses of a class body. A subclass inherits every ancestor's
    /// clauses; each check runs at method and constructor exits.
    pub invariants: Vec<Node>,
    pub methods: BTreeMap<String, Vec<FunctionType>>,
    /// Methods whose written result is `Self`. An override may narrow one to a
    /// subclass, which no other result allows.
    pub self_results: BTreeSet<DeclarationId>,
    /// Method declaration spans whose trait declarations have bodies. Kept
    /// separately from signatures because an abstract and a default method
    /// otherwise have the same callable type.
    pub default_methods: BTreeSet<Span>,
    pub static_methods: BTreeSet<String>,
    pub parent: Option<Type>,
    pub traits: Vec<Type>,
    pub constructors: Vec<FunctionType>,
    pub variants: Vec<Variant>,
    pub alias: Option<Type>,
    pub visibility: Visibility,
    pub member_visibility: BTreeMap<String, Visibility>,
    pub file: String,
}

#[derive(Clone, Debug)]
pub(crate) struct Variant {
    pub name: String,
    pub declaration: DeclarationId,
    pub fields: Vec<Type>,
    /// The payload name each field declares, in the same order as `fields`.
    /// A case names either every payload or none of them.
    pub field_names: Vec<Option<String>>,
    pub result: Type,
}

impl Variant {
    /// The case as a callable. A named payload reaches its field by label, so the
    /// names travel in the signature and the argument rule mirrors ordinary calls.
    fn function(&self) -> Option<FunctionType> {
        if self.fields.is_empty() {
            return None;
        }
        let Type::Function(mut function) = Type::function(self.fields.clone(), self.result.clone())
        else {
            unreachable!()
        };
        for (parameter, name) in function.parameters.iter_mut().zip(&self.field_names) {
            parameter.name = name.clone();
        }
        Some(function)
    }
}

#[derive(Clone)]
struct TraitDefault {
    name: String,
    signature: FunctionType,
    declaration: DeclarationId,
    provider: String,
    is_static: bool,
    visibility: Visibility,
}

/// A local theorem's ordinary (non-hypothesis) arguments. Theorem names stay
/// outside the value environment because they have no run-time value.
type TheoremSignature = FunctionType;

pub fn check(parsed: &Parsed) -> Checked {
    let (environment, diagnostics) = crate::modules::source_imports(parsed);
    let mut result = check_with_imports(parsed, "", environment);
    result.diagnostics.splice(0..0, diagnostics);
    result
}

/// Reuse the language's trait rules when the proof kernel checks a bound.
pub(crate) fn proof_implements(
    parsed: &Parsed,
    checked: &Checked,
    bounds: &BTreeMap<String, Vec<Type>>,
    actual: &Type,
    requirement: &Type,
) -> bool {
    let mut valid = true;
    for ty in [actual, requirement] {
        ty.map(&mut |part| {
            if matches!(part, Type::Invalid | Type::Variable(_))
                || matches!(part,Type::Parameter(name) if name.starts_with('?'))
            {
                valid = false;
            }
            None
        });
    }
    if !valid {
        return false;
    }
    let mut checker = Checker::new(
        &parsed.source,
        "",
        Environment {
            names: checked.environment.names.clone(),
            definitions: checked.environment.definitions.clone(),
            ..Environment::default()
        },
    );
    checker.bounds = bounds.clone();
    checker.implements(actual, requirement, &mut BTreeSet::new())
}

/// A closed declaration interface that can be used before checking bodies in
/// an import cycle. Copying primitive results need no deferred borrow summary.
/// Whether a file in an import cycle declares only what a header interface can
/// describe: imports, functions, classes, traits, enums and aliases, with every
/// parameter and result typed. Default presence is part of each signature;
/// default expressions are checked with the body. Call-free runtime contracts
/// keep guards at header calls. Theorem requirements, constants and theorems
/// need checked bodies, so a cycle cannot contain them.
pub(crate) fn cycle_certifiable(parsed: &Parsed) -> bool {
    fn header(node: &Node, defaults: bool) -> bool {
        node.child(K::ParameterList).is_none_or(|list| {
            list.children.iter().all(|parameter| {
                let parts = parameter
                    .children
                    .iter()
                    .filter(|c| c.kind != K::Modifier)
                    .collect::<Vec<_>>();
                matches!(parts.as_slice(), [name, ty] if name.kind == K::Name && is_type(ty.kind))
                    || (defaults
                        && matches!(parts.as_slice(), [name, ty, _] if name.kind == K::Name && is_type(ty.kind)))
            })
        })
    }
    fn function(node: &Node) -> bool {
        node.child(K::ReturnType).is_some() && header(node, true)
    }
    parsed.is_ok()
        && !parsed.root.descendants().any(|n| {
            n.kind == K::Requires
                && n.descendants()
                    .any(|part| matches!(part.kind, K::Call | K::Pipe))
        })
        && parsed.root.children.iter().all(|node| match node.kind {
            K::Import | K::Alias => true,
            // A cycle header records a constant's written type; its value is
            // checked with the member's body after the headers settle.
            K::Const => node.children.iter().any(|child| is_type(child.kind)),
            K::Function => function(node),
            K::Enum => header(node, false),
            K::Class | K::Trait => node.children.iter().all(|member| match member.kind {
                K::Function => function(member),
                K::Constructor => header(member, true),
                _ => true,
            }),
            _ => false,
        })
}

/// The first round of a cycle's interfaces: each type a file declares, with
/// nothing inside it yet. Starting from names, rather than from nothing, keeps
/// a type another member mentions from looking unknown in the first round.
pub(crate) fn cycle_skeleton(parsed: &Parsed, module: &str) -> Environment {
    let mut checker = Checker::new(&parsed.source, module, Environment::default());
    let builtin = checker.environment.names.clone();
    checker.register_types(&parsed.root.children);
    let mut names = std::mem::take(&mut checker.environment.names);
    names.retain(|name, id| builtin.get(name) != Some(id));
    let definitions = std::mem::take(&mut checker.environment.definitions)
        .into_iter()
        .filter(|(id, _)| names.values().any(|own| own == id))
        .collect();
    Environment {
        names,
        definitions,
        ..Environment::default()
    }
}

/// The declarations a file in an import cycle shows its importers, built from
/// headers alone. Callers cannot see the bodies yet, so every function, method
/// and constructor may return a borrow of each argument and of `this`.
pub(crate) fn cycle_interface(
    parsed: &Parsed,
    module: &str,
    imports: Environment,
) -> (Environment, Vec<Diagnostic>) {
    let mut checker = Checker::new(&parsed.source, module, imports);
    checker.declarations(&parsed.root.children);
    let _copy_definitions = crate::types::copy_definitions(&checker.environment.definitions);
    // An alias that fails to expand in any round is part of a real loop, even
    // when a later round only sees the silent result of that failure.
    let mut loops = vec![];
    for node in parsed.root.children.iter().filter(|n| n.kind == K::Alias) {
        if let Some(id) = checker
            .environment
            .names
            .get(&checker.named_child(node))
            .cloned()
            && checker
                .expand(&Type::named(&id), &mut BTreeSet::new())
                .is_none()
        {
            loops.push(
                Diagnostic::new(
                    "E_TYPE_CYCLE",
                    "This alias is cyclic or exceeds the expansion limit.",
                    node.span,
                )
                .with_hint("Make aliases end in a concrete type and split long chains."),
            );
        }
    }
    let slice = |node: &Node| parsed.source.slice(node.span).to_owned();
    // Numbers, Bool and String hold no class value a result could borrow.
    let plain = |ty: &Type| crate::types::scalar(ty) || *ty == Type::named("String");
    let mut calls = BTreeMap::<(String, String), CallFacts>::new();
    let mut add =
        |key: (String, String), signatures: &[FunctionType], borrows: bool, this: bool| {
            let facts = calls.entry(key).or_default();
            for signature in signatures {
                let names = signature
                    .parameters
                    .iter()
                    .map(|p| p.name.clone().unwrap_or_default())
                    .collect::<Vec<_>>();
                if borrows {
                    facts.borrows.extend(
                        signature
                            .parameters
                            .iter()
                            .filter(|p| !plain(&p.ty))
                            .filter_map(|p| p.name.clone()),
                    );
                }
                facts.parameters.push(names);
            }
            if this {
                facts.borrows.insert("this".into());
            }
            facts.header = true;
        };
    let mut functions = BTreeSet::new();
    let mut constants = BTreeSet::new();
    let mut types = BTreeSet::new();
    for node in &parsed.root.children {
        let name = if node.kind == K::Const {
            binding_name(node, &parsed.source)
        } else {
            node.child(K::Name).map(slice).unwrap_or_default()
        };
        if name.is_empty() {
            continue;
        }
        match node.kind {
            K::Function => {
                let signatures = match checker.environment.values.get(&name).map(|s| &s.ty) {
                    Some(Type::Function(f)) => vec![f.clone()],
                    Some(Type::Overloaded(fs)) => fs.clone(),
                    _ => vec![],
                };
                if !module.is_empty() {
                    add(
                        (String::new(), format!("{module}.{name}")),
                        &signatures,
                        true,
                        false,
                    );
                }
                add((String::new(), name.clone()), &signatures, true, false);
                functions.insert(name);
            }
            K::Const => {
                constants.insert(name);
            }
            K::Class | K::Trait | K::Enum | K::Alias => {
                if let Some(id) = checker.environment.names.get(&name).cloned()
                    && let Some(definition) = checker.environment.definitions.get(&id)
                {
                    for (method, signatures) in &definition.methods {
                        add((id.clone(), method.clone()), signatures, true, true);
                    }
                    if !definition.constructors.is_empty() {
                        // Only a stored borrow field lets a new object keep an argument.
                        let stores = definition.fields.values().any(holds_borrow);
                        add(
                            (id.clone(), "new".into()),
                            &definition.constructors,
                            stores,
                            false,
                        );
                    }
                }
                types.insert(name);
            }
            _ => {}
        }
    }
    let mut values = std::mem::take(&mut checker.environment.values);
    values.retain(|name, _| functions.contains(name) || constants.contains(name));
    for symbol in values.values_mut() {
        symbol.ty = checker.inference.resolve(&symbol.ty);
    }
    let mut names = std::mem::take(&mut checker.environment.names);
    names.retain(|name, _| types.contains(name));
    let interface = Environment {
        values,
        names,
        definitions: std::mem::take(&mut checker.environment.definitions),
        calls,
        ..Environment::default()
    };
    (interface, loops)
}

fn holds_borrow(ty: &Type) -> bool {
    match ty {
        Type::Borrow(_) | Type::StoredView(_) | Type::Guard(_) => true,
        Type::Named(_, args) | Type::Tuple(args) | Type::Union(args) => {
            args.iter().any(holds_borrow)
        }
        Type::Lazy(inner, _) | Type::Cursor(inner, _) => holds_borrow(inner),
        _ => false,
    }
}

pub(crate) fn check_with_imports(parsed: &Parsed, module: &str, imports: Environment) -> Checked {
    if !parsed.is_ok() {
        return Checked {
            binding_types: BTreeMap::new(),
            diagnostics: parsed.diagnostics.clone(),
            expression_types: vec![],
            resolved_calls: BTreeMap::new(),
            references: BTreeMap::new(),
            static_references: BTreeMap::new(),
            static_fields: BTreeMap::new(),
            argument_modes: BTreeMap::new(),
            mutated_receivers: BTreeSet::new(),
            mutation_roots: BTreeMap::new(),
            owned_returns: BTreeSet::new(),
            autoboxed: BTreeMap::new(),
            widened: BTreeMap::new(),
            borrowed_views: BTreeMap::new(),
            contract_checks: BTreeMap::new(),
            theorem_evidence: BTreeSet::new(),
            state_proof_dispatches: vec![],
            theorem_requirements: BTreeSet::new(),
            theorem_bounds: BTreeMap::new(),
            python: BTreeMap::new(),
            environment: imports,
        };
    }
    let mut checker = Checker::new(&parsed.source, module, imports);
    let mut theorem_names = parsed
        .root
        .children
        .iter()
        .filter(|n| matches!(n.kind, K::Theorem | K::Axiom | K::Example))
        .filter_map(|n| n.child(K::Name))
        .map(|n| parsed.source.slice(n.span).to_owned())
        .collect::<BTreeSet<_>>();
    theorem_names.extend(checker.environment.theorems.keys().cloned());
    checker.theorem_requirements = parsed
        .root
        .descendants()
        .filter(|n| n.kind == K::Requires)
        .filter(|n| {
            n.children.first().is_some_and(|c| {
                c.kind == K::Call
                    && c.children.first().is_some_and(|head| {
                        matches!(head.kind, K::Name | K::Member)
                            && theorem_names
                                .contains(&crate::lexer::path_name(parsed.source.slice(head.span)))
                    })
            })
        })
        .map(|n| n.span)
        .collect();
    checker.declarations(&parsed.root.children);
    checker.collect_local_theorems(&parsed.root.children);
    // Bodies see the same copy rule as the ownership pass and native lowering: an enum,
    // record or tuple whose parts all copy is copied, not moved.
    let _copy_definitions = crate::types::copy_definitions(&checker.environment.definitions);
    for node in &parsed.root.children {
        if is_declaration(node.kind) {
            checker.check_declarations(std::slice::from_ref(node));
        } else {
            checker.script_level = true;
            checker.infer(node, None);
            checker.script_level = false;
        }
    }
    checker.finish_to_array();
    let expression_types: Vec<(Span, Type)> = std::mem::take(&mut checker.expression_types)
        .into_iter()
        .map(|(span, ty)| (span, checker.inference.resolve(&ty)))
        .collect();
    checker.unused_errors(&parsed.root, &expression_types);
    for (name, symbol) in checker.environment.values.iter_mut() {
        symbol.ty = checker.inference.resolve(&symbol.ty);
        if symbol.visibility != Visibility::Private
            && !symbol.ty.variables().is_subset(&symbol.quantified)
        {
            checker.diagnostics.push(
                Diagnostic::new(
                    "E_EXPORT_INFERENCE",
                    format!("Export `{name}` still has an undetermined type."),
                    parsed.root.span,
                )
                .with_hint("Add an explicit type to this exported declaration."),
            );
            symbol.ty = Type::Invalid;
        }
    }
    // A binding whose value never settles its type, such as `let xs := []` with no
    // later use, cannot be laid out; ask for the annotation by name (REVIEW 1.4).
    if checker
        .diagnostics
        .iter()
        .all(|d| d.severity != crate::diagnostic::Severity::Error)
    {
        // Only names a `let` introduces; a lambda's parameters may stay generic.
        let declared: BTreeSet<Span> = parsed
            .root
            .descendants()
            .filter(|node| node.kind == K::Binding)
            .filter_map(|node| node.children.first())
            .flat_map(|pattern| {
                [
                    Some(pattern.span),
                    pattern.child(K::Name).map(|name| name.span),
                ]
            })
            .flatten()
            .collect();
        // An empty list literal used directly, as in `[].isEmpty()`, has no item type either.
        let expression_lookup: BTreeMap<Span, &Type> = expression_types
            .iter()
            .map(|(span, ty)| (*span, ty))
            .collect();
        for node in parsed.root.descendants() {
            if node.kind == K::List
                && node.children.is_empty()
                && expression_lookup
                    .get(&node.span)
                    .is_some_and(|ty| !ty.variables().is_empty())
            {
                checker.diagnostics.push(
                    Diagnostic::new(
                        "E_TYPE_UNKNOWN",
                        "The item type of this empty list is not known.",
                        node.span,
                    )
                    .with_hint("Give it a type first, such as `let xs : List[Int] = []`."),
                );
            }
        }
        for (span, ty) in &checker.binding_types {
            if !declared.contains(span) {
                continue;
            }
            let ty = checker.inference.resolve(ty);
            if !matches!(ty, Type::Function(_) | Type::Overloaded(_)) && !ty.variables().is_empty()
            {
                let shown =
                    ty.map(&mut |part| matches!(part, Type::Variable(_)).then(|| Type::named("_")));
                checker.diagnostics.push(
                    Diagnostic::new(
                        "E_TYPE_UNKNOWN",
                        format!("The type of this value is not known; it is `{shown}` so far."),
                        *span,
                    )
                    .with_hint("Add a type, such as `let xs : List[Int] = []`."),
                );
            }
        }
    }
    // An opaque declaration exists only in proofs; a run-time call has no body
    // to lower. Report the call site instead of failing at code generation.
    let opaque_spans = parsed
        .root
        .children
        .iter()
        .filter(|node| node.kind == K::Opaque)
        .filter_map(|node| node.children.first())
        .map(|node| node.span)
        .collect::<BTreeSet<_>>();
    if !opaque_spans.is_empty() {
        let mut reported = BTreeSet::new();
        for (span, call) in &checker.resolved_calls {
            let Callable::Declaration { id, kind, .. } = &call.target else {
                continue;
            };
            if id.file == parsed.source.name
                && opaque_spans.contains(&id.span)
                && *kind == DeclarationKind::Function
                && reported.insert(*span)
            {
                checker.diagnostics.push(
                    Diagnostic::new(
                        "E_OPAQUE_BODY",
                        "An opaque function has no body and cannot run.",
                        *span,
                    )
                    .with_hint("Use the opaque declaration in theorem statements only."),
                );
            }
        }
    }
    let mut checked = Checked {
        binding_types: checker
            .binding_types
            .into_iter()
            .map(|(span, ty)| (span, checker.inference.resolve(&ty)))
            .collect(),
        diagnostics: checker.diagnostics,
        expression_types,
        argument_modes: checker.argument_modes,
        mutated_receivers: checker.mutated_receivers,
        mutation_roots: checker.mutation_roots,
        owned_returns: BTreeSet::new(),
        autoboxed: checker
            .autoboxed
            .into_iter()
            .map(|(span, ty)| (span, checker.inference.resolve(&ty)))
            .collect(),
        widened: checker
            .widened
            .into_iter()
            .map(|(span, ty)| (span, checker.inference.resolve(&ty)))
            .collect(),
        borrowed_views: checker
            .borrowed_views
            .into_iter()
            .map(|(span, ty)| (span, checker.inference.resolve(&ty)))
            .collect(),
        contract_checks: BTreeMap::new(),
        theorem_evidence: checker.theorem_evidence,
        state_proof_dispatches: vec![],
        theorem_requirements: checker.theorem_requirements,
        theorem_bounds: checker.theorem_bounds,
        resolved_calls: checker
            .resolved_calls
            .into_iter()
            .map(|(span, mut call)| {
                if let Type::Function(signature) =
                    checker.inference.resolve(&Type::Function(call.signature))
                {
                    call.signature = signature;
                } else {
                    unreachable!()
                }
                for ty in call.type_arguments.values_mut() {
                    *ty = checker.inference.resolve(ty);
                }
                (span, call)
            })
            .collect(),
        references: checker.references,
        static_references: checker.static_references,
        static_fields: checker.static_fields,
        python: checker
            .python
            .into_iter()
            .map(|(span, mut operation)| {
                operation.result = checker.inference.resolve(&operation.result);
                (span, operation)
            })
            .collect(),
        environment: checker.environment,
    };
    // Ownership and contracts run on well-typed files only, so their messages
    // never repeat a type error.
    if checked.diagnostics.is_empty() {
        let (found, exports) = crate::ownership::check(parsed, &checked, module);
        checked.diagnostics.extend(found);
        checked.owned_returns = exports.owned_returns;
        // Check evidence before contract reasoning can use a required theorem.
        let (found, proofs, definitions) = crate::proof::check(parsed, &checked);
        checked.diagnostics.extend(found);
        checked.environment.proofs = definitions;
        let (found, statuses, requires, state, dispatches) =
            crate::contracts::check(parsed, &checked, module, &proofs);
        checked.diagnostics.extend(found);
        checked.contract_checks = statuses;
        checked.state_proof_dispatches = dispatches;
        for (key, parameters) in exports.parameters {
            let facts = checked.environment.calls.entry(key.clone()).or_default();
            facts.parameters = parameters;
            facts.borrows = exports.borrows.get(&key).cloned().unwrap_or_default();
            facts.requires = requires.get(&key).cloned().unwrap_or_default();
            facts.state = state.get(&key).cloned().unwrap_or_default();
            facts.header = false;
            if key.0.is_empty() && !module.is_empty() {
                let facts = facts.clone();
                checked
                    .environment
                    .calls
                    .insert((String::new(), format!("{module}.{}", key.1)), facts);
            }
        }
    }
    tidy_diagnostics(&mut checked.diagnostics, &parsed.source);
    checked
}

/// One mistake reports once (REVIEW2 section 2): exact repeats go, a capture-type note
/// beside the same permission error goes, and an error about a type the checker never
/// worked out goes when an earlier error on its line caused it. A remaining unknown
/// type shows as `_`, never as an internal `?14`.
fn tidy_diagnostics(diagnostics: &mut Vec<Diagnostic>, source: &SourceFile) {
    let mut seen = BTreeSet::new();
    diagnostics.retain(|d| seen.insert((d.code, d.span, d.message.clone())));
    let permission: BTreeSet<Span> = diagnostics
        .iter()
        .filter(|d| d.code == "E_MUT_PARAMETER")
        .map(|d| d.span)
        .collect();
    diagnostics.retain(|d| !(d.code == "E_CAPTURE_TYPE" && permission.contains(&d.span)));
    fn mentions_variable(text: &str) -> bool {
        let bytes = text.as_bytes();
        bytes
            .windows(2)
            .any(|pair| pair[0] == b'?' && pair[1].is_ascii_digit())
    }
    fn hide_variables(text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '?' && chars.peek().is_some_and(char::is_ascii_digit) {
                while chars.peek().is_some_and(char::is_ascii_digit) {
                    chars.next();
                }
                out.push('_');
            } else {
                out.push(c);
            }
        }
        out
    }
    let line = |span: Span| source.position(span.start.min(source.text.len())).0;
    let solid: Vec<(usize, usize)> = diagnostics
        .iter()
        .enumerate()
        .filter(|(_, d)| {
            d.severity == crate::diagnostic::Severity::Error && !mentions_variable(&d.message)
        })
        .map(|(i, d)| (i, line(d.span)))
        .collect();
    let mut index = 0;
    diagnostics.retain(|d| {
        let keep = !mentions_variable(&d.message)
            || !solid
                .iter()
                .any(|(other, other_line)| *other != index && *other_line == line(d.span));
        index += 1;
        keep
    });
    for d in diagnostics.iter_mut() {
        if mentions_variable(&d.message) {
            d.message = hide_variables(&d.message);
        }
    }
}

/// Display is an implicit debug/text call at the session boundary. Keep the same
/// Text requirement as print; bindings can still retain values without Text.
pub(crate) fn session_display(
    parsed: &Parsed,
    checked: &Checked,
    start: usize,
    notebook: bool,
) -> Vec<Diagnostic> {
    let checker = Checker::new(&parsed.source, "", checked.environment.clone());
    let mut errors = vec![];
    for node in parsed
        .root
        .children
        .iter()
        .filter(|n| n.span.start >= start)
    {
        if matches!(
            node.kind,
            K::Import
                | K::Function
                | K::Class
                | K::Trait
                | K::Enum
                | K::Alias
                | K::Const
                | K::Binding
                | K::Assignment
                | K::For
                | K::While
                | K::Break
                | K::Continue
                | K::Return
                | K::Theorem
                | K::Axiom
                | K::Example
        ) {
            continue;
        }
        let Some((_, ty)) = checked
            .expression_types
            .iter()
            .find(|(span, _)| *span == node.span)
        else {
            continue;
        };
        if *ty == Type::named("void")
            || matches!(ty, Type::Function(_) | Type::Overloaded(_) | Type::Never)
        {
            continue;
        }
        let mut viewed = ty;
        while let Type::Named(name, args) = viewed {
            if name == "Box" && args.len() == 1 {
                viewed = &args[0];
            } else {
                break;
            }
        }
        // A notebook shows the success part of `X or Error`, or the Error.
        let shown = match viewed {
            Type::Union(parts) if notebook && parts.iter().any(|p| checker.is_error_type(p)) => {
                parts
                    .iter()
                    .find(|p| !checker.is_error_type(p))
                    .unwrap_or(viewed)
            }
            _ => viewed,
        };
        let shown = if shown.clone() == Type::named("Error") || checker.is_error_type(shown) {
            &Type::named("Text")
        } else {
            shown
        };
        if shown != &Type::named("Text")
            && !checker.implements(shown, &Type::named("Text"), &mut BTreeSet::new())
        {
            errors.push(
                Diagnostic::new(
                    "E_SESSION_DISPLAY",
                    format!("A {ty} does not provide text for session display."),
                    node.span,
                )
                .with_hint(
                    "Bind it with let, inspect a field, or implement Text with text(): String.",
                ),
            );
        }
    }
    errors
}

struct Checker<'a> {
    binding_types: BTreeMap<Span, Type>,
    /// Array-returning toArray calls. Their element types are checked once the whole file is known.
    to_array_results: Vec<(Span, Type)>,
    source: &'a SourceFile,
    module: &'a str,
    environment: Environment,
    inference: Inference,
    scopes: Vec<BTreeMap<String, Symbol>>,
    diagnostics: Vec<Diagnostic>,
    expression_types: Vec<(Span, Type)>,
    resolved_calls: BTreeMap<Span, ResolvedCall>,
    references: BTreeMap<Span, Reference>,
    argument_modes: BTreeMap<Span, Passing>,
    mutated_receivers: BTreeSet<Span>,
    mutation_roots: BTreeMap<Span, BTreeSet<String>>,
    place_aliases: BTreeMap<(usize, String), BTreeSet<String>>,
    autoboxed: BTreeMap<Span, Type>,
    widened: BTreeMap<Span, Type>,
    borrowed_views: BTreeMap<Span, Type>,
    theorem_requirements: BTreeSet<Span>,
    theorem_bounds: BTreeMap<Span, BTreeMap<String, Vec<Type>>>,
    local_theorems: BTreeMap<String, TheoremSignature>,
    theorem_evidence: BTreeSet<Span>,
    owner: Option<String>,
    /// What `Self` names here: the class being declared, or the marker in a
    /// trait. None outside both, where `Self` is an error.
    self_type: Option<Type>,
    return_type: Option<Type>,
    /// True while checking a script's top-level statements, where `?` ends the program.
    script_level: bool,
    bounds: BTreeMap<String, Vec<Type>>,
    loop_depth: usize,
    /// Blocks whose exit can fail: the loop depth at entry, and the jumps that leave them.
    exit_barriers: Vec<(usize, Vec<Span>)>,
    /// The `m.lock` callee of the enclosing `using` header, the one place a lock may be taken.
    lock_call: Option<Span>,
    /// Number of `scope` blocks around this point in the current function.
    scope_depth: usize,
    /// Whether a task in the innermost scope can fail with Error.
    scope_fails: bool,
    rigid_parameters: BTreeSet<String>,
    associated_names: BTreeSet<String>,
    rigid_indices: BTreeSet<String>,
    effects_allowed: bool,
    effects_observed: bool,
    receiver_mutable: bool,
    /// For each lambda being checked: the scope depth outside it, and whether its body
    /// changes something it captured (Q40).
    lambda_mutations: Vec<(usize, bool)>,
    /// How many `unsafe { }` blocks enclose the expression being checked.
    unsafe_depth: usize,
    python: BTreeMap<Span, PythonOperation>,
    /// A member expression that names a Python attribute about to be called.
    python_pending: BTreeMap<Span, (Option<String>, Option<Span>, String)>,
    /// A member that names an `extend` function: its full type and the receiver's type.
    extension_members: BTreeMap<Span, (Type, Type)>,
    /// The expected type of the member expression being checked.
    member_expected: Option<Type>,
    /// The receiver span currently allowed to name a Python module.
    member_receiver: Option<Span>,
    /// False while a `let` pattern binds: a name there keeps the whole union, Error included.
    narrow_unions: bool,
    direct_callee: Option<Span>,
    initializing_fields: Option<BTreeSet<String>>,
    super_start: Option<Span>,
    /// Constructor field declarations that sit directly in a `new` block and
    /// were registered as class fields. A nested declaration is an error.
    constructor_field_spans: BTreeSet<Span>,
    assignment_target_span: Option<Span>,
    script_bindings: BTreeSet<String>,
    named_function_depth: usize,
    borrowed_parameters: BTreeMap<String, bool>,
    /// True only while the direct operand of a unary minus is inferred, so
    /// `-9223372036854775808` can spell INT64_MIN.
    allow_min_literal: bool,
    /// True while a `const` initializer is inferred: it may read only stable
    /// values, because the initializer is evaluated at each use.
    constant_initializer: bool,
    static_references: BTreeMap<Span, (String, String)>,
    static_fields: BTreeMap<(String, String), StaticFieldInfo>,
}

impl<'a> Checker<'a> {
    fn new(source: &'a SourceFile, module: &'a str, mut environment: Environment) -> Self {
        for name in [
            "Int",
            "double",
            "Bool",
            "String",
            "Bytes",
            "void",
            "BigInt",
            "Object",
            "Error",
            "Text",
            "Equals",
            "Copy",
            "Comparable",
            "Sortable",
            "Iterable",
            "Iterator",
            "Multipliable",
            "RightMultipliable",
            "Addable",
            "RightAddable",
            "Subtractable",
            "RightSubtractable",
            "Divisible",
            "RightDivisible",
            "Powerable",
            "RightPowerable",
            "Negatable",
            "List",
            "Array",
            "Tensor",
            "DynTensor",
            "Variable",
            "Map",
            "MutMap",
            "Set",
            "MutSet",
            "SortedSet",
            "Option",
            "Box",
            "Mutex",
            "Atomic",
            "Channel",
            "Reader",
            "Writer",
        ] {
            environment
                .names
                .entry(name.into())
                .or_insert_with(|| name.into());
        }
        let mut checker = Self {
            binding_types: BTreeMap::new(),
            to_array_results: Vec::new(),
            source,
            module,
            environment,
            inference: Inference::default(),
            scopes: vec![],
            diagnostics: vec![],
            expression_types: vec![],
            resolved_calls: BTreeMap::new(),
            references: BTreeMap::new(),
            argument_modes: BTreeMap::new(),
            mutated_receivers: BTreeSet::new(),
            mutation_roots: BTreeMap::new(),
            place_aliases: BTreeMap::new(),
            autoboxed: BTreeMap::new(),
            widened: BTreeMap::new(),
            borrowed_views: BTreeMap::new(),
            theorem_requirements: BTreeSet::new(),
            theorem_bounds: BTreeMap::new(),
            local_theorems: BTreeMap::new(),
            theorem_evidence: BTreeSet::new(),
            owner: None,
            self_type: None,
            return_type: None,
            script_level: false,
            bounds: BTreeMap::new(),
            loop_depth: 0,
            exit_barriers: vec![],
            lock_call: None,
            scope_depth: 0,
            scope_fails: false,
            rigid_parameters: BTreeSet::new(),
            associated_names: BTreeSet::new(),
            rigid_indices: BTreeSet::new(),
            effects_allowed: true,
            effects_observed: false,
            receiver_mutable: false,
            lambda_mutations: vec![],
            unsafe_depth: 0,
            python: BTreeMap::new(),
            python_pending: BTreeMap::new(),
            extension_members: BTreeMap::new(),
            member_expected: None,
            member_receiver: None,
            narrow_unions: true,
            direct_callee: None,
            initializing_fields: None,
            super_start: None,
            constructor_field_spans: BTreeSet::new(),
            assignment_target_span: None,
            script_bindings: BTreeSet::new(),
            named_function_depth: 0,
            borrowed_parameters: BTreeMap::new(),
            allow_min_literal: false,
            constant_initializer: false,
            static_references: BTreeMap::new(),
            static_fields: BTreeMap::new(),
        };
        checker.builtins();
        checker
    }
    fn text(&self, node: &Node) -> &str {
        self.source.slice(node.span)
    }
    fn declaration(
        &self,
        node: &Node,
        name: String,
        owner: Option<String>,
        kind: DeclarationKind,
    ) -> Callable {
        Callable::Declaration {
            id: DeclarationId {
                file: self.source.name.clone(),
                span: node.span,
            },
            name,
            owner,
            kind,
        }
    }

    /// `(a, b) = value` stores each item of a tuple value in its own place, so every place must be
    /// assignable and copyable, and each item type must match its place (Q130).
    fn destructuring_assignment(&mut self, node: &Node) -> Type {
        let targets = node.children[0].children.clone();
        let value = self.infer(&node.children[1], None);
        let Type::Tuple(parts) = self.inference.resolve(&value) else {
            self.error(
                node,
                "E_DESTRUCTURE_VALUE",
                format!(
                    "A tuple target needs a tuple value, found {}.",
                    self.inference.resolve(&value)
                ),
                "Write the right side as a tuple of the same length, such as (b, a).",
            );
            return Type::named("void");
        };
        if parts.len() != targets.len() {
            self.error(
                node,
                "E_DESTRUCTURE_ARITY",
                format!(
                    "The target has {} places, but the value has {} items.",
                    targets.len(),
                    parts.len()
                ),
                "Give one place for each item of the tuple value.",
            );
            return Type::named("void");
        }
        for (target, part) in targets.iter().zip(parts) {
            match target.kind {
                K::Name => {
                    let name = self.text(target).to_owned();
                    let mutable = self
                        .scopes
                        .iter()
                        .rev()
                        .find_map(|scope| scope.get(&name))
                        .or_else(|| self.environment.values.get(&name))
                        .is_none_or(|symbol| symbol.mutable);
                    if !mutable {
                        self.error(
                            target,
                            "E_IMMUTABLE_ASSIGNMENT",
                            format!("`{name}` was not declared with let mut."),
                            "Use let mut for a variable that you need to assign again.",
                        );
                    }
                    self.note_mutation(&name);
                }
                K::Member | K::Index => self.require_mutable_receiver(node, &target.children[0]),
                _ => {
                    self.error(
                        target,
                        "E_ASSIGN_TARGET",
                        "A tuple target can only hold names, fields and array slots.",
                        "Assign each item to a name, a field or an array slot.",
                    );
                    continue;
                }
            }
            let place = self.infer(target, None);
            if target.kind == K::Index {
                self.require_mutable_index(node, target);
            }
            let place = self.inference.resolve(&place);
            if !crate::types::copies(&place) {
                self.error(
                    target,
                    "E_DESTRUCTURE_COPY",
                    format!("{place} cannot be stored through a tuple target, because only copyable items can be taken out of a tuple."),
                    "Assign the item on its own, with a separate statement.",
                );
            }
            self.require(target, &place, &part);
        }
        Type::named("void")
    }

    fn bind_receiver(&self, ty: Type, receiver: &Type) -> Type {
        match ty {
            Type::Function(mut function) => {
                function.receiver = Some(Box::new(receiver.clone()));
                Type::Function(function)
            }
            Type::Overloaded(functions) => Type::Overloaded(
                functions
                    .into_iter()
                    .map(|mut function| {
                        function.receiver = Some(Box::new(receiver.clone()));
                        function
                    })
                    .collect(),
            ),
            other => other,
        }
    }

    fn qualified(&self, name: &str) -> String {
        if self.module.is_empty() {
            name.into()
        } else {
            format!("{}.{name}", self.module)
        }
    }
    fn error(&mut self, node: &Node, code: &'static str, message: impl Into<String>, hint: &str) {
        self.diagnostics
            .push(Diagnostic::new(code, message, node.span).with_hint(hint));
    }

    /// An error that the editor can repair with generated boilerplate.
    fn error_with_fixes(
        &mut self,
        node: &Node,
        code: &'static str,
        message: impl Into<String>,
        hint: &str,
        fixes: Vec<crate::actions::Fix>,
    ) {
        self.diagnostics.push(
            Diagnostic::new(code, message, node.span)
                .with_hint(hint)
                .with_fixes(fixes),
        );
    }
    /// An integer literal must fit its inferred numeric type. Native lowering
    /// also checked this, so `check` used to accept a program that could not
    /// build. BigInt has no limit; Nat tops out at 64 unsigned bits.
    fn check_integer_literal(&mut self, node: &Node, ty: &Type) {
        if *ty != Type::named("Int") && *ty != Type::named("Nat") && *ty != Type::named("double") {
            return;
        }
        let text = self.text(node).to_owned();
        if text.starts_with('"')
            || matches!(text.as_str(), "true" | "false")
            || crate::lexer::is_double_literal(&text)
        {
            return;
        }
        if *ty == Type::named("double") {
            self.check_exact_double_literal(node, &text);
            return;
        }
        let (negative, unsigned) = match text.as_bytes().first() {
            Some(b'-') => (true, &text[1..]),
            Some(b'+') => (false, &text[1..]),
            _ => (false, text.as_str()),
        };
        let mut value: u64 = 0;
        let digits = unsigned
            .strip_prefix("0x")
            .or_else(|| unsigned.strip_prefix("0X"))
            .unwrap_or(unsigned);
        let radix: u64 = if digits.len() == unsigned.len() {
            10
        } else {
            16
        };
        let mut too_large = false;
        for character in digits.chars() {
            if character == '_' {
                continue;
            }
            let Some(digit) = character.to_digit(radix as u32) else {
                return;
            };
            let Some(next) = value
                .checked_mul(radix)
                .and_then(|value| value.checked_add(u64::from(digit)))
            else {
                too_large = true;
                break;
            };
            value = next;
        }
        let limit = if *ty == Type::named("Int") {
            if negative || self.allow_min_literal {
                1u64 << 63
            } else {
                i64::MAX as u64
            }
        } else {
            u64::MAX
        };
        if (negative && *ty == Type::named("Nat")) || too_large || value > limit {
            let (kind, hint) = if *ty == Type::named("Int") {
                (
                    "a 64-bit Int",
                    "Use an explicit BigInt annotation for larger values.",
                )
            } else {
                ("Nat", "Use BigInt for values beyond 64 unsigned bits.")
            };
            self.error(
                node,
                "E_INTEGER_LITERAL_RANGE",
                format!("This literal does not fit in {kind}."),
                hint,
            );
        }
    }

    fn check_exact_double_literal(&mut self, node: &Node, text: &str) {
        let unsigned = text.strip_prefix(['+', '-']).unwrap_or(text);
        let digits = unsigned
            .strip_prefix("0x")
            .or_else(|| unsigned.strip_prefix("0X"))
            .unwrap_or(unsigned);
        let radix = if digits.len() == unsigned.len() {
            10
        } else {
            16
        };
        let mut value: u128 = 0;
        for character in digits.chars() {
            if character == '_' {
                continue;
            }
            let Some(digit) = character.to_digit(radix) else {
                return;
            };
            let Some(next) = value
                .checked_mul(u128::from(radix))
                .and_then(|value| value.checked_add(u128::from(digit)))
            else {
                self.error(
                    node,
                    "E_DOUBLE_LITERAL_EXACT",
                    "This integer literal is too large to represent exactly as double.",
                    "Use a double literal, or keep the value as an Int.",
                );
                return;
            };
            value = next;
        }
        let converted = value as f64;
        // 2^128 is the first double at or above the u128 range; rejecting it avoids
        // a saturating cast making u128::MAX look exact.
        const LIMIT: f64 = 340282366920938463463374607431768211456.0;
        if !converted.is_finite() || converted >= LIMIT || converted as u128 != value {
            self.error(
                node,
                "E_DOUBLE_LITERAL_EXACT",
                "This integer literal cannot be represented exactly as double.",
                "Use a nearby double literal, or keep the value as an Int.",
            );
        }
    }

    fn require_effect(&mut self, node: &Node, operation: &str) {
        self.effects_observed = true;
        if !self.effects_allowed {
            self.error(
                node,
                "E_EFFECT_REQUIRED",
                format!("{operation} requires with Effects in this function's type."),
                "Mark the function with Effects when it performs outside work.",
            );
        }
    }
    fn place_access(&self, node: &Node) -> Option<bool> {
        let root = place_root(node, self.source)?;
        if root == "this" {
            return Some(self.receiver_mutable);
        }
        if let Some(mutable) = self.borrowed_parameters.get(root) {
            return Some(*mutable);
        }
        let symbol = self
            .scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(root))
            .or_else(|| self.environment.values.get(root))?;
        if matches!(
            self.inference.resolve(&symbol.ty),
            Type::Borrow(_) | Type::StoredView(_)
        ) {
            Some(false)
        } else {
            symbol.access
        }
    }

    fn resolved_place(&self, node: &Node) -> Option<Vec<String>> {
        let path = place_path(node, self.source)?;
        let symbol = self
            .scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(&path[0]))
            .or_else(|| self.environment.values.get(&path[0]));
        // A name with no variable behind it, such as the case `None`, makes a
        // fresh value each time, so two of them never overlap.
        if symbol.is_none() && path[0] != "this"
            || symbol.is_some_and(|symbol| {
                matches!(
                    symbol.reference,
                    Some(Reference::Builtin(_) | Reference::Constant(_))
                )
            })
        {
            return None;
        }
        if let Some(origin) = symbol.and_then(|symbol| symbol.origin.as_ref()) {
            Some(
                origin
                    .iter()
                    .cloned()
                    .chain(path.into_iter().skip(1))
                    .collect(),
            )
        } else {
            Some(path)
        }
    }

    fn record_binding_access(&mut self, pattern: &Node, value: &Node) {
        let roots = self.place_roots(value);
        let known = self
            .expression_types
            .iter()
            .rev()
            .find(|(span, _)| *span == value.span)
            .map(|(_, ty)| self.inference.resolve(ty));
        let access = self.place_access(value);
        let origin = self.resolved_place(value).or_else(|| {
            (value.kind == K::Call
                && value.children.iter().skip(1).any(|argument| {
                    argument
                        .children
                        .last()
                        .is_some_and(|node| node.kind == K::Hole)
                }))
            .then(|| self.resolved_place(&value.children[0]))
            .flatten()
        });
        for name in pattern
            .descendants()
            .filter(|node| node.kind == K::PatternName)
            .map(|node| self.named_child(node))
            .collect::<Vec<_>>()
        {
            if let Some(depth) = self
                .scopes
                .iter()
                .rposition(|scope| scope.contains_key(&name))
            {
                self.place_aliases
                    .insert((depth, name.clone()), roots.clone());
            }
            if let Some(symbol) = self
                .scopes
                .iter_mut()
                .rev()
                .find_map(|scope| scope.get_mut(&name))
                .or_else(|| self.environment.values.get_mut(&name))
                && !crate::types::scalar(&self.inference.resolve(&symbol.ty))
            {
                if !symbol.mutable && matches!(pattern.kind, K::PatternName | K::Name) {
                    match (&mut symbol.ty, &known) {
                        (Type::Function(function), Some(Type::Function(source))) => {
                            function.callable = source.callable.clone();
                            function.receiver = source.receiver.clone();
                        }
                        (Type::Overloaded(functions), Some(Type::Overloaded(source))) => {
                            for (function, source) in functions.iter_mut().zip(source) {
                                function.callable = source.callable.clone();
                                function.receiver = source.receiver.clone();
                            }
                        }
                        _ => {}
                    }
                }
                symbol.access = access;
                symbol.origin = origin.clone();
            }
        }
    }

    /// `value.name` on a PythonValue or Python module reads an attribute; as a callee it
    /// names the method the call runs (Q49).
    fn python_member(&mut self, node: &Node, name: &str) -> Type {
        let receiver = ungroup(&node.children[0]);
        let module = (receiver.kind == K::Name)
            .then(|| self.environment.values.get(self.text(receiver)))
            .flatten()
            .and_then(|symbol| match &symbol.reference {
                Some(Reference::Builtin(target)) => {
                    target.strip_prefix("python:").map(str::to_owned)
                }
                _ => None,
            });
        let receiver_span = module.is_none().then_some(receiver.span);
        if self.direct_callee == Some(node.span) {
            self.python_pending
                .insert(node.span, (module, receiver_span, name.to_owned()));
            return Type::named("PythonCallable");
        }
        self.require_effect(node, "Reading a Python attribute");
        let expected = self.member_expected.clone();
        let result = self.python_result(node, expected.as_ref());
        self.python.insert(
            node.span,
            PythonOperation {
                module,
                receiver: receiver_span,
                attribute: Some(name.to_owned()),
                call: false,
                arguments: vec![],
                result: result.clone(),
            },
        );
        result
    }

    fn python_call(
        &mut self,
        node: &Node,
        module: Option<String>,
        receiver: Option<Span>,
        attribute: Option<String>,
        expected: Option<&Type>,
    ) -> Type {
        let mut arguments = vec![];
        let mut keyword = false;
        for argument in &node.children[1..] {
            let value = argument.children.last().unwrap();
            let name = (argument.kind == K::NamedArgument)
                .then(|| self.text(&argument.children[0]).to_owned());
            if value.kind == K::Hole || argument.kind == K::ArrowArgument {
                self.error(
                    argument,
                    "E_PYTHON_ARGUMENT",
                    "A Python call takes values, not open slots or arrows.",
                    "Pass each argument as a value; wrap the call in a lambda to delay it.",
                );
                continue;
            }
            if name.is_none() && keyword {
                self.error(
                    argument,
                    "E_ARGUMENT_ORDER",
                    "Positional arguments come before named ones.",
                    "Move this argument before the keyword arguments.",
                );
            }
            keyword |= name.is_some();
            let ty = self.infer(value, None);
            let ty = self.inference.resolve(&ty);
            if !self.python_input(&ty) {
                self.error(value, "E_PYTHON_TYPE", format!("A {ty} cannot be passed to Python."), "Pass numbers, Bool, String, collections and tuples of those, a PythonValue, or a function with annotated parameters of those types.");
            }
            arguments.push((name, value.span));
        }
        self.require_effect(node, "Calling Python");
        let result = self.python_result(node, expected);
        self.python.insert(
            node.span,
            PythonOperation {
                module,
                receiver,
                attribute,
                call: true,
                arguments,
                result: result.clone(),
            },
        );
        result
    }

    /// A written `T or Error` is checked against the Python value when the call runs;
    /// otherwise the result is an opaque PythonValue (Q46).
    fn python_result(&mut self, node: &Node, expected: Option<&Type>) -> Type {
        let fallback = Type::union(Type::named("PythonValue"), Type::named("Error"));
        let Some(expected) = expected.map(|ty| self.inference.resolve(ty)) else {
            return fallback;
        };
        if let Type::Union(parts) = &expected
            && parts.len() == 2
            && let Some(error) = parts.iter().position(|p| *p == Type::named("Error"))
        {
            let success = parts[1 - error].clone();
            if self.python_output(&success) {
                return expected;
            }
            self.error(node, "E_PYTHON_TYPE", format!("A Python result cannot become a {success}."), "Expect numbers, Bool, String, void, collections or tuples of those, or a PythonValue.");
            return Type::Invalid;
        }
        if matches!(expected, Type::Variable(_)) {
            return fallback;
        }
        if self.python_output(&expected) {
            self.error(
                node,
                "E_PYTHON_RESULT",
                "A Python call can fail, so its expected type must include Error.",
                "Write the type as T or Error, then handle the Error case.",
            );
            return Type::Invalid;
        }
        fallback
    }

    fn python_input(&self, ty: &Type) -> bool {
        match ty {
            Type::Borrow(inner) | Type::StoredView(inner) => self.python_input(inner),
            // A callback: Python's arguments are checked on each call, and the result is
            // converted back (Q54). A closure that changes what it captured is refused.
            Type::Function(f) => {
                let result = match f.result.as_ref() {
                    Type::Union(parts) if parts.iter().any(|p| self.is_error_type(p)) => {
                        let values = parts
                            .iter()
                            .filter(|p| !self.is_error_type(p))
                            .collect::<Vec<_>>();
                        values.len() == 1 && self.python_callback_result(values[0])
                    }
                    other => self.python_callback_result(other),
                };
                !f.receiver_mutates
                    && result
                    && f.parameters
                        .iter()
                        .all(|p| p.passing != Passing::Mut && self.python_output(&p.ty))
            }
            Type::Tuple(parts) => parts.iter().all(|p| self.python_input(p)),
            Type::Named(name, args) => match name.as_str() {
                "Int" | "double" | "Bool" | "String" | "PythonValue" => args.is_empty(),
                "List" | "Set" | "SortedSet" | "Map" => args.iter().all(|p| self.python_input(p)),
                _ => false,
            },
            _ => false,
        }
    }

    fn python_callback_result(&self, ty: &Type) -> bool {
        *ty == Type::named("void") || !matches!(ty, Type::Function(_)) && self.python_input(ty)
    }

    fn python_output(&self, ty: &Type) -> bool {
        match ty {
            Type::Tuple(parts) => parts.iter().all(|p| self.python_output(p)),
            Type::Named(name, args) => match name.as_str() {
                "Int" | "double" | "Bool" | "String" | "PythonValue" | "void" => args.is_empty(),
                "List" | "Set" => args.len() == 1 && self.python_output(&args[0]),
                "Map" => {
                    args.len() == 2
                        && matches!(&args[0], Type::Named(k, a) if a.is_empty() && matches!(k.as_str(), "Int" | "String" | "Bool"))
                        && self.python_output(&args[1])
                }
                _ => false,
            },
            _ => false,
        }
    }

    /// A change to a name bound outside a lambda makes that lambda a mutating closure.
    fn note_mutation(&mut self, root: &str) {
        let bound = self
            .scopes
            .iter()
            .rposition(|scope| scope.contains_key(root));
        for (depth, mutated) in &mut self.lambda_mutations {
            if bound.is_none_or(|index| index < *depth) {
                *mutated = true;
            }
        }
    }

    fn place_roots(&self, node: &Node) -> BTreeSet<String> {
        let node = ungroup(node);
        if let Some(root) = place_root(node, self.source) {
            if let Some(depth) = self
                .scopes
                .iter()
                .rposition(|scope| scope.contains_key(root))
                && let Some(roots) = self.place_aliases.get(&(depth, root.to_owned()))
                && !roots.is_empty()
            {
                return roots.clone();
            }
            return self
                .resolved_place(node)
                .and_then(|path| path.first().cloned())
                .into_iter()
                .collect();
        }
        if matches!(node.kind, K::If | K::Match | K::MatchArm | K::Block) {
            return node
                .children
                .iter()
                .flat_map(|child| self.place_roots(child))
                .collect();
        }
        BTreeSet::new()
    }

    fn require_mutable_receiver(&mut self, node: &Node, receiver: &Node) {
        let roots = self.place_roots(receiver);
        for root in &roots {
            self.note_mutation(root);
        }
        self.mutation_roots
            .entry(receiver.span)
            .or_default()
            .extend(roots);
        if self.place_access(receiver) != Some(false) {
            return;
        }
        let root = place_root(receiver, self.source).unwrap_or("");
        if root == "this" {
            self.error(node, "E_MUT_RECEIVER", "Changing this requires a mut method declaration.", "Write public mut function ... for a method that changes this; add with Effects only for outside work.");
        } else if self.borrowed_parameters.get(root) == Some(&false) {
            self.error(
                node,
                "E_MUT_PARAMETER",
                format!("Mutating through borrowed parameter `{root}` requires mut."),
                "Declare mut name : Type, or move ownership into the function.",
            );
        } else {
            self.error(
                node,
                "E_MUT_BORROW",
                "This value is a read-only borrowed view.",
                "Use an owned value or an explicit mut parameter to change an object.",
            );
        }
    }

    /// A bracket write on a class also needs MutableIndexable, beside the read trait.
    fn require_mutable_index(&mut self, node: &Node, place: &Node) {
        let Some(receiver) = self
            .expression_types
            .iter()
            .rev()
            .find(|(span, _)| *span == place.children[0].span)
            .map(|(_, ty)| match self.inference.resolve(ty) {
                Type::Borrow(inner) | Type::StoredView(inner) => *inner,
                other => other,
            })
        else {
            return;
        };
        let Type::Named(name, _) = &receiver else {
            return;
        };
        if !self
            .environment
            .definitions
            .get(name)
            .is_some_and(|definition| definition.kind == K::Class)
        {
            return;
        }
        let key = self.inference.fresh();
        let item = self.inference.fresh();
        let readable = self
            .environment
            .names
            .get("Indexable")
            .cloned()
            .is_some_and(|name| {
                self.trait_instance(
                    &receiver,
                    &name,
                    &[key.clone(), item.clone()],
                    &mut BTreeSet::new(),
                )
                .is_some()
            });
        let mutable = self
            .environment
            .names
            .get("MutableIndexable")
            .cloned()
            .is_some_and(|name| {
                self.trait_instance(&receiver, &name, &[key, item], &mut BTreeSet::new())
                    .is_some()
            });
        if readable && !mutable {
            self.error(
                node,
                "E_INDEX_WRITE",
                format!("{receiver} does not implement MutableIndexable, so a bracket cannot be assigned."),
                "Add `has MutableIndexable[Key, Item]` and write `public mut function setAt(key : Key, value : Item) : void`, or call a method.",
            );
        }
    }

    fn check_call_access(
        &mut self,
        node: &Node,
        function: &FunctionType,
        order: &[Option<usize>],
        arguments: &[(Option<String>, Type, &Node, bool)],
    ) {
        for (parameter, argument) in function.parameters.iter().zip(order) {
            let Some(index) = argument else { continue };
            let (_, ty, expression, hole) = &arguments[*index];
            if !*hole {
                self.record_borrowed_view(expression, &parameter.ty, ty);
            }
            if !*hole
                && matches!(function.callable.as_ref(), Some(Callable::Builtin(name)) if matches!(name.as_str(), "std.core.Arena.insert" | "std.core.Arena.replace"))
                && parameter.passing == Passing::Move
                && !crate::types::same_capture_permissions(
                    &self.inference.resolve(&parameter.ty),
                    &self.inference.resolve(ty),
                )
            {
                self.error(expression, "E_ARENA_CAPTURE", "This arena's item type would erase the closure's permission to change captured state.", "Use Arena() to infer the item type from the first inserted closure.");
            }
            if !*hole
                && parameter.passing == Passing::Borrow
                && matches!(self.inference.resolve(ty), Type::Function(f) if f.receiver_mutates)
            {
                self.error(expression, "E_MUT_PARAMETER", "This function changes what it captured, so it can only be passed to a mut or move parameter.", "Declare the parameter as mut f : ..., then call it as usual.");
            }
            if !*hole
                && !capture_permissions_fit(
                    &self.inference.resolve(&parameter.ty),
                    &self.inference.resolve(ty),
                )
            {
                self.error(expression, "E_CAPTURE_TYPE", "This parameter type would erase a function's permission to change captured state.", "Use mut (...) for a mutable callback type, and mut or move before its parameter name.");
            }
        }
        let mut accesses = vec![];
        let callable = match node.kind {
            K::Call => node.children.first(),
            K::Pipe => node.children.get(2),
            _ => None,
        };
        if let Some(callable) = callable.map(ungroup)
            && callable.kind == K::Member
        {
            if function.receiver_mutates {
                self.require_mutable_receiver(node, &callable.children[0]);
                self.mutated_receivers.insert(callable.children[0].span);
            }
            if let Some(place) = self.resolved_place(&callable.children[0]) {
                accesses.push((place, function.receiver_mutates));
            }
        } else if function.receiver_mutates
            && let Some(callable) = callable
        {
            self.require_mutable_receiver(node, callable);
            self.mutated_receivers.insert(callable.span);
            if let Some(mut place) = self.resolved_place(callable)
                && place.len() > 1
            {
                place.pop();
                accesses.push((place, true));
            }
        }
        for (parameter, argument) in function.parameters.iter().zip(order) {
            let Some(index) = argument else {
                continue;
            };
            let (_, ty, expression, hole) = &arguments[*index];
            if *hole {
                continue;
            }
            self.argument_modes
                .insert(expression.span, parameter.passing);
            if parameter.passing == Passing::Mut {
                self.require_mutable_receiver(node, expression);
            }
            if (parameter.passing == Passing::Mut
                || !crate::types::scalar(&self.inference.resolve(ty)))
                && let Some(place) = self.resolved_place(expression)
            {
                // Handing over a copyable value hands over a copy, which needs no exclusivity.
                let exclusive = parameter.passing == Passing::Mut
                    || parameter.passing == Passing::Move
                        && !crate::types::copies(&self.inference.resolve(ty));
                accesses.push((place, exclusive));
            }
        }
        for (index, (place, exclusive)) in accesses.iter().enumerate() {
            if let Some((other, _)) = accesses[..index].iter().find(|(other, other_exclusive)| {
                (*exclusive || *other_exclusive)
                    && (place.starts_with(other) || other.starts_with(place))
            }) {
                self.error(node, "E_BORROW_CONFLICT", format!("This call overlaps access to `{}` and `{}`; one requires exclusive access.", render_place(other), render_place(place)), "Pass distinct objects or disjoint fields when a receiver or argument is mut or move.");
            }
        }
    }

    fn mismatch(&mut self, node: &Node, expected: &Type, found: &Type) {
        let expected = self.inference.resolve(expected);
        let found = self.inference.resolve(found);
        if !capture_permissions_fit(&expected, &found) {
            self.error(node, "E_CAPTURE_TYPE", "This type would erase a function's permission to change captured state.", "Keep the inferred type or use a mut function parameter. Concrete stored or returned mutable callback types are not implemented yet.");
            return;
        }
        // An owned place keeps its exact class (Q85), so say how to keep the subclass.
        let hint = if self.superclass_steps(&found, &expected).is_some() {
            format!(
                "A place declared as {expected} holds exactly a {expected}. Leave the type off to keep the {found}, then pass it wherever a {expected} is borrowed."
            )
        } else {
            "Use a value of the expected type; number conversions must be explicit.".to_owned()
        };
        self.error(
            node,
            "E_TYPE",
            format!(
                "Expected {expected}, found {found} in `{}`.",
                self.text(node)
            ),
            &hint,
        );
    }

    /// How many `extends` steps lead from class `found` up to class `expected`,
    /// when `expected` is a proper superclass of `found` (Q85).
    fn superclass_steps(&self, found: &Type, expected: &Type) -> Option<usize> {
        let Type::Named(target, _) = expected else {
            return None;
        };
        let mut current = found.clone();
        for steps in 0..64 {
            let Type::Named(id, args) = &current else {
                return None;
            };
            if steps > 0 && id == target {
                return self
                    .inference
                    .clone()
                    .unify(&current, expected)
                    .then_some(steps);
            }
            let definition = self.environment.definitions.get(id)?;
            if definition.kind != K::Class {
                return None;
            }
            let replacements = definition
                .parameters
                .iter()
                .cloned()
                .zip(args.iter().cloned())
                .collect();
            current = definition
                .parent
                .as_ref()?
                .replace_parameters(&replacements);
        }
        None
    }
    /// An Int converts to BigInt exactly wherever a BigInt is expected, so the conversion needs no spelling.
    /// The reverse stays explicit (Q125).
    fn widens(&self, expected: &Type, found: &Type) -> bool {
        let found = self.inference.resolve(found);
        self.inference.resolve(expected) == Type::named("BigInt")
            && (found == Type::named("Int") || found == Type::named("Nat"))
    }
    /// The operands of a binary operator: an Int operand beside a BigInt one widens to BigInt, and the
    /// widened operand is recorded for core to convert (Q125).
    fn widen_operands(&mut self, node: &Node, left: Type, right: Type) -> (Type, Type) {
        if self.widens(&left, &right) {
            self.widened
                .insert(node.children[2].span, Type::named("BigInt"));
            (left, Type::named("BigInt"))
        } else if self.widens(&right, &left) {
            self.widened
                .insert(node.children[0].span, Type::named("BigInt"));
            (Type::named("BigInt"), right)
        } else {
            (left, right)
        }
    }
    /// The value part of an expected `T or Error`: when the only non-error branch
    /// is T, a literal or collection literal is built as T and the surrounding
    /// check still accepts an error value (`T or Error`). A union with several
    /// success branches leaves the hint unchanged, because no branch can be
    /// chosen before the value is known.
    fn value_expected(&self, expected: Option<&Type>) -> Option<Type> {
        let resolved = self.inference.resolve(expected?);
        let Type::Union(parts) = resolved else {
            return Some(resolved);
        };
        let values = parts
            .iter()
            .filter(|part| !self.is_error_type(part))
            .collect::<Vec<_>>();
        (values.len() == 1).then(|| values[0].clone())
    }

    fn require(&mut self, node: &Node, expected: &Type, found: &Type) {
        if self.widens(expected, found) {
            self.widened.insert(node.span, Type::named("BigInt"));
            return;
        }
        let mut obligations = vec![];
        let saved = self.inference.clone();
        if self.constrain_argument(expected, found, &mut obligations) {
            self.record_borrowed_view(node, expected, found);
            self.check_obligations(node, &obligations);
            return;
        }
        self.inference = saved;
        obligations.clear();
        if self.box_content(expected, found, &mut obligations) {
            self.autoboxed.insert(node.span, expected.clone());
            self.check_obligations(node, &obligations);
        } else {
            self.mismatch(node, expected, found);
        }
    }
    fn record_borrowed_view(&mut self, node: &Node, expected: &Type, found: &Type) {
        let expected = self.inference.resolve(expected);
        if let Type::Union(parts) = &expected
            && !matches!(self.inference.resolve(found), Type::Union(_))
            && !self.is_error_type(found)
        {
            for part in parts {
                if matches!(part, Type::Borrow(inner) if matches!(inner.as_ref(), Type::Dynamic(_)))
                    && self.constrain_argument(part, found, &mut vec![])
                {
                    self.borrowed_views.insert(node.span, part.clone());
                    return;
                }
            }
        }
        if matches!(&expected, Type::Borrow(inner) if matches!(inner.as_ref(), Type::Dynamic(_)))
            && expected != self.inference.resolve(found)
        {
            self.borrowed_views.insert(node.span, expected);
        }
    }

    /// Whether a plain value can go into a new box where `Box[T]` is expected:
    /// `Box[dyn Shape]` takes anything with Shape, and `Box[T]` takes a T (Q86).
    fn box_content(
        &mut self,
        expected: &Type,
        found: &Type,
        obligations: &mut Vec<(Type, Type)>,
    ) -> bool {
        let Type::Named(name, args) = self.inference.resolve(expected) else {
            return false;
        };
        let found = self.inference.resolve(found);
        if name != "Box"
            || args.len() != 1
            || matches!(&found, Type::Named(inner, _) if inner == "Box")
            || matches!(found, Type::Variable(_) | Type::Invalid | Type::Borrow(_))
        {
            return false;
        }
        match &args[0] {
            Type::Dynamic(requirement) => {
                self.implements(&found, requirement, &mut BTreeSet::new())
            }
            content => self.constrain_argument(content, &found, obligations),
        }
    }

    fn constrain_argument(
        &mut self,
        expected: &Type,
        found: &Type,
        obligations: &mut Vec<(Type, Type)>,
    ) -> bool {
        let expected = self.inference.resolve(expected);
        let found = self.inference.resolve(found);
        if matches!(&expected, Type::Named(name, args) if name == "DynTensor" && args == &[Type::named("double")])
            && crate::types::is_tensor(&found)
        {
            return true;
        }
        if expected == Type::named("Error") && self.is_error_type(&found) {
            return true;
        }
        if let (Type::Named(want, targets), Type::Named(actual, sources)) = (&expected, &found)
            && matches!(want.as_str(), "CPointer" | "CConstPointer")
            && matches!(actual.as_str(), "CPointer" | "CConstPointer")
            && targets.len() == 1
            && sources.len() == 1
        {
            return !(want == "CPointer" && actual == "CConstPointer")
                && (targets[0] == Type::named("CVoid")
                    || self.inference.unify(&targets[0], &sources[0]));
        }
        if let Type::Union(parts) = &expected {
            if let Type::Union(found_parts) = &found {
                return found_parts
                    .iter()
                    .all(|part| self.constrain_argument(&expected, part, obligations));
            }
            let mut order = (0..parts.len()).collect::<Vec<_>>();
            if self.is_error_type(&found) {
                order.sort_by_key(|index| !self.is_error_type(&parts[*index]));
            }
            for index in order {
                let saved = self.inference.clone();
                let count = obligations.len();
                if self.constrain_argument(&parts[index], &found, obligations) {
                    return true;
                }
                self.inference = saved;
                obligations.truncate(count);
            }
            return false;
        }
        if let Type::Borrow(inner) = &found
            && !matches!(expected, Type::Borrow(_))
        {
            return self.constrain_argument(&expected, inner, obligations);
        }
        // A stored view, such as the item from first(), is lent to a parameter of its type;
        // the ownership pass still refuses handing it to a `move` parameter (REVIEW2 4).
        if let Type::StoredView(inner) = &found
            && !matches!(
                expected,
                Type::StoredView(_) | Type::Borrow(_) | Type::Variable(_)
            )
            && !crate::types::is_tensor(inner)
        {
            return self.constrain_argument(&expected, inner, obligations);
        }
        if let Type::Borrow(inner) = &expected
            && let Type::Dynamic(requirement) = inner.as_ref()
        {
            return self.implements(&found, requirement, &mut BTreeSet::new());
        }
        if let (Type::Named(expected_name, expected_args), Type::Named(found_name, found_args)) =
            (&expected, &found)
            && expected_name == "Box"
            && found_name == "Box"
            && expected_args.len() == 1
            && found_args.len() == 1
            && let Type::Dynamic(requirement) = &expected_args[0]
        {
            return self.implements(&found_args[0], requirement, &mut BTreeSet::new());
        }
        if let Type::Function(function) = &found {
            let (instantiated, substitutions) = self
                .inference
                .instantiate_with_parameters(&Type::Function(function.clone()));
            if !self.inference.accepts(&expected, &instantiated) {
                return false;
            }
            if let Type::Function(function) = instantiated {
                obligations.extend(function.requirements.clone());
                for (parameter, requirements) in &function.bounds {
                    if let Some(ty) = substitutions.get(parameter) {
                        obligations.extend(
                            requirements
                                .iter()
                                .map(|requirement| (ty.clone(), requirement.clone())),
                        );
                    }
                }
            }
            return true;
        }
        if let Type::Overloaded(functions) = &found {
            let saved = self.inference.clone();
            let mut candidates = vec![];
            for function in functions {
                self.inference = saved.clone();
                let mut constraints = vec![];
                if self.constrain_argument(
                    &expected,
                    &Type::Function(function.clone()),
                    &mut constraints,
                ) {
                    candidates.push((self.inference.clone(), constraints));
                }
            }
            self.inference = saved;
            if candidates.len() == 1 {
                let (inference, constraints) = candidates.pop().unwrap();
                self.inference = inference;
                obligations.extend(constraints);
                return true;
            }
            return false;
        }
        self.inference.accepts(&expected, &found)
    }

    fn check_obligations(&mut self, node: &Node, obligations: &[(Type, Type)]) {
        for (ty, requirement) in obligations {
            let ty = self.inference.resolve(ty);
            let requirement = self.inference.resolve(requirement);
            if !self.satisfy_trait(&ty, &requirement) {
                self.error(
                    node,
                    "E_TRAIT_BOUND",
                    format!("Expected {ty} to provide {requirement}, but it does not."),
                    "Use a function whose type and trait requirements fit the argument type.",
                );
            }
        }
    }
    fn visibility(&self, node: &Node) -> Visibility {
        if self.modifier(node, "public") {
            Visibility::Public
        } else if self.modifier(node, "package") {
            Visibility::Package
        } else {
            Visibility::Private
        }
    }
    fn modifier(&self, node: &Node, name: &str) -> bool {
        node.children
            .iter()
            .any(|n| n.kind == K::Modifier && self.text(n) == name)
    }

    /// Whether a declaration writes `Self` as its direct result type.
    fn self_annotation(&self, node: &Node) -> bool {
        node.child(K::ReturnType)
            .and_then(|result| result.children.first())
            .is_some_and(|ty| {
                let ty = ungroup(ty);
                ty.kind == K::TypeName && crate::lexer::path_name(self.text(ty)) == "Self"
            })
    }

    /// A trait's default body cannot name the implementing class, so it may
    /// return `this` where the signature says `Self`.
    fn self_default_body(&self, result: &Type, found: &Type) -> bool {
        crate::types::is_self_type(result)
            && self.owner.as_ref().is_some_and(|owner| {
                self.environment
                    .definitions
                    .get(owner)
                    .is_some_and(|definition| definition.kind == K::Trait)
                    && base_name(found) == Some(owner)
            })
    }

    /// Whether a method's written result is `Self`. An override may narrow such
    /// a result to a subclass; no other result allows a narrower one.
    fn self_result_method(&self, function: &FunctionType) -> bool {
        let Some(Callable::Declaration {
            id,
            name,
            owner: Some(owner),
            ..
        }) = &function.callable
        else {
            return false;
        };
        let Some(definition) = self.environment.definitions.get(owner) else {
            return false;
        };
        definition.self_results.contains(id)
            || self.satisfies_trait_method(owner, definition, name, function, true)
    }

    /// A method marked `@override`. The old `override` modifier still counts, so one mistake
    /// gives one diagnostic.
    fn overriding(&self, member: &Node) -> bool {
        self.modifier(member, "override")
            || member.children.iter().any(|n| {
                n.kind == K::Attribute
                    && n.children
                        .first()
                        .is_some_and(|name| self.text(name) == "override")
            })
    }

    /// Reject a modifier in a place where it has no meaning, so a typo or a
    /// copied keyword never changes nothing silently.
    fn check_modifiers(&mut self, node: &Node, allowed: &[&str]) {
        let mut seen = BTreeSet::new();
        let mut visibility = BTreeSet::new();
        for modifier in node.children.iter().filter(|n| n.kind == K::Modifier) {
            let name = self.text(modifier).to_owned();
            if !seen.insert(name.clone()) {
                self.error(
                    modifier,
                    "E_MODIFIER_DUPLICATE",
                    format!("`{name}` is written twice."),
                    "Write each modifier once.",
                );
                continue;
            }
            if matches!(name.as_str(), "public" | "private" | "package") {
                visibility.insert(name.clone());
            }
            if allowed.contains(&name.as_str()) {
                continue;
            }
            let (code, hint) = match name.as_str() {
                "override" if node.kind == K::Function => (
                    "E_OVERRIDE_SYNTAX",
                    "Write @override on the line before the method, as in @override public function area().",
                ),
                "override" => (
                    "E_OVERRIDE_CONTEXT",
                    "Write @override only on a class method whose parameter types match an inherited method.",
                ),
                "sealed" => ("E_SEALED_CONTEXT", "Write sealed only on a trait."),
                "static" => (
                    "E_STATIC_CONTEXT",
                    "Static belongs on a class method; pass or own the value explicitly.",
                ),
                _ => ("E_MODIFIER_CONTEXT", "Remove this modifier."),
            };
            self.error(
                modifier,
                code,
                format!("`{name}` is not allowed here."),
                hint,
            );
        }
        if visibility.len() > 1
            && let Some(modifier) = node.children.iter().find(|n| {
                n.kind == K::Modifier
                    && matches!(self.text(n), "public" | "private" | "package")
                    && !visibility.is_empty()
            })
        {
            let names = visibility.into_iter().collect::<Vec<_>>().join(" and ");
            self.error(
                modifier,
                "E_MODIFIER_CONFLICT",
                format!("A declaration has one visibility, but this one writes {names}."),
                "Keep exactly one of public, private and package.",
            );
        }
    }

    /// Top-level declarations accept visibility and, for a trait, `sealed`.
    /// `mut` keeps its dedicated diagnostic in function_body.
    fn check_declaration_modifiers(&mut self, node: &Node) {
        let allowed: &[&str] = match node.kind {
            // Imports are file-local; this version has no re-export rule.
            K::Import => &[],
            K::Trait => &["public", "private", "package", "sealed", "mut"],
            // `record` marks the header that declares the fields of a record class (Q131).
            K::Class => &["public", "private", "package", "mut", "record"],
            _ => &["public", "private", "package", "mut"],
        };
        self.check_modifiers(node, allowed);
    }

    fn check_member_modifiers(&mut self, member: &Node, in_trait: bool) {
        match member.kind {
            K::Function if in_trait => {
                self.check_modifiers(member, &["public", "private", "package", "static", "mut"])
            }
            K::Function => {
                self.check_modifiers(member, &["public", "private", "package", "static", "mut"])
            }
            K::Constructor => {
                self.check_modifiers(member, &["public", "private", "package", "mut"])
            }
            K::Binding => self.check_modifiers(
                member,
                &[
                    "public", "private", "package", "final", "var", "static", "record",
                ],
            ),
            K::AssociatedTypeDeclaration => self.check_modifiers(member, &[]),
            K::Invariant => self.check_modifiers(member, &[]),
            _ => {}
        }
    }

    /// Accept only the attributes the compiler implements, on the declarations
    /// that read them.
    fn check_attributes(&mut self, node: &Node, top_level: bool) {
        let mut seen = BTreeSet::new();
        for attribute in node.children.iter().filter(|n| n.kind == K::Attribute) {
            let name = attribute
                .children
                .first()
                .map(|n| self.text(n))
                .unwrap_or_default()
                .to_owned();
            if !seen.insert(name.clone()) {
                self.error(
                    attribute,
                    "E_ATTRIBUTE_DUPLICATE",
                    format!("`@{name}` is repeated."),
                    "Write each attribute once.",
                );
                continue;
            }
            match (name.as_str(), node.kind, top_level) {
                ("test", K::Function, true) | ("simp", K::Theorem, true) => {}
                ("skip", K::Function, true) if node.children.iter().any(|child| child.kind == K::Attribute && child.children.first().is_some_and(|name| self.text(name) == "test")) => {}
                ("skip", _, _) => self.error(attribute, "E_ATTRIBUTE_CONTEXT", "@skip belongs on a top-level @test function.", "Use @test and @skip on the same function."),
                ("test", K::Function, false) => self.error(
                    attribute,
                    "E_ATTRIBUTE_CONTEXT",
                    "A test marks a top-level function, not a method.",
                    "Move the test to the package's tests folder as a top-level function.",
                ),
                ("override", K::Function, false) => {}
                ("override", _, _) => self.error(
                    attribute,
                    "E_ATTRIBUTE_CONTEXT",
                    "`@override` marks a method that replaces an inherited one.",
                    "Put @override on the method, not on a top-level function or another declaration.",
                ),
                ("test" | "simp", _, _) => self.error(
                    attribute,
                    "E_ATTRIBUTE_CONTEXT",
                    format!("`@{name}` is not allowed on this declaration."),
                    "A test marks a top-level function; simp marks a theorem.",
                ),
                _ => self.error(
                    attribute,
                    "E_ATTRIBUTE_UNKNOWN",
                    format!("Unknown attribute `@{name}`."),
                    "Use @test, @skip, @simp, or @override.",
                ),
            }
        }
    }
    /// The name a top-level function is stored under: `List.take` for one in `extend List`.
    fn function_key(&self, node: &Node) -> String {
        let name = self.named_child(node);
        match node.child(K::ExtendTarget) {
            Some(target) => format!("{}.{name}", self.text(&target.children[0])),
            None => name,
        }
    }
    fn named_child(&self, node: &Node) -> String {
        node.child(K::Name)
            .map(|n| self.text(n).to_owned())
            .unwrap_or_default()
    }
    fn naming(&mut self, node: &Node, name: &str, form: &str) {
        if self.source.name == "<session>"
            && (name.starts_with("session__") || name.starts_with("Session__"))
        {
            return;
        }
        if name == "this" {
            self.error(
                node,
                "E_RESERVED_NAME",
                "`this` names the current object.",
                "Choose another name for this declaration.",
            );
            return;
        }
        if !crate::identifiers::valid(name, form) {
            self.error(
                node,
                "E_NAMING",
                format!("`{name}` must use {form}."),
                "Rename this declaration and its uses.",
            );
        }
    }
    fn symbol(&self, ty: Type, visibility: Visibility) -> Symbol {
        Symbol {
            ty,
            quantified: BTreeSet::new(),
            mutable: false,
            visibility,
            access: None,
            origin: None,
            reference: None,
        }
    }
    fn builtins(&mut self) {
        // Q71: the standard Nat. Run time stores it as an unsigned 64-bit number.
        self.environment
            .names
            .entry("Nat".into())
            .or_insert_with(|| "Nat".into());
        self.environment
            .definitions
            .entry("Nat".into())
            .or_insert_with(|| Definition {
                kind: K::Enum,
                sealed: false,
                record: false,
                parameters: vec![],
                associated_types: BTreeSet::new(),
                bounds: BTreeMap::new(),
                fields: BTreeMap::new(),
                constructor_fields: BTreeSet::new(),
                static_fields: BTreeMap::new(),
                static_vars: BTreeSet::new(),
                mutable_fields: BTreeSet::new(),
                initialized_fields: BTreeSet::new(),
                invariants: vec![],
                methods: BTreeMap::new(),
                default_methods: BTreeSet::new(),
                self_results: BTreeSet::new(),
                static_methods: BTreeSet::new(),
                parent: None,
                traits: vec![],
                constructors: vec![],
                variants: [("Zero", vec![]), ("Succ", vec![Type::named("Nat")])]
                    .into_iter()
                    .enumerate()
                    .map(|(index, (name, fields))| Variant {
                        name: name.into(),
                        declaration: DeclarationId {
                            file: "<std.core>".into(),
                            span: Span::new(100 + index, 101 + index),
                        },
                        field_names: vec![None; fields.len()],
                        fields,
                        result: Type::named("Nat"),
                    })
                    .collect(),
                alias: None,
                visibility: Visibility::Public,
                member_visibility: BTreeMap::new(),
                file: "<std.core>".into(),
            });
        let runtime_id = "std.core.RuntimeError";
        self.environment
            .names
            .entry("RuntimeError".into())
            .or_insert_with(|| runtime_id.into());
        self.environment
            .definitions
            .entry(runtime_id.into())
            .or_insert_with(|| Definition {
                kind: K::Enum,
                sealed: false,
                record: false,
                parameters: vec![],
                associated_types: BTreeSet::new(),
                bounds: BTreeMap::new(),
                fields: BTreeMap::new(),
                constructor_fields: BTreeSet::new(),
                static_fields: BTreeMap::new(),
                static_vars: BTreeSet::new(),
                mutable_fields: BTreeSet::new(),
                initialized_fields: BTreeSet::new(),
                invariants: vec![],
                methods: BTreeMap::new(),
                default_methods: BTreeSet::new(),
                self_results: BTreeSet::new(),
                static_methods: BTreeSet::new(),
                parent: Some(Type::named("Error")),
                traits: vec![],
                constructors: vec![],
                variants: [
                    "Arithmetic",
                    "Contract",
                    "Parse",
                    "Io",
                    "Python",
                    "EndOfFile",
                    "Message",
                ]
                .into_iter()
                .enumerate()
                .map(|(index, name)| Variant {
                    name: name.into(),
                    declaration: DeclarationId {
                        file: "<std.core>".into(),
                        span: Span::new(index, index + 1),
                    },
                    field_names: vec![None],
                    fields: vec![Type::named("String")],
                    result: Type::named(runtime_id),
                })
                .collect(),
                alias: None,
                visibility: Visibility::Public,
                member_visibility: BTreeMap::new(),
                file: "<std.core>".into(),
            });

        let t = Type::Parameter("T".into());
        let int_parameter = |name: &str| Parameter {
            name: Some(name.into()),
            ty: Type::named("Int"),
            default: false,
            passing: Passing::Borrow,
        };
        let range_function = |parameters: Vec<Parameter>| {
            let Type::Function(mut function) = Type::function(
                parameters
                    .iter()
                    .map(|parameter| parameter.ty.clone())
                    .collect(),
                Type::Lazy(Box::new(Type::named("Int")), Default::default()),
            ) else {
                unreachable!()
            };
            function.parameters = parameters;
            Type::Function(function)
        };
        let items = [
            (
                "print",
                Type::function(vec![t.clone()], Type::named("void")),
            ),
            (
                "parseInt",
                Type::function(
                    vec![Type::named("String")],
                    Type::union(Type::named("Int"), Type::named("Error")),
                ),
            ),
            (
                "parseDouble",
                Type::function(
                    vec![Type::named("String")],
                    Type::union(Type::named("double"), Type::named("Error")),
                ),
            ),
            (
                "parseBigInt",
                Type::function(
                    vec![Type::named("String")],
                    Type::union(Type::named("BigInt"), Type::named("Error")),
                ),
            ),
            (
                "box",
                Type::function(vec![t.clone()], Type::applied("Box", vec![t.clone()])),
            ),
            (
                "Some",
                Type::function(vec![t.clone()], Type::applied("Option", vec![t.clone()])),
            ),
            ("None", Type::applied("Option", vec![t])),
            (
                "open",
                Type::function(
                    vec![Type::named("String")],
                    Type::union(Type::named("Reader"), Type::named("Error")),
                ),
            ),
            (
                "create",
                Type::function(
                    vec![Type::named("String")],
                    Type::union(Type::named("Writer"), Type::named("Error")),
                ),
            ),
            (
                "readLine",
                Type::function(
                    vec![],
                    Type::union(Type::named("String"), Type::named("Error")),
                ),
            ),
            (
                "range",
                Type::Overloaded(vec![
                    match range_function(vec![int_parameter("stop")]) {
                        Type::Function(function) => function,
                        _ => unreachable!(),
                    },
                    match range_function(vec![int_parameter("start"), int_parameter("stop")]) {
                        Type::Function(function) => function,
                        _ => unreachable!(),
                    },
                    match range_function(vec![
                        int_parameter("start"),
                        int_parameter("stop"),
                        int_parameter("step"),
                    ]) {
                        Type::Function(function) => function,
                        _ => unreachable!(),
                    },
                ]),
            ),
        ];
        for (name, ty) in items {
            let ty = if matches!(name, "open" | "create" | "readLine") {
                let Type::Function(mut function) = ty else {
                    unreachable!()
                };
                function.effects = true;
                Type::Function(function)
            } else if name == "print" {
                let Type::Function(mut function) = ty else {
                    unreachable!()
                };
                function.effects = true;
                function.parameters[0].name = Some("value".into());
                function
                    .bounds
                    .insert("T".into(), vec![Type::named("Text")]);
                Type::Function(function)
            } else {
                ty
            };
            let mut ty = ty;
            match &mut ty {
                Type::Function(function) => {
                    function.callable = Some(Callable::Builtin(format!("std.core.{name}")));
                }
                Type::Overloaded(functions) => {
                    for function in functions {
                        function.callable = Some(Callable::Builtin(format!("std.core.{name}")));
                    }
                }
                _ => {}
            }
            let mut symbol = self.symbol(ty, Visibility::Public);
            symbol.reference = Some(Reference::Builtin(format!("std.core.{name}")));
            self.environment.values.entry(name.into()).or_insert(symbol);
        }
    }
    /// Registers each type a file declares, with an empty definition.
    fn register_types(&mut self, nodes: &[Node]) {
        for wrapper in nodes {
            let opaque_type = wrapper.kind == K::Opaque;
            let node = if opaque_type {
                wrapper.children.first().unwrap_or(wrapper)
            } else {
                wrapper
            };
            if node.kind != K::Enum && opaque_type {
                continue;
            }
            if is_declaration(node.kind) && node.kind != K::Function && self.modifier(node, "mut") {
                self.error(node, "E_MUT_METHOD_CONTEXT", "The mut item modifier is for instance methods.", "Use let mut for locals, var for fields, and mut parameters for borrowed arguments.");
            }
            if matches!(node.kind, K::Class | K::Trait | K::Enum | K::Alias) {
                let name = self.named_child(node);
                self.naming(node, &name, "PascalCase");
                if name == "Error" {
                    self.error(
                        node,
                        "E_ERROR_ROOT_REDEFINITION",
                        "Error is a standard root and cannot be replaced by a local enum.",
                        "Write a named enum such as `enum ParseError extends Error { ... }`.",
                    );
                    continue;
                }
                // A file's own type replaces a prelude type of the same name, so a program
                // may write its own Stack or Counter (Q112).
                let from_prelude = self.environment.names.get(&name).is_some_and(|id| {
                    self.environment
                        .definitions
                        .get(id)
                        .is_some_and(|definition| definition.file.ends_with("kestrel_prelude.kes"))
                });
                if self.environment.names.contains_key(&name) && !from_prelude
                    || matches!(
                        self.environment.values.get(&name).map(|s| &s.ty),
                        Some(Type::Namespace(_))
                    )
                {
                    self.error(
                        node,
                        "E_DUPLICATE_NAME",
                        format!("The type `{name}` is already defined."),
                        "Use a different type name; built-in types cannot be replaced.",
                    );
                    continue;
                }
                let id = self.qualified(&name);
                let parameters = node
                    .child(K::TypeParameters)
                    .map(|p| p.children.iter().map(|n| self.named_child(n)).collect())
                    .unwrap_or_default();
                self.environment.names.insert(name, id.clone());
                self.environment.definitions.insert(
                    id,
                    Definition {
                        kind: node.kind,
                        sealed: self.modifier(node, "sealed"),
                        record: node.kind == K::Class && self.modifier(node, "record"),
                        parameters,
                        associated_types: node
                            .children
                            .iter()
                            .filter(|n| n.kind == K::AssociatedTypeDeclaration)
                            .map(|n| self.named_child(n))
                            .collect(),
                        bounds: BTreeMap::new(),
                        fields: BTreeMap::new(),
                        constructor_fields: BTreeSet::new(),
                        static_fields: BTreeMap::new(),
                        static_vars: BTreeSet::new(),
                        mutable_fields: BTreeSet::new(),
                        initialized_fields: BTreeSet::new(),
                        invariants: vec![],
                        methods: BTreeMap::new(),
                        default_methods: BTreeSet::new(),
                        self_results: BTreeSet::new(),
                        static_methods: BTreeSet::new(),
                        parent: None,
                        traits: vec![],
                        constructors: vec![],
                        variants: vec![],
                        alias: None,
                        visibility: self.visibility(node),
                        member_visibility: BTreeMap::new(),
                        file: self.source.name.clone(),
                    },
                );
            }
        }
    }
    fn declarations(&mut self, nodes: &[Node]) {
        self.register_types(nodes);
        for node in nodes.iter().filter(|n| n.kind == K::Alias) {
            self.definition(node);
        }
        for node in nodes {
            if matches!(node.kind, K::Class | K::Trait | K::Enum)
                && self.named_child(node) != "Error"
            {
                self.definition(node);
            }
        }
        for wrapper in nodes.iter().filter(|n| n.kind == K::Opaque) {
            let Some(function) = wrapper.children.first().filter(|n| n.kind == K::Function) else {
                continue;
            };
            let name = self.named_child(function);
            self.naming(function, &name, "camelCase");
            let signature = self.signature(function);
            self.add_function(function, &name, signature);
        }
        for node in nodes {
            match node.kind {
                K::Function => {
                    let name = self.named_child(node);
                    self.naming(node, &name, "camelCase");
                    let signature = self.signature(node);
                    // A function in `extend List { ... }` is stored as `List.take`, a key no
                    // plain name can spell, and is reached only through a member call.
                    let key = self.function_key(node);
                    if let Some(target) = node.child(K::ExtendTarget) {
                        let expected = self.text(&target.children[0]).to_owned();
                        let first = signature
                            .parameters
                            .first()
                            .map(|p| self.inference.resolve(&p.ty));
                        if first
                            .as_ref()
                            .and_then(base_name)
                            .is_none_or(|b| b != expected)
                            && !(expected == "Error" && first.as_ref().is_some_and(contains_error))
                        {
                            self.error(
                                node,
                                "E_EXTEND_RECEIVER",
                                format!("A function in `extend {expected}` takes a {expected} as its first parameter."),
                                "Write the receiver first, such as `function take[T](xs : List[T], n : Int)`.",
                            );
                        }
                    }
                    self.add_function(node, &key, signature);
                }
                K::Const => {
                    let name = binding_name(node, self.source);
                    self.naming(node, &name, "SCREAMING_SNAKE_CASE");
                    let annotation = node.children.iter().find(|n| is_type(n.kind));
                    let ty = annotation
                        .map(|n| self.annotation(n, false))
                        .unwrap_or_else(|| self.inference.fresh());
                    let mut symbol = self.symbol(ty, self.visibility(node));
                    symbol.reference = Some(Reference::Constant(DeclarationId {
                        file: self.source.name.clone(),
                        span: node.span,
                    }));
                    if self
                        .environment
                        .values
                        .insert(name.clone(), symbol)
                        .is_some()
                    {
                        self.error(
                            node,
                            "E_DUPLICATE_NAME",
                            format!("`{name}` is already defined."),
                            "Use a different name.",
                        );
                    }
                }
                _ => {}
            }
        }
        self.check_constructor_field_inheritance(nodes);
        self.synthesize_trait_defaults(nodes);
    }

    fn collect_local_theorems(&mut self, nodes: &[Node]) {
        for node in nodes.iter().filter(|node| node.kind == K::Theorem) {
            let Some(name) = node.child(K::Name).map(|name| self.text(name).to_owned()) else {
                continue;
            };
            let bounds = self.parameter_bounds(node);
            let mut parameters = vec![];
            if let Some(list) = node.child(K::ParameterList) {
                for parameter in &list.children {
                    if parameter.child(K::Proposition).is_some() {
                        continue;
                    }
                    let Some(parameter_name) = parameter.child(K::Name) else {
                        continue;
                    };
                    let Some(annotation) =
                        parameter.children.iter().find(|part| is_type(part.kind))
                    else {
                        continue;
                    };
                    parameters.push((
                        self.text(parameter_name).to_owned(),
                        self.annotation(annotation, true),
                    ));
                }
            }
            let mut signature = match Type::function(
                parameters.iter().map(|(_, ty)| ty.clone()).collect(),
                Type::named("Proof"),
            ) {
                Type::Function(signature) => signature,
                _ => unreachable!(),
            };
            for (parameter, (name, _)) in signature.parameters.iter_mut().zip(parameters) {
                parameter.name = Some(name);
            }
            signature.bounds = bounds;
            signature.callable = Some(Callable::Builtin("kestrel.theorem-evidence".into()));
            self.local_theorems.entry(name).or_insert(signature);
        }
    }

    fn theorem_signature(&self, name: &str) -> Option<TheoremSignature> {
        let name = crate::lexer::path_name(name);
        if let Some(signature) = self.local_theorems.get(&name) {
            return Some(signature.clone());
        }
        let theorem = crate::proof::runtime_theorem(&self.environment, &name)?;
        let mut signature = match Type::function(
            theorem.params.iter().map(|(_, ty)| ty.clone()).collect(),
            Type::named("Proof"),
        ) {
            Type::Function(signature) => signature,
            _ => unreachable!(),
        };
        for (parameter, (name, _)) in signature.parameters.iter_mut().zip(&theorem.params) {
            parameter.name = Some(name.clone());
        }
        signature.bounds = theorem.bounds.clone();
        signature.callable = Some(Callable::Builtin("kestrel.theorem-evidence".into()));
        Some(signature)
    }

    fn synthesize_trait_defaults(&mut self, nodes: &[Node]) {
        let mut complete = BTreeSet::new();
        let mut visiting = BTreeSet::new();
        for node in nodes.iter().filter(|node| node.kind == K::Class) {
            let id = self.qualified(&self.named_child(node));
            self.synthesize_class_defaults(&id, &mut complete, &mut visiting);
        }
    }

    fn synthesize_class_defaults(
        &mut self,
        id: &str,
        complete: &mut BTreeSet<String>,
        visiting: &mut BTreeSet<String>,
    ) {
        if complete.contains(id) || !visiting.insert(id.to_owned()) {
            return;
        }
        let Some(definition) = self.environment.definitions.get(id).cloned() else {
            return;
        };
        if let Some(Type::Named(parent, _)) = &definition.parent
            && self
                .environment
                .definitions
                .get(parent)
                .is_some_and(|parent| parent.kind == K::Class)
        {
            self.synthesize_class_defaults(parent, complete, visiting);
        }

        let receiver = Type::Named(
            id.to_owned(),
            definition
                .parameters
                .iter()
                .map(|parameter| Type::Parameter(parameter.clone()))
                .collect(),
        );
        let groups = self.grouped_trait_defaults(&receiver, &definition.traits);
        let class_substitutions = definition
            .parameters
            .iter()
            .cloned()
            .zip(match &receiver {
                Type::Named(_, arguments) => arguments.clone(),
                _ => vec![],
            })
            .collect::<BTreeMap<_, _>>();
        for group in groups.into_iter().filter(|group| group.len() == 1) {
            let candidate = group.into_iter().next().unwrap();
            let name = candidate.name.clone();
            let own = definition.methods.get(&name).is_some_and(|methods| {
                methods
                    .iter()
                    .any(|method| method.same_parameters(&candidate.signature))
            });
            let inherited = definition.parent.as_ref().is_some_and(|parent| {
                self.methods(
                    &parent.replace_parameters(&class_substitutions),
                    &name,
                    &mut BTreeSet::new(),
                )
                .iter()
                .any(|method| method.same_parameters(&candidate.signature))
            });
            if own || inherited {
                continue;
            }
            let mut signature = candidate.signature;
            signature.callable = Some(Callable::Declaration {
                id: candidate.declaration,
                name: name.clone(),
                owner: Some(id.to_owned()),
                kind: if candidate.is_static {
                    DeclarationKind::StaticMethod
                } else {
                    DeclarationKind::Method
                },
            });
            let Some(target) = self.environment.definitions.get_mut(id) else {
                continue;
            };
            target
                .methods
                .entry(name.clone())
                .or_default()
                .push(signature);
            target
                .member_visibility
                .entry(name.clone())
                .or_insert(candidate.visibility);
            if candidate.is_static {
                target.static_methods.insert(name);
            }
        }
        visiting.remove(id);
        complete.insert(id.to_owned());
    }

    /// Kestrel has no `super(...)` call, so a subclass constructor cannot run a
    /// parent constructor. A parent whose state lives in constructor-declared
    /// private fields would therefore reach a subclass uninitialized.
    fn check_constructor_field_inheritance(&mut self, nodes: &[Node]) {
        for node in nodes.iter().filter(|n| n.kind == K::Class) {
            let name = self.named_child(node);
            let id = self.qualified(&name);
            let Some(definition) = self.environment.definitions.get(&id) else {
                continue;
            };
            let Some(mut parent) = definition.parent.clone() else {
                continue;
            };
            let mut seen = BTreeSet::new();
            // Each field that only its class's constructor sets, its class, and whether that
            // class has a constructor that super(...) could run.
            let mut unset: Vec<(String, String, bool)> = vec![];
            while let Type::Named(parent_id, _) = &parent {
                if !seen.insert(parent_id.clone()) {
                    break;
                }
                let Some(parent_definition) = self.environment.definitions.get(parent_id) else {
                    break;
                };
                if !parent_definition.constructor_fields.is_empty()
                    && !node
                        .descendants()
                        .any(|n| n.kind == K::Name && self.text(n) == "super")
                {
                    let fields = parent_definition
                        .constructor_fields
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ");
                    let parent_name = parent_id.rsplit('.').next().unwrap_or(parent_id);
                    self.error(
                        node,
                        "E_CONSTRUCTOR_FIELD_INHERITANCE",
                        format!(
                            "Class `{parent_name}` declares constructor field {fields}; a subclass cannot initialize them, because constructors do not chain."
                        ),
                        "Keep the fields of an extensible base class in its class body, or add an explicit super-constructor call before extending it.",
                    );
                    break;
                }
                // A parent field that only its constructor sets stays unset unless every
                // subclass constructor runs that constructor through super(...).
                let parent_name = parent_id.rsplit('.').next().unwrap_or(parent_id).to_owned();
                let constructs = !parent_definition.constructors.is_empty();
                for field in parent_definition.fields.keys() {
                    if !parent_definition.initialized_fields.contains(field) {
                        unset.push((field.clone(), parent_name.clone(), constructs));
                    }
                }
                parent = parent_definition
                    .parent
                    .clone()
                    .unwrap_or(Type::named("Object"));
            }
            for constructor in node.children.iter().filter(|n| n.kind == K::Constructor) {
                let calls_super = constructor.descendants().any(|n| {
                    n.kind == K::Call
                        && n.children.first().is_some_and(|callee| {
                            callee.kind == K::Name && self.text(callee) == "super"
                        })
                });
                // Without super, the constructor must set each such field itself; a `final`
                // one cannot be, so it needs super.
                let assigned = constructor
                    .descendants()
                    .filter(|n| n.kind == K::Assignment)
                    .filter_map(|n| n.children.first())
                    .filter(|target| {
                        target.kind == K::Member
                            && target
                                .children
                                .first()
                                .is_some_and(|this| self.text(this) == "this")
                    })
                    .filter_map(|target| {
                        target
                            .children
                            .get(1)
                            .map(|name| self.text(name).to_owned())
                    })
                    .collect::<BTreeSet<_>>();
                let missing = unset
                    .iter()
                    .filter(|(field, _, _)| !assigned.contains(field))
                    .collect::<Vec<_>>();
                if !calls_super && let Some((_, owner, constructs)) = missing.first() {
                    let fields = missing
                        .iter()
                        .map(|(field, _, _)| field.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    if *constructs {
                        self.error(
                            constructor,
                            "E_SUPER_REQUIRED",
                            format!("This constructor must call super(...) first: `{owner}` sets {fields} in its own constructor."),
                            "Start the constructor with super(...), passing what the parent constructor needs.",
                        );
                    } else {
                        // The parent has no constructor to run, so only this one can set them.
                        self.error(
                            constructor,
                            "E_FIELD_UNINITIALIZED",
                            format!("This constructor leaves the inherited field {fields} of `{owner}` unset."),
                            "Set each inherited field in this constructor, or give it a default where it is declared.",
                        );
                    }
                }
            }
        }
    }

    fn add_function(&mut self, node: &Node, name: &str, signature: FunctionType) {
        if let Some(previous) = self.environment.values.get(name) {
            let mut signatures = match &previous.ty {
                Type::Function(f) => vec![f.clone()],
                Type::Overloaded(fs) => fs.clone(),
                _ => vec![],
            };
            if signatures.is_empty() || signatures.iter().any(|f| f.same_parameters(&signature)) {
                self.error(
                    node,
                    "E_DUPLICATE_SIGNATURE",
                    format!("`{name}` already has a declaration with these parameter types."),
                    "A different return type does not create an overload.",
                );
                return;
            }
            signatures.push(signature);
            self.environment.values.get_mut(name).unwrap().ty = Type::Overloaded(signatures);
        } else {
            self.environment.values.insert(
                name.into(),
                self.symbol(Type::Function(signature), self.visibility(node)),
            );
        }
    }
    fn parameter_bounds(&mut self, node: &Node) -> BTreeMap<String, Vec<Type>> {
        let mut bounds = BTreeMap::new();
        if let Some(list) = node.child(K::TypeParameters) {
            for parameter in &list.children {
                let name = self.named_child(parameter);
                let mut constraints = vec![];
                for bound in parameter.children.iter().filter(|n| n.kind == K::Bound) {
                    let ty = self.annotation(&bound.children[0], true);
                    self.require_trait(bound, &ty);
                    constraints.push(ty);
                }
                bounds.insert(name, constraints);
            }
        }
        bounds
    }
    fn signature(&mut self, node: &Node) -> FunctionType {
        let bounds = self.parameter_bounds(node);
        for name in bounds
            .keys()
            .filter(|name| self.associated_names.contains(*name))
            .cloned()
            .collect::<Vec<_>>()
        {
            self.error(
                node,
                "E_ASSOCIATED_TYPE",
                format!("Method parameter `{name}` conflicts with an associated type."),
                "Give the method's type parameter a different name.",
            );
        }
        let mut parameters = vec![];
        if let Some(list) = node.child(K::ParameterList) {
            let mut names = BTreeSet::new();
            for parameter in &list.children {
                let name = self.named_child(parameter);
                self.naming(parameter, &name, "value identifier");
                if !names.insert(name.clone()) {
                    self.error(
                        parameter,
                        "E_DUPLICATE_NAME",
                        format!("Parameter `{name}` is repeated."),
                        "Give each parameter a different name.",
                    );
                }
                let mut ty = if parameter.children.iter().any(|n| n.kind == K::Proposition) {
                    // A proof parameter has the type Proof; its claim is checked in the body's scope (Q115).
                    Type::named("Proof")
                } else if let Some(annotation) = parameter.children.iter().find(|n| is_type(n.kind))
                {
                    self.annotation(annotation, true)
                } else {
                    self.error(
                        parameter,
                        "E_SIGNATURE_ANNOTATION",
                        "A declared function parameter needs its type.",
                        "Write `name : Type`; lambdas may infer parameter types.",
                    );
                    Type::Invalid
                };
                if self.modifier(parameter, "mut")
                    && let Type::Function(function) = &mut ty
                {
                    function.receiver_mutates = true;
                }
                self.validate_borrow_position(parameter, &ty, false);
                let default = parameter
                    .children
                    .last()
                    .is_some_and(|n| !is_type(n.kind) && n.kind != K::Modifier);
                parameters.push(Parameter {
                    name: Some(name),
                    ty,
                    default,
                    passing: if self.modifier(parameter, "move") {
                        Passing::Move
                    } else if self.modifier(parameter, "mut") {
                        Passing::Mut
                    } else {
                        Passing::Borrow
                    },
                });
            }
        }
        let result = if node.kind == K::Constructor {
            Type::named("void")
        } else if let Some(ty) = node.child(K::ReturnType).and_then(|n| n.children.first()) {
            if ty.kind == K::Proposition {
                Type::named("Proof")
            } else {
                self.annotation(ty, true)
            }
        } else {
            self.error(
                node,
                "E_SIGNATURE_ANNOTATION",
                "A declared function needs its return type.",
                "Write `: Type` before the body.",
            );
            Type::Invalid
        };
        self.validate_borrow_position(node, &result, false);
        FunctionType {
            parameters,
            result: Box::new(result),
            bounds,
            requirements: vec![],
            effects: node.child(K::Effects).is_some(),
            receiver_mutates: self.modifier(node, "mut"),
            callable: Some(self.declaration(
                node,
                self.qualified(&self.function_key(node)),
                None,
                DeclarationKind::Function,
            )),
            receiver: None,
        }
    }
    fn reject_tensor_view_return(&mut self, node: &Node, expected: &Type, found: &Type) {
        let found = self.inference.resolve(found);
        let Type::StoredView(inner) = &found else {
            return;
        };
        if !crate::types::is_tensor(inner) {
            return;
        }
        let expected = self.inference.resolve(expected);
        if matches!(expected, Type::StoredView(_) | Type::Borrow(_)) {
            return;
        }
        self.error(
            node,
            "E_VIEW_ESCAPE",
            "A transpose view cannot be returned from a function.",
            "Return an owned tensor with `.copy()`.",
        );
    }

    fn require_trait(&mut self, node: &Node, ty: &Type) {
        let known = if let Some((name, _, _)) = trait_parts(ty) {
            matches!(
                name,
                "Text" | "Equals" | "Copy" | "Comparable" | "Sortable" | "Iterable" | "Iterator"
            ) || operator_trait(name)
                || unary_trait(name).is_some()
                || self
                    .environment
                    .definitions
                    .get(name)
                    .is_some_and(|d| d.kind == K::Trait)
        } else {
            false
        };
        if !known {
            self.error(
                node,
                "E_UNKNOWN_TRAIT",
                format!("`{ty}` is not a declared trait."),
                "Declare the trait or import it. Text is the text-conversion trait.",
            );
        }
    }
    /// Collect a `static final` field. Its initializer is checked later, in
    /// the class declaration pass, so it can use constants and other classes.
    fn static_field(&mut self, member: &Node, name: &str, id: &str, definition: &mut Definition) {
        self.naming(member, name, "camelCase");
        let is_var = self.modifier(member, "var");
        let is_final = self.modifier(member, "final");
        if is_var == is_final {
            self.error(
                member,
                "E_STATIC_MODIFIER",
                "A static field is either final or var.",
                "Write `static final` for an immutable value or `static var` for synchronized shared state.",
            );
            return;
        }
        if !definition.parameters.is_empty() {
            self.error(
                member,
                "E_STATIC_GENERIC",
                "A static field cannot use a class type parameter.",
                "Move the field to a non-generic class, or make it an instance field.",
            );
            return;
        }
        let Some(annotation) = member.children.iter().find(|n| is_type(n.kind)) else {
            self.error(
                member,
                "E_STATIC_TYPE",
                "A static field needs a written type.",
                "Write `static final name : Type = value` or `static var name : Type = value`.",
            );
            return;
        };
        let ty = self.annotation(annotation, true);
        if !crate::types::copies(&ty)
            || matches!(ty, Type::Function(_))
            || !self.immutable_type(&ty, &mut BTreeSet::new())
        {
            self.error(
                member,
                "E_STATIC_TYPE",
                format!("A static field cannot hold {ty}."),
                "Use a copyable immutable type such as Int, double, Bool, String, List, Map, Set or SortedSet.",
            );
            return;
        }
        let name_node = member.child(K::Name);
        let Some(initializer) = member.children.iter().rev().find(|n| {
            !is_type(n.kind)
                && !matches!(n.kind, K::Modifier | K::Attribute)
                && !name_node.is_some_and(|name_node| std::ptr::eq(*n, name_node))
        }) else {
            self.error(
                member,
                "E_STATIC_INIT",
                "A static field needs an initializer.",
                "Write `static final name : Type = value`.",
            );
            return;
        };
        if definition.fields.contains_key(name) {
            self.error(
                member,
                "E_DUPLICATE_NAME",
                format!("`{name}` is already an instance field of this class."),
                "Give the static field a different name.",
            );
            return;
        }
        if definition.methods.contains_key(name) || definition.static_methods.contains(name) {
            self.error(
                member,
                "E_DUPLICATE_NAME",
                format!("`{name}` is already a method of this class."),
                "Give the static field a different name.",
            );
            return;
        }
        if definition
            .static_fields
            .insert(name.to_owned(), ty.clone())
            .is_some()
        {
            self.error(
                member,
                "E_DUPLICATE_NAME",
                format!("Static field `{name}` is repeated."),
                "Declare each static field once.",
            );
            return;
        }
        definition
            .member_visibility
            .insert(name.to_owned(), self.visibility(member));
        if is_var {
            definition.static_vars.insert(name.to_owned());
        }
        self.static_fields.insert(
            (id.to_owned(), name.to_owned()),
            StaticFieldInfo {
                ty,
                mutable: is_var,
                file: self.source.name.clone(),
                initializer: initializer.span,
            },
        );
    }

    /// Registers the private fields a class introduces directly in a constructor.
    /// Public fields remain class-body declarations; this form is only shorthand
    /// for hidden state, so it never changes the class's visible surface.
    fn constructor_fields(&mut self, node: &Node, definition: &mut Definition) {
        let Some(body) = node.children.iter().find(|n| n.kind == K::Block) else {
            return;
        };
        if definition.kind != K::Class {
            for field in body
                .children
                .iter()
                .filter(|n| n.kind == K::FieldDeclaration)
            {
                self.error(
                    field,
                    "E_FIELD_DECLARATION_CONTEXT",
                    "Only a class constructor can declare fields.",
                    "Declare fields in a class body, or remove this declaration.",
                );
            }
            return;
        }
        for field in body
            .children
            .iter()
            .filter(|n| n.kind == K::FieldDeclaration)
        {
            self.constructor_field(field, definition);
        }
    }

    fn constructor_field(&mut self, node: &Node, definition: &mut Definition) {
        self.constructor_field_spans.insert(node.span);
        let Some(member) = node.child(K::Member) else {
            return;
        };
        let Some(name_node) = member.children.get(1) else {
            return;
        };
        let name = self.text(name_node).to_owned();
        self.naming(name_node, &name, "camelCase");
        if self.modifier(node, "public") || self.modifier(node, "package") {
            self.error(
                node,
                "E_CONSTRUCTOR_FIELD_VISIBILITY",
                "A constructor field is private.",
                "Declare a public or package field in the class body, where its type and visibility are visible to subclasses and importers.",
            );
        }
        if self.modifier(node, "static") {
            self.error(
                node,
                "E_CONSTRUCTOR_FIELD_STATIC",
                "A static field belongs to the class, not to one instance.",
                "Declare it in the class body with static.",
            );
        }
        if definition.methods.contains_key(&name) || definition.static_methods.contains(&name) {
            self.error(
                node,
                "E_DUPLICATE_NAME",
                format!("`{name}` is already a method of this class."),
                "Give the field a different name.",
            );
            return;
        }
        if definition.static_fields.contains_key(&name) {
            self.error(
                node,
                "E_DUPLICATE_NAME",
                format!("`{name}` is already a static field of this class."),
                "Give the instance field a different name.",
            );
            return;
        }
        if definition.fields.contains_key(&name) {
            self.error(
                node,
                "E_DUPLICATE_NAME",
                format!("`{name}` is already an instance field of this class."),
                "Declare each field once, in the class body or in one constructor.",
            );
            return;
        }
        let Some(annotation) = node.children.iter().find(|n| is_type(n.kind)) else {
            self.error(
                node,
                "E_FIELD_TYPE",
                "A constructor field needs a written type.",
                "Write `final this.name : Type := value`.",
            );
            return;
        };
        let ty = self.annotation(annotation, true);
        ty.map(&mut |part| {
            if let Type::Parameter(parameter) = part
                && !definition.parameters.contains(parameter)
            {
                self.error(
                    node,
                    "E_UNBOUND_TYPE_PARAMETER",
                    format!("Field `{name}` uses undeclared type parameter `{parameter}`."),
                    "Declare the parameter in the class header.",
                );
            }
            None
        });
        definition.fields.insert(name.clone(), ty);
        definition.constructor_fields.insert(name.clone());
        definition
            .member_visibility
            .insert(name.clone(), Visibility::Private);
        if self.modifier(node, "var") {
            definition.mutable_fields.insert(name);
        }
    }

    /// Checks one `final this.x : T := value` inside a constructor body.
    fn constructor_field_declaration(&mut self, node: &Node) -> Type {
        if !self.constructor_field_spans.contains(&node.span) {
            self.error(
                node,
                "E_FIELD_DECLARATION_CONTEXT",
                "A field declaration must be a direct statement of a constructor.",
                "Move it to the top level of new(...), or assign the field with this.name = value.",
            );
            return Type::named("void");
        }
        let Some(member) = node.child(K::Member) else {
            return Type::named("void");
        };
        let Some(name_node) = member.children.get(1) else {
            return Type::named("void");
        };
        let name = self.text(name_node).to_owned();
        let Some(owner) = self.owner.clone() else {
            return Type::named("void");
        };
        if !self
            .environment
            .definitions
            .get(&owner)
            .is_some_and(|definition| definition.fields.contains_key(&name))
        {
            self.error(
                node,
                "E_UNKNOWN_FIELD",
                format!("Class `{owner}` has no field `{name}` to declare."),
                "Declare the field in the class body or in this constructor.",
            );
            return Type::named("void");
        }
        let previous = self.assignment_target_span.replace(member.span);
        let target = self.infer(member, None);
        self.assignment_target_span = previous;
        let target = self.inference.resolve(&target);
        let Some(value_node) = node.children.last() else {
            return Type::named("void");
        };
        let value = self.infer(value_node, Some(&target));
        self.require(value_node, &target, &value);
        if let Some(fields) = self.initializing_fields.as_mut()
            && !fields.insert(name.clone())
        {
            self.error(
                node,
                "E_FIELD_REINITIALIZED",
                format!("Field `{name}` is already initialized in this constructor."),
                "Initialize each field once. Use this.name = value for a later assignment to a var field.",
            );
        }
        Type::named("void")
    }

    fn definition(&mut self, node: &Node) {
        let name = self.named_child(node);
        let Some(id) = self.environment.names.get(&name).cloned() else {
            return;
        };
        let Some(mut definition) = self.environment.definitions.get(&id).cloned() else {
            return;
        };
        definition.bounds = self.parameter_bounds(node);
        if node.kind == K::Alias {
            definition.alias = node
                .children
                .iter()
                .find(|n| is_type(n.kind))
                .map(|n| self.annotation(n, false));
        }
        let instance = Type::Named(
            id.clone(),
            definition
                .parameters
                .iter()
                .map(|n| Type::Parameter(n.clone()))
                .collect(),
        );
        // `Self` is this class. In a trait it stays the marker that each
        // implementation binds to its own class when `has` is checked.
        let previous_self = self.self_type.clone();
        if matches!(node.kind, K::Class | K::Trait) {
            self.self_type = Some(if node.kind == K::Trait {
                crate::types::self_type()
            } else {
                instance.clone()
            });
        }
        let previous_associated = std::mem::replace(
            &mut self.associated_names,
            definition.associated_types.clone(),
        );
        let mut declared_associated = BTreeSet::new();
        for member in &node.children {
            if self.modifier(member, "mut") && !matches!(member.kind, K::Function | K::Constructor)
            {
                self.error(
                    member,
                    "E_MUT_METHOD_CONTEXT",
                    "The mut item modifier is for instance methods.",
                    "Use var for a field that can change.",
                );
            }
            match member.kind {
                K::AssociatedTypeDeclaration => {
                    let name = self.named_child(member);
                    self.naming(member, &name, "PascalCase");
                    if node.kind != K::Trait {
                        self.error(member, "E_ASSOCIATED_CONTEXT", "An associated type declaration belongs inside a trait.", "Declare type Item in the trait; implementations infer it from their methods or bind it in has.");
                    }
                    if !declared_associated.insert(name.clone())
                        || definition.parameters.contains(&name)
                    {
                        self.error(
                            member,
                            "E_ASSOCIATED_TYPE",
                            format!("Type `{name}` is already declared in this trait."),
                            "Give each generic parameter and associated type a different name.",
                        );
                    }
                }
                K::TypeName | K::TypeApplication if matches!(node.kind, K::Class | K::Enum) => {
                    let previous = self.associated_names.clone();
                    self.associated_names
                        .extend(definition.parameters.iter().cloned());
                    definition.parent = Some(self.annotation(member, false));
                    self.associated_names = previous;
                }
                K::Binding => {
                    let name = self.named_child(member);
                    if definition.kind == K::Trait {
                        self.error(
                            member,
                            "E_TRAIT_FIELD",
                            "A trait declares methods and associated types, not fields.",
                            "Move the field into each class, or expose it through a method.",
                        );
                        continue;
                    }
                    if self.modifier(member, "static") {
                        self.static_field(member, &name, &id, &mut definition);
                        continue;
                    }
                    if definition.static_fields.contains_key(&name) {
                        self.error(
                            member,
                            "E_DUPLICATE_NAME",
                            format!("`{name}` is already a static field of this class."),
                            "Give the instance field a different name.",
                        );
                    }
                    self.naming(member, &name, "camelCase");
                    if self.modifier(member, "var") {
                        definition.mutable_fields.insert(name.clone());
                    }
                    // A record field is public and read-only, as the record header says (Q131).
                    // A header field has neither final nor var; a body field must say one.
                    let visibility = if definition.record
                        && !self.modifier(member, "var")
                        && !self.modifier(member, "final")
                    {
                        Visibility::Public
                    } else {
                        self.visibility(member)
                    };
                    definition
                        .member_visibility
                        .insert(name.clone(), visibility);
                    if let Some(ty) = member.children.iter().find(|n| is_type(n.kind)) {
                        if member
                            .children
                            .last()
                            .is_some_and(|last| last.span.start >= ty.span.end)
                        {
                            definition.initialized_fields.insert(name.clone());
                        }
                        let ty = self.annotation(ty, true);
                        ty.map(&mut |part| {
                            if let Type::Parameter(parameter) = part
                                && !definition.parameters.contains(parameter) { self.error(member, "E_UNBOUND_TYPE_PARAMETER", format!("Field `{name}` uses undeclared type parameter `{parameter}`."), "Declare the parameter in the class header."); }
                            None
                        });
                        if definition.fields.insert(name.clone(), ty).is_some() {
                            self.error(
                                member,
                                "E_DUPLICATE_NAME",
                                format!("Field `{name}` is repeated."),
                                "Declare each field once.",
                            );
                        }
                    }
                }
                K::Function => {
                    let name = self.named_child(member);
                    self.naming(member, &name, "camelCase");
                    if definition.static_fields.contains_key(&name) {
                        self.error(
                            member,
                            "E_DUPLICATE_NAME",
                            format!("`{name}` is already a static field of this class."),
                            "Give the method a different name.",
                        );
                    }
                    if definition.constructor_fields.contains(&name) {
                        self.error(
                            member,
                            "E_DUPLICATE_NAME",
                            format!("`{name}` is already a constructor field of this class."),
                            "Give the method a different name.",
                        );
                        continue;
                    }
                    let visibility = self.visibility(member);
                    if definition
                        .member_visibility
                        .insert(name.clone(), visibility)
                        .is_some_and(|previous| previous != visibility)
                    {
                        self.error(member, "E_OVERLOAD_VISIBILITY_UNSUPPORTED", "Different visibility on overloads of the same method is not implemented yet.", "The checker cannot certify this overload set yet.");
                    }
                    let mut signature = self.signature(member);
                    signature.callable = Some(self.declaration(
                        member,
                        name.clone(),
                        Some(id.clone()),
                        if definition.kind == K::Trait {
                            DeclarationKind::TraitMethod
                        } else if self.modifier(member, "static") {
                            DeclarationKind::StaticMethod
                        } else {
                            DeclarationKind::Method
                        },
                    ));
                    let is_static = self.modifier(member, "static");
                    if definition.kind == K::Class && self.self_annotation(member) {
                        definition.self_results.insert(DeclarationId {
                            file: self.source.name.clone(),
                            span: member.span,
                        });
                    }
                    if definition.kind == K::Trait && function_body(member).is_some() {
                        definition.default_methods.insert(member.span);
                    }
                    if definition
                        .methods
                        .get(&name)
                        .is_some_and(|existing| !existing.is_empty())
                        && definition.static_methods.contains(&name) != is_static
                    {
                        self.error(
                            member,
                            "E_STATIC_OVERLOAD",
                            "Static and instance methods cannot share a name.",
                            "Give the static method another name, or make both methods static.",
                        );
                    }
                    let methods = definition.methods.entry(name.clone()).or_default();
                    if methods.iter().any(|f| f.same_parameters(&signature)) {
                        self.error(
                            member,
                            "E_DUPLICATE_SIGNATURE",
                            format!("Method `{name}` repeats parameter types."),
                            "A different return type cannot create an overload.",
                        );
                    } else {
                        methods.push(signature);
                    }
                    if is_static {
                        definition.static_methods.insert(name);
                    }
                }
                K::Constructor => {
                    self.constructor_fields(member, &mut definition);
                    if let Some(return_type) = member.child(K::ReturnType) {
                        self.error(
                            return_type,
                            "E_CONSTRUCTOR_RETURN",
                            "A constructor returns its class; do not write a return type.",
                            "Remove the return type. Write the result type on a static factory function if you need one.",
                        );
                    }
                    let mut signature = self.signature(member);
                    signature.callable = Some(self.declaration(
                        member,
                        "new".into(),
                        Some(id.clone()),
                        DeclarationKind::Constructor,
                    ));
                    signature.bounds.extend(definition.bounds.clone());
                    signature.result = Box::new(instance.clone());
                    if definition
                        .constructors
                        .iter()
                        .any(|existing| existing.same_parameters(&signature))
                    {
                        self.error(
                            member,
                            "E_DUPLICATE_SIGNATURE",
                            "Constructor `new` repeats parameter types.",
                            "Give each constructor different parameter types.",
                        );
                    } else {
                        definition.constructors.push(signature);
                    }
                }
                K::Invariant => {
                    if node.kind != K::Class {
                        self.error(
                            member,
                            "E_INVARIANT_CONTEXT",
                            "An invariant belongs in a class body.",
                            "Move it into the class whose state it describes.",
                        );
                    } else if definition.record {
                        self.error(
                            member,
                            "E_INVARIANT_RECORD",
                            "A record has only immutable fields, so it needs no invariant.",
                            "Keep the check in a function, or use a class with var fields.",
                        );
                    } else {
                        definition.invariants.push(member.clone());
                    }
                }
                K::Has => {
                    for item in &member.children {
                        let ty = self.annotation(item, true);
                        self.require_trait(item, &ty);
                        // A trait cannot infer Item from its own methods, so a trait object of it would not
                        // know what next() or items() return.
                        if definition.kind == K::Trait
                            && let Some((name, _, bindings)) = trait_parts(&ty)
                            && name == "Iterator"
                            && !bindings.is_some_and(|bindings| bindings.contains_key("Item"))
                        {
                            self.error(
                                item,
                                "E_ASSOCIATED_INFER",
                                "A trait that has Iterator must give its item type.",
                                "Write has Iterator(Item = Int) with the item type that next() returns.",
                            );
                        }
                        if definition
                            .traits
                            .iter()
                            .any(|existing| existing.same_type(&ty))
                        {
                            self.error(
                                item,
                                "E_DUPLICATE_TRAIT",
                                "This trait is already listed.",
                                "List each trait once.",
                            );
                        } else {
                            definition.traits.push(ty);
                        }
                    }
                }
                K::Variant => {
                    let name = self.named_child(member);
                    self.naming(member, &name, "PascalCase");
                    if definition.variants.iter().any(|v| v.name == name) {
                        self.error(
                            member,
                            "E_DUPLICATE_NAME",
                            format!("Enum case `{name}` is repeated."),
                            "Give every case a different name.",
                        );
                    }
                    let mut fields = vec![];
                    let mut field_names = vec![];
                    let mut result = instance.clone();
                    for child in member.children.iter().filter(|n| n.kind != K::Name) {
                        if child.kind == K::Parameter {
                            if let Some(ty) = child.children.iter().find(|n| is_type(n.kind)) {
                                fields.push(self.annotation(ty, true));
                                match child.child(K::Name) {
                                    Some(label) => {
                                        let payload = self.text(label).to_owned();
                                        self.naming(label, &payload, "camelCase");
                                        field_names.push(Some(payload));
                                    }
                                    None => field_names.push(None),
                                }
                            }
                        } else if is_type(child.kind) {
                            if self.text(child).starts_with(&name) {
                                result = self.annotation(child, true);
                            } else {
                                fields.push(self.annotation(child, true));
                                field_names.push(None);
                            }
                        }
                    }
                    // The final type follows ':'; payload types are inside parentheses.
                    if let Some(last) = member.children.last().filter(|n| is_type(n.kind)) {
                        let between = &self.source.text[member.span.start..last.span.start];
                        if between.trim_end().ends_with(':') {
                            result = self.annotation(last, true);
                            if fields.last() == Some(&result) {
                                fields.pop();
                                field_names.pop();
                            }
                        }
                    }
                    // A case names every payload or none of them, never a mix.
                    if field_names.iter().any(Option::is_some)
                        && field_names.iter().any(Option::is_none)
                    {
                        self.error(
                            member,
                            "E_PAYLOAD_MIXED",
                            format!("Case `{name}` names some payloads and not others."),
                            "Name every payload, as in `Jump(height : Int, speed : Int)`, or none of them.",
                        );
                    }
                    let mut seen = BTreeSet::new();
                    for payload in field_names.iter().flatten() {
                        if !seen.insert(payload.clone()) {
                            self.error(
                                member,
                                "E_DUPLICATE_NAME",
                                format!("Payload `{payload}` of case `{name}` is repeated."),
                                "Give every payload a different name.",
                            );
                        }
                    }
                    // A payload that holds the enum itself is stored in a Box, so the declaration needs no
                    // explicit Box, and an ordinary value is boxed where the constructor takes it (Q132).
                    definition.variants.push(Variant {
                        declaration: DeclarationId {
                            file: self.source.name.clone(),
                            span: member.span,
                        },
                        name,
                        field_names,
                        fields,
                        result,
                    });
                }
                _ => {}
            }
        }
        if node.kind == K::Enum
            && let Some(parent) = &definition.parent
        {
            if *parent != Type::named("Error") {
                self.error(
                    node,
                    "E_ENUM_EXTENSION",
                    "An enum may extend only the standard Error root.",
                    "Keep ordinary enums closed, or extend Error directly.",
                );
            }
            if !definition.parameters.is_empty() {
                self.error(
                    node,
                    "E_GENERIC_ERROR_UNSUPPORTED",
                    "Generic error extensions are not implemented yet.",
                    "Use a concrete error payload type for this version of the checker.",
                );
            }
        }
        self.associated_names = previous_associated;
        if node.kind == K::Class && self.modifier(node, "record") {
            let fields: Vec<&Node> = node
                .children
                .iter()
                .filter(|n| n.kind == K::Binding)
                .collect();
            let mut parameters = vec![];
            let mut names = vec![];
            for field in fields {
                let name = self.named_child(field);
                // A generic record's fields name its type parameters; the field check above
                // already refuses an undeclared one.
                let ty = field
                    .children
                    .iter()
                    .find(|n| is_type(n.kind))
                    .map(|n| self.annotation(n, true))
                    .unwrap_or(Type::Invalid);
                parameters.push(Parameter {
                    name: Some(name.clone()),
                    ty,
                    default: false,
                    passing: Passing::Move,
                });
                names.push(name);
            }
            let constructor = FunctionType {
                parameters,
                result: Box::new(instance.clone()),
                bounds: definition.bounds.clone(),
                requirements: vec![],
                effects: false,
                receiver_mutates: false,
                callable: Some(Callable::Builtin(format!(
                    "record.{id}|{}",
                    names.join(",")
                ))),
                receiver: None,
            };
            definition.constructors.push(constructor);
        }
        self.self_type = previous_self;
        self.environment.definitions.insert(id, definition);
    }
    fn annotation(&mut self, node: &Node, implicit: bool) -> Type {
        match node.kind {
            K::TypeName => {
                let name = node
                    .children
                    .first()
                    .map(|n| crate::lexer::path_name(self.text(n)))
                    .unwrap_or_default();
                if name == "Self" {
                    if let Some(ty) = self.self_type.clone() {
                        return ty;
                    }
                    self.error(
                        node,
                        "E_SELF_OUTSIDE",
                        "`Self` names the class or trait that this declaration belongs to.",
                        "Use Self inside a class or trait, or write the concrete type here.",
                    );
                    return Type::Invalid;
                }
                if self.rigid_parameters.contains(&name) {
                    return Type::Rigid(name);
                }
                if self.associated_names.contains(&name) {
                    return Type::Parameter(name);
                }
                if name.split('.').next().is_some_and(|head| {
                    matches!(
                        self.environment.values.get(head).map(|symbol| &symbol.ty),
                        Some(Type::Namespace(_))
                    )
                }) && !self.environment.names.contains_key(&name)
                {
                    self.error(
                        node,
                        "E_NAMESPACE_TYPE",
                        format!("`{name}` does not name an exported type."),
                        "Use a qualified exported type, such as geometry.Point.",
                    );
                    return Type::Invalid;
                }
                if name == "Prop" {
                    return Type::named("Prop");
                }
                if name == "Display" {
                    self.error(
                        node,
                        "E_TEXT_TRAIT_NAME",
                        "The text trait is named Text. Display is not an alias.",
                        "Replace Display with Text.",
                    );
                    return Type::Invalid;
                }
                if let Some(id) = self.environment.names.get(&name) {
                    let ty = Type::Named(id.clone(), vec![]);
                    self.expand(&ty, &mut BTreeSet::new())
                        .unwrap_or(Type::Invalid)
                } else if implicit {
                    Type::Parameter(name)
                } else {
                    self.error(
                        node,
                        "E_UNKNOWN_TYPE",
                        format!("Type `{name}` is not defined."),
                        "Declare or import the type.",
                    );
                    Type::Invalid
                }
            }
            K::TypeApplication => {
                let head = self.annotation(&node.children[0], implicit);
                let args = node
                    .children
                    .iter()
                    .skip(1)
                    .map(|n| self.annotation(n, implicit))
                    .collect::<Vec<_>>();
                match head {
                    Type::Named(name, _) => {
                        let arity = self
                            .environment
                            .definitions
                            .get(&name)
                            .map(|d| d.parameters.len())
                            .or_else(|| builtin_arity(&name));
                        if arity.is_some_and(|n| n != args.len()) {
                            self.error(
                                node,
                                "E_TYPE_ARITY",
                                format!(
                                    "Type `{name}` expects {} type arguments, found {}.",
                                    arity.unwrap(),
                                    args.len()
                                ),
                                "Supply one argument for each type parameter.",
                            );
                        }
                        if matches!(name.as_str(), "Tensor" | "DynTensor")
                            && args.first() != Some(&Type::named("double"))
                        {
                            self.error(
                                node,
                                "E_TENSOR_ELEMENT",
                                "Phase 1 tensors hold only double values.",
                                "Use double as the element type.",
                            );
                        }
                        Type::Named(name, args)
                    }
                    Type::Invalid => Type::Invalid,
                    _ => {
                        self.error(
                            node,
                            "E_UNKNOWN_TYPE",
                            "Only a declared type can take bracket arguments.",
                            "Declare or import the generic type.",
                        );
                        Type::Invalid
                    }
                }
            }
            K::ShapeType => Type::Tuple(
                node.children
                    .iter()
                    .map(|n| {
                        self.index(n)
                            .map(|i| Type::Indexed(Box::new(Type::named("Nat")), i))
                            .unwrap_or(Type::Invalid)
                    })
                    .collect(),
            ),
            K::UnionType => Type::union(
                self.annotation(&node.children[0], implicit),
                self.annotation(&node.children[1], implicit),
            ),
            K::BorrowType => Type::Borrow(Box::new(self.annotation(&node.children[0], implicit))),
            K::MutableType => {
                let mut ty = self.annotation(&node.children[0], implicit);
                if let Type::Function(function) = &mut ty {
                    function.receiver_mutates = true;
                } else if ty != Type::Invalid {
                    self.error(node, "E_MUT_FUNCTION_TYPE", "Only a function type can carry captured-state mutation permission.", "Write mut (Int -> Int), or put mut before a parameter name to grant a mutable borrow.");
                    return Type::Invalid;
                }
                ty
            }
            K::DynType => {
                let requirement = self.annotation(&node.children[0], implicit);
                self.require_trait(node, &requirement);
                if !self.object_safe(&requirement) {
                    self.error(node, "E_DYN_OBJECT_SAFETY", format!("{requirement} has methods that need the hidden concrete type or generic method parameters."), "Use a generic trait bound, or a trait whose methods have fixed callable signatures.");
                }
                Type::Dynamic(Box::new(requirement))
            }
            // Parentheses around one type only group it.
            K::TupleType if node.children.len() == 1 => {
                self.annotation(&node.children[0], implicit)
            }
            K::TupleType => Type::Tuple(
                node.children
                    .iter()
                    .map(|n| self.annotation(n, implicit))
                    .collect(),
            ),
            K::FunctionType => {
                let mut input = &node.children[0];
                while input.kind == K::TupleType
                    && input.children.len() == 1
                    && input.children[0].kind == K::TupleType
                {
                    input = &input.children[0];
                }
                if input.kind == K::TupleType
                    && input
                        .children
                        .iter()
                        .any(|child| child.kind == K::Parameter)
                {
                    let mut names = BTreeSet::new();
                    let mut parameters = vec![];
                    for parameter in &input.children {
                        let (name, passing, mut ty) = if parameter.kind == K::Parameter {
                            let name = self.named_child(parameter);
                            self.naming(parameter, &name, "value identifier");
                            if !names.insert(name.clone()) {
                                self.error(
                                    parameter,
                                    "E_DUPLICATE_NAME",
                                    format!("Parameter `{name}` is repeated."),
                                    "Give each parameter a different name.",
                                );
                            }
                            let ty = self.annotation(parameter.children.last().unwrap(), implicit);
                            let mode = if self.modifier(parameter, "mut") {
                                Passing::Mut
                            } else if self.modifier(parameter, "move") {
                                Passing::Move
                            } else {
                                Passing::Borrow
                            };
                            (Some(name), mode, ty)
                        } else {
                            (None, Passing::Borrow, self.annotation(parameter, implicit))
                        };
                        if passing == Passing::Mut
                            && let Type::Function(function) = &mut ty
                        {
                            function.receiver_mutates = true;
                        }
                        parameters.push(Parameter {
                            name,
                            passing,
                            ty,
                            default: false,
                        });
                    }
                    let Type::Function(mut function) =
                        Type::function(vec![], self.annotation(&node.children[1], implicit))
                    else {
                        unreachable!()
                    };
                    function.parameters = parameters;
                    function.effects = node.child(K::Effects).is_some();
                    return Type::Function(function);
                }
                let argument = self.annotation(&node.children[0], implicit);
                let arguments = if let Type::Tuple(parts) = argument {
                    parts
                } else {
                    vec![argument]
                };
                let mut function =
                    Type::function(arguments, self.annotation(&node.children[1], implicit));
                if node.child(K::Effects).is_some()
                    && let Type::Function(function) = &mut function
                {
                    function.effects = true;
                }
                function
            }
            K::Parameter => {
                self.error(
                    node,
                    "E_FUNCTION_TYPE_PARAMETER",
                    "A named type input needs a function arrow.",
                    "Write (name : Type) -> Result; tuple types contain only types.",
                );
                Type::Invalid
            }
            K::IndexedType => {
                let base = self.annotation(&node.children[0], implicit);
                if node
                    .children
                    .iter()
                    .skip(1)
                    .any(|child| child.kind == K::AssociatedType)
                {
                    self.require_trait(node, &base);
                    let mut bindings = BTreeMap::new();
                    for child in node.children.iter().skip(1) {
                        if child.kind != K::AssociatedType || child.children.len() != 2 {
                            self.error(child, "E_ASSOCIATED_TYPE", "A trait binding needs Name = Type.", "Keep length indices and associated types in separate applications.");
                            continue;
                        }
                        let name = self.text(&child.children[0]).to_owned();
                        let value = self.annotation(&child.children[1], implicit);
                        if bindings.insert(name, value).is_some() {
                            self.error(
                                child,
                                "E_ASSOCIATED_TYPE",
                                "This associated type is bound twice.",
                                "Bind each associated type once.",
                            );
                        }
                    }
                    let allowed: BTreeSet<String> = match base_name(&base) {
                        Some(name) if operator_trait(name) || unary_trait(name).is_some() => {
                            ["Output"].into_iter().map(str::to_owned).collect()
                        }
                        Some("Iterator") | Some("Sortable") => {
                            ["Item"].into_iter().map(str::to_owned).collect()
                        }
                        Some("Iterable") => ["Item", "Iterator"]
                            .into_iter()
                            .map(str::to_owned)
                            .collect(),
                        Some(name) => self
                            .environment
                            .definitions
                            .get(name)
                            .map(|definition| definition.associated_types.clone())
                            .unwrap_or_default(),
                        _ => BTreeSet::new(),
                    };
                    if bindings.keys().any(|name| !allowed.contains(name)) {
                        self.error(node, "E_ASSOCIATED_TYPE", format!("{base} does not declare one of these associated types."), "Bind a type declared by this trait, or a standard protocol's documented associated type.");
                    }
                    return Type::Associated(Box::new(base), bindings);
                }
                if node.children.len() != 2 {
                    self.error(
                        node,
                        "E_INDEX_COMPLEX",
                        "Only one length index is supported.",
                        "Use the specified Vec[T](n) form.",
                    );
                    return Type::Invalid;
                }
                match self.index(&node.children[1]) {
                    Some(index) => Type::Indexed(Box::new(base), index),
                    None => Type::Invalid,
                }
            }
            _ => {
                self.error(
                    node,
                    "E_UNSUPPORTED_CHECK",
                    "This type form is not implemented by the checker yet.",
                    "The parser retains this form, but checking cannot accept it yet.",
                );
                Type::Invalid
            }
        }
    }
    fn index(&mut self, node: &Node) -> Option<Index> {
        match node.kind {
            K::Name => Some(if self.rigid_indices.contains(self.text(node)) {
                Index::Rigid(self.text(node).into())
            } else {
                Index::Variable(self.text(node).into())
            }),
            K::Literal => {
                let text = self.text(node).replace('_', "");
                let parsed = if let Some(hex) = text.strip_prefix("0x") {
                    u64::from_str_radix(hex, 16)
                } else {
                    text.parse()
                };
                match parsed {
                    Ok(number) => Some(Index::Number(number)),
                    Err(_) => {
                        self.error(node, "E_INDEX_RANGE", "This length exceeds the checker's supported integer range.", "Use a representable length; arbitrary-size index arithmetic is not implemented yet.");
                        None
                    }
                }
            }
            K::Binary if self.text(&node.children[1]) == "+" => {
                let a = self.index(&node.children[0])?;
                let b = self.index(&node.children[2])?;
                if let (Index::Number(a), Index::Number(b)) = (&a, &b) {
                    match a.checked_add(*b) {
                        Some(sum) => Some(Index::Number(sum)),
                        None => {
                            self.error(node, "E_INDEX_RANGE", "This index sum exceeds the checker's supported integer range.", "Use a representable length; arbitrary-size index arithmetic is not implemented yet.");
                            None
                        }
                    }
                } else {
                    let index = Index::Add(Box::new(a), Box::new(b));
                    if index.representable() {
                        Some(index)
                    } else {
                        self.error(
                            node,
                            "E_INDEX_RANGE",
                            "This length sum exceeds the checker's supported integer range.",
                            "Use representable constants and coefficients.",
                        );
                        None
                    }
                }
            }
            K::Binary if self.text(&node.children[1]) == "*" => {
                let a = self.index(&node.children[0])?;
                let b = self.index(&node.children[2])?;
                let (factor, index) = match (a, b) {
                    (Index::Number(factor), index) | (index, Index::Number(factor)) => {
                        (factor, index)
                    }
                    _ => {
                        self.error(
                            node,
                            "E_INDEX_COMPLEX",
                            "Length multiplication needs a constant number on one side.",
                            "Write 2 * n or n * 3; products of two length names are not supported.",
                        );
                        return None;
                    }
                };
                match Index::scaled(factor, index) {
                    Some(index) => Some(index),
                    None => {
                        self.error(
                            node,
                            "E_INDEX_RANGE",
                            "This length product exceeds the checker's supported integer range.",
                            "Use representable constants and coefficients.",
                        );
                        None
                    }
                }
            }
            K::Group => self.index(&node.children[0]),
            _ => {
                self.error(
                    node,
                    "E_INDEX_COMPLEX",
                    "A length index may contain numbers, names, addition and multiplication by a constant.",
                    "Write an additive index or a fixed multiple such as 2 * n; broader arithmetic refinements are not implemented yet.",
                );
                None
            }
        }
    }
}

impl Checker<'_> {
    fn check_declarations(&mut self, nodes: &[Node]) {
        for node in nodes {
            self.check_attributes(node, true);
            self.check_declaration_modifiers(node);
            match node.kind {
                K::Function => {
                    let signature = self.signature(node);
                    self.function_body(node, &signature, None);
                }
                K::OperatorDeclaration => {
                    let name = node
                        .children
                        .last()
                        .map(|name| crate::lexer::path_name(self.text(name)))
                        .unwrap_or_default();
                    let binary =
                        self.environment
                            .values
                            .get(&name)
                            .is_some_and(|symbol| match &symbol.ty {
                                Type::Function(function) => function.parameters.len() == 2,
                                Type::Overloaded(functions) => functions
                                    .iter()
                                    .any(|function| function.parameters.len() == 2),
                                _ => false,
                            });
                    if !binary {
                        self.error(
                            node,
                            "E_OPERATOR_FUNCTION",
                            format!("`{name}` is not a function of two parameters."),
                            "An infix operator needs a visible function with two parameters.",
                        );
                    }
                }
                K::Class | K::Trait => {
                    let id = self.qualified(&self.named_child(node));
                    let Some(definition) = self.environment.definitions.get(&id).cloned() else {
                        continue;
                    };
                    // `Self` in a member signature is this class, or the marker in a trait.
                    let previous_self = self.self_type.replace(if definition.kind == K::Trait {
                        crate::types::self_type()
                    } else {
                        Type::Named(
                            id.clone(),
                            definition
                                .parameters
                                .iter()
                                .map(|name| Type::Parameter(name.clone()))
                                .collect(),
                        )
                    });
                    let previous_bounds =
                        std::mem::replace(&mut self.bounds, definition.bounds.clone());
                    self.validate_type(
                        node,
                        &Type::Named(
                            id.clone(),
                            definition
                                .parameters
                                .iter()
                                .map(|name| Type::Parameter(name.clone()))
                                .collect(),
                        ),
                    );
                    if let Some(parent) = &definition.parent {
                        self.validate_type(node, parent);
                    }
                    self.bounds = previous_bounds;
                    self.check_inheritance(node, &id, &definition);
                    self.check_traits(node, &id, &definition);
                    self.field_initializers(node, &id, &definition);
                    for member in &node.children {
                        if member.kind == K::Binding && self.modifier(member, "static") {
                            self.check_static_initializer(member, &definition);
                        }
                    }
                    for member in &node.children {
                        self.check_attributes(member, false);
                        self.check_member_modifiers(member, definition.kind == K::Trait);
                        if member.kind == K::Invariant
                            && definition.kind == K::Class
                            && !definition.record
                        {
                            self.check_invariant(member, &id, &definition);
                        }
                        if matches!(member.kind, K::Function | K::Constructor) {
                            let signature = self.signature(member);
                            self.function_body(member, &signature, Some(id.clone()));
                        }
                    }
                    self.self_type = previous_self;
                }
                K::Const => {
                    let name = binding_name(node, self.source);
                    let expected = self
                        .environment
                        .values
                        .get(&name)
                        .map(|s| s.ty.clone())
                        .unwrap_or(Type::Invalid);
                    let pattern = node.children.iter().find(|n| {
                        matches!(n.kind, K::PatternName | K::PatternConstructor | K::Name)
                    });
                    if let Some(value) = node.children.iter().rev().find(|n| {
                        !is_type(n.kind)
                            && !matches!(n.kind, K::Modifier | K::Attribute)
                            && !pattern.is_some_and(|pattern| std::ptr::eq(*n, pattern))
                    }) {
                        let old_effects = (self.effects_allowed, self.effects_observed);
                        self.effects_allowed = false;
                        let old_constant = std::mem::replace(&mut self.constant_initializer, true);
                        let found = self.infer(value, Some(&expected));
                        self.constant_initializer = old_constant;
                        self.require(value, &expected, &found);
                        (self.effects_allowed, self.effects_observed) = old_effects;
                        let ty = self.inference.resolve(&expected);
                        if ty != Type::Invalid && !self.immutable_type(&ty, &mut BTreeSet::new()) {
                            self.error(
                                value,
                                "E_CONST_TYPE",
                                format!("Constant `{name}` has mutable type {ty}."),
                                "Use an immutable type, such as Int, String, a List of immutable values, or a class whose fields are final.",
                            );
                        }
                    }
                }
                K::Alias => {
                    let ty = Type::named(&self.qualified(&self.named_child(node)));
                    if self.expand(&ty, &mut BTreeSet::new()).is_none() {
                        self.error(
                            node,
                            "E_TYPE_CYCLE",
                            "This alias is cyclic or exceeds the expansion limit.",
                            "Make aliases end in a concrete type and split long chains.",
                        );
                    }
                }
                K::Theorem | K::Axiom | K::Example => {
                    if let Some(name) = node.child(K::Name) {
                        let name = self.text(name).to_owned();
                        self.naming(node, &name, "camelCase");
                    }
                    let mut names = BTreeSet::new();
                    if let Some(parameters) = node.child(K::TypeParameters) {
                        for parameter in &parameters.children {
                            let name = self.named_child(parameter);
                            self.naming(parameter, &name, "PascalCase");
                            if !names.insert(name.clone()) {
                                self.error(
                                    parameter,
                                    "E_DUPLICATE_NAME",
                                    format!("Type parameter `{name}` is repeated."),
                                    "Declare each type parameter once.",
                                );
                            }
                        }
                    }
                    let bounds = self.parameter_bounds(node);
                    self.theorem_bounds.insert(node.span, bounds.clone());
                    let previous = std::mem::replace(&mut self.bounds, bounds);
                    if let Some(parameters) = node.child(K::ParameterList) {
                        for parameter in &parameters.children {
                            if parameter.children.iter().any(|n| n.kind == K::Proposition) {
                                // A hypothesis such as `h : x ∈ A` is part of
                                // the theorem's assumptions, not a data parameter.
                                continue;
                            }
                            if let Some(ty) = parameter.children.iter().find(|n| is_type(n.kind)) {
                                // Type variables are implicit, as in function signatures.
                                let ty = self.annotation(ty, true);
                                self.validate_type(parameter, &ty);
                                self.binding_types.insert(parameter.span, ty);
                            } else {
                                self.error(
                                    parameter,
                                    "E_PROOF_PARAMETER",
                                    "A theorem parameter needs a type.",
                                    "Write `n : Nat`.",
                                );
                            }
                        }
                    }
                    self.bounds = previous;
                }
                _ => {}
            }
        }
    }

    fn expand(&self, ty: &Type, visiting: &mut BTreeSet<String>) -> Option<Type> {
        if visiting.len() >= 96 {
            return None;
        }
        if let Type::Named(name, args) = ty
            && let Some(definition) = self.environment.definitions.get(name)
            && let Some(alias) = &definition.alias
        {
            if !visiting.insert(name.clone()) {
                return None;
            }
            let replacements = definition
                .parameters
                .iter()
                .cloned()
                .zip(args.iter().cloned())
                .collect();
            let expanded = self.expand(&alias.replace_parameters(&replacements), visiting);
            visiting.remove(name);
            return expanded;
        }
        let mut failed = false;
        let mapped = ty.map(&mut |part| {
            if let Type::Named(name, _) = part
                && self
                    .environment
                    .definitions
                    .get(name)
                    .is_some_and(|d| d.alias.is_some())
            {
                return Some(self.expand(part, visiting).unwrap_or_else(|| {
                    failed = true;
                    Type::Invalid
                }));
            }
            None
        });
        if failed { None } else { Some(mapped) }
    }

    /// Type-check a static final initializer in a pure, receiver-free scope.
    /// It runs lazily on first access, so it may read constants and other
    /// static finals but no mutable state.
    fn check_static_initializer(&mut self, member: &Node, definition: &Definition) {
        let name = self.named_child(member);
        let Some(ty) = definition.static_fields.get(&name).cloned() else {
            return;
        };
        let name_node = member.child(K::Name);
        let Some(initializer) = member.children.iter().rev().find(|n| {
            !is_type(n.kind)
                && !matches!(n.kind, K::Modifier | K::Attribute)
                && !name_node.is_some_and(|name_node| std::ptr::eq(*n, name_node))
        }) else {
            return;
        };
        let old_effects = (self.effects_allowed, self.effects_observed);
        self.effects_allowed = false;
        let old_constant = std::mem::replace(&mut self.constant_initializer, true);
        let old_receiver = std::mem::replace(&mut self.receiver_mutable, false);
        let found = self.infer(initializer, Some(&ty));
        self.require(initializer, &ty, &found);
        self.receiver_mutable = old_receiver;
        self.constant_initializer = old_constant;
        (self.effects_allowed, self.effects_observed) = old_effects;
    }

    fn field_initializers(&mut self, node: &Node, id: &str, definition: &Definition) {
        let previous_owner = self.owner.replace(id.into());
        let previous_bounds = std::mem::replace(&mut self.bounds, definition.bounds.clone());
        let previous_rigid = self.rigid_parameters.clone();
        self.rigid_parameters
            .extend(definition.parameters.iter().cloned());
        let old_effects = (self.effects_allowed, self.effects_observed);
        self.effects_allowed = false;
        self.scopes.push(BTreeMap::new());
        for field in node.children.iter().filter(|n| n.kind == K::Binding) {
            let Some(annotation) = field.children.iter().find(|n| is_type(n.kind)) else {
                continue;
            };
            let name = self.named_child(field);
            let Some(expected) = definition.fields.get(&name) else {
                continue;
            };
            let expected = expected.map(&mut |ty| {
                if let Type::Parameter(name) = ty {
                    Some(Type::Rigid(name.clone()))
                } else {
                    None
                }
            });
            self.validate_type(field, &expected);
            let Some(initializer) = field
                .children
                .last()
                .filter(|n| n.span.start >= annotation.span.end)
            else {
                continue;
            };
            let found = self.infer(initializer, Some(&expected));
            self.require(initializer, &expected, &found);
        }
        self.scopes.pop();
        self.owner = previous_owner;
        (self.effects_allowed, self.effects_observed) = old_effects;
        self.bounds = previous_bounds;
        self.rigid_parameters = previous_rigid;
    }

    /// Checks one `invariant` clause in its class's scope: a pure, read-only Bool
    /// that may read `this`, read fields and call read-only methods.
    fn check_invariant(&mut self, member: &Node, id: &str, definition: &Definition) {
        let Some(expression) = member.children.first() else {
            return;
        };
        let instance = Type::Named(
            id.to_owned(),
            definition
                .parameters
                .iter()
                .map(|name| Type::Parameter(name.clone()))
                .collect(),
        );
        let previous_owner = self.owner.replace(id.into());
        let previous_bounds = std::mem::replace(&mut self.bounds, definition.bounds.clone());
        let previous_return = self.return_type.take();
        let old_effects = (self.effects_allowed, self.effects_observed);
        let previous_receiver = std::mem::replace(&mut self.receiver_mutable, false);
        let previous_parameters = std::mem::take(&mut self.borrowed_parameters);
        self.effects_allowed = false;
        self.scopes.push(BTreeMap::new());
        let receiver = self.symbol(instance, Visibility::Private);
        self.scopes
            .last_mut()
            .unwrap()
            .insert("this".into(), receiver);
        let found = self.infer(expression, Some(&Type::named("Bool")));
        self.require(expression, &Type::named("Bool"), &found);
        for child in expression.descendants() {
            if matches!(
                child.kind,
                K::Return | K::Propagate | K::Break | K::Continue
            ) {
                self.error(
                    child,
                    "E_INVARIANT_CONTROL",
                    "An invariant cannot leave its class or propagate an Error.",
                    "Use a pure Bool expression that reads this and finishes the check.",
                );
            }
            if matches!(child.kind, K::Call | K::Pipe)
                && let Some(call) = self.resolved_calls.get(&child.span)
                && (call.signature.receiver_mutates
                    || call
                        .signature
                        .parameters
                        .iter()
                        .any(|parameter| parameter.passing == Passing::Mut))
            {
                self.error(
                    child,
                    "E_INVARIANT_MUT",
                    "An invariant calls only read-only methods.",
                    "Call a method that leaves its receiver and arguments unchanged.",
                );
            }
        }
        self.scopes.pop();
        self.owner = previous_owner;
        self.bounds = previous_bounds;
        self.return_type = previous_return;
        (self.effects_allowed, self.effects_observed) = old_effects;
        self.receiver_mutable = previous_receiver;
        self.borrowed_parameters = previous_parameters;
    }

    fn uninitialized_fields(&self) -> Vec<String> {
        let Some(initialized) = &self.initializing_fields else {
            return vec![];
        };
        let Some(definition) = self
            .owner
            .as_ref()
            .and_then(|id| self.environment.definitions.get(id))
        else {
            return vec![];
        };
        definition
            .fields
            .keys()
            .filter(|name| !initialized.contains(*name))
            .cloned()
            .collect()
    }

    fn ensure_initialized(&mut self, node: &Node) {
        let missing = self.uninitialized_fields();
        if !missing.is_empty() {
            self.error(
                node,
                "E_FIELD_UNINITIALIZED",
                format!(
                    "Constructor can finish before initializing {}.",
                    missing.join(", ")
                ),
                "Initialize every field on every path that finishes construction.",
            );
        }
    }

    fn require_initialized_this(&mut self, node: &Node) {
        let missing = self.uninitialized_fields();
        if !missing.is_empty() {
            self.error(node, "E_THIS_UNINITIALIZED", format!("This object still has uninitialized fields: {}.", missing.join(", ")), "Initialize every field before passing this, taking a method value, calling its methods or capturing it. Direct field reads may use fields already initialized.");
        }
    }

    fn validate_borrow_position(&mut self, node: &Node, ty: &Type, stored: bool) {
        let ty = self
            .expand(ty, &mut BTreeSet::new())
            .unwrap_or(Type::Invalid);
        match &ty {
            Type::Borrow(inner) => {
                if !stored && !matches!(inner.as_ref(), Type::Dynamic(_)) {
                    self.error(node, "E_BORROW_POSITION", "An ordinary &T borrow belongs in a stored field or payload type.", "Parameters borrow by default: write T. Explicit &dyn Trait views are allowed.");
                }
                self.validate_borrow_position(node, inner, stored);
            }
            Type::Named(name, args) => {
                let payload = matches!(
                    name.as_str(),
                    "Option"
                        | "List"
                        | "Array"
                        | "Map"
                        | "MutMap"
                        | "Set"
                        | "MutSet"
                        | "SortedSet"
                        | "Box"
                ) || self
                    .environment
                    .definitions
                    .get(name)
                    .is_some_and(|definition| matches!(definition.kind, K::Class | K::Enum));
                for arg in args {
                    self.validate_borrow_position(node, arg, payload);
                }
            }
            Type::Tuple(fields) => {
                for field in fields {
                    self.validate_borrow_position(node, field, true);
                }
            }
            Type::Function(function) => {
                for parameter in &function.parameters {
                    self.validate_borrow_position(node, &parameter.ty, false);
                }
                self.validate_borrow_position(node, &function.result, false);
            }
            Type::Union(parts) => {
                for part in parts {
                    self.validate_borrow_position(node, part, stored);
                }
            }
            Type::Indexed(inner, _) | Type::Dynamic(inner) => {
                self.validate_borrow_position(node, inner, stored)
            }
            _ => {}
        }
    }

    fn validate_type(&mut self, node: &Node, ty: &Type) {
        let expanded = self
            .expand(ty, &mut BTreeSet::new())
            .unwrap_or(Type::Invalid);
        let mut obligations = vec![];
        expanded.map(&mut |part| {
            if let Type::Named(name, args) = part {
                let definition = self.environment.definitions.get(name).cloned();
                let arity = definition
                    .as_ref()
                    .map(|d| d.parameters.len())
                    .or_else(|| builtin_arity(name));
                if arity.is_some_and(|n| n != args.len()) {
                    self.error(
                        node,
                        "E_TYPE_ARITY",
                        format!(
                            "Type `{name}` expects {} type arguments, found {}.",
                            arity.unwrap(),
                            args.len()
                        ),
                        "Write the complete type, including bracket arguments.",
                    );
                }
                if let Some(definition) = definition {
                    let substitutions = definition
                        .parameters
                        .iter()
                        .cloned()
                        .zip(args.iter().cloned())
                        .collect();
                    for (name, ty) in &definition.fields {
                        let ty = ty.replace_parameters(&substitutions);
                        if !definition.mutable_fields.contains(name) && !self.immutable_type(&ty, &mut BTreeSet::new()) {
                            self.error(node, "E_FINAL_TYPE", format!("Final field `{name}` has mutable type {ty}."), "Use var for a mutable field; Array and mutable collections cannot be final.");
                        }
                    }
                    for (parameter, argument) in definition.parameters.iter().zip(args) {
                        for requirement in definition.bounds.get(parameter).into_iter().flatten() {
                            obligations.push((
                                argument.clone(),
                                requirement.replace_parameters(&substitutions),
                            ));
                        }
                    }
                }
            }
            None
        });
        self.check_obligations(node, &obligations);
    }

    fn methods(&self, ty: &Type, name: &str, seen: &mut BTreeSet<String>) -> Vec<FunctionType> {
        self.methods_with_access(ty, name, seen, true)
    }
    fn methods_with_access(
        &self,
        ty: &Type,
        name: &str,
        seen: &mut BTreeSet<String>,
        check_access: bool,
    ) -> Vec<FunctionType> {
        if seen.len() >= 96 {
            return vec![];
        }
        let ty = match ty {
            Type::Borrow(inner) | Type::Dynamic(inner) => inner.as_ref(),
            _ => ty,
        };
        if let Type::Associated(base, bindings) = ty {
            return self
                .methods_with_access(base, name, seen, check_access)
                .into_iter()
                .map(|method| {
                    let Type::Function(mut method) =
                        Type::Function(method).replace_parameters(bindings)
                    else {
                        unreachable!()
                    };
                    method.receiver = Some(Box::new(ty.clone()));
                    method
                })
                .collect();
        }
        let Type::Named(id, args) = ty else {
            return vec![];
        };
        if !seen.insert(id.clone()) {
            return vec![];
        }
        let Some(definition) = self.environment.definitions.get(id) else {
            return vec![];
        };
        let substitutions = definition
            .parameters
            .iter()
            .cloned()
            .zip(args.iter().cloned())
            .chain(
                // `Self` in a class method is the class that declares it, even
                // when a subclass uses the inherited method.
                (definition.kind == K::Class).then(|| ("Self".to_owned(), ty.clone())),
            )
            .collect();
        let own = if !check_access || self.can_access(id, definition, name) {
            definition.methods.get(name).cloned().unwrap_or_default()
        } else {
            vec![]
        };
        let mut methods: Vec<_> = own
            .into_iter()
            .map(
                |f| match Type::Function(f).replace_parameters(&substitutions) {
                    Type::Function(mut f) => {
                        if !definition.static_methods.contains(name)
                            && !matches!(
                                f.callable,
                                Some(Callable::Declaration {
                                    kind: DeclarationKind::StaticMethod,
                                    ..
                                })
                            )
                        {
                            f.receiver = Some(Box::new(ty.clone()));
                        }
                        f
                    }
                    _ => unreachable!(),
                },
            )
            .collect();
        if let Some(parent) = &definition.parent {
            for inherited in self.methods_with_access(
                &parent.replace_parameters(&substitutions),
                name,
                seen,
                check_access,
            ) {
                if !methods.iter().any(|own| own.same_parameters(&inherited)) {
                    methods.push(inherited);
                }
            }
        }
        // A trait's `has` list adds the methods of the traits it names, so `trait Expr { has Text }`
        // gives an Expr value the text() method.
        if definition.kind == K::Trait {
            for declared in &definition.traits {
                for inherited in self.methods_with_access(
                    &declared.replace_parameters(&substitutions),
                    name,
                    seen,
                    check_access,
                ) {
                    if !methods.iter().any(|own| own.same_parameters(&inherited)) {
                        methods.push(inherited);
                    }
                }
            }
        }
        methods
    }

    /// The class that declares `name` and whether it declares it static,
    /// looking through inherited classes. A name is either static or instance
    /// there, so the per-name set is exact.
    fn method_declaring_class(
        &self,
        ty: &Type,
        name: &str,
        parameters: &FunctionType,
    ) -> Option<(String, bool)> {
        let mut current = ty.clone();
        for _ in 0..96 {
            let Type::Named(id, args) = current else {
                return None;
            };
            let definition = self.environment.definitions.get(&id)?;
            if definition
                .methods
                .get(name)
                .is_some_and(|methods| methods.iter().any(|m| m.same_parameters(parameters)))
            {
                return Some((id.clone(), definition.static_methods.contains(name)));
            }
            let substitutions = definition
                .parameters
                .iter()
                .cloned()
                .zip(args.iter().cloned())
                .collect::<BTreeMap<_, _>>();
            current = definition
                .parent
                .as_ref()?
                .replace_parameters(&substitutions);
        }
        None
    }

    fn field(
        &self,
        ty: &Type,
        name: &str,
        seen: &mut BTreeSet<String>,
    ) -> Option<(String, Type, bool)> {
        let ty = self.inference.resolve(ty);
        if let Type::Borrow(inner) = &ty {
            return self.field(inner, name, seen);
        }
        let Type::Named(id, args) = &ty else {
            return None;
        };
        if !seen.insert(id.clone()) || seen.len() > 96 {
            return None;
        }
        let definition = self.environment.definitions.get(id)?;
        let substitutions = definition
            .parameters
            .iter()
            .cloned()
            .zip(args.iter().cloned())
            .collect();
        if let Some(field) = definition.fields.get(name) {
            return Some((
                id.clone(),
                field.replace_parameters(&substitutions),
                definition.mutable_fields.contains(name),
            ));
        }
        definition
            .parent
            .as_ref()
            .and_then(|parent| self.field(&parent.replace_parameters(&substitutions), name, seen))
    }

    fn can_access(&self, id: &str, definition: &Definition, name: &str) -> bool {
        definition.kind == K::Trait
            || self.owner.as_deref() == Some(id)
            || match definition.member_visibility.get(name) {
                Some(Visibility::Public) => true,
                Some(Visibility::Package) => {
                    std::path::Path::new(&definition.file).parent()
                        == std::path::Path::new(&self.source.name).parent()
                }
                _ => false,
            }
    }

    fn check_inheritance(&mut self, node: &Node, id: &str, definition: &Definition) {
        let mut seen = BTreeSet::from([id.to_owned()]);
        let mut parent = definition.parent.clone();
        while let Some(Type::Named(name, _)) = parent {
            if !seen.insert(name.clone()) {
                self.error(
                    node,
                    "E_INHERITANCE_CYCLE",
                    "This class inherits from itself through its parents.",
                    "Remove the cycle in extends declarations.",
                );
                return;
            }
            if name == "Object" {
                break;
            }
            match self.environment.definitions.get(&name) {
                Some(parent_definition) if parent_definition.kind == K::Class => {
                    parent = parent_definition.parent.clone()
                }
                _ => {
                    self.error(
                        node,
                        "E_SUPERCLASS",
                        format!("`{name}` is not a class."),
                        "A class may extend one class.",
                    );
                    return;
                }
            }
        }
        for member in node.children.iter().filter(|n| n.kind == K::Function) {
            let name = self.named_child(member);
            let signature = self.signature(member);
            let inherited = definition
                .parent
                .as_ref()
                .map(|ty| self.methods(ty, &name, &mut BTreeSet::new()))
                .unwrap_or_default();
            if let Some(parent) = inherited.iter().find(|p| p.same_parameters(&signature)) {
                if let Some(parent_type) = &definition.parent
                    && let Some((declaring, parent_static)) =
                        self.method_declaring_class(parent_type, &name, &signature)
                {
                    if parent_static != self.modifier(member, "static") {
                        self.error(
                            member,
                            "E_OVERRIDE_STATIC",
                            "An override must keep the parent method's static or instance form.",
                            "Match the parent method's declaration form.",
                        );
                    }
                    let parent_visibility = self
                        .environment
                        .definitions
                        .get(&declaring)
                        .and_then(|parent| parent.member_visibility.get(&name))
                        .copied()
                        .unwrap_or(Visibility::Private);
                    if self.visibility(member) < parent_visibility {
                        self.error(
                            member,
                            "E_OVERRIDE_VISIBILITY",
                            "An override cannot be less visible than the parent method.",
                            "Raise this method's visibility to the parent's, or leave the parent method alone.",
                        );
                    }
                }
                if parent.receiver_mutates != signature.receiver_mutates {
                    self.error(
                        member,
                        "E_OVERRIDE_RECEIVER",
                        "An override must keep the parent's receiver mode.",
                        "Keep mut on both declarations, or on neither.",
                    );
                }

                if !parent
                    .parameters
                    .iter()
                    .zip(&signature.parameters)
                    .all(|(a, b)| a.passing == b.passing)
                {
                    self.error(member, "E_OVERRIDE_PARAMETER_MODE", "An override cannot change whether a parameter borrows, mutates or takes ownership.", "Keep the parent's parameter modes; equal parameter types still identify an override.");
                }
                if signature.effects && !parent.effects {
                    self.error(
                        member,
                        "E_OVERRIDE_EFFECTS",
                        "An override cannot add outside effects that the parent's type does not declare.",
                        "Keep the override pure, or declare with Effects on the parent method too.",
                    );
                }
                if !parent.result.same_type(&signature.result) {
                    // A `Self` result may be narrowed to the subclass that
                    // overrides it; every other result must stay identical.
                    let narrowed = self.self_result_method(parent)
                        && self
                            .superclass_steps(&signature.result, &parent.result)
                            .is_some();
                    if !narrowed {
                        self.error(member, "E_OVERRIDE_RETURN", format!("`{name}` has the parent's parameter types but changes its return type from {} to {}.", parent.result, signature.result), "Keep the parent's return type. A different return type is not an overload.");
                    }
                } else if !self.overriding(member) {
                    self.error(
                        member,
                        "E_OVERRIDE_REQUIRED",
                        format!("`{name}` overrides an inherited method."),
                        "Mark this method `@override`.",
                    );
                }
            } else if self.overriding(member)
                && !self.satisfies_trait_method(id, definition, &name, &signature, false)
            {
                self.error(
                    member,
                    "E_OVERRIDE_MISSING",
                    format!("`{name}` has no inherited method with these parameter types."),
                    "Remove @override, or match a parent's parameter types exactly.",
                );
            }
        }
    }

    /// Whether a method implements a trait requirement of that name with the
    /// same parameter types, so `@override` may mark it. With `self_result`,
    /// only a requirement whose result is `Self` counts.
    fn satisfies_trait_method(
        &self,
        id: &str,
        definition: &Definition,
        name: &str,
        signature: &FunctionType,
        self_result: bool,
    ) -> bool {
        let instance = Type::Named(
            id.to_owned(),
            definition
                .parameters
                .iter()
                .map(|parameter| Type::Parameter(parameter.clone()))
                .collect(),
        );
        self.expanded_traits(&definition.traits)
            .iter()
            .any(|requirement| {
                let Some((trait_name, args, _)) = trait_parts(requirement) else {
                    return false;
                };
                let Some(trait_definition) = self.environment.definitions.get(trait_name) else {
                    return false;
                };
                let substitutions: BTreeMap<String, Type> = trait_definition
                    .parameters
                    .iter()
                    .cloned()
                    .zip(args.iter().cloned())
                    .chain(
                        (definition.kind == K::Class)
                            .then(|| ("Self".to_owned(), instance.clone())),
                    )
                    .collect();
                trait_definition.methods.get(name).is_some_and(|methods| {
                    methods.iter().any(|method| {
                        (!self_result || crate::types::is_self_type(&method.result))
                            && match Type::Function(method.clone())
                                .replace_parameters(&substitutions)
                            {
                                Type::Function(contract) => signature.same_parameters(&contract),
                                _ => false,
                            }
                    })
                })
            })
    }

    fn implements(&self, ty: &Type, requirement: &Type, seen: &mut BTreeSet<String>) -> bool {
        if matches!(ty, Type::Invalid) || matches!(requirement, Type::Invalid) {
            return true;
        }
        let Some((name, args, _)) = trait_parts(requirement) else {
            return false;
        };
        let Some(instance) = self.trait_instance(ty, name, args, seen) else {
            return false;
        };
        match_trait(&mut self.inference.clone(), &instance, requirement)
    }

    fn satisfy_trait(&mut self, ty: &Type, requirement: &Type) -> bool {
        let ty = self.inference.resolve(ty);
        let requirement = self.inference.resolve(requirement);
        if matches!(ty, Type::Invalid) || matches!(requirement, Type::Invalid) {
            return true;
        }
        let Some((name, args, _)) = trait_parts(&requirement) else {
            return false;
        };
        let Some(instance) = self.trait_instance(&ty, name, args, &mut BTreeSet::new()) else {
            return false;
        };
        match_trait(&mut self.inference, &instance, &requirement)
    }

    fn trait_output(&mut self, ty: &Type, name: &str, args: &[Type]) -> Option<Type> {
        let requirement = Type::applied(name, args.to_vec());
        let instance = self.trait_instance(ty, name, args, &mut BTreeSet::new())?;
        if !match_trait(&mut self.inference, &instance, &requirement) {
            return None;
        }
        trait_parts(&instance)
            .and_then(|(_, _, bindings)| bindings)
            .and_then(|bindings| bindings.get("Output").cloned())
    }

    /// The given traits and every trait they name in turn: `trait Expr { has Text }` makes an Expr
    /// also a Text, so a class that has Expr also has Text. Each trait appears once.
    fn expanded_traits(&self, traits: &[Type]) -> Vec<Type> {
        let mut result: Vec<Type> = Vec::new();
        let mut pending: Vec<Type> = traits.iter().rev().cloned().collect();
        while let Some(next) = pending.pop() {
            if result.len() >= 96 || result.iter().any(|known| known.same_type(&next)) {
                continue;
            }
            if let Some((name, args, _)) = trait_parts(&next)
                && let Some(definition) = self.environment.definitions.get(name)
                && definition.kind == K::Trait
            {
                let args: Vec<Type> = args.to_vec();
                let substitutions = definition
                    .parameters
                    .iter()
                    .cloned()
                    .zip(args.iter().cloned())
                    .collect();
                pending.extend(
                    definition
                        .traits
                        .iter()
                        .rev()
                        .map(|declared| declared.replace_parameters(&substitutions)),
                );
            }
            result.push(next);
        }
        result
    }

    fn trait_defaults(&self, concrete: &Type, traits: &[Type]) -> Vec<TraitDefault> {
        let mut defaults = vec![];
        for requirement in self.expanded_traits(traits) {
            let Some((name, arguments, explicit)) = trait_parts(&requirement) else {
                continue;
            };
            let Some(trait_definition) = self
                .environment
                .definitions
                .get(name)
                .filter(|definition| definition.kind == K::Trait)
            else {
                continue;
            };
            let mut substitutions = trait_definition
                .parameters
                .iter()
                .cloned()
                .zip(arguments.iter().cloned())
                .collect::<BTreeMap<_, _>>();
            if let Some(bindings) = explicit {
                substitutions.extend(bindings.clone());
            } else if !trait_definition.associated_types.is_empty()
                && let Ok(bindings) =
                    self.infer_associated(concrete, trait_definition, arguments, None)
            {
                substitutions.extend(bindings);
            }
            // A default body runs once per implementing class, so its `Self` is
            // that class.
            substitutions.insert("Self".to_owned(), concrete.clone());
            for (method_name, methods) in &trait_definition.methods {
                let is_static = trait_definition.static_methods.contains(method_name);
                for method in methods {
                    let Some(Callable::Declaration { id, .. }) = &method.callable else {
                        continue;
                    };
                    if id.file != trait_definition.file
                        || !trait_definition.default_methods.contains(&id.span)
                    {
                        continue;
                    }
                    let Type::Function(mut signature) =
                        Type::Function(method.clone()).replace_parameters(&substitutions)
                    else {
                        unreachable!()
                    };
                    signature.receiver = (!is_static).then(|| Box::new(concrete.clone()));
                    defaults.push(TraitDefault {
                        name: method_name.clone(),
                        signature,
                        declaration: id.clone(),
                        provider: name.to_owned(),
                        is_static,
                        visibility: trait_definition
                            .member_visibility
                            .get(method_name)
                            .copied()
                            .unwrap_or(Visibility::Private),
                    });
                }
            }
        }
        defaults
    }

    fn grouped_trait_defaults(&self, concrete: &Type, traits: &[Type]) -> Vec<Vec<TraitDefault>> {
        let mut groups: Vec<Vec<TraitDefault>> = vec![];
        for candidate in self.trait_defaults(concrete, traits) {
            if let Some(group) = groups.iter_mut().find(|group| {
                group[0].name == candidate.name
                    && group[0].signature.same_parameters(&candidate.signature)
            }) {
                if !group
                    .iter()
                    .any(|prior| prior.declaration == candidate.declaration)
                {
                    group.push(candidate);
                }
            } else {
                groups.push(vec![candidate]);
            }
        }
        groups
    }

    fn trait_instance(
        &self,
        ty: &Type,
        trait_name: &str,
        wanted: &[Type],
        seen: &mut BTreeSet<String>,
    ) -> Option<Type> {
        let ty = self.inference.resolve(ty);
        // A recursive enum or record assumes its own structural instance while its
        // payloads are checked, so `Json` holding a `List[Json]` still compares.
        if seen.contains(&format!("structural:{ty}:{trait_name}")) {
            return Some(Type::applied(trait_name, wanted.to_vec()));
        }
        if seen.len() >= 96 {
            return None;
        }
        if !seen.insert(format!("{ty}:{trait_name}")) {
            // Within a structural derivation, meeting the same query again closes a cycle
            // through the recursive type, as `List[Json]` inside `Json` does: assume it.
            let structural = seen.iter().any(|key| key.starts_with("structural:"));
            return structural.then(|| Type::applied(trait_name, wanted.to_vec()));
        }
        let plain = Type::applied(trait_name, wanted.to_vec());
        if let Type::Borrow(inner) = &ty {
            return self.trait_instance(inner, trait_name, wanted, seen);
        }
        if let Type::Dynamic(requirement) = &ty {
            return self
                .expanded_traits(std::slice::from_ref(requirement.as_ref()))
                .into_iter()
                .find(|found| base_name(found) == Some(trait_name));
        }
        if matches!(ty, Type::Associated(_, _)) && base_name(&ty) == Some(trait_name) {
            return Some(ty);
        }
        if let Type::Parameter(name) | Type::Rigid(name) = &ty {
            let bounds = self.expanded_traits(self.bounds.get(name)?);
            return bounds.into_iter().find(|bound| {
                trait_parts(bound).is_some_and(|(name, args, _)| {
                    name == trait_name
                        && self
                            .inference
                            .clone()
                            .unify(&Type::Tuple(args.to_vec()), &Type::Tuple(wanted.to_vec()))
                })
            });
        }
        if crate::types::is_variable(&ty)
            && matches!(
                trait_name,
                "Multipliable"
                    | "RightMultipliable"
                    | "Addable"
                    | "RightAddable"
                    | "Subtractable"
                    | "RightSubtractable"
                    | "Divisible"
                    | "RightDivisible"
            )
            && wanted.len() == 1
        {
            let output = if wanted[0] == Type::named("double")
                || wanted[0] == Type::named("Int")
                || crate::types::is_variable(&wanted[0])
            {
                Some(Type::named("Variable"))
            } else {
                None
            };
            return output.map(|output| associated(plain, [("Output", output)]));
        }
        if crate::types::is_variable(&ty) && trait_name == "Negatable" && wanted.is_empty() {
            return Some(associated(plain, [("Output", Type::named("Variable"))]));
        }
        if crate::types::is_variable(&ty)
            && trait_name == "Powerable"
            && wanted.len() == 1
            && (wanted[0] == Type::named("Int")
                || wanted[0] == Type::named("double")
                || crate::types::is_variable(&wanted[0]))
        {
            return Some(associated(plain, [("Output", Type::named("Variable"))]));
        }
        if crate::types::is_variable(&ty) && trait_name == "Text" && wanted.is_empty() {
            return Some(plain);
        }
        if crate::types::is_tensor(&ty)
            && matches!(
                trait_name,
                "Multipliable"
                    | "RightMultipliable"
                    | "Addable"
                    | "RightAddable"
                    | "Subtractable"
                    | "RightSubtractable"
                    | "Divisible"
            )
            && wanted.len() == 1
        {
            let output = if wanted[0] == Type::named("double") {
                Some(ty.clone())
            } else if matches!(trait_name, "Multipliable" | "Addable" | "Subtractable")
                && crate::types::is_tensor(&wanted[0])
            {
                self.tensor_equal_shape_type(&ty, &wanted[0])
            } else {
                None
            };
            return output.map(|output| associated(plain, [("Output", output)]));
        }
        if crate::types::is_tensor(&ty) && trait_name == "Negatable" && wanted.is_empty() {
            let output = match &ty {
                Type::StoredView(inner) | Type::Borrow(inner) if crate::types::is_tensor(inner) => {
                    inner.as_ref().clone()
                }
                _ => ty.clone(),
            };
            return Some(associated(plain, [("Output", output)]));
        }
        if crate::types::is_tensor(&ty)
            && trait_name == "Powerable"
            && wanted.len() == 1
            && (wanted[0] == Type::named("Int") || wanted[0] == Type::named("double"))
        {
            let output = match &ty {
                Type::StoredView(inner) | Type::Borrow(inner) if crate::types::is_tensor(inner) => {
                    inner.as_ref().clone()
                }
                _ => ty.clone(),
            };
            return Some(associated(plain, [("Output", output)]));
        }
        // `T : Copy` accepts the types whose values copy: numbers, Bool, String, List, Map,
        // Set and closures that change nothing (REVIEW2 section 4).
        if trait_name == "Copy" && wanted.is_empty() && crate::types::plain_copy(&ty) {
            return Some(plain);
        }
        // A tensor prints its entries, so it satisfies Text.
        if trait_name == "Text" && wanted.is_empty() && crate::types::is_tensor(&ty) {
            return Some(plain);
        }
        if trait_name == "Equals"
            && wanted.is_empty()
            && let Type::Named(name, args) = &ty
            && name == "SortedSet"
            && args.len() == 1
        {
            return self
                .implements(&args[0], &Type::named("Comparable"), &mut seen.clone())
                .then_some(plain);
        }
        if matches!(trait_name, "Equals" | "Comparable")
            && ty == Type::named("Bytes")
            && wanted.is_empty()
        {
            return Some(plain);
        }
        if matches!(trait_name, "Equals" | "Text") && wanted.is_empty() {
            let elements = match &ty {
                Type::Named(name, args)
                    if name == "ArenaRef" && args.len() == 1 && trait_name == "Equals" =>
                {
                    return Some(plain);
                }
                Type::Tuple(parts) => Some(parts.clone()),
                Type::Named(name, args)
                    if matches!(
                        name.as_str(),
                        "Int" | "double" | "Bool" | "String" | "BigInt" | "Nat"
                    ) && args.is_empty() =>
                {
                    return Some(plain);
                }
                // C boundary numbers print and compare as the numbers they hold.
                Type::Named(name, args)
                    if crate::interop::C_TYPES.contains(&name.as_str()) && args.is_empty() =>
                {
                    return Some(plain);
                }
                Type::Named(name, args)
                    if matches!(
                        name.as_str(),
                        "List" | "Set" | "SortedSet" | "Map" | "Option" | "Array"
                    ) =>
                {
                    Some(args.clone())
                }
                // Mutable collections print like their persistent forms (REVIEW2 section 2).
                Type::Named(name, args)
                    if trait_name == "Text" && matches!(name.as_str(), "MutMap" | "MutSet") =>
                {
                    Some(args.clone())
                }
                // An Error prints its case and message, as a failing script shows it.
                Type::Named(name, _) if trait_name == "Text" && self.is_error_type(&ty) => {
                    let _ = name;
                    return Some(plain);
                }
                _ => None,
            };
            if let Some(elements) = elements {
                return elements
                    .iter()
                    .all(|part| self.implements(part, &plain, &mut seen.clone()))
                    .then_some(plain);
            }
        }
        // Enums and records compare and print by their cases and fields when every
        // payload does, unless they declare the trait and write the method (REVIEW 2.2).
        if matches!(trait_name, "Equals" | "Text" | "Comparable")
            && wanted.is_empty()
            && let Type::Named(name, args) = &ty
            && let Some(definition) = self.environment.definitions.get(name)
            && (definition.kind == K::Enum && definition.parent.is_none() && name != "Nat"
                || definition.kind == K::Class && definition.record)
            && !self
                .expanded_traits(&definition.traits)
                .iter()
                .any(|declared| base_name(declared) == Some(trait_name))
        {
            let substitutions: BTreeMap<String, Type> = definition
                .parameters
                .iter()
                .cloned()
                .zip(args.iter().cloned())
                .collect();
            let parts: Vec<Type> = if definition.kind == K::Enum {
                definition
                    .variants
                    .iter()
                    .flat_map(|variant| variant.fields.iter().cloned())
                    .collect()
            } else {
                definition
                    .constructors
                    .first()
                    .map(|constructor| {
                        constructor
                            .parameters
                            .iter()
                            .map(|parameter| parameter.ty.clone())
                            .collect()
                    })
                    .unwrap_or_default()
            };
            seen.insert(format!("structural:{ty}:{trait_name}"));
            return parts
                .iter()
                .all(|part| {
                    self.implements(
                        &part.replace_parameters(&substitutions),
                        &plain,
                        &mut seen.clone(),
                    )
                })
                .then_some(plain);
        }
        // A tuple orders by its parts from left to right (REVIEW2 section 3).
        if trait_name == "Comparable"
            && wanted.is_empty()
            && let Type::Tuple(parts) = &ty
        {
            return parts
                .iter()
                .all(|part| self.implements(part, &plain, &mut seen.clone()))
                .then_some(plain);
        }
        if trait_name == "Comparable"
            && wanted.is_empty()
            && matches!(&ty,Type::Named(name,args) if args.is_empty() && matches!(name.as_str(),"Int" | "double" | "Bool" | "String" | "Nat" | "BigInt"))
        {
            return Some(plain);
        }
        // A List or Array is Sortable when its items are Comparable. Item is the item type (Q123).
        if trait_name == "Sortable"
            && wanted.is_empty()
            && let Type::Named(name, args) = &ty
            && matches!(name.as_str(), "List" | "Array")
            && args.len() == 1
        {
            return self
                .implements(&args[0], &Type::named("Comparable"), &mut seen.clone())
                .then(|| associated(plain, [("Item", args[0].clone())]));
        }
        if trait_name == "Negatable" && wanted.is_empty() && numeric(&ty) {
            return Some(associated(plain, [("Output", ty)]));
        }
        if trait_name == "Powerable"
            && wanted.len() == 1
            && numeric(&ty)
            && let Some(output) = builtin_power_output(&ty, &wanted[0])
        {
            return Some(associated(plain, [("Output", output)]));
        }
        if operator_trait(trait_name)
            && wanted.len() == 1
            && numeric(&ty)
            && self.inference.clone().unify(&ty, &wanted[0])
        {
            return Some(associated(
                Type::applied(trait_name, vec![ty.clone()]),
                [("Output", ty)],
            ));
        }
        if trait_name == "Iterator"
            && wanted.is_empty()
            && let Type::Cursor(item, _) = &ty
        {
            return Some(associated(plain, [("Item", *item.clone())]));
        }
        if trait_name == "Iterable" && wanted.is_empty() {
            let item = match &ty {
                Type::Lazy(item, _) => Some(*item.clone()),
                Type::Named(name, args)
                    if matches!(
                        name.as_str(),
                        "List" | "Set" | "SortedSet" | "Array" | "MutSet"
                    ) && args.len() == 1 =>
                {
                    Some(args[0].clone())
                }
                Type::Named(name, args)
                    if matches!(name.as_str(), "Map" | "MutMap") && args.len() == 2 =>
                {
                    Some(Type::Tuple(args.clone()))
                }
                // A String iterates its characters, each a one-character String (chosen 2026-11-08).
                Type::Named(name, args) if name == "String" && args.is_empty() => {
                    Some(Type::named("String"))
                }
                _ => None,
            };
            if let Some(item) = item {
                return Some(associated(
                    plain,
                    [
                        ("Item", item.clone()),
                        (
                            "Iterator",
                            Type::Cursor(
                                Box::new(item),
                                matches!(&ty, Type::Lazy(_, traversal) if traversal.effects),
                            ),
                        ),
                    ],
                ));
            }
        }
        let Type::Named(name, args) = &ty else {
            return None;
        };
        let definition = self.environment.definitions.get(name)?;
        let substitutions = definition
            .parameters
            .iter()
            .cloned()
            .zip(args.iter().cloned())
            .collect();
        for declared in &self.expanded_traits(&definition.traits) {
            let declared = declared.replace_parameters(&substitutions);
            let Some((name, args, explicit)) = trait_parts(&declared) else {
                continue;
            };
            // `has Addable` with no operand takes whatever its add method takes (Q87).
            let open_operand = operator_trait(name) && args.is_empty();
            let args: Vec<Type> = if open_operand {
                wanted.to_vec()
            } else {
                args.to_vec()
            };
            let args = &args[..];
            if name != trait_name
                || !self
                    .inference
                    .clone()
                    .unify(&Type::Tuple(args.to_vec()), &Type::Tuple(wanted.to_vec()))
            {
                continue;
            }
            // A trait type promises the Iterator or Iterable protocol that its `has` list names, with
            // its Item binding. Each class that has the trait is checked for the protocol where it is written.
            if definition.kind == K::Trait && trait_name == "Iterator" {
                return Some(declared);
            }
            let method_name = match trait_name {
                name if operator_trait(name) => operator_method(name).unwrap(),
                name if unary_trait(name).is_some() => unary_trait(name).unwrap(),
                "Iterable" => "items",
                "Iterator" => "next",
                _ => {
                    let Some(trait_definition) = self.environment.definitions.get(trait_name)
                    else {
                        return Some(declared);
                    };
                    if trait_definition.associated_types.is_empty() {
                        return Some(declared);
                    }
                    let bindings = self
                        .infer_associated(&ty, trait_definition, args, explicit)
                        .ok()?;
                    return Some(Type::Associated(
                        Box::new(Type::applied(trait_name, args.to_vec())),
                        bindings,
                    ));
                }
            };
            let methods = self.methods(&ty, method_name, &mut BTreeSet::new());
            let candidates = methods
                .iter()
                .filter(|method| {
                    (!method.effects || matches!(trait_name, "Iterator" | "Iterable"))
                        && method.bounds.is_empty()
                        && match trait_name {
                            name if operator_trait(name) => {
                                method.parameters.len() == 1
                                    && (args.is_empty() && open_operand
                                        || args.len() == 1
                                            && method.parameters[0].ty.same_type(&args[0]))
                                    && method.parameters[0].passing == Passing::Borrow
                                    && !method.receiver_mutates
                            }
                            name if unary_trait(name).is_some() => {
                                method.parameters.is_empty()
                                    && !method.effects
                                    && !method.receiver_mutates
                            }
                            _ => method.parameters.is_empty(),
                        }
                })
                .collect::<Vec<_>>();
            if open_operand && args.is_empty() {
                // The declaration itself: some pure add(other) method must exist.
                return (!candidates.is_empty()).then(|| declared.clone());
            }
            if candidates.len() != 1 {
                return None;
            }
            let result = *candidates[0].result.clone();
            let inferred = match trait_name {
                name if operator_trait(name) || unary_trait(name).is_some() => associated(
                    Type::applied(trait_name, args.to_vec()),
                    [("Output", result)],
                ),
                "Iterator" => {
                    let Type::Named(name, items) = result else {
                        return None;
                    };
                    if name != "Option" || items.len() != 1 {
                        return None;
                    }
                    associated(plain.clone(), [("Item", items[0].clone())])
                }
                "Iterable" => {
                    let iterator = self.trait_instance(&result, "Iterator", &[], seen)?;
                    let item = trait_parts(&iterator)?.2?.get("Item")?.clone();
                    associated(plain.clone(), [("Item", item), ("Iterator", result)])
                }
                _ => unreachable!(),
            };
            if let Some(explicit) = explicit {
                let actual = trait_parts(&inferred)?.2?;
                if !explicit
                    .iter()
                    .all(|(name, ty)| actual.get(name) == Some(ty))
                {
                    return None;
                }
            }
            return Some(inferred);
        }
        definition.parent.as_ref().and_then(|parent| {
            self.trait_instance(
                &parent.replace_parameters(&substitutions),
                trait_name,
                wanted,
                seen,
            )
        })
    }

    /// `a[k]` on a class that has `Indexable[Key, Item]` reads through at(k). The key
    /// argument resolves Key, and Item is the result. Returns None for the built-in
    /// bracket forms and for the types that keep their old bracket errors.
    fn indexable(&mut self, node: &Node, receiver: &Type) -> Option<Type> {
        let receiver = self.inference.resolve(receiver);
        let receiver = match receiver {
            Type::Borrow(inner) | Type::StoredView(inner) => *inner,
            other => other,
        };
        // The built-in brackets, trait-object views and broken receivers keep their own errors.
        if matches!(&receiver, Type::Tuple(_) | Type::Invalid | Type::Dynamic(_))
            || matches!(&receiver, Type::Named(name, _) if builtin_arity(name).is_some())
        {
            return None;
        }
        let trait_name = self.environment.names.get("Indexable").cloned()?;
        let key = self.infer(&node.children[1], None);
        let item = self.inference.fresh();
        if let Some(instance) = self.trait_instance(
            &receiver,
            &trait_name,
            &[key.clone(), item],
            &mut BTreeSet::new(),
        ) && let Type::Named(_, args) = self.inference.resolve(&instance)
        {
            return Some(args.get(1).cloned().unwrap_or(Type::Invalid));
        }
        let found = self.inference.resolve(&key);
        if matches!(found, Type::Invalid) {
            return Some(Type::Invalid);
        }
        // The class has Indexable with another key, or has no Indexable at all.
        let expected = self.inference.fresh();
        let witness = self.inference.fresh();
        if let Some(instance) = self.trait_instance(
            &receiver,
            &trait_name,
            &[expected, witness],
            &mut BTreeSet::new(),
        ) && let Type::Named(_, args) = self.inference.resolve(&instance)
        {
            let expected = args.first().cloned().unwrap_or(Type::Invalid);
            self.error(
                &node.children[1],
                "E_INDEX_KEY",
                format!("{receiver} indexes through Indexable with key type {expected}, but the index has type {found}."),
                "Pass a value of the trait's Key type, or read the value another way.",
            );
            return Some(Type::Invalid);
        }
        if let Type::Named(name, _) = &receiver
            && self
                .environment
                .definitions
                .get(name)
                .is_some_and(|definition| matches!(definition.kind, K::Class | K::Trait))
        {
            self.error(
                node,
                "E_INDEX_TRAIT",
                format!("{receiver} does not implement Indexable with key type {found}."),
                "Add `has Indexable[Key, Item]` and write `public function at(key : Key) : Item`, or call one of its methods.",
            );
            return Some(Type::Invalid);
        }
        None
    }

    fn object_safe(&self, requirement: &Type) -> bool {
        let Some((name, args, bindings)) = trait_parts(requirement) else {
            return false;
        };
        if matches!(name, "Iterator" | "Iterable" | "Sortable")
            || operator_trait(name)
            || unary_trait(name).is_some()
        {
            let needed: &[&str] = match name {
                "Iterator" => &["Item"],
                "Iterable" => &["Item"],
                "Sortable" => &["Item"],
                _ => &["Output"],
            };
            return needed
                .iter()
                .all(|slot| bindings.is_some_and(|bindings| bindings.contains_key(*slot)));
        }
        if name == "Text" {
            return true;
        }
        let Some(definition) = self.environment.definitions.get(name) else {
            return false;
        };
        if definition
            .associated_types
            .iter()
            .any(|slot| !bindings.is_some_and(|bindings| bindings.contains_key(slot)))
        {
            return false;
        }
        let mut substitutions: BTreeMap<_, _> = definition
            .parameters
            .iter()
            .cloned()
            .zip(args.iter().cloned())
            .collect();
        if let Some(bindings) = bindings {
            substitutions.extend(bindings.clone());
        }
        definition.methods.values().flatten().all(|function| {
            let mut generic = false;
            Type::Function(function.clone())
                .replace_parameters(&substitutions)
                .map(&mut |ty| {
                    generic |= matches!(ty, Type::Parameter(_));
                    None
                });
            !generic && function.bounds.is_empty()
        })
    }

    /// Whether an ancestor already declares this exact method, so the stub
    /// needs `@override`.
    fn overrides_ancestor(
        &self,
        definition: &Definition,
        name: &str,
        contract: &FunctionType,
        is_static: bool,
    ) -> bool {
        let mut parent = definition.parent.clone();
        let mut seen = BTreeSet::new();
        while let Some(Type::Named(id, _)) = parent {
            if !seen.insert(id.clone()) {
                break;
            }
            let Some(ancestor) = self.environment.definitions.get(&id) else {
                break;
            };
            if ancestor.static_methods.contains(name) == is_static
                && ancestor.methods.get(name).is_some_and(|methods| {
                    methods
                        .iter()
                        .any(|method| method.same_parameters(contract))
                })
            {
                return true;
            }
            parent = ancestor.parent.clone();
        }
        false
    }

    fn explicit_default_implementation<'b>(
        &self,
        class_node: &'b Node,
        definition: &Definition,
        name: &str,
        contract: &FunctionType,
    ) -> Option<&'b Node> {
        definition
            .methods
            .get(name)?
            .iter()
            .filter(|method| method.same_parameters(contract))
            .filter_map(|method| match &method.callable {
                Some(Callable::Declaration { id, .. }) => Some(id.span),
                _ => None,
            })
            .find_map(|span| {
                class_node
                    .children
                    .iter()
                    .find(|member| member.kind == K::Function && member.span == span)
            })
    }

    fn is_trait_default_method(&self, method: &FunctionType) -> bool {
        let Some(Callable::Declaration { id, .. }) = &method.callable else {
            return false;
        };
        self.environment.definitions.values().any(|definition| {
            definition.kind == K::Trait
                && definition.file == id.file
                && definition.default_methods.contains(&id.span)
        })
    }

    fn check_traits(&mut self, node: &Node, id: &str, definition: &Definition) {
        if definition.kind == K::Trait {
            return;
        }
        let receiver = Type::Named(
            id.into(),
            definition
                .parameters
                .iter()
                .map(|name| Type::Parameter(name.clone()))
                .collect(),
        );
        let default_groups = self.grouped_trait_defaults(&receiver, &definition.traits);
        let mut unresolved_conflicts = vec![];
        for group in default_groups.iter().filter(|group| group.len() > 1) {
            let candidate = &group[0];
            let implemented = self
                .methods_with_access(&receiver, &candidate.name, &mut BTreeSet::new(), false)
                .iter()
                .any(|method| method.same_parameters(&candidate.signature));
            if implemented {
                continue;
            }
            let providers = group
                .iter()
                .map(|method| {
                    method
                        .provider
                        .rsplit('.')
                        .next()
                        .unwrap_or(&method.provider)
                })
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            self.error(
                node,
                "E_TRAIT_CONFLICT",
                format!(
                    "`{id}` inherits conflicting defaults for `{}` from {}.",
                    candidate.name,
                    providers.join(" and ")
                ),
                "Write the method in this class and mark it `@override`.",
            );
            unresolved_conflicts.push(candidate.clone());
        }
        let mut checked_overrides = BTreeSet::new();
        for group in &default_groups {
            for candidate in group {
                if let Some(method) = self.explicit_default_implementation(
                    node,
                    definition,
                    &candidate.name,
                    &candidate.signature,
                ) && checked_overrides.insert(method.span)
                    && !self.overriding(method)
                {
                    self.error(
                        method,
                        "E_OVERRIDE_REQUIRED",
                        format!("`{}` replaces a trait default method.", candidate.name),
                        "Mark this method `@override`.",
                    );
                }
            }
        }
        if self.implements(&receiver, &Type::named("Text"), &mut BTreeSet::new())
            && self
                .methods(&receiver, "debug", &mut BTreeSet::new())
                .iter()
                .filter(|method| method.parameters.is_empty())
                .any(|method| {
                    method.effects
                        || method.receiver_mutates
                        || *method.result != Type::named("String")
                        || !method.bounds.is_empty()
                })
        {
            self.error(node,"E_TRAIT_METHOD","debug() used by Text must be pure, read-only and return String.","Give debug() the same permissions and return type as text(), or omit it to use text().");
        }
        for requirement in &self.expanded_traits(&definition.traits) {
            let Some((name, args, _)) = trait_parts(requirement) else {
                continue;
            };
            if let Some(trait_definition) = self.environment.definitions.get(name)
                && !trait_definition.associated_types.is_empty()
            {
                let explicit = trait_parts(requirement).and_then(|(_, _, bindings)| bindings);
                if let Err(reason) =
                    self.infer_associated(&receiver, trait_definition, args, explicit)
                {
                    self.error(node, "E_ASSOCIATED_INFER", format!("Cannot infer {requirement} for `{id}`: {reason}."), "Make the implementation methods agree, or bind the associated type explicitly in has Trait(Item = Type).");
                }
                continue;
            }
            if self
                .environment
                .definitions
                .get(name)
                .is_some_and(|trait_def| trait_def.sealed && trait_def.file != self.source.name)
            {
                self.error(
                    node,
                    "E_SEALED_IMPLEMENTATION",
                    format!("Sealed trait `{name}` can be implemented only in its own module."),
                    "Keep the implementing class in the trait's file.",
                );
            }
            let trait_static_methods = self
                .environment
                .definitions
                .get(name)
                .map(|trait_def| trait_def.static_methods.clone())
                .unwrap_or_default();
            let receiver = Type::Named(
                id.into(),
                definition
                    .parameters
                    .iter()
                    .map(|name| Type::Parameter(name.clone()))
                    .collect(),
            );
            if matches!(name, "Iterable" | "Iterator") || operator_trait(name) {
                if !self.implements(&receiver, requirement, &mut BTreeSet::new()) {
                    self.error(node, "E_TRAIT_METHOD", format!("`{id}` declares has {requirement}, but its protocol methods do not match."), "add, subtract, multiply and divide (and their right forms) take one borrowed operand and infer Output; items returns an Iterator; next returns Option[Item]. Methods must be pure and callable here.");
                }
                continue;
            }
            let contracts = match name {
                "Text" => vec![(
                    "text".into(),
                    FunctionType {
                        parameters: vec![],
                        result: Box::new(Type::named("String")),
                        bounds: BTreeMap::new(),
                        requirements: vec![],
                        effects: false,
                        receiver_mutates: false,
                        callable: None,
                        receiver: None,
                    },
                )],
                "Equals" | "Comparable" if args.is_empty() => vec![(
                    if name == "Comparable" {
                        "compare".into()
                    } else {
                        "equals".into()
                    },
                    FunctionType {
                        parameters: vec![Parameter {
                            name: None,
                            ty: Type::Named(
                                id.into(),
                                definition
                                    .parameters
                                    .iter()
                                    .map(|p| Type::Parameter(p.clone()))
                                    .collect(),
                            ),
                            default: false,
                            passing: Passing::Borrow,
                        }],
                        result: Box::new(Type::named(if name == "Comparable" {
                            "Int"
                        } else {
                            "Bool"
                        })),
                        bounds: BTreeMap::new(),
                        requirements: vec![],
                        effects: false,
                        receiver_mutates: false,
                        callable: None,
                        receiver: None,
                    },
                )],
                "Equals" => {
                    self.error(node, "E_TRAIT_CONTRACT_UNSPECIFIED", format!("The complete method contract for `{requirement}` is not specified yet."), "Write the complete contract in the trait, or declare the method in the class. A `has` entry cannot certify an unknown contract.");
                    continue;
                }
                _ => {
                    if let Some(trait_def) = self.environment.definitions.get(name) {
                        // In a trait signature `Self` is the class that has the
                        // trait, so its contract holds that class here.
                        let substitutions = trait_def
                            .parameters
                            .iter()
                            .cloned()
                            .zip(args.iter().cloned())
                            .chain([("Self".to_owned(), receiver.clone())])
                            .collect();
                        trait_def
                            .methods
                            .iter()
                            .flat_map(|(name, methods)| {
                                methods
                                    .iter()
                                    .map(|method| {
                                        (
                                            name.clone(),
                                            match Type::Function(method.clone())
                                                .replace_parameters(&substitutions)
                                            {
                                                Type::Function(fun) => fun,
                                                _ => unreachable!(),
                                            },
                                        )
                                    })
                                    .collect::<Vec<_>>()
                            })
                            .collect()
                    } else {
                        continue;
                    }
                }
            };
            let mut missing: Vec<(usize, String, FunctionType, bool, bool)> = vec![];
            for (name, contract) in contracts {
                let contract_static = trait_static_methods.contains(&name);
                let methods =
                    self.methods_with_access(&receiver, &name, &mut BTreeSet::new(), false);
                let found = methods.iter().any(|method| {
                    method.same_parameters(&contract)
                        && method
                            .parameters
                            .iter()
                            .zip(&contract.parameters)
                            .all(|(a, b)| a.passing == b.passing)
                        && method.receiver_mutates == contract.receiver_mutates
                        && method.result.same_type(&contract.result)
                        && method.effects == contract.effects
                        && method.bounds == contract.bounds
                        && matches!(
                            method.callable,
                            Some(Callable::Declaration {
                                kind: DeclarationKind::StaticMethod,
                                ..
                            })
                        ) == contract_static
                });
                let conflicted = unresolved_conflicts.iter().any(|candidate| {
                    candidate.name == name && candidate.signature.same_parameters(&contract)
                });
                let class_collision = definition.methods.get(&name).is_some_and(|methods| {
                    methods
                        .iter()
                        .any(|method| method.same_parameters(&contract))
                });
                let supplied_by_default =
                    self.is_trait_default_method(&contract) && !conflicted && !class_collision;
                let found = found || supplied_by_default;
                if !found {
                    if conflicted {
                        continue;
                    }
                    // A method with the same parameters but the wrong result or
                    // effects cannot get a second overload, so leave the plain
                    // diagnostic instead of generating unusable code.
                    let collides = definition.methods.get(&name).is_some_and(|methods| {
                        methods
                            .iter()
                            .any(|method| method.same_parameters(&contract))
                    });
                    let diagnostic = self.diagnostics.len();
                    let overrides =
                        self.overrides_ancestor(definition, &name, &contract, contract_static);
                    self.error(node, "E_TRAIT_METHOD", format!("`{id}` declares has {requirement}, but does not write the required `{name}` method with type {}.", Type::Function(contract.clone())), "Write the required method with matching parameter and return types.");
                    if !collides {
                        missing.push((diagnostic, name, contract, contract_static, overrides));
                    }
                }
            }
            if !missing.is_empty() {
                let entries = missing
                    .iter()
                    .map(|(_, name, contract, is_static, overrides)| {
                        crate::actions::MissingMethod {
                            name,
                            contract,
                            is_static: *is_static,
                            overrides: *overrides,
                        }
                    })
                    .collect::<Vec<_>>();
                let mut fixes = crate::actions::trait_method_fixes(
                    self.source,
                    node,
                    name,
                    &definition.parameters,
                    &entries,
                );
                // The combined action is last when several methods are missing.
                let combined = (fixes.len() > 1).then(|| fixes.pop()).flatten();
                for (index, (diagnostic, _, _, _, _)) in missing.iter().enumerate() {
                    let mut found = crate::actions::trait_method_fixes(
                        self.source,
                        node,
                        name,
                        &definition.parameters,
                        &[crate::actions::MissingMethod {
                            name: &missing[index].1,
                            contract: &missing[index].2,
                            is_static: missing[index].3,
                            overrides: missing[index].4,
                        }],
                    );
                    if index == 0
                        && let Some(combined) = &combined
                    {
                        found.insert(0, combined.clone());
                    }
                    if let Some(diagnostic) = self.diagnostics.get_mut(*diagnostic) {
                        diagnostic.fixes = found;
                    }
                }
            }
        }
    }

    fn function_body(&mut self, node: &Node, signature: &FunctionType, owner: Option<String>) {
        let mut copies: BTreeSet<String> = signature
            .bounds
            .iter()
            .filter(|(_, bounds)| bounds.contains(&Type::named("Copy")))
            .map(|(name, _)| name.clone())
            .collect();
        if let Some(definition) = owner
            .as_ref()
            .and_then(|id| self.environment.definitions.get(id))
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
        self.function_body_inner(node, signature, owner);
        crate::types::pop_copy_parameters();
    }
    fn function_body_inner(
        &mut self,
        node: &Node,
        signature: &FunctionType,
        owner: Option<String>,
    ) {
        if owner.is_none()
            && node.children.iter().any(|child| {
                child.kind == K::Attribute
                    && child
                        .children
                        .first()
                        .is_some_and(|name| self.text(name) == "test")
            })
        {
            // A test runs on its own and fails through an assertion or an Error (Q60).
            let result = signature.result.as_ref();
            let returns = *result == Type::named("void")
                || matches!(result, Type::Union(parts) if parts.len() == 2
                    && parts.contains(&Type::named("void"))
                    && parts.iter().any(|p| self.is_error_type(p)));
            let generic = node.child(K::TypeParameters).is_some();
            if !signature.parameters.is_empty() || !returns || generic {
                self.error(
                    node,
                    "E_TEST_SIGNATURE",
                    "A @test function takes no parameters and returns void or `void or Error`.",
                    "Build the test's inputs inside it, and use ? or assertEq for failures.",
                );
            }
        }
        if signature.receiver_mutates
            && (owner.is_none() || node.kind == K::Constructor || self.modifier(node, "static"))
        {
            self.error(node, "E_MUT_METHOD_CONTEXT", "The mut method modifier is for instance methods.", "Use mut on parameters to change a borrowed argument; constructors initialize fresh fields.");
        }
        let old_rigid = self.rigid_parameters.clone();
        let old_indices = self.rigid_indices.clone();
        let signature = signature.map(&mut |ty| {
            if let Type::Parameter(name) = ty {
                self.rigid_parameters.insert(name.clone());
                Some(Type::Rigid(name.clone()))
            } else {
                None
            }
        });
        let signature = match Type::Function(signature).map_indices(&mut |index| {
            if let Index::Variable(name) = index {
                self.rigid_indices.insert(name.clone());
                Index::Rigid(name.clone())
            } else {
                index.clone()
            }
        }) {
            Type::Function(function) => function,
            _ => unreachable!(),
        };
        let old_bounds = std::mem::replace(&mut self.bounds, signature.bounds.clone());
        if let Some(definition) = owner
            .as_ref()
            .and_then(|id| self.environment.definitions.get(id))
        {
            self.rigid_parameters
                .extend(definition.parameters.iter().cloned());
            self.rigid_parameters
                .extend(definition.associated_types.iter().cloned());
            self.bounds.extend(definition.bounds.clone());
        }
        self.validate_type(node, &Type::Function(signature.clone()));
        let body = function_body(node);
        if body.is_none() {
            if let Some(condition) = node.child(K::Ensures) {
                self.error(
                    condition,
                    "E_ENSURES_ABSTRACT",
                    "A postcondition needs a function body to check.",
                    "Write ensures on the method implementation.",
                );
            }
            if owner
                .as_ref()
                .and_then(|id| self.environment.definitions.get(id))
                .is_none_or(|d| d.kind != K::Trait)
            {
                self.error(
                    node,
                    "E_ABSTRACT_FUNCTION",
                    "A function without a body is allowed only in a trait.",
                    "Give this function an expression or block body.",
                );
            }
        }
        let old_owner = self.owner.replace(owner.clone().unwrap_or_default());
        if owner.is_none() {
            self.owner = None;
        }
        // `Self` in a body names the class or trait that declares the member. A
        // function declared inside a method keeps the enclosing meaning.
        let old_self = self.self_type.clone();
        if let Some(id) = &owner {
            let declared = self.environment.definitions.get(id).map(|definition| {
                (
                    definition.kind,
                    definition
                        .parameters
                        .iter()
                        .map(|parameter| Type::Parameter(parameter.clone()))
                        .collect::<Vec<_>>(),
                )
            });
            if let Some((kind, arguments)) = declared {
                self.self_type = Some(if kind == K::Trait {
                    crate::types::self_type()
                } else {
                    Type::Named(id.clone(), arguments)
                });
            }
        }
        let old_return = self.return_type.replace(if node.kind == K::Constructor {
            Type::named("void")
        } else {
            *signature.result.clone()
        });
        let old_effects = (self.effects_allowed, self.effects_observed);
        let old_receiver = std::mem::replace(
            &mut self.receiver_mutable,
            signature.receiver_mutates || node.kind == K::Constructor,
        );
        self.effects_allowed = signature.effects;
        self.effects_observed = false;
        let old_initializing = self.initializing_fields.take();
        let old_super = self.super_start.take();
        if node.kind == K::Constructor {
            self.super_start = body
                .and_then(|body| body.children.first())
                .filter(|first| {
                    first.kind == K::Call
                        && first.children.first().is_some_and(|callee| {
                            callee.kind == K::Name && self.text(callee) == "super"
                        })
                })
                .map(|first| first.span);
        }
        if node.kind == K::Constructor {
            self.initializing_fields = Some(
                owner
                    .as_ref()
                    .and_then(|id| self.environment.definitions.get(id))
                    .map(|definition| definition.initialized_fields.clone())
                    .unwrap_or_default(),
            );
        }
        let old_loop = std::mem::replace(&mut self.loop_depth, 0);
        let old_barriers = std::mem::take(&mut self.exit_barriers);
        let old_scope = std::mem::replace(&mut self.scope_depth, 0);
        let old_parameters = std::mem::take(&mut self.borrowed_parameters);
        self.named_function_depth += 1;
        self.scopes.push(BTreeMap::new());
        // A function declared inside a method reads that method's `this`.
        if let Some(owner) = &owner
            && !self.modifier(node, "static")
            && !self.scopes.iter().any(|scope| scope.contains_key("this"))
        {
            let definition = self.environment.definitions.get(owner);
            let args = definition
                .map(|definition| {
                    definition
                        .parameters
                        .iter()
                        .map(|parameter| Type::Rigid(parameter.clone()))
                        .collect()
                })
                .unwrap_or_default();
            let mut this_type = Type::Named(owner.clone(), args);
            if let Some(definition) = definition
                && definition.kind == K::Trait
                && !definition.associated_types.is_empty()
            {
                let bindings = definition
                    .associated_types
                    .iter()
                    .map(|name| (name.clone(), Type::Rigid(name.clone())))
                    .collect();
                this_type = Type::Associated(Box::new(this_type), bindings);
            }
            self.bind(node, "this", this_type, false, false);
        }
        if let Some(parameters) = node.child(K::ParameterList) {
            for (parameter, ty) in parameters.children.iter().zip(&signature.parameters) {
                if let Some(claim) = parameter.children.iter().find(|n| n.kind == K::Proposition) {
                    let found = self.infer(&claim.children[0], Some(&Type::named("Bool")));
                    self.require(&claim.children[0], &Type::named("Bool"), &found);
                }
                if let Some(name) = &ty.name {
                    self.bind(parameter, name, ty.ty.clone(), false, false);
                    if !self.modifier(parameter, "move") {
                        self.borrowed_parameters
                            .insert(name.clone(), self.modifier(parameter, "mut"));
                    }
                }
                if let Some(default) = parameter
                    .children
                    .last()
                    .filter(|n| !is_type(n.kind) && n.kind != K::Modifier)
                {
                    if default_exits(default) {
                        self.error(default,"E_DEFAULT_CONTROL","A default argument must produce a value without returning or propagating Error from its caller.","Put return or ? in the function body; a fallible default can declare an Error union parameter.");
                    }
                    let permissions = self.borrowed_parameters.clone();
                    for name in signature.parameters.iter().filter_map(|p| p.name.as_ref()) {
                        self.borrowed_parameters.insert(name.clone(), false);
                    }
                    let receiver = std::mem::replace(&mut self.receiver_mutable, false);
                    let found = self.infer(default, Some(&ty.ty));
                    self.require(default, &ty.ty, &found);
                    self.receiver_mutable = receiver;
                    self.borrowed_parameters = permissions;
                }
            }
        }
        if let Some(claim) = node
            .child(K::ReturnType)
            .and_then(|r| r.child(K::Proposition))
        {
            if self
                .owner
                .as_ref()
                .and_then(|o| self.environment.definitions.get(o))
                .is_some_and(|d| d.kind == K::Trait)
            {
                self.error(
                    claim,
                    "E_PROOF_RESULT_UNSUPPORTED",
                    "A trait method cannot yet declare a state-proof result.",
                    "Use a concrete method whose producer body the compiler can check.",
                );
            }
            let witness = Node::new(K::ProofWitness, claim.span, claim.children.clone());
            self.infer(&witness, Some(&Type::named("Proof")));
        }
        for requirement in node.children.iter().filter(|n| n.kind == K::Requires) {
            if self.theorem_requirements.contains(&requirement.span) {
                continue;
            }
            if !contains_error(&signature.result) {
                self.error(requirement, "E_CONTRACT_ERROR_TYPE", "A function with requires must expose Error in its return type.", "Use a return type such as Int or Error, including for a condition known at one call site.");
            }
            let allowed = std::mem::replace(&mut self.effects_allowed, false);
            let receiver_mutable = std::mem::replace(&mut self.receiver_mutable, false);
            let parameters = self.borrowed_parameters.clone();
            for parameter in &signature.parameters {
                if let Some(name) = &parameter.name {
                    self.borrowed_parameters.insert(name.clone(), false);
                }
            }
            if let Some(expression) = requirement.children.first() {
                let ty = self.infer(expression, Some(&Type::named("Bool")));
                self.require(expression, &Type::named("Bool"), &ty);
            }
            self.effects_allowed = allowed;
            self.receiver_mutable = receiver_mutable;
            self.borrowed_parameters = parameters;
        }
        for condition in node.children.iter().filter(|n| n.kind == K::Ensures) {
            if node.kind == K::Constructor {
                self.error(
                    condition,
                    "E_ENSURES_CONSTRUCTOR",
                    "A constructor has no return value for ensures.",
                    "Put the postcondition on a function or method.",
                );
                continue;
            }
            if signature
                .parameters
                .iter()
                .any(|p| p.name.as_deref() == Some("result"))
            {
                self.error(
                    condition,
                    "E_ENSURES_RESULT_NAME",
                    "The name result is reserved for the return value inside ensures.",
                    "Rename the parameter.",
                );
                continue;
            }
            let success = match signature.result.as_ref() {
                Type::Union(parts) => {
                    let parts = parts
                        .iter()
                        .filter(|p| !self.is_error_type(p))
                        .cloned()
                        .collect::<Vec<_>>();
                    match parts.as_slice() {
                        [] => Type::Never,
                        [only] => only.clone(),
                        _ => Type::Union(parts),
                    }
                }
                ty if self.is_error_type(ty) => Type::Never,
                ty => ty.clone(),
            };
            let allowed = std::mem::replace(&mut self.effects_allowed, false);
            let receiver = std::mem::replace(&mut self.receiver_mutable, false);
            let parameters = self.borrowed_parameters.clone();
            for parameter in &signature.parameters {
                if let Some(name) = &parameter.name {
                    self.borrowed_parameters.insert(name.clone(), false);
                }
            }
            self.borrowed_parameters.insert("result".into(), false);
            self.scopes.push(BTreeMap::new());
            self.bind(condition, "result", success, false, false);
            if let Some(expression) = condition.children.first() {
                let found = self.infer(expression, Some(&Type::named("Bool")));
                self.require(expression, &Type::named("Bool"), &found);
                for child in expression.descendants() {
                    if matches!(
                        child.kind,
                        K::Return | K::Propagate | K::Break | K::Continue
                    ) {
                        self.error(
                            child,
                            "E_ENSURES_CONTROL",
                            "A postcondition cannot leave its function or propagate an Error.",
                            "Use a pure Bool expression that finishes the check.",
                        );
                    }
                    if child.kind == K::Name
                        && self
                            .expression_types
                            .iter()
                            .any(|(span, _)| *span == child.span)
                        && signature.parameters.iter().any(|p| {
                            p.name.as_deref() == Some(self.text(child))
                                && !crate::types::copies(&p.ty)
                        })
                    {
                        self.error(child,"E_ENSURES_PARAMETER","A postcondition cannot read a parameter whose value moves.","Read result or a copyable parameter; an owned argument may already have moved before return.");
                    }
                }
            }
            self.scopes.pop();
            self.effects_allowed = allowed;
            self.receiver_mutable = receiver;
            self.borrowed_parameters = parameters;
        }
        if let Some(body) = body {
            let proposition_result = matches!(&*signature.result, Type::Named(name, args) if name == "Prop" && args.is_empty());
            if proposition_result && body.kind == K::Proposition {
                // Proof-only predicate: check the proposition, never lower it
                // to a run-time value.
                self.check_prop_value(body);
            } else {
                let found = self.infer(body, Some(signature.result.as_ref()));
                if node.kind != K::Constructor {
                    self.reject_tensor_view_return(body, &signature.result, &found);
                    if !self.self_default_body(&signature.result, &found) {
                        self.require(body, &signature.result, &found);
                    }
                } else if found != Type::Never {
                    // A constructor produces the object, not a body value.
                    self.require(body, &Type::named("void"), &found);
                    self.ensure_initialized(body);
                }
            }
        }
        self.scopes.pop();
        self.owner = old_owner;
        self.self_type = old_self;
        self.return_type = old_return;
        (self.effects_allowed, self.effects_observed) = old_effects;
        self.receiver_mutable = old_receiver;
        self.initializing_fields = old_initializing;
        self.super_start = old_super;
        self.loop_depth = old_loop;
        self.exit_barriers = old_barriers;
        self.scope_depth = old_scope;
        self.borrowed_parameters = old_parameters;
        self.named_function_depth -= 1;
        self.bounds = old_bounds;
        self.rigid_parameters = old_rigid;
        self.rigid_indices = old_indices;
    }

    /// Type-checks a function body that returns `Prop`. The kernel rechecks it
    /// when a theorem unfolds the predicate; this pass catches mistakes in a
    /// predicate that no theorem cites yet.
    fn check_prop_value(&mut self, node: &Node) {
        match node.kind {
            K::Proposition | K::Group => {
                if let Some(inner) = node.children.first() {
                    self.check_prop_value(inner);
                }
            }
            K::Literal if matches!(self.text(node), "true" | "false") => {}
            K::Forall | K::Exists => {
                let Some((body, binders)) = node.children.split_last() else {
                    return;
                };
                self.scopes.push(BTreeMap::new());
                for binder in binders {
                    let Some(name_node) = binder.child(K::Name) else {
                        continue;
                    };
                    let name = self.text(name_node).to_owned();
                    let Some(ty_node) = binder.children.last() else {
                        continue;
                    };
                    let ty = self.annotation(ty_node, false);
                    self.bind(name_node, &name, ty, false, false);
                }
                self.check_prop_value(body);
                self.scopes.pop();
            }
            K::Unary
                if matches!(
                    self.text(&node.children[0]),
                    "not" | "¬" | "\\not" | "\\neg"
                ) =>
            {
                self.check_prop_value(&node.children[1]);
            }
            K::Binary => {
                let operator = self.text(&node.children[1]).to_owned();
                let (left, right) = (&node.children[0], &node.children[2]);
                match operator.as_str() {
                    "and" | "∧" | "\\land" | "or" | "∨" | "\\lor" | "->" | "→" | "\\to" | "↔"
                    | "\\iff" => {
                        self.check_prop_value(left);
                        self.check_prop_value(right);
                    }
                    "=" => {
                        let left_ty = self.infer(left, None);
                        if left_ty == Type::Invalid {
                            self.check_prop_value(right);
                            return;
                        }
                        let right_ty = self.infer(right, Some(&left_ty));
                        self.require(right, &left_ty, &right_ty);
                    }
                    "∈" | "\\in" | "∉" | "\\notin" => {
                        let element = self.infer(left, None);
                        if element == Type::Invalid {
                            return;
                        }
                        let set = Type::applied("Set", vec![element]);
                        let found = self.infer(right, Some(&set));
                        self.require(right, &set, &found);
                    }
                    "⊆" | "\\subseteq" => {
                        let left_ty = self.infer(left, None);
                        if left_ty == Type::Invalid {
                            return;
                        }
                        let right_ty = self.infer(right, Some(&left_ty));
                        self.require(right, &left_ty, &right_ty);
                    }
                    "==" | "!=" | "<" | "<=" | ">" | ">=" => {
                        let found = self.infer(node, Some(&Type::named("Bool")));
                        self.require(node, &Type::named("Bool"), &found);
                    }
                    _ => {
                        let found = self.infer(node, Some(&Type::named("Prop")));
                        self.require(node, &Type::named("Prop"), &found);
                    }
                }
            }
            _ => {
                let found = self.infer(node, Some(&Type::named("Prop")));
                self.require(node, &Type::named("Prop"), &found);
            }
        }
    }

    fn bind(&mut self, node: &Node, name: &str, ty: Type, mutable: bool, generalize: bool) {
        if !self.scopes.is_empty() {
            self.place_aliases
                .remove(&(self.scopes.len() - 1, name.to_owned()));
        }
        self.binding_types.insert(node.span, ty.clone());
        if name == "this" {
            // The implicit receiver is bound on the function itself; any other
            // declaration named `this` would collide with it.
            if !matches!(node.kind, K::Function | K::Constructor) {
                self.error(
                    node,
                    "E_RESERVED_NAME",
                    "`this` names the current object.",
                    "Choose another name for this declaration.",
                );
            }
        } else {
            self.naming(node, name, "value identifier");
        }
        let soft_range = name == "range"
            && self.scopes.is_empty()
            && self.environment.values.get(name).is_some_and(|symbol| {
                matches!(
                    &symbol.reference,
                    Some(Reference::Builtin(reference)) if reference == "std.core.range"
                )
            });
        if !soft_range
            && (self.scopes.iter().any(|scope| scope.contains_key(name))
                || (self.scopes.is_empty() && self.environment.values.contains_key(name)))
        {
            self.error(
                node,
                "E_SHADOW",
                format!(
                    "`{name}` is already defined in this scope or an enclosing function scope."
                ),
                "Choose a new name. Sibling blocks may reuse names.",
            );
            return;
        }
        let ty = self.inference.resolve(&ty);
        let ty = if node.kind == K::Function {
            ty
        } else {
            erase_callable_identities(&ty)
        };
        let mut quantified =
            if generalize && !mutable && self.immutable_type(&ty, &mut BTreeSet::new()) {
                ty.variables()
            } else {
                BTreeSet::new()
            };
        // Generalization only drops variables another symbol still mentions. Most
        // bindings have none to generalize, so they skip the scan of every symbol,
        // which made checking quadratic in the number of functions (REVIEW2 1.5).
        if !quantified.is_empty() {
            for symbol in self
                .scopes
                .iter()
                .flat_map(|scope| scope.values())
                .chain(self.environment.values.values())
            {
                for variable in self
                    .inference
                    .resolve(&symbol.ty)
                    .variables()
                    .difference(&symbol.quantified)
                {
                    quantified.remove(variable);
                }
                if quantified.is_empty() {
                    break;
                }
            }
        }
        let symbol = Symbol {
            ty,
            quantified,
            mutable,
            visibility: Visibility::Private,
            access: None,
            origin: None,
            reference: self.scopes.is_empty().then(|| {
                Reference::ScriptBinding(DeclarationId {
                    file: self.source.name.clone(),
                    span: node.span,
                })
            }),
        };
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.into(), symbol);
        } else {
            self.script_bindings.insert(name.into());
            self.environment.values.insert(name.into(), symbol);
        }
    }

    fn immutable_type(&self, ty: &Type, seen: &mut BTreeSet<String>) -> bool {
        match ty {
            Type::Named(name, args) => {
                if name == "ArenaRef" {
                    return true;
                }
                if matches!(
                    name.as_str(),
                    "Array"
                        | "Tensor"
                        | "DynTensor"
                        | "Arena"
                        | "MutMap"
                        | "MutSet"
                        | "Mutex"
                        | "Atomic"
                        | "Channel"
                        | "Reader"
                        | "Writer"
                ) {
                    return false;
                }
                // One visited set for the whole walk: a class that contains a list
                // of itself must not restart the walk at each level.
                if !args.iter().all(|arg| self.immutable_type(arg, seen)) {
                    return false;
                }
                if let Some(definition) = self.environment.definitions.get(name) {
                    if !seen.insert(name.clone()) {
                        return true;
                    }
                    if definition.kind == K::Trait
                        && (definition
                            .methods
                            .values()
                            .flatten()
                            .any(|method| method.receiver_mutates)
                            || !definition
                                .traits
                                .iter()
                                .all(|parent| self.immutable_type(parent, seen)))
                    {
                        return false;
                    }
                    if !definition.mutable_fields.is_empty() {
                        return false;
                    }
                    let substitutions = definition
                        .parameters
                        .iter()
                        .cloned()
                        .zip(args.iter().cloned())
                        .collect();
                    if let Some(alias) = &definition.alias {
                        return self
                            .immutable_type(&alias.replace_parameters(&substitutions), seen);
                    }
                    if !definition.fields.values().all(|field| {
                        self.immutable_type(&field.replace_parameters(&substitutions), seen)
                    }) {
                        return false;
                    }
                    if let Some(parent) = &definition.parent {
                        return self
                            .immutable_type(&parent.replace_parameters(&substitutions), seen);
                    }
                }
                true
            }
            Type::Tuple(parts) | Type::Union(parts) => {
                parts.iter().all(|part| self.immutable_type(part, seen))
            }
            Type::Borrow(inner)
            | Type::Indexed(inner, _)
            | Type::Dynamic(inner)
            | Type::Associated(inner, _) => self.immutable_type(inner, seen),
            Type::Function(function) => !function.receiver_mutates,
            Type::Overloaded(functions) => {
                functions.iter().all(|function| !function.receiver_mutates)
            }
            Type::Lazy(inner, traversal) => !traversal.mutates && self.immutable_type(inner, seen),
            Type::Cursor(..) => false,
            _ => true,
        }
    }

    fn lookup(&mut self, node: &Node, name: &str, expected: Option<&Type>) -> Type {
        if let Some(symbol) = self.scopes.iter().rev().find_map(|s| s.get(name)).cloned() {
            return self
                .inference
                .instantiate_variables(&symbol.ty, &symbol.quantified);
        }
        if matches!(name, "Some" | "None") {
            let cases = self.cases(name, expected);
            if !cases.is_empty() {
                let option_expected = expected.is_none_or(|expected| {
                    self.case_expected(expected, &Type::applied("Option", vec![Type::Invalid]))
                });
                if cases.len() == 1 && !option_expected {
                    return self.inference.instantiate(&cases[0]);
                }
                self.error(
                    node,
                    "E_CASE_AMBIGUOUS",
                    format!("Several enums have a `{name}` case."),
                    "Qualify the case with its enum name, or supply an expected type.",
                );
                return Type::Invalid;
            }
        }
        let found = self
            .environment
            .values
            .get(name)
            .or_else(|| self.environment.implicit.get(name))
            .cloned();
        if let Some(symbol) = found {
            if matches!(&symbol.ty, Type::Namespace(_)) && self.member_receiver != Some(node.span) {
                self.error(
                    node,
                    "E_NAMESPACE_VALUE",
                    format!("`{name}` names a module, not a value."),
                    "Use an exported member, such as geometry.Point or geometry.area(...).",
                );
                return Type::Invalid;
            }
            if let Some(reference) = &symbol.reference {
                self.references.insert(node.span, reference.clone());
            }
            if self.constant_initializer
                && (symbol.mutable
                    || !self
                        .immutable_type(&self.inference.resolve(&symbol.ty), &mut BTreeSet::new()))
            {
                self.error(
                    node,
                    "E_CONST_DEPENDENCY",
                    format!(
                        "A constant or static initializer cannot read `{name}`, because its value can change."
                    ),
                    "Read an immutable binding, a constant or a static final field, or write the value directly.",
                );
            }
            if matches!(&symbol.reference, Some(crate::resolution::Reference::Builtin(target)) if target.starts_with("python:"))
                && self.member_receiver != Some(node.span)
            {
                self.error(
                    node,
                    "E_PYTHON_MODULE",
                    format!("`{name}` names a Python module; read one of its attributes."),
                    "Write name.attribute or name.function(...).",
                );
            }
            if matches!(&symbol.reference, Some(crate::resolution::Reference::Builtin(target)) if target.starts_with("c:"))
            {
                if self.direct_callee != Some(node.span) {
                    self.error(node, "E_UNSAFE_CALL", format!("The C function `{name}` can only be called directly."), "Call it inside unsafe { ... }; C functions cannot be passed around as values.");
                } else if self.unsafe_depth == 0 {
                    self.error(node, "E_UNSAFE_CALL", format!("A call to the C function `{name}` must sit inside an unsafe block."), "Write unsafe { ... } around the call; the compiler cannot check what C code does.");
                }
            }
            if self.named_function_depth > 0
                && self.script_bindings.contains(name)
                && (symbol.mutable
                    || !self
                        .immutable_type(&self.inference.resolve(&symbol.ty), &mut BTreeSet::new()))
            {
                self.error(node, "E_SCRIPT_MUTABLE_CAPTURE", format!("A named function cannot capture mutable script value `{name}`."), "Pass it as a parameter. Immutable script values and global const declarations may be read directly.");
                return Type::Invalid;
            }
            let ty = self
                .inference
                .instantiate_variables(&symbol.ty, &symbol.quantified);
            return if matches!(ty, Type::Function(_) | Type::Overloaded(_)) {
                ty
            } else {
                self.inference.instantiate(&ty)
            };
        }
        if let Some(id) = self.environment.names.get(name)
            && !self
                .environment
                .definitions
                .get(id)
                .is_some_and(|d| d.kind == K::Enum && d.variants.iter().any(|v| v.name == name))
        {
            return match self.expand(&Type::named(id), &mut BTreeSet::new()) {
                Some(Type::Named(name, args)) => Type::Constructor(name, args),
                _ => Type::Invalid,
            };
        }
        let mut cases = self.cases(name, expected);
        if cases.len() == 1 {
            return self.inference.instantiate(&cases.remove(0));
        }
        if cases.len() > 1 {
            self.error(
                node,
                "E_CASE_AMBIGUOUS",
                format!("Several enums have a `{name}` case."),
                "Write the enum name before the case, or supply an expected type.",
            );
        } else if name == "__kesdasReadCsv" {
            let result = expected.cloned().unwrap_or_else(|| {
                Type::Tuple(vec![
                    Type::applied("List", vec![Type::named("String")]),
                    Type::applied("List", vec![Type::named("Column")]),
                    Type::named("String"),
                ])
            });
            let mut function = match Type::function(
                vec![
                    Type::named("String"),
                    Type::named("String"),
                    Type::named("Bool"),
                    Type::named("Int"),
                    Type::named("Int"),
                ],
                result,
            ) {
                Type::Function(function) => function,
                _ => unreachable!(),
            };
            function.effects = true;
            function.callable = Some(Callable::Builtin("__kesdasReadCsv".into()));
            return Type::Function(function);
        } else if name == "__columnTop" {
            let result = expected
                .cloned()
                .unwrap_or_else(|| Type::applied("List", vec![Type::named("Int")]));
            let mut function = match Type::function(
                vec![
                    Type::named("std.kesdas.Column"),
                    Type::named("Int"),
                    Type::named("Bool"),
                ],
                result,
            ) {
                Type::Function(function) => function,
                _ => unreachable!(),
            };
            function.callable = Some(Callable::Builtin("__columnTop".into()));
            return Type::Function(function);
        } else if name == "__mergeLeftPositions" {
            let result = expected.cloned().unwrap_or_else(|| {
                Type::Tuple(vec![
                    Type::applied("List", vec![Type::named("Int")]),
                    Type::applied("List", vec![Type::named("Int")]),
                ])
            });
            let mut function = match Type::function(
                vec![
                    Type::applied("List", vec![Type::named("String")]),
                    Type::applied("List", vec![Type::named("String")]),
                ],
                result,
            ) {
                Type::Function(function) => function,
                _ => unreachable!(),
            };
            function.callable = Some(Callable::Builtin("__mergeLeftPositions".into()));
            return Type::Function(function);
        } else if name == "__factorCodes" {
            let result = expected.cloned().unwrap_or_else(|| {
                Type::Tuple(vec![
                    Type::applied("List", vec![Type::named("Int")]),
                    Type::named("Int"),
                ])
            });
            let mut function = match Type::function(
                vec![Type::applied("List", vec![Type::named("String")])],
                result,
            ) {
                Type::Function(function) => function,
                _ => unreachable!(),
            };
            function.callable = Some(Callable::Builtin("__factorCodes".into()));
            return Type::Function(function);
        } else if name == "__columnArithmetic" {
            let result = expected
                .cloned()
                .unwrap_or_else(|| Type::named("std.kesdas.Column"));
            let mut function = match Type::function(
                vec![
                    Type::named("Int"),
                    Type::named("std.kesdas.Column"),
                    Type::named("std.kesdas.Column"),
                ],
                result,
            ) {
                Type::Function(function) => function,
                _ => unreachable!(),
            };
            function.callable = Some(Callable::Builtin("__columnArithmetic".into()));
            return Type::Function(function);
        } else if name == "__columnCodeCounts" {
            let mut function = match Type::function(
                vec![
                    Type::applied("List", vec![Type::named("Int")]),
                    Type::named("Int"),
                ],
                Type::applied("List", vec![Type::named("Int")]),
            ) {
                Type::Function(function) => function,
                _ => unreachable!(),
            };
            function.callable = Some(Callable::Builtin("__columnCodeCounts".into()));
            return Type::Function(function);
        } else if name == "__columnGroupStat" {
            let mut function = match Type::function(
                vec![
                    Type::named("std.kesdas.Column"),
                    Type::applied("List", vec![Type::named("Int")]),
                    Type::applied("List", vec![Type::named("Int")]),
                    Type::named("Int"),
                    Type::named("Int"),
                ],
                Type::named("std.kesdas.Column"),
            ) {
                Type::Function(function) => function,
                _ => unreachable!(),
            };
            function.callable = Some(Callable::Builtin("__columnGroupStat".into()));
            return Type::Function(function);
        } else if name == "__columnCodes" {
            let result = expected.cloned().unwrap_or_else(|| {
                Type::Tuple(vec![
                    Type::applied("List", vec![Type::named("Int")]),
                    Type::named("Int"),
                ])
            });
            let mut function = match Type::function(vec![Type::named("std.kesdas.Column")], result)
            {
                Type::Function(function) => function,
                _ => unreachable!(),
            };
            function.callable = Some(Callable::Builtin("__columnCodes".into()));
            return Type::Function(function);
        } else if name == "__columnTile" {
            let mut function = match Type::function(
                vec![Type::named("std.kesdas.Column"), Type::named("Int")],
                Type::named("std.kesdas.Column"),
            ) {
                Type::Function(function) => function,
                _ => unreachable!(),
            };
            function.callable = Some(Callable::Builtin("__columnTile".into()));
            return Type::Function(function);
        } else if name == "__columnConcat" {
            let mut function = match Type::function(
                vec![
                    Type::named("std.kesdas.Column"),
                    Type::named("std.kesdas.Column"),
                ],
                Type::named("std.kesdas.Column"),
            ) {
                Type::Function(function) => function,
                _ => unreachable!(),
            };
            function.callable = Some(Callable::Builtin("__columnConcat".into()));
            return Type::Function(function);
        } else if name == "__columnIndex" {
            let mut function =
                match Type::function(vec![Type::named("Int")], Type::named("std.kesdas.Column")) {
                    Type::Function(function) => function,
                    _ => unreachable!(),
                };
            function.callable = Some(Callable::Builtin("__columnIndex".into()));
            return Type::Function(function);
        } else if name == "__columnRepeatNames" {
            let mut function = match Type::function(
                vec![
                    Type::applied("List", vec![Type::named("String")]),
                    Type::named("Int"),
                ],
                Type::named("std.kesdas.Column"),
            ) {
                Type::Function(function) => function,
                _ => unreachable!(),
            };
            function.callable = Some(Callable::Builtin("__columnRepeatNames".into()));
            return Type::Function(function);
        } else if name == "__columnCompareFloat" {
            let mut function = match Type::function(
                vec![
                    Type::named("std.kesdas.Column"),
                    Type::named("Int"),
                    Type::named("double"),
                ],
                Type::named("std.kesdas.Column"),
            ) {
                Type::Function(function) => function,
                _ => unreachable!(),
            };
            function.callable = Some(Callable::Builtin("__columnCompareFloat".into()));
            return Type::Function(function);
        } else if name == "__columnBoth" {
            let mut function = match Type::function(
                vec![
                    Type::named("std.kesdas.Column"),
                    Type::named("std.kesdas.Column"),
                ],
                Type::named("std.kesdas.Column"),
            ) {
                Type::Function(function) => function,
                _ => unreachable!(),
            };
            function.callable = Some(Callable::Builtin("__columnBoth".into()));
            return Type::Function(function);
        } else if name == "__columnRollingMean" {
            let mut function = match Type::function(
                vec![
                    Type::named("std.kesdas.Column"),
                    Type::named("Int"),
                    Type::named("Int"),
                ],
                Type::named("std.kesdas.Column"),
            ) {
                Type::Function(function) => function,
                _ => unreachable!(),
            };
            function.callable = Some(Callable::Builtin("__columnRollingMean".into()));
            return Type::Function(function);
        } else if name == "__columnCorrelation" {
            let mut function = match Type::function(
                vec![
                    Type::named("std.kesdas.Column"),
                    Type::named("std.kesdas.Column"),
                ],
                Type::named("double"),
            ) {
                Type::Function(function) => function,
                _ => unreachable!(),
            };
            function.callable = Some(Callable::Builtin("__columnCorrelation".into()));
            return Type::Function(function);
        } else if name == "__columnGather" {
            let result = expected
                .cloned()
                .unwrap_or_else(|| Type::named("std.kesdas.Column"));
            let mut function = match Type::function(
                vec![
                    Type::named("std.kesdas.Column"),
                    Type::applied("List", vec![Type::named("Int")]),
                ],
                result,
            ) {
                Type::Function(function) => function,
                _ => unreachable!(),
            };
            function.effects = false;
            function.callable = Some(Callable::Builtin("__columnGather".into()));
            return Type::Function(function);
        } else if name == "__columnOrder" {
            let result = expected
                .cloned()
                .unwrap_or_else(|| Type::applied("List", vec![Type::named("Int")]));
            let mut function = match Type::function(
                vec![
                    Type::named("std.kesdas.Column"),
                    Type::applied("List", vec![Type::named("Int")]),
                    Type::named("Bool"),
                    Type::named("Bool"),
                ],
                result,
            ) {
                Type::Function(function) => function,
                _ => unreachable!(),
            };
            function.effects = false;
            function.callable = Some(Callable::Builtin("__columnOrder".into()));
            return Type::Function(function);
        } else if let Some(reason) = self.environment.foreign.get(name).cloned() {
            self.error(
                node,
                "E_INTEROP_TYPE",
                format!("The C function `{name}` cannot be called from Kestrel yet: {reason}."),
                "The C boundary covers numbers and Bool; see the std.c types.",
            );
        } else {
            self.error(
                node,
                "E_UNKNOWN_NAME",
                format!("Name `{name}` is not defined."),
                "Declare or import it before use.",
            );
        }
        Type::Invalid
    }

    fn cases(&self, name: &str, expected: Option<&Type>) -> Vec<Type> {
        let expected = expected.map(|t| self.inference.resolve(t));
        let mut cases = vec![];
        for (id, definition) in self.environment.definitions.iter().filter(|(id, d)| {
            d.kind == K::Enum && self.environment.names.values().any(|name| name == *id)
        }) {
            for variant in &definition.variants {
                if variant.name != name {
                    continue;
                }
                if expected
                    .as_ref()
                    .is_some_and(|expected| !self.case_expected(expected, &variant.result))
                {
                    continue;
                }
                let ty = match variant.function() {
                    Some(mut function) => {
                        function.callable = Some(Callable::Declaration {
                            id: variant.declaration.clone(),
                            name: variant.name.clone(),
                            owner: Some(id.clone()),
                            kind: DeclarationKind::Variant,
                        });
                        Type::Function(function)
                    }
                    None => variant.result.clone(),
                };
                cases.push(ty);
            }
        }
        cases
    }

    fn infer(&mut self, node: &Node, expected: Option<&Type>) -> Type {
        let ty = self.infer_inner(node, expected);
        self.expression_types.push((node.span, ty.clone()));
        ty
    }

    fn infer_inner(&mut self, node: &Node, expected: Option<&Type>) -> Type {
        match node.kind {
            K::Name => {
                let name = self.text(node).to_owned();
                if name == "super" {
                    let parent = self
                        .owner
                        .as_ref()
                        .and_then(|owner| self.environment.definitions.get(owner))
                        .and_then(|d| d.parent.clone());
                    let Some(Type::Named(parent, arguments)) = parent else {
                        self.error(
                            node,
                            "E_SUPER_CONTEXT",
                            "super needs a class with a parent.",
                            "Use super in a subclass constructor or method.",
                        );
                        return Type::Invalid;
                    };
                    if self.direct_callee == Some(node.span) {
                        if self
                            .super_start
                            .is_none_or(|span| span.start != node.span.start)
                            || self.initializing_fields.is_none()
                        {
                            self.error(
                                node,
                                "E_SUPER_POSITION",
                                "A parent constructor call must be the first statement of new.",
                                "Put super(...) first, exactly once.",
                            );
                        }
                        return Type::Constructor(parent, arguments);
                    }
                    if self.member_receiver != Some(node.span) {
                        self.error(
                            node,
                            "E_SUPER_VALUE",
                            "super is used to call a parent method.",
                            "Write super.method(...).",
                        );
                    }
                    self.require_initialized_this(node);
                    return Type::Named(parent, arguments);
                }
                if name == "this" && self.member_receiver != Some(node.span) {
                    self.require_initialized_this(node);
                }
                self.lookup(node, &name, expected)
            }
            K::One => {
                let resolved = self.value_expected(expected);
                if resolved.as_ref() == Some(&Type::named("BigInt")) {
                    Type::named("BigInt")
                } else if resolved.as_ref() == Some(&Type::named("Nat")) {
                    Type::named("Nat")
                } else if resolved.as_ref() == Some(&Type::named("double")) {
                    Type::named("double")
                } else {
                    Type::named("Int")
                }
            }
            K::Literal => {
                let text = self.text(node);
                let resolved = self.value_expected(expected);
                let ty = if text.starts_with('"') {
                    Type::named("String")
                } else if matches!(text, "true" | "false") {
                    Type::named("Bool")
                } else if crate::lexer::is_double_literal(text)
                    || resolved.as_ref() == Some(&Type::named("double"))
                {
                    Type::named("double")
                } else if resolved.as_ref().is_some_and(bigint_context) {
                    Type::named("BigInt")
                } else if resolved.as_ref() == Some(&Type::named("Nat")) {
                    // Q71: a numeral is a Nat where a Nat is expected.
                    Type::named("Nat")
                } else if let Some(Type::Named(c_name, c_args)) = resolved.as_ref()
                    && c_args.is_empty()
                    && c_literal_fits(c_name, text)
                {
                    // An integer literal that fits becomes a C number where one is
                    // expected, so `abs(-5)` from stdlib.h needs no CInt32(...) (REVIEW2).
                    Type::named(c_name)
                } else {
                    Type::named("Int")
                };
                self.check_integer_literal(node, &ty);
                ty
            }
            K::Group => self.infer(&node.children[0], expected),
            K::Tuple => {
                let parts = if let Some(Type::Tuple(parts)) = self.value_expected(expected) {
                    parts
                } else {
                    vec![]
                };
                let mut items = Vec::with_capacity(node.children.len());
                for (i, item) in node.children.iter().enumerate() {
                    let want = parts.get(i).cloned();
                    let ty = self.infer(item, want.as_ref());
                    // A tuple item gets the same conversions as a list item: an
                    // Int value widens to BigInt, and a static tensor stands in
                    // for DynTensor[double], when the expected tuple says so.
                    // The tuple's item type is the expected one after that
                    // conversion, so it matches the declared tuple as a whole.
                    if let Some(want) = &want {
                        self.require(item, want, &ty);
                        items.push(self.inference.resolve(want));
                    } else {
                        items.push(ty);
                    }
                }
                Type::Tuple(items)
            }
            K::List => {
                let element = match self.value_expected(expected) {
                    Some(Type::Named(name, args)) if name == "List" && args.len() == 1 => {
                        args[0].clone()
                    }
                    _ => self.inference.fresh(),
                };
                for item in &node.children {
                    let ty = self.infer(item, Some(&element));
                    self.require(item, &element, &ty);
                }
                Type::applied("List", vec![self.inference.resolve(&element)])
            }
            K::Binary => self.binary(node, expected),
            K::Unary => {
                let operator = self.text(&node.children[0]).to_owned();
                let direct_literal =
                    operator == "-" && ungroup(&node.children[1]).kind == K::Literal;
                let previous = self.allow_min_literal;
                self.allow_min_literal = direct_literal;
                let ty = self.infer(&node.children[1], expected);
                self.allow_min_literal = previous;
                let resolved = self.inference.resolve(&ty);
                if operator == "!" {
                    self.require(node, &Type::named("Bool"), &ty);
                    return ty;
                }
                // Bitwise operators work on Int only, as the binary forms do.
                if operator == "~" {
                    self.require(node, &Type::named("Int"), &ty);
                    return Type::named("Int");
                }
                // `-5` where a C integer is expected is a negative C literal.
                if operator == "-"
                    && direct_literal
                    && let Type::Named(c_name, c_args) = &resolved
                    && c_args.is_empty()
                    && crate::interop::C_TYPES.contains(&c_name.as_str())
                {
                    let text = format!("-{}", self.text(ungroup(&node.children[1])));
                    if !c_literal_fits(c_name, &text) {
                        self.error(
                            node,
                            "E_INTEGER_LITERAL_RANGE",
                            format!("{text} does not fit in a {c_name}."),
                            "Use a value inside the C type's range.",
                        );
                    }
                    return resolved;
                }
                if operator == "-" && !numeric(&resolved) && !matches!(resolved, Type::Invalid) {
                    if crate::types::is_variable(&resolved) {
                        self.record_unary_operator(
                            node,
                            &resolved,
                            Type::named("Variable"),
                            ("Variable", "negate"),
                        );
                        return Type::named("Variable");
                    }
                    if self.satisfy_trait(&resolved, &Type::named("Negatable")) {
                        let output = self
                            .trait_output(&resolved, "Negatable", &[])
                            .unwrap_or_else(|| resolved.clone());
                        self.record_unary_operator(
                            node,
                            &resolved,
                            output.clone(),
                            ("Negatable", "negate"),
                        );
                        return output;
                    }
                    self.error(
                        node,
                        "E_OPERATOR_TRAIT",
                        format!("{resolved} does not provide Negatable."),
                        "Declare has Negatable and write negate(), or use a numeric value.",
                    );
                    return Type::Invalid;
                }
                if !numeric(&resolved) && !matches!(resolved, Type::Invalid) {
                    self.error(
                        node,
                        "E_OPERATOR_TYPE",
                        format!("Operator `{operator}` cannot use {ty}."),
                        "Use a numeric value, or Bool with !.",
                    );
                }
                ty
            }
            K::Call => self.call(node, expected),
            K::Member => {
                let previous = std::mem::replace(&mut self.member_expected, expected.cloned());
                let ty = self.member(node);
                self.member_expected = previous;
                ty
            }
            K::Index => {
                let head = self.infer(&node.children[0], None);
                if matches!(
                    self.inference.resolve(&head),
                    Type::Function(_) | Type::Overloaded(_)
                ) {
                    self.error(
                        node,
                        "E_CALL_TYPE_ARGUMENTS",
                        "Function calls do not take explicit type arguments.",
                        "Let arguments or an annotation on the binding determine the types.",
                    );
                    return Type::Invalid;
                }
                if crate::types::is_variable(&self.inference.resolve(&head)) {
                    return self.variable_index(node, &self.inference.resolve(&head));
                }
                if crate::types::is_tensor(&self.inference.resolve(&head)) {
                    return self.tensor_index(node, &self.inference.resolve(&head));
                }
                // `s[1:3]` and `xs[2:]` take part of a String or List, as tensors do.
                if node.children[1].kind == K::Slice {
                    let base = match self.inference.resolve(&head) {
                        Type::Borrow(inner) | Type::StoredView(inner) => *inner,
                        other => other,
                    };
                    let sliceable = base == Type::named("String")
                        || matches!(&base, Type::Named(name, args) if name == "List" && args.len() == 1);
                    if !sliceable {
                        self.error(
                            &node.children[1],
                            "E_SLICE_TYPE",
                            format!("{base} cannot be sliced with `a:b`."),
                            "Slice a String, a List or a Tensor.",
                        );
                        return Type::Invalid;
                    }
                    for bound in &node.children[1].children {
                        if bound.kind != K::Hole {
                            let ty = self.infer(bound, Some(&Type::named("Int")));
                            self.require(bound, &Type::named("Int"), &ty);
                        }
                    }
                    return base;
                }
                // A class opts into brackets with Indexable[Key, Item]: the key argument
                // selects the instance, and Item is the result (feature: trait indexing).
                if let Some(result) = self.indexable(node, &head) {
                    return result;
                }
                let index = self.infer(&node.children[1], Some(&Type::named("Int")));
                self.require(&node.children[1], &Type::named("Int"), &index);
                match self.inference.resolve(&head) {
                    Type::Named(name, args) if name == "Array" && args.len() == 1 => {
                        args[0].clone()
                    }
                    // A tuple's items have different types, so its index must be a literal that names one of them (Q128).
                    Type::Tuple(parts) => {
                        let position = (node.children[1].kind == K::Literal)
                            .then(|| self.text(&node.children[1]).parse::<usize>().ok())
                            .flatten();
                        match position {
                            Some(position) if position < parts.len() => parts[position].clone(),
                            _ => {
                                self.error(
                                    node,
                                    "E_TUPLE_INDEX",
                                    format!("A tuple index must be a literal from 0 to {}.", parts.len().saturating_sub(1)),
                                    "Write the position as a number, such as pair[0], since each item can have its own type.",
                                );
                                Type::Invalid
                            }
                        }
                    }
                    // s[i] reads one character as a String, like Array brackets (chosen 2026-11-08).
                    Type::Named(name, args) if name == "String" && args.is_empty() => {
                        Type::named("String")
                    }
                    Type::Invalid => Type::Invalid,
                    other => {
                        self.error(
                            node,
                            "E_INDEX_TYPE",
                            format!("{other} cannot be indexed with brackets."),
                            "Brackets index an Array or a String. Use get for a Map, and a pattern or a for loop for a List.",
                        );
                        Type::Invalid
                    }
                }
            }
            K::BracketApplication => {
                let head = self.infer(&node.children[0], None);
                let args = node
                    .children
                    .iter()
                    .skip(1)
                    .map(|n| self.annotation(n, false))
                    .collect::<Vec<_>>();
                if let Type::Constructor(name, _) = head {
                    Type::Constructor(name, args)
                } else {
                    self.error(
                        node,
                        "E_CALL_TYPE_ARGUMENTS",
                        "Function calls do not take explicit type arguments.",
                        "Let arguments or an annotation on the binding determine the types.",
                    );
                    Type::Invalid
                }
            }
            K::Lambda => self.lambda(node, expected),
            K::Pipe => {
                // A conversion name is callable as a pipe target even though it
                // has no first-class value: `1 |> double`, `[0, 0] |> Tensor`.
                if let Some(result) = self.conversion_pipe(node, expected) {
                    return result;
                }
                // `xs |> (v => v.sum())`: the lambda's parameter is the left side's type,
                // so the left side is checked first (REVIEW2 section 2).
                if ungroup(&node.children[2]).kind == K::Lambda {
                    let value = self.infer(&node.children[0], None);
                    let result = self.inference.fresh();
                    let lambda_type = Type::function(vec![value.clone()], result);
                    let callable = self.infer(&node.children[2], Some(&lambda_type));
                    let argument = (None, value, &node.children[0], false);
                    return self.apply(node, callable, &[argument], expected);
                }
                let previous_callee = self.direct_callee.replace(ungroup(&node.children[2]).span);
                let callable = self.infer(&node.children[2], None);
                self.direct_callee = previous_callee;
                // The left side fills the callee's first open parameter, so it
                // sees that parameter as its expected type. This keeps pipes and
                // direct calls equivalent for literals, lists, tuples and lambdas.
                let preview_type = self.preview_expected(&callable, expected);
                let preview = self.call_preview(&callable, preview_type.as_ref());
                let hint = preview
                    .as_ref()
                    .and_then(|function| function.parameters.first())
                    .map(|parameter| self.inference.resolve(&parameter.ty))
                    .or_else(|| match self.inference.resolve(&callable) {
                        Type::Overloaded(functions) => {
                            self.pipe_argument_hint(&functions, &node.children[0], expected)
                        }
                        _ => None,
                    });
                let value = self.infer(&node.children[0], hint.as_ref());
                let argument = (None, value, &node.children[0], false);
                self.apply(node, callable, &[argument], expected)
            }
            K::Block => {
                self.scopes.push(BTreeMap::new());
                let mut result = Type::named("void");
                let mut returns = false;
                for (i, child) in node.children.iter().enumerate() {
                    let ty = self.infer(
                        child,
                        if i + 1 == node.children.len() {
                            expected
                        } else {
                            None
                        },
                    );
                    if !returns {
                        result = ty.clone();
                    }
                    returns |= ty == Type::Never;
                }
                self.scopes.pop();
                if returns { Type::Never } else { result }
            }
            K::Binding => {
                self.binding(node);
                Type::named("void")
            }
            K::FieldDeclaration => self.constructor_field_declaration(node),
            K::Assignment if node.children[0].kind == K::Tuple => {
                self.destructuring_assignment(node)
            }
            K::Assignment => {
                let place = &node.children[0];
                let initializes = place.kind == K::Member
                    && self.text(ungroup(&place.children[0])) == "this"
                    && self.initializing_fields.as_ref().is_some_and(|fields| {
                        !fields.contains(self.source.slice(place.children[1].span))
                    });
                if !initializes && matches!(place.kind, K::Member | K::Index) {
                    self.require_mutable_receiver(node, &place.children[0]);
                }
                let old_target = self.assignment_target_span.replace(place.span);
                let target = self.infer(&node.children[0], None);
                self.assignment_target_span = old_target;
                if place.kind == K::Index
                    && place.children[0].kind != K::Name
                    && let Some(receiver) = self
                        .expression_types
                        .iter()
                        .rev()
                        .find(|(span, _)| *span == place.children[0].span)
                        .map(|(_, ty)| ty.clone())
                    && matches!(
                        self.inference.resolve(&receiver),
                        Type::StoredView(_) | Type::Borrow(_)
                    )
                {
                    self.error(
                        node,
                        "E_TENSOR_VIEW_WRITE",
                        "A transpose view is read-only.",
                        "Copy it first with `.copy()`, then change the copy.",
                    );
                }
                // A bracket write on a class needs MutableIndexable, beside the read trait.
                if place.kind == K::Index {
                    self.require_mutable_index(node, place);
                }
                if let Some((owner, name)) = self.static_references.get(&place.span).cloned() {
                    let mutable = self
                        .environment
                        .definitions
                        .get(&owner)
                        .is_some_and(|definition| definition.static_vars.contains(&name));
                    if mutable {
                        self.require_effect(node, "Assigning a static var");
                    } else {
                        self.error(
                            node,
                            "E_STATIC_ASSIGNMENT",
                            "A static final field cannot be assigned.",
                            "Assign it once at its declaration, or use a local value.",
                        );
                    }
                }
                let value = self.infer(&node.children[1], Some(&target));
                self.require(&node.children[1], &target, &value);
                if initializes && let Some(fields) = self.initializing_fields.as_mut() {
                    fields.insert(self.source.slice(place.children[1].span).into());
                }
                if place.kind == K::Member {
                    let receiver = self
                        .expression_types
                        .iter()
                        .rev()
                        .find(|(span, _)| *span == place.children[0].span)
                        .map(|(_, ty)| ty.clone());
                    if let Some((owner, _, mutable)) = receiver.and_then(|ty| {
                        self.field(&ty, self.text(&place.children[1]), &mut BTreeSet::new())
                    }) && !mutable
                        && (!initializes
                            || self.owner.as_ref() != Some(&owner)
                            || self.loop_depth > 0)
                    {
                        self.error(node, "E_FINAL_ASSIGNMENT", "A final field can be initialized once in its own constructor or declaration.", "Use var for a field that changes after construction.");
                    }
                }
                if node.children[0].kind == K::Name {
                    let root = self.text(&node.children[0]).to_owned();
                    self.note_mutation(&root);
                }
                if node.children[0].kind == K::Name {
                    let access = self.place_access(&node.children[1]);
                    let origin = self.resolved_place(&node.children[1]);
                    let name = self.text(&node.children[0]).to_owned();
                    let roots = self.place_roots(&node.children[1]);
                    if let Some(depth) = self
                        .scopes
                        .iter()
                        .rposition(|scope| scope.contains_key(&name))
                    {
                        self.place_aliases
                            .entry((depth, name.clone()))
                            .or_default()
                            .extend(roots);
                    }
                    if let Some(symbol) = self
                        .scopes
                        .iter_mut()
                        .rev()
                        .find_map(|scope| scope.get_mut(&name))
                        .or_else(|| self.environment.values.get_mut(&name))
                        && !crate::types::scalar(&self.inference.resolve(&symbol.ty))
                    {
                        symbol.access = if symbol.access == Some(false) || access == Some(false) {
                            Some(false)
                        } else {
                            access
                        };
                        symbol.origin = if symbol.origin == origin {
                            origin
                        } else {
                            None
                        };
                    }
                    let name = name.as_str();
                    if self
                        .scopes
                        .iter()
                        .rev()
                        .find_map(|s| s.get(name))
                        .or_else(|| self.environment.values.get(name))
                        .is_some_and(|s| !s.mutable)
                    {
                        self.error(
                            node,
                            "E_IMMUTABLE_ASSIGNMENT",
                            format!("`{name}` was not declared with let mut."),
                            "Use let mut for a variable that you need to assign again.",
                        );
                    }
                }
                Type::named("void")
            }
            K::If => {
                let condition = self.infer(&node.children[0], Some(&Type::named("Bool")));
                self.require(&node.children[0], &Type::named("Bool"), &condition);
                let before = self.initializing_fields.clone();
                let left = self.infer(&node.children[1], expected);
                let after_left = self.initializing_fields.clone();
                self.initializing_fields = before.clone();
                if node.children.len() == 2 {
                    if left != Type::Never {
                        self.initializing_fields = intersect_initialization(after_left, before);
                    }
                    return Type::named("void");
                }
                let right = self.infer(&node.children[2], expected);
                self.initializing_fields = if left == Type::Never {
                    self.initializing_fields.clone()
                } else if right == Type::Never {
                    after_left
                } else {
                    intersect_initialization(after_left, self.initializing_fields.clone())
                };
                if left == Type::Never && right == Type::Never {
                    Type::Never
                } else if let Some(expected) = expected {
                    self.require(&node.children[1], expected, &left);
                    self.require(&node.children[2], expected, &right);
                    erase_callable_identities(&self.inference.resolve(expected))
                } else if left == Type::Never {
                    erase_callable_identities(&right)
                } else if right == Type::Never {
                    erase_callable_identities(&left)
                } else {
                    if !self.inference.unify(&left, &right) {
                        self.mismatch(node, &left, &right);
                    }
                    erase_callable_identities(&self.inference.resolve(&left))
                }
            }
            K::Return => {
                let Some(expected) = self.return_type.clone() else {
                    self.error(
                        node,
                        "E_RETURN_CONTEXT",
                        "return needs an enclosing function.",
                        "Put this return inside a function.",
                    );
                    return Type::Invalid;
                };
                let ty = node
                    .children
                    .first()
                    .map(|n| self.infer(n, Some(&expected)))
                    .unwrap_or(Type::named("void"));
                // The Int value itself widens, so core converts the expression that is returned (Q125).
                if let Some(value) = node.children.first()
                    && self.widens(&expected, &ty)
                {
                    self.widened.insert(value.span, Type::named("BigInt"));
                }
                self.reject_tensor_view_return(node, &expected, &ty);
                if !self.self_default_body(&expected, &ty) {
                    self.require(node, &expected, &ty);
                }
                self.ensure_initialized(node);
                Type::Never
            }
            K::While | K::For => self.loop_statement(node),
            K::Break | K::Continue => {
                let depth = self.loop_depth;
                if let Some((barrier, exits)) = self.exit_barriers.last_mut()
                    && *barrier == depth
                {
                    exits.push(node.span);
                }
                if self.loop_depth == 0 {
                    self.error(
                        node,
                        "E_LOOP_CONTEXT",
                        "break and continue need an enclosing loop.",
                        "Put this statement inside a for or while loop.",
                    );
                }
                Type::Never
            }
            K::Match => self.match_expression(node, expected),
            K::Comprehension => {
                let collection = self.infer(&node.children[2], None);
                let element = self.iteration_item(node, &collection, &node.children[2]);
                self.scopes.push(BTreeMap::new());
                self.pattern(&node.children[1], &element, false, true);
                if let Some(guard) = node.child(K::Guard) {
                    let ty = self.infer(&guard.children[0], Some(&Type::named("Bool")));
                    self.require(guard, &Type::named("Bool"), &ty);
                }
                for clause in node
                    .children
                    .iter()
                    .filter(|n| n.kind == K::ComprehensionFor)
                {
                    let collection = self.infer(&clause.children[1], None);
                    let element = self.iteration_item(clause, &collection, &clause.children[1]);
                    self.pattern(&clause.children[0], &element, false, true);
                    if let Some(guard) = clause.child(K::Guard) {
                        let ty = self.infer(&guard.children[0], Some(&Type::named("Bool")));
                        self.require(guard, &Type::named("Bool"), &ty);
                    }
                }
                let result = self.infer(&node.children[0], None);
                self.scopes.pop();
                Type::applied("List", vec![result])
            }
            K::FormatString => {
                for interpolation in node.children.iter().filter(|n| n.kind == K::Interpolation) {
                    if let Some(value) = interpolation.children.first() {
                        let ty = self.infer(value, None);
                        if !self.implements(&ty, &Type::named("Text"), &mut BTreeSet::new()) {
                            self.error(
                                value,
                                "E_TEXT_REQUIRED",
                                format!("{ty} does not provide Text."),
                                "Declare has Text and write text() : String.",
                            );
                        }
                        // A number format takes a double, or an Int or Nat shown as one; an
                        // integer format takes Int or Nat; width and alignment take any Text.
                        if let Some(spec) = interpolation.child(K::FormatSpec) {
                            let integer = matches!(&self.inference.resolve(&ty), Type::Named(n, a) if a.is_empty() && matches!(n.as_str(), "Int" | "Nat"));
                            match crate::strings::format_kind(self.text(spec)) {
                                Some(
                                    crate::strings::FormatKind::Fixed(_)
                                    | crate::strings::FormatKind::Number,
                                ) if !integer => {
                                    self.require(spec, &Type::named("double"), &ty);
                                }
                                Some(crate::strings::FormatKind::Integer) if !integer => {
                                    self.error(
                                        spec,
                                        "E_FORMAT_SPEC",
                                        format!("This integer format needs an Int or Nat, found {}.", self.inference.resolve(&ty)),
                                        "Use d, x, X, o, b or c with an Int, or f, e, g or % with a double.",
                                    );
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Type::named("String")
            }
            K::Propagate => {
                // `?` returns Error from a function that can return it, or None from a function
                // that returns Option (Q115).
                let returns_option = self.return_type.as_ref().is_some_and(is_option_type);
                // A session entry stops on an Error that `?` meets at its top level, and a
                // script reports it and exits with status 1.
                // A named error enum such as `Int or AppError` also receives `?` (REVIEW2 1.2).
                let returns_error = self
                    .return_type
                    .clone()
                    .is_some_and(|ty| self.returns_some_error(&ty))
                    || self.return_type.is_none()
                        && (self.script_level || self.source.name == "<session>");
                if !returns_option && !returns_error {
                    self.error(node, "E_PROPAGATION_RETURN", "? needs an enclosing function whose return type includes Error or is Option.", "Return a value-or-Error type or an Option from this function, or match the value here.");
                }
                // `let s : String = py.str(v)?` tells the call it should give `String or Error`.
                let inner = expected
                    .filter(|ty| !contains_error(ty) && **ty != Type::Invalid)
                    .map(|ty| {
                        if returns_option && !returns_error {
                            Type::applied("Option", vec![ty.clone()])
                        } else {
                            Type::union(ty.clone(), Type::named("Error"))
                        }
                    });
                let ty = self.infer(&node.children[0], inner.as_ref());
                // An expected type can make a call that cannot fail look like `T or Error`;
                // the call's own declared result decides (REVIEW2 1.3).
                let operand = ungroup(&node.children[0]);
                let cannot_fail = matches!(operand.kind, K::Call | K::Pipe)
                    && !self.python.contains_key(&operand.span)
                    && self.resolved_calls.get(&operand.span).is_some_and(|call| {
                        let result = self.inference.resolve(&call.signature.result);
                        !crate::types::is_variable(&result)
                            && !matches!(&result, Type::Variable(_))
                            && !is_option_type(&result)
                            && !matches!(&result, Type::Union(parts) if parts.iter().any(|p| self.is_error_type(p)))
                            && !self.is_error_type(&result)
                            && result != Type::Invalid
                    });
                if cannot_fail {
                    let found = self
                        .resolved_calls
                        .get(&operand.span)
                        .map(|call| self.inference.resolve(&call.signature.result))
                        .unwrap_or(Type::Invalid);
                    self.error(
                        node,
                        "E_PROPAGATION_TYPE",
                        format!("Expected a value-or-Error type before ?, found {found}."),
                        "This call cannot fail, so it needs no ?.",
                    );
                    return found;
                }
                match self.inference.resolve(&ty) {
                    Type::Named(name, args) if name == "Option" && args.len() == 1 => {
                        if !returns_option {
                            self.error(
                                node,
                                "E_PROPAGATION_OPTION",
                                "? on an Option needs an enclosing function that returns Option.",
                                "Return an Option from this function, or match the Option here.",
                            );
                            return Type::Invalid;
                        }
                        args[0].clone()
                    }
                    Type::Union(parts) if parts.iter().any(|ty| self.is_error_type(ty)) => {
                        if !returns_error {
                            self.error(node, "E_PROPAGATION_RETURN", "? on an Error needs an enclosing function whose return type includes Error.", "Return a value-or-Error type, or match the Error here.");
                        } else if let Some(result) = self.return_type.clone() {
                            // A closed error enum receives only its own errors; the open
                            // Error root receives every one.
                            let accepted: Vec<Type> = match self.inference.resolve(&result) {
                                Type::Union(result_parts) => result_parts
                                    .into_iter()
                                    .filter(|p| self.is_error_type(p))
                                    .collect(),
                                other if self.is_error_type(&other) => vec![other],
                                _ => vec![],
                            };
                            let open = accepted
                                .iter()
                                .any(|p| matches!(p, Type::Named(n, _) if n == "Error"));
                            if !accepted.is_empty() && !open {
                                // The call's declared result names its real error type; the
                                // expectation above may have widened it to the open root.
                                let declared = self
                                    .resolved_calls
                                    .get(&operand.span)
                                    .map(|call| self.inference.resolve(&call.signature.result));
                                let source_parts = match declared {
                                    Some(Type::Union(declared_parts)) => declared_parts,
                                    _ => parts.clone(),
                                };
                                let errors: Vec<Type> = source_parts
                                    .iter()
                                    .filter(|p| self.is_error_type(p))
                                    .cloned()
                                    .collect();
                                for part in &errors {
                                    if !accepted.iter().any(|a| a.same_type(part)) {
                                        self.error(
                                            node,
                                            "E_PROPAGATION_TYPE",
                                            format!("? would pass on an error of type {part}, but this function returns only {}.", accepted.iter().map(|a| a.to_string()).collect::<Vec<_>>().join(" or ")),
                                            "Match the error and convert it, or return the open Error type.",
                                        );
                                    }
                                }
                            }
                        }
                        let values: Vec<_> = parts
                            .into_iter()
                            .filter(|ty| !self.is_error_type(ty))
                            .collect();
                        if values.len() == 1 {
                            values[0].clone()
                        } else {
                            Type::Union(values)
                        }
                    }
                    Type::Invalid => Type::Invalid,
                    found => {
                        self.error(
                            node,
                            "E_PROPAGATION_TYPE",
                            format!("Expected a value-or-Error type before ?, found {found}."),
                            "Use ? with a call that can return Error.",
                        );
                        Type::Invalid
                    }
                }
            }
            K::Unsafe => {
                self.unsafe_depth += 1;
                let ty = self.infer(&node.children[0], expected);
                self.unsafe_depth -= 1;
                ty
            }
            // The proof that a block's condition holds. It has no run-time value (Q115).
            K::ProofWitness => {
                let effects = std::mem::replace(&mut self.effects_allowed, false);
                let receiver = std::mem::replace(&mut self.receiver_mutable, false);
                let parameters = self.borrowed_parameters.clone();
                for mutable in self.borrowed_parameters.values_mut() {
                    *mutable = false;
                }
                let condition = self.infer(&node.children[0], Some(&Type::named("Bool")));
                self.effects_allowed = effects;
                self.receiver_mutable = receiver;
                self.borrowed_parameters = parameters;
                if self.inference.resolve(&condition) != Type::named("Bool") {
                    self.error(
                        &node.children[0],
                        "E_TYPE",
                        "A named proof needs a Bool condition.",
                        "Name a condition such as n > 0.",
                    );
                }
                Type::named("Proof")
            }
            K::Using => self.using(node, expected),
            K::Scope => {
                let fails = std::mem::replace(&mut self.scope_fails, false);
                self.scope_depth += 1;
                self.exit_barriers.push((self.loop_depth, vec![]));
                for child in &node.children {
                    self.infer(child, None);
                }
                self.scope_depth -= 1;
                let found = std::mem::replace(&mut self.scope_fails, fails);
                self.close_exit_barrier(found, "a scope whose tasks can fail");
                // The scope waits for every task, then gives the first Error (confirmed 2026-11-03).
                if found {
                    fn returns(node: &Node) -> bool {
                        node.kind == K::Return
                            || !matches!(node.kind, K::Function | K::Lambda)
                                && node.children.iter().any(returns)
                    }
                    if node.children.iter().any(returns)
                        && !self.return_type.as_ref().is_some_and(contains_error)
                    {
                        self.error(node,"E_PROPAGATION_RETURN","Returning from a fallible task scope needs Error in the function return type.","Include Error so a joined task's failure can be returned.");
                    }
                    Type::union(Type::named("void"), Type::named("Error"))
                } else {
                    Type::named("void")
                }
            }
            K::Spawn => {
                self.spawn(node);
                Type::named("void")
            }
            K::OperatorDeclaration => Type::named("void"),
            K::Record => {
                self.error(
                    node,
                    "E_UNSUPPORTED_CHECK",
                    "Checking this form is not implemented yet.",
                    "See the limits section of README.md for the remaining checking work.",
                );
                Type::Invalid
            }
            K::Function => {
                let signature = self.signature(node);
                let name = self.named_child(node);
                self.bind(node, &name, Type::Function(signature.clone()), false, true);
                self.function_body(node, &signature, self.owner.clone());
                Type::named("void")
            }
            K::Error => Type::Invalid,
            K::Hole => {
                self.error(
                    node,
                    "E_HOLE_CONTEXT",
                    "An open slot must be a direct argument of a call.",
                    "Write a lambda for an expression with an open operand.",
                );
                Type::Invalid
            }
            _ => {
                self.error(
                    node,
                    "E_UNSUPPORTED_CHECK",
                    format!("Checking {:?} is not implemented yet.", node.kind),
                    "Parsing this form does not certify its type.",
                );
                Type::Invalid
            }
        }
    }

    fn binding(&mut self, node: &Node) {
        let Some(pattern) = node.children.iter().find(|n| {
            matches!(
                n.kind,
                K::Name
                    | K::PatternName
                    | K::PatternTuple
                    | K::PatternList
                    | K::PatternConstructor
                    | K::PatternWildcard
            )
        }) else {
            return;
        };
        let expected = node
            .children
            .iter()
            .find(|n| is_type(n.kind))
            .map(|n| self.annotation(n, false));
        if let Some(expected) = &expected {
            self.validate_type(node, expected);
            self.validate_borrow_position(node, expected, false);
        }
        let Some(value) = node.children.iter().rev().find(|n| {
            n.kind != K::Otherwise
                && n.kind != K::Modifier
                && !is_type(n.kind)
                && !std::ptr::eq(*n, pattern)
        }) else {
            return;
        };
        let ty = self.infer(value, expected.as_ref());
        if let Some(expected) = &expected {
            self.require(value, expected, &ty);
        }
        let ty = expected.unwrap_or(ty);
        if let Some(otherwise) = node.child(K::Otherwise) {
            if irrefutable(pattern) {
                self.error(
                    otherwise,
                    "E_OTHERWISE_IRREFUTABLE",
                    "Every value matches this pattern, so the otherwise block never runs.",
                    "Remove otherwise, or use a pattern that can fail.",
                );
            }
            let branch = self.infer(&otherwise.children[0], None);
            if branch != Type::Never {
                self.error(
                    otherwise,
                    "E_OTHERWISE_EXIT",
                    "The otherwise block must leave this path before pattern names can be used.",
                    "Return, break or continue from the failing branch.",
                );
            }
        } else if !irrefutable(pattern) {
            self.error(
                pattern,
                "E_REFUTABLE_BINDING",
                "This pattern can fail, but the binding has no otherwise block.",
                "Add `otherwise { ... }` that leaves this path.",
            );
        }
        let previous = std::mem::replace(&mut self.narrow_unions, false);
        self.pattern(
            pattern,
            &ty,
            self.modifier(node, "mut"),
            self.non_expansive(value),
        );
        self.narrow_unions = previous;
        self.record_binding_access(pattern, value);
    }

    fn non_expansive(&self, node: &Node) -> bool {
        match node.kind {
            K::Name | K::Literal | K::Hole | K::Lambda => true,
            K::Group | K::Tuple | K::List | K::Member | K::Argument | K::NamedArgument => {
                node.children.iter().all(|child| self.non_expansive(child))
            }
            K::Call => {
                let partial =
                    node.children.iter().skip(1).any(|argument| {
                        argument.children.last().is_some_and(|n| n.kind == K::Hole)
                    });
                partial && node.children.iter().all(|child| self.non_expansive(child))
            }
            _ => false,
        }
    }

    fn lambda(&mut self, node: &Node, expected: Option<&Type>) -> Type {
        if node.children.last().is_some_and(|body| {
            body.descendants()
                .any(|part| part.kind == K::Name && self.text(part) == "this")
        }) {
            self.require_initialized_this(node);
        }
        let expected = expected.map(|t| self.inference.resolve(t));
        let expected = if let Some(Type::Function(fun)) = expected {
            Some(fun)
        } else {
            None
        };
        let mut parameters = vec![];
        let old_parameters = self.borrowed_parameters.clone();
        self.lambda_mutations.push((self.scopes.len(), false));
        self.scopes.push(BTreeMap::new());
        if let Some(list) = node.child(K::LambdaParameters) {
            for (i, parameter) in list.children.iter().enumerate() {
                let name = self.named_child(parameter);
                let mut ty =
                    if let Some(annotation) = parameter.children.iter().find(|n| is_type(n.kind)) {
                        self.annotation(annotation, false)
                    } else if let Some(p) = expected.as_ref().and_then(|f| f.parameters.get(i)) {
                        p.ty.clone()
                    } else {
                        self.inference.fresh()
                    };
                // An unmarked parameter takes `mut` from the expected function type, so
                // `m.update(key, c => c.inc())` needs no repeated marker.
                let passing = if self.modifier(parameter, "move") {
                    Passing::Move
                } else if self.modifier(parameter, "mut")
                    || expected
                        .as_ref()
                        .and_then(|f| f.parameters.get(i))
                        .is_some_and(|p| p.passing == Passing::Mut)
                {
                    Passing::Mut
                } else {
                    Passing::Borrow
                };
                if passing == Passing::Mut
                    && let Type::Function(function) = &mut ty
                {
                    function.receiver_mutates = true;
                }
                self.bind(parameter, &name, ty.clone(), false, false);
                if passing != Passing::Move {
                    self.borrowed_parameters
                        .insert(name.clone(), passing == Passing::Mut);
                }
                parameters.push(Parameter {
                    name: Some(name),
                    ty,
                    default: false,
                    passing,
                });
            }
        }
        let annotated = node
            .child(K::ReturnType)
            .and_then(|n| n.children.first())
            .map(|n| self.annotation(n, false));
        let result = annotated
            .or_else(|| expected.as_ref().map(|f| *f.result.clone()))
            .unwrap_or_else(|| self.inference.fresh());
        let previous = self.return_type.replace(result.clone());
        let old_effects = (self.effects_allowed, self.effects_observed);
        self.effects_allowed = true;
        self.effects_observed = false;
        let old_initializing = self.initializing_fields.take();
        let old_loop = std::mem::replace(&mut self.loop_depth, 0);
        let old_barriers = std::mem::take(&mut self.exit_barriers);
        let old_scope = std::mem::replace(&mut self.scope_depth, 0);
        let found = self.infer(node.children.last().unwrap(), Some(&result));
        self.require(node.children.last().unwrap(), &result, &found);
        self.return_type = previous;
        self.borrowed_parameters = old_parameters;
        let effects = self.effects_observed;
        (self.effects_allowed, self.effects_observed) = old_effects;
        self.initializing_fields = old_initializing;
        self.loop_depth = old_loop;
        self.exit_barriers = old_barriers;
        self.scope_depth = old_scope;
        self.scopes.pop();
        let (_, mutates) = self.lambda_mutations.pop().unwrap_or_default();
        Type::Function(FunctionType {
            parameters,
            result: Box::new(erase_callable_identities(&self.inference.resolve(&result))),
            bounds: BTreeMap::new(),
            requirements: vec![],
            effects,
            receiver_mutates: mutates,
            callable: Some(Callable::Closure(DeclarationId {
                file: self.source.name.clone(),
                span: node.span,
            })),
            receiver: None,
        })
    }

    fn record_unary_operator(
        &mut self,
        node: &Node,
        receiver: &Type,
        result: Type,
        protocol: (&str, &str),
    ) {
        let (protocol, method) = protocol;
        let signature = self
            .methods(receiver, method, &mut BTreeSet::new())
            .into_iter()
            .find(|function| function.parameters.is_empty() && !function.receiver_mutates);
        let mut signature =
            signature.unwrap_or_else(|| match Type::function(vec![], result.clone()) {
                Type::Function(function) => function,
                _ => unreachable!(),
            });
        signature.receiver = Some(Box::new(receiver.clone()));
        signature.result = Box::new(result);
        let target = signature
            .callable
            .clone()
            .unwrap_or_else(|| Callable::Builtin(format!("std.core.{protocol}.{method}")));
        signature.callable = Some(target.clone());
        self.resolved_calls.insert(
            node.span,
            ResolvedCall {
                target,
                callee: None,
                overload: 0,
                signature,
                type_arguments: BTreeMap::new(),
                arguments: vec![],
                evaluation_order: vec![node.children[1].span],
                receiver: Some(node.children[1].span),
                partial: false,
            },
        );
    }

    fn record_operator(
        &mut self,
        node: &Node,
        receiver: &Type,
        operand: &Type,
        result: Type,
        protocol: (&str, &str),
        right_dispatch: bool,
    ) {
        let (protocol, method) = protocol;
        let signature = self
            .methods(receiver, method, &mut BTreeSet::new())
            .into_iter()
            .find(|function| {
                function.parameters.len() == 1
                    && self
                        .inference
                        .clone()
                        .unify(&function.parameters[0].ty, operand)
            });
        let mut signature = signature.unwrap_or_else(|| {
            match Type::function(vec![operand.clone()], result.clone()) {
                Type::Function(function) => function,
                _ => unreachable!(),
            }
        });
        signature.receiver = Some(Box::new(receiver.clone()));
        signature.result = Box::new(result);
        let target = signature
            .callable
            .clone()
            .unwrap_or_else(|| Callable::Builtin(format!("std.core.{protocol}.{method}")));
        signature.callable = Some(target.clone());
        let (receiver, argument) = if right_dispatch {
            (node.children[2].span, node.children[0].span)
        } else {
            (node.children[0].span, node.children[2].span)
        };
        self.resolved_calls.insert(
            node.span,
            ResolvedCall {
                target,
                callee: None,
                overload: 0,
                signature,
                type_arguments: BTreeMap::new(),
                arguments: vec![ResolvedArgument::Supplied {
                    expression: argument,
                    hole: false,
                }],
                evaluation_order: vec![node.children[0].span, node.children[2].span],
                receiver: Some(receiver),
                partial: false,
            },
        );
    }

    fn binary(&mut self, node: &Node, expected: Option<&Type>) -> Type {
        let operator = self.text(&node.children[1]).to_owned();
        let bigint = Type::named("BigInt");
        let double = Type::named("double");
        let value_expected = self.value_expected(expected);
        let expected_double = value_expected.as_ref() == Some(&double);
        let wanted = value_expected.as_ref().filter(|ty| bigint_context(ty));
        let mut left = self.infer(
            &node.children[0],
            if expected_double {
                Some(&double)
            } else {
                wanted
            },
        );
        // A numeral beside a Nat is a Nat (Q71).
        let nat_left = self.inference.resolve(&left) == Type::named("Nat");
        let big_left = self.inference.resolve(&left) == bigint;
        let nat_wanted = nat_left.then(|| Type::named("Nat"));
        let right_double = expected_double
            || (self.inference.resolve(&left) == double
                && int_literal_expression(&node.children[2], self.source));
        let mut right = self.infer(
            &node.children[2],
            if big_left {
                Some(&bigint)
            } else if right_double {
                Some(&double)
            } else {
                nat_wanted.as_ref()
            },
        );
        // An integer literal adopts double when the other operand is already double.
        if self.inference.resolve(&left) == double
            && int_literal_expression(&node.children[2], self.source)
        {
            right = self.infer(&node.children[2], Some(&double));
        }
        if self.inference.resolve(&right) == double
            && int_literal_expression(&node.children[0], self.source)
        {
            left = self.infer(&node.children[0], Some(&double));
        }
        if self.inference.resolve(&right) == bigint
            && !big_left
            && bigint_literal_expression(&node.children[0], self.source)
        {
            left = self.infer(&node.children[0], Some(&bigint));
        }
        if self.inference.resolve(&right) == Type::named("Nat")
            && node.children[0].kind == K::Literal
            && !nat_left
        {
            left = self.infer(&node.children[0], Some(&Type::named("Nat")));
        }
        if matches!(operator.as_str(), "+" | "-" | "*" | "/") {
            let left_tensor = crate::types::is_tensor(&self.inference.resolve(&left));
            let right_tensor = crate::types::is_tensor(&self.inference.resolve(&right));
            if left_tensor
                && !right_tensor
                && int_literal_expression(&node.children[2], self.source)
            {
                right = self.infer(&node.children[2], Some(&double));
            }
            if right_tensor
                && !left_tensor
                && int_literal_expression(&node.children[0], self.source)
            {
                left = self.infer(&node.children[0], Some(&double));
            }
        }
        if crate::types::is_variable(&self.inference.resolve(&left))
            || crate::types::is_variable(&self.inference.resolve(&right))
        {
            return self.variable_binary(
                node,
                &operator,
                &self.inference.resolve(&left),
                &self.inference.resolve(&right),
            );
        }
        if crate::types::is_tensor(&self.inference.resolve(&left))
            || crate::types::is_tensor(&self.inference.resolve(&right))
            || operator == "@"
        {
            return self.tensor_binary(
                node,
                &operator,
                &self.inference.resolve(&left),
                &self.inference.resolve(&right),
            );
        }
        // Python's repetition: a String times an Int, in either order, is a String (Q136).
        if operator == "*" {
            let (l, r) = (
                self.inference.resolve(&left),
                self.inference.resolve(&right),
            );
            let string = Type::named("String");
            let int = Type::named("Int");
            if l == string && r == int || l == int && r == string {
                return string;
            }
        }
        // Q125: an Int operand widens beside a BigInt operand, so both operands have one type.
        let (left, right) = self.widen_operands(node, left, right);
        let equality = matches!(operator.as_str(), "==" | "!=");
        if operator_family(&operator).is_some() {
            for child in [&node.children[0], &node.children[2]] {
                if child.kind == K::Binary {
                    let other = self.text(&child.children[1]);
                    // A comparison may sit directly inside && and || (Q11).
                    let comparison_in_logic =
                        operator_family(&operator) == Some(3) && operator_family(other) == Some(2);
                    // Arithmetic under a comparison needs no parentheses: `v % 2 == 0` (user request). Bitwise
                    // operators still need them, since `a & b == c` reads differently in other languages.
                    let arithmetic_in_comparison =
                        operator_family(&operator) == Some(2) && operator_family(other) == Some(0);
                    if operator_family(&operator) != operator_family(other)
                        && !comparison_in_logic
                        && !arithmetic_in_comparison
                    {
                        self.error(
                            node,
                            "E_OPERATOR_MIX",
                            "Operators from different families need parentheses.",
                            "Parenthesize each arithmetic, bitwise, comparison or logical group.",
                        );
                        break;
                    }
                }
            }
        }
        if equality {
            // A borrowed view compares with a value of its type: `m.get(k) == Some(v)`.
            let views = |ty: &Type| {
                ty.map(&mut |part| match part {
                    Type::StoredView(inner) | Type::Borrow(inner) => Some(inner.as_ref().clone()),
                    _ => None,
                })
            };
            let same_without_views = {
                let mut probe = self.inference.clone();
                let (l, r) = (
                    views(&self.inference.resolve(&left)),
                    views(&self.inference.resolve(&right)),
                );
                probe.unify(&l, &r)
            };
            if same_without_views && !self.inference.clone().unify(&left, &right) {
                // Accepted: core compares the values the views reach.
            } else if !self.inference.unify(&left, &right) {
                self.error(
                    node,
                    "E_EQUALITY_TYPE",
                    format!(
                        "Equality requires the same type on both sides; expected {}, found {}.",
                        self.inference.resolve(&left),
                        self.inference.resolve(&right)
                    ),
                    "Convert one value explicitly before comparing.",
                );
            } else if !self.implements(
                &self.inference.resolve(&left),
                &Type::named("Equals"),
                &mut BTreeSet::new(),
            ) {
                let resolved = self.inference.resolve(&left);
                let structural = matches!(&resolved, Type::Named(name, _)
                if self.environment.definitions.get(name).is_some_and(|definition| {
                    definition.kind == K::Enum && definition.parent.is_none()
                        || definition.kind == K::Class && definition.record
                }));
                if structural {
                    self.error(
                        node,
                        "E_OPERATOR_TRAIT",
                        format!("{resolved} compares by its fields, but one of them does not provide Equals."),
                        "Give that field's type an Equals instance, or declare has Equals here and write equals yourself.",
                    );
                } else {
                    self.error(
                        node,
                        "E_OPERATOR_TRAIT",
                        format!("{left} does not declare Equals."),
                        "Declare has Equals and write equals with the matching type.",
                    );
                }
            }
            let left = self.inference.resolve(&left);
            let right = self.inference.resolve(&right);
            if self.implements(&left, &Type::named("Equals"), &mut BTreeSet::new()) {
                self.record_operator(
                    node,
                    &left,
                    &right,
                    Type::named("Bool"),
                    ("Equals", "equals"),
                    false,
                );
            }
            return Type::named("Bool");
        }
        if matches!(operator.as_str(), "&&" | "||") {
            self.require(node, &Type::named("Bool"), &left);
            self.require(node, &Type::named("Bool"), &right);
            return Type::named("Bool");
        }
        if operator == "*"
            && ((matches!(self.inference.resolve(&left), Type::Variable(_))
                && numeric(&self.inference.resolve(&right)))
                || (matches!(self.inference.resolve(&right), Type::Variable(_))
                    && numeric(&self.inference.resolve(&left))))
        {
            self.inference.unify(&left, &right);
        }
        if let Some(&(_, left_trait, left_method, right_trait, right_method)) =
            OPERATOR_TRAITS.iter().find(|entry| entry.0 == operator)
        {
            let left_resolved = self.inference.resolve(&left);
            let right_resolved = self.inference.resolve(&right);
            // Classes, enums and bounded parameters use their operator traits.
            // Numbers, Nat and String concatenation keep their built-in rules.
            let user = |ty: &Type| {
                matches!(ty, Type::Parameter(_) | Type::Rigid(_))
                    || matches!(ty, Type::Named(name, _) if name != "Nat" && self.environment.definitions.get(name).is_some_and(|d| matches!(d.kind, K::Class | K::Enum)))
            };
            // Nat is a built-in number too, so Nat `*` takes the built-in rule like Nat `+` (Q129).
            let built_in = |ty: &Type| numeric(ty) || *ty == Type::named("Nat");
            let dispatch = if built_in(&left_resolved) && built_in(&right_resolved)
                || operator != "*" && !user(&left_resolved) && !user(&right_resolved)
            {
                None
            } else if builtin_value(&left_resolved) {
                Some((&right_resolved, right_trait, &left_resolved))
            } else {
                Some((&left_resolved, left_trait, &right_resolved))
            };
            if let Some((receiver, name, operand)) = dispatch {
                let output = self.inference.fresh();
                let requirement = associated(
                    Type::applied(name, vec![operand.clone()]),
                    [("Output", output.clone())],
                );
                let (matched, output) = if self.satisfy_trait(receiver, &requirement) {
                    (true, self.inference.resolve(&output))
                } else {
                    let open = Type::applied(name, vec![operand.clone()]);
                    if self.satisfy_trait(receiver, &open) {
                        let output = self
                            .trait_output(receiver, name, std::slice::from_ref(operand))
                            .unwrap_or_else(|| receiver.clone());
                        (true, output)
                    } else {
                        (false, Type::Invalid)
                    }
                };
                if matched {
                    self.record_operator(
                        node,
                        receiver,
                        operand,
                        output.clone(),
                        (
                            name,
                            if name == left_trait {
                                left_method
                            } else {
                                right_method
                            },
                        ),
                        name == right_trait,
                    );
                    return output;
                }
                let hint = format!(
                    "Declare has {left_trait} and write {left_method}(other). The right-side form {right_trait} with {right_method}(left) is used only when the left operand is a built-in value."
                );
                self.error(
                    node,
                    "E_OPERATOR_TRAIT",
                    format!("{receiver} does not provide {requirement}."),
                    &hint,
                );
                return Type::Invalid;
            }
        }
        if !self.inference.unify(&left, &right) {
            let left_resolved = self.inference.resolve(&left);
            let right_resolved = self.inference.resolve(&right);
            if numeric(&left_resolved) && numeric(&right_resolved) {
                self.error(node, "E_NUMERIC_TYPE", format!("Operator `{operator}` requires matching numeric types; expected {left_resolved}, found {right_resolved}."), "Convert explicitly, or use matching literals such as 0.0 for double.");
            } else {
                self.error(node, "E_OPERATOR_TRAIT", format!("The left operand {left_resolved} cannot apply `{operator}` to {right_resolved}."), "For a class or enum, declare has Addable, Subtractable, Multipliable or Divisible and write add, subtract, multiply or divide; a built-in left operand uses the Right forms.");
            }
            return Type::Invalid;
        }
        let ty = self.inference.resolve(&left);
        if matches!(operator.as_str(), "<" | "<=" | ">" | ">=")
            && !numeric(&ty)
            && ty != Type::named("Nat")
        {
            if !self.implements(&ty, &Type::named("Comparable"), &mut BTreeSet::new()) {
                self.error(
                    node,
                    "E_OPERATOR_TRAIT",
                    format!("{ty} does not provide Comparable for `{operator}`."),
                    "Implement a pure compare(other): Int method and declare has Comparable.",
                );
            }
            return Type::named("Bool");
        }
        // Q73: Nat has checked plain addition/subtraction, like Int.
        // Nat `/` and `%` round down and stop on a zero divisor, like Int (REVIEW2 section 3).
        let nat_comparison = ty == Type::named("Nat")
            && matches!(
                operator.as_str(),
                "+" | "-" | "*" | "/" | "%" | "<" | "<=" | ">" | ">="
            );
        // `xs + ys` is a new List with the items of both (REVIEW2 section 4).
        let list_concat = operator == "+"
            && matches!(&ty, Type::Named(name, args) if name == "List" && args.len() == 1);
        if !(numeric(&ty)
            || nat_comparison
            || list_concat
            || operator == "+" && ty == Type::named("String"))
            && ty != Type::Invalid
        {
            self.error(node, "E_OPERATOR_TRAIT", format!("{ty} has no specified implementation of `{operator}`."), "Declare the appropriate trait. Operator method contracts still need specification in Q17.");
        }
        if matches!(operator.as_str(), "&" | "|" | "^" | "<<" | ">>") {
            self.require(node, &Type::named("Int"), &ty);
        }
        if matches!(operator.as_str(), "<" | "<=" | ">" | ">=") {
            Type::named("Bool")
        } else {
            ty
        }
    }

    fn overload_argument_hint(
        &self,
        functions: &[FunctionType],
        inputs: &[Node],
        arguments: &InferredArguments<'_>,
        index: usize,
        expected: Option<&Type>,
    ) -> Option<Type> {
        let skeleton = inputs
            .iter()
            .enumerate()
            .map(|(i, input)| {
                let value = input.children.last().unwrap();
                (
                    arguments[i].as_ref().and_then(|a| a.0.clone()).or_else(|| {
                        (input.kind == K::NamedArgument)
                            .then(|| self.text(&input.children[0]).to_owned())
                    }),
                    Type::Invalid,
                    value,
                    value.kind == K::Hole,
                )
            })
            .collect::<Vec<_>>();
        let mut hints = vec![];
        for function in functions {
            let mut inference = self.inference.clone();
            let Type::Function(function) = inference.instantiate(&Type::Function(function.clone()))
            else {
                unreachable!();
            };
            if expected.is_some_and(|ty| bigint_context(&inference.resolve(ty)))
                && !inference.unify(&function.result, &Type::named("BigInt"))
            {
                continue;
            }
            let Some(order) = argument_order(&function, &skeleton) else {
                continue;
            };
            // An integer literal already typed Int could still become a double, so it does
            // not rule out an overload that takes a double there (REVIEW2 section 2).
            if !order.iter().enumerate().all(|(parameter, argument)| {
                argument.is_none_or(|i| {
                    arguments[i].as_ref().is_none_or(|(_, ty, _, hole)| {
                        *hole
                            || inference.unify(&function.parameters[parameter].ty, ty)
                            || int_literal_expression(
                                inputs[i].children.last().unwrap(),
                                self.source,
                            ) && inference.resolve(&function.parameters[parameter].ty)
                                == Type::named("double")
                    })
                })
            }) {
                continue;
            }
            let parameter = order.iter().position(|argument| *argument == Some(index))?;
            let hint = inference.resolve(&function.parameters[parameter].ty);
            // A numeral can become double, but it cannot become a tensor or another
            // non-numeric parameter; do not let such an overload suppress the hint.
            if int_literal_expression(inputs[index].children.last().unwrap(), self.source)
                && !numeric(&hint)
                && hint != Type::named("Nat")
                && !matches!(
                    hint,
                    Type::Variable(_) | Type::Parameter(_) | Type::Rigid(_)
                )
            {
                continue;
            }
            // A hint may still hold open variables, such as a lambda's result, so the argument can be typed from
            // the parameter types it does know (Q135).
            hints.push(Some(hint));
        }
        let first = hints.first()?.as_ref()?;
        hints
            .iter()
            .all(|hint| hint.as_ref().is_some_and(|hint| hint.same_type(first)))
            .then(|| first.clone())
    }

    /// Instantiates a callee for argument hints and lets the caller's expected
    /// result choose a type variable, such as `Set[BigInt]` choosing T for
    /// `Set(1, 1, n + 7)`. A variable shared with a parameter must not become a
    /// bare `dyn Trait`: that would pass `dyn Shape` to the argument, and a
    /// nested call argument would report a mismatch before boxing gets its
    /// chance. The box conversion belongs to the value that goes into the box.
    /// The parameter a pipe value fills, for an overloaded callee. A pipe is an
    /// ordinary call whose first argument comes from the left side, so it must
    /// see the same hints: `1 |> f` with a double parameter must let the
    /// integer literal adopt double, exactly as `f(1)` does.
    fn pipe_argument_hint(
        &self,
        functions: &[FunctionType],
        value: &Node,
        expected: Option<&Type>,
    ) -> Option<Type> {
        let mut hints = vec![];
        for function in functions {
            let mut inference = self.inference.clone();
            let Type::Function(function) = inference.instantiate(&Type::Function(function.clone()))
            else {
                unreachable!();
            };
            if expected.is_some_and(|ty| bigint_context(&inference.resolve(ty)))
                && !inference.unify(&function.result, &Type::named("BigInt"))
            {
                continue;
            }
            let Some(parameter) = function.parameters.first() else {
                continue;
            };
            let hint = inference.resolve(&parameter.ty);
            // A numeral can become double, but it cannot become a tensor or another
            // non-numeric parameter; do not let such an overload suppress the hint.
            if int_literal_expression(value, self.source)
                && !numeric(&hint)
                && hint != Type::named("Nat")
                && !matches!(
                    hint,
                    Type::Variable(_) | Type::Parameter(_) | Type::Rigid(_)
                )
            {
                continue;
            }
            hints.push(hint);
        }
        let first = hints.first()?;
        hints
            .iter()
            .all(|hint| hint.same_type(first))
            .then(|| first.clone())
    }

    /// The expected type used to instantiate a callee. `T or Error` guides a
    /// callee whose result is an ordinary value, so `Set[double] or Error`
    /// chooses T = double for `Set(1, 2)`. A callee that can itself return an
    /// Error keeps the union, because unifying the union with only its success
    /// branch would bind an inference variable to the whole union.
    fn preview_expected(&self, callable: &Type, expected: Option<&Type>) -> Option<Type> {
        let expected = expected?;
        let returns_error = match callable {
            Type::Function(function) => contains_error(&function.result),
            Type::Overloaded(functions) => functions
                .iter()
                .any(|function| contains_error(&function.result)),
            _ => true,
        };
        if returns_error {
            return Some(expected.clone());
        }
        self.value_expected(Some(expected))
            .or_else(|| Some(expected.clone()))
    }

    fn call_preview(&mut self, callable: &Type, expected: Option<&Type>) -> Option<FunctionType> {
        let Type::Function(function) = callable else {
            return None;
        };
        let Type::Function(function) = self
            .inference
            .instantiate(&Type::Function(function.clone()))
        else {
            unreachable!()
        };
        let Some(expected) = expected else {
            return Some(function);
        };
        let shared = function
            .parameters
            .iter()
            .flat_map(|parameter| parameter.ty.variables())
            .collect::<BTreeSet<_>>();
        let before = self.inference.clone();
        if self.inference.unify(&function.result, expected) {
            let dynamic = function
                .result
                .variables()
                .into_iter()
                .filter(|variable| shared.contains(variable))
                .any(|variable| {
                    matches!(
                        self.inference.resolve(&Type::Variable(variable)),
                        Type::Dynamic(_)
                    )
                });
            if dynamic {
                self.inference = before;
            }
        }
        Some(function)
    }

    /// `"{} has {:.1f}".format(name, x)` on a literal template is checked like an f-string:
    /// every slot names an argument, every argument is used, and each spec fits its value (Q165).
    fn format_call(&mut self, node: &Node, expected: Option<&Type>) -> Option<Type> {
        let callee = ungroup(&node.children[0]);
        if callee.kind != K::Member || self.text(&callee.children[1]) != "format" {
            return None;
        }
        let receiver = ungroup(&callee.children[0]);
        let literal = self.text(receiver).to_owned();
        if receiver.kind != K::Literal || !literal.starts_with('"') {
            // A template held in a variable could only fail at run time. Only a plain name
            // is looked at here, so no expression is checked twice.
            let name = self.text(receiver).to_owned();
            let local = self.scopes.iter().any(|scope| scope.contains_key(&name));
            let global = self
                .environment
                .values
                .get(&name)
                .is_some_and(|symbol| !matches!(symbol.ty, Type::Namespace(_)));
            if receiver.kind != K::Name || !(local || global) {
                return None;
            }
            let ty = self.infer(receiver, None);
            if self.inference.resolve(&ty) == Type::named("String") {
                self.error(
                    callee,
                    "E_FORMAT_TEMPLATE",
                    "format needs its template written in place, so the checker can read it.",
                    "Write the template as a literal, as in \"{} items\".format(n), or use an f-string.",
                );
                return Some(Type::Invalid);
            }
            return None;
        }
        self.infer(receiver, None);
        let template = match crate::strings::decode(
            &literal[1..literal.len() - 1],
            receiver.span.start + 1,
            false,
        ) {
            Ok(text) => text,
            Err(_) => return Some(Type::Invalid),
        };
        let inputs = &node.children[1..];
        let mut arguments = vec![];
        for input in inputs {
            if input.kind != K::Argument {
                self.error(
                    input,
                    "E_FORMAT_ARGUMENTS",
                    "format takes its values in order, without names.",
                    "Write \"{0} {1}\".format(a, b).",
                );
                return Some(Type::Invalid);
            }
            let value = input.children.last().unwrap();
            let ty = self.infer(value, None);
            arguments.push((None, ty, value, false));
        }
        let pieces = match crate::strings::template_pieces(&template) {
            Ok(pieces) => pieces,
            Err(message) => {
                self.error(
                    receiver,
                    "E_FORMAT_TEMPLATE",
                    message,
                    "Use {} or {0} slots, with an optional :spec.",
                );
                return Some(Type::Invalid);
            }
        };
        let mut used = vec![false; arguments.len()];
        for piece in &pieces {
            let crate::strings::TemplatePiece::Slot(index, spec) = piece else {
                continue;
            };
            let Some((_, ty, value, _)) = arguments.get(*index) else {
                self.error(
                    receiver,
                    "E_FORMAT_ARGUMENTS",
                    format!(
                        "The template uses argument {index}, but the call passes {}.",
                        arguments.len()
                    ),
                    "Pass one value for each slot.",
                );
                return Some(Type::Invalid);
            };
            let (ty, value) = (ty.clone(), *value);
            used[*index] = true;
            if !self.implements(&ty, &Type::named("Text"), &mut BTreeSet::new()) {
                self.error(
                    value,
                    "E_TEXT_REQUIRED",
                    format!("{} does not provide Text.", self.inference.resolve(&ty)),
                    "Declare has Text and write text() : String.",
                );
            }
            let integer = matches!(&self.inference.resolve(&ty), Type::Named(n, a) if a.is_empty() && matches!(n.as_str(), "Int" | "Nat"));
            match spec.as_deref().and_then(crate::strings::format_kind) {
                Some(crate::strings::FormatKind::Fixed(_) | crate::strings::FormatKind::Number)
                    if !integer =>
                {
                    self.require(value, &Type::named("double"), &ty);
                }
                Some(crate::strings::FormatKind::Integer) if !integer => {
                    self.error(
                        value,
                        "E_FORMAT_SPEC",
                        format!(
                            "This integer format needs an Int or Nat, found {}.",
                            self.inference.resolve(&ty)
                        ),
                        "Use d, x, X, o, b or c with an Int, or f, e, g or % with a double.",
                    );
                }
                _ => {}
            }
        }
        if let Some(unused) = used.iter().position(|used| !used) {
            self.error(
                arguments[unused].2,
                "E_FORMAT_ARGUMENTS",
                format!("Argument {unused} is never used by the template."),
                "Add a slot for it or remove the argument.",
            );
        }
        let function = FunctionType {
            parameters: arguments
                .iter()
                .map(|(_, ty, _, _)| Parameter {
                    name: None,
                    ty: ty.clone(),
                    default: false,
                    passing: Passing::Borrow,
                })
                .collect(),
            result: Box::new(Type::named("String")),
            bounds: BTreeMap::new(),
            requirements: vec![],
            effects: false,
            receiver_mutates: false,
            callable: Some(Callable::Builtin("std.core.String.format".into())),
            receiver: Some(Box::new(Type::named("String"))),
        };
        Some(self.apply(node, Type::Function(function), &arguments, expected))
    }

    fn call(&mut self, node: &Node, expected: Option<&Type>) -> Type {
        let expected = if ungroup(&node.children[0]).kind == K::Name
            && self.text(ungroup(&node.children[0])) == "super"
        {
            None
        } else {
            expected
        };
        let callee_node = ungroup(&node.children[0]);
        let theorem = if expected
            .is_some_and(|expected| self.inference.resolve(expected) == Type::named("Proof"))
            && matches!(callee_node.kind, K::Name | K::Member)
        {
            let name = crate::lexer::path_name(self.text(callee_node));
            let shadowed = !name.contains('.')
                && (self.scopes.iter().any(|scope| scope.contains_key(&name))
                    || self.environment.values.contains_key(&name)
                    || self.environment.implicit.contains_key(&name)
                    || self.environment.names.contains_key(&name));
            (!shadowed).then(|| self.theorem_signature(&name)).flatten()
        } else {
            None
        };
        if theorem.is_some()
            && node.children.iter().skip(1).any(|argument| {
                argument.kind == K::NamedArgument
                    || argument
                        .children
                        .last()
                        .is_some_and(|value| value.kind == K::Hole)
            })
        {
            self.error(
                node,
                "E_PROOF_ARGUMENTS",
                "Direct theorem evidence takes complete positional arguments.",
                "Pass every theorem value parameter positionally; named and partial evidence calls are not supported.",
            );
            return Type::Invalid;
        }
        if theorem.is_none() {
            if let Some(result) = self.probability_static_call(node, expected) {
                return result;
            }
            if let Some(result) = self.numerical_call(node, expected) {
                return result;
            }
            if let Some(result) = self.conversion_call(node, expected) {
                return result;
            }
            if let Some(result) = self.variable_call(node, expected) {
                return result;
            }
            if let Some(result) = self.tensor_call(node, expected) {
                return result;
            }
            if let Some(result) = self.format_call(node, expected) {
                return result;
            }
        }

        let previous_callee = self.direct_callee.replace(callee_node.span);
        // The expected type describes this call's result. A callee that is itself a call
        // returns the function being called here, so that expectation does not apply to it.
        let callee_expected = if ungroup(&node.children[0]).kind == K::Call {
            None
        } else {
            expected
        };
        let callee = theorem
            .as_ref()
            .map(|signature| Type::Function(signature.clone()))
            .unwrap_or_else(|| self.infer(&node.children[0], callee_expected));
        self.direct_callee = previous_callee;
        if let Some((module, receiver, attribute)) = self.python_pending.remove(&callee_node.span) {
            return self.python_call(node, module, receiver, Some(attribute), expected);
        }
        if self.inference.resolve(&callee) == Type::named("PythonValue") {
            return self.python_call(node, None, Some(callee_node.span), None, expected);
        }
        if matches!(&callee, Type::Constructor(name, _) if name == "Map") {
            return self.map_literal(node, callee, expected);
        }
        let mut callable = if let Type::Constructor(name, args) = callee {
            self.constructor(node, &name, args)
        } else {
            callee
        };
        let preview_type = self.preview_expected(&callable, expected);
        let preview = self.call_preview(&callable, preview_type.as_ref());
        let inputs = &node.children[1..];
        let mut arguments = vec![None; inputs.len()];
        let mut order = (0..inputs.len()).collect::<Vec<_>>();
        order.sort_by_key(|i| {
            let value = inputs[*i].children.last().unwrap();
            if value.kind == K::Lambda {
                2
            } else if matches!(callable, Type::Overloaded(_))
                && bigint_literal_expression(value, self.source)
            {
                1
            } else {
                0
            }
        });
        for index in order {
            let argument = &inputs[index];
            if argument.kind == K::ArrowArgument {
                self.error(
                    argument,
                    "E_MAP_ARROW_UNSPECIFIED",
                    "Map arrow arguments need the rule recorded in Q05.",
                    "Their syntax is retained, but their meaning has not been confirmed.",
                );
                return Type::Invalid;
            }
            let name = if argument.kind == K::NamedArgument {
                Some(self.text(&argument.children[0]).to_owned())
            } else {
                None
            };
            let value = argument.children.last().unwrap();
            let hole = value.kind == K::Hole;
            let wanted = preview
                .as_ref()
                .and_then(|function| {
                    let parameter = if let Some(name) = &name {
                        function
                            .parameters
                            .iter()
                            .find(|p| p.name.as_ref() == Some(name))
                    } else {
                        function.parameters.get(index)
                    };
                    parameter.map(|p| self.inference.resolve(&p.ty))
                })
                .or_else(|| match &callable {
                    Type::Overloaded(functions) => {
                        self.overload_argument_hint(functions, inputs, &arguments, index, expected)
                    }
                    _ => None,
                });
            let ty = if hole {
                self.inference.fresh()
            } else {
                let mut hint = wanted.clone();
                if let Some(Type::Indexed(inner, index)) = &hint {
                    let mut open = BTreeSet::new();
                    index.map(&mut |part| {
                        if let Index::Variable(name) = part {
                            open.insert(name.clone());
                        }
                        part.clone()
                    });
                    if open.len() > 1 {
                        // The element type can guide a constructor before later
                        // arguments determine how its symbolic length is split.
                        hint = Some(self.inference.instantiate(&Type::Indexed(
                            inner.clone(),
                            Index::Variable("hint".into()),
                        )));
                    }
                }
                self.infer(value, hint.as_ref())
            };
            if let Some(wanted) = &wanted {
                self.constrain_argument(wanted, &ty, &mut vec![]);
            }
            arguments[index] = Some((name, ty, value, hole));
        }
        let mut arguments = arguments
            .into_iter()
            .map(Option::unwrap)
            .collect::<Vec<_>>();
        if let Some((full, receiver)) = self.extension_members.remove(&callee_node.span) {
            arguments.insert(0, (None, receiver, &callee_node.children[0], false));
            callable = full;
        }
        if let Type::Function(function) = &mut callable
            && matches!(&function.callable, Some(Callable::Builtin(name)) if name == "std.array.generate")
            && let Some((_, ty, _, false)) = arguments.get(1)
            && let Type::Function(factory) = self.inference.resolve(ty)
        {
            function.effects = factory.effects;
            function.parameters[1].passing = if factory.receiver_mutates {
                Passing::Mut
            } else {
                Passing::Borrow
            };
        }
        let result = self.apply(node, callable, &arguments, expected);
        if theorem.is_some() && result != Type::Invalid {
            self.theorem_evidence.insert(node.span);
            return Type::named("Proof");
        }
        if callee_node.kind == K::Name && self.text(callee_node) == "super" {
            if let Some(owner) = self
                .owner
                .as_ref()
                .and_then(|owner| self.environment.definitions.get(owner))
                && let Some(Type::Named(parent, _)) = &owner.parent
                && let Some(parent) = self.environment.definitions.get(parent)
                && let Some(initialized) = self.initializing_fields.as_mut()
            {
                initialized.extend(parent.fields.keys().cloned());
            }
            return Type::named("void");
        }
        if matches!(&result, Type::Named(name,args) if name == "Array" && args.len()==1 && !crate::types::copies(&args[0]))
            && self.resolved_calls.get(&node.span).is_some_and(
                |call| matches!(&call.target, Callable::Builtin(name) if name == "std.core.Array"),
            )
        {
            self.error(node,"E_ARRAY_FILL","Array(size, value) needs a copyable element.","Use Array.generate(size, i => value) to create a separate owned value for each slot.");
        }
        if let Some(expected) = expected {
            let mut obligations = vec![];
            if self.constrain_argument(expected, &result, &mut obligations) {
                self.note_to_array(node, &result);
                self.record_borrowed_view(node, expected, &result);
                self.check_obligations(node, &obligations);
                return self.inference.resolve(expected);
            }
        }
        self.note_to_array(node, &result);
        result
    }

    /// `toArray` copies each element into its slot, so its element type must be known and copyable.
    /// Later code can still fix the element type, as `xs.append(3)` does for `let xs := [].toArray()`,
    /// so the check waits for the whole file.
    fn note_to_array(&mut self, node: &Node, result: &Type) {
        let is_to_array = self.resolved_calls.get(&node.span).is_some_and(
            |call| matches!(&call.target, Callable::Builtin(name) if name.ends_with(".toArray")),
        );
        if is_to_array {
            self.to_array_results.push((node.span, result.clone()));
        }
    }

    /// Checks each recorded toArray result once every type in the file is as known as it will get.
    fn finish_to_array(&mut self) {
        for (span, result) in std::mem::take(&mut self.to_array_results) {
            let Type::Named(name, args) = self.inference.resolve(&result) else {
                continue;
            };
            if name != "Array" || args.len() != 1 {
                continue;
            }
            let element = self.inference.resolve(&args[0]);
            let mut invalid = false;
            let mut unknown = false;
            element.map(&mut |part| {
                if matches!(part, Type::Invalid) {
                    invalid = true;
                }
                if matches!(part, Type::Variable(_))
                    || matches!(part, Type::Parameter(name) if name.starts_with('?'))
                {
                    unknown = true;
                }
                None
            });
            if invalid {
                continue;
            }
            if unknown {
                self.diagnostics.push(
                    Diagnostic::new(
                        "E_UNKNOWN_ELEMENT",
                        "The element type of this empty collection is not known.",
                        span,
                    )
                    .with_hint("Annotate the list, for example `let e : List[Int] = []`, or the result, for example `let xs : Array[Int] = [].toArray()`."),
                );
            } else if !crate::types::copies(&element) {
                self.diagnostics.push(
                    Diagnostic::new("E_TO_ARRAY", "toArray needs copyable elements.", span)
                        .with_hint("Keep values that cannot be copied in a List, or build an Array with Array.generate(size, factory)."),
                );
            }
        }
    }

    fn using(&mut self, node: &Node, expected: Option<&Type>) -> Type {
        let (Some(name), Some(init), Some(body)) = (
            node.children.first(),
            node.children.get(1),
            node.children.last(),
        ) else {
            return Type::Invalid;
        };
        let call = ungroup(init);
        let call = if call.kind == K::Propagate {
            ungroup(&call.children[0])
        } else {
            call
        };
        let lock = (call.kind == K::Call)
            .then(|| ungroup(&call.children[0]))
            .filter(|callee| callee.kind == K::Member && self.text(&callee.children[1]) == "lock")
            .map(|callee| callee.span);
        let previous = std::mem::replace(&mut self.lock_call, lock);
        let ty = self.infer(init, None);
        self.lock_call = previous;
        let ty = self.inference.resolve(&ty);
        let resource = matches!(ty, Type::Guard(_) | Type::Invalid)
            || matches!(&ty, Type::Named(n, a) if a.is_empty() && matches!(n.as_str(), "Reader" | "Writer"));
        if !resource {
            self.error(
                init,
                "E_USING_RESOURCE",
                format!("using needs a Reader, a Writer or a Mutex lock, found {ty}."),
                "Use ? to take the resource out of open(path), create(path) or m.lock().",
            );
        }
        self.scopes.push(BTreeMap::new());
        let text = self.text(name).to_owned();
        let file = matches!(&ty, Type::Named(n, _) if matches!(n.as_str(), "Reader" | "Writer"));
        self.bind(name, &text, ty, false, false);
        if let Some(symbol) = self.scopes.last_mut().unwrap().get_mut(&text) {
            // The block holds the resource, so it may read, write or change it.
            symbol.access = Some(true);
        }
        self.exit_barriers.push((self.loop_depth, vec![]));
        let result = self.infer(body, expected);
        self.close_exit_barrier(file, "a file using block");
        self.scopes.pop();
        if file {
            self.require_effect(node, "Closing a file");
            fn returns(node: &Node) -> bool {
                node.kind == K::Return
                    || !matches!(node.kind, K::Lambda | K::Function)
                        && node.children.iter().any(returns)
            }
            if returns(body) && !self.return_type.as_ref().is_some_and(contains_error) {
                self.error(body, "E_PROPAGATION_RETURN", "Returning from a file using block needs Error in the function's return type because closing the file can fail.", "Include Error in the enclosing return type.");
            }
            Type::union(result, Type::named("Error"))
        } else {
            result
        }
    }

    /// A jump out of a block that can fail would skip the block's Error result (Q48).
    fn close_exit_barrier(&mut self, fails: bool, what: &str) {
        let Some((depth, exits)) = self.exit_barriers.pop() else {
            return;
        };
        if fails {
            for span in exits {
                self.diagnostics.push(
                    Diagnostic::new(
                        "E_LOOP_EXIT",
                        format!("break and continue cannot leave {what}, because its Error result would be lost."),
                        span,
                    )
                    .with_hint("Keep the decision in a variable and break or continue after the block."),
                );
            }
        } else if let Some((outer, outer_exits)) = self.exit_barriers.last_mut()
            && *outer == depth
        {
            // The same jump also leaves the enclosing block.
            outer_exits.extend(exits);
        }
    }

    fn spawn(&mut self, node: &Node) {
        self.require_effect(node, "Starting a task");
        if self.scope_depth == 0 {
            self.error(
                node,
                "E_SPAWN_SCOPE",
                "spawn must be inside a scope block, so the task finishes before the code that started it.",
                "Wrap the spawn in scope { ... }.",
            );
        }
        let Some(call) = node.children.first() else {
            return;
        };
        if !matches!(ungroup(call).kind, K::Call | K::Pipe) {
            self.error(
                call,
                "E_SPAWN_CALL",
                "spawn starts a task from a function call.",
                "Write spawn f(x).",
            );
        }
        let ty = self.infer(call, None);
        match self.inference.resolve(&ty) {
            Type::Named(name, _) if name == "void" => {}
            Type::Invalid | Type::Never => {}
            Type::Union(parts)
                if parts.iter().all(|part| {
                    matches!(part, Type::Named(name, _) if name == "void") || contains_error(part)
                }) =>
            {
                self.scope_fails = true;
            }
            found => self.error(
                call,
                "E_SPAWN_RESULT",
                format!("A task's call returns void or `void or Error`, found {found}."),
                "Send other results through a Channel or a Mutex.",
            ),
        }
    }

    fn constructor(&mut self, node: &Node, name: &str, args: Vec<Type>) -> Type {
        if name == "Bytes" {
            let Type::Function(mut signature) = Type::function(
                vec![Type::applied("List", vec![Type::named("Int")])],
                Type::union(Type::named("Bytes"), Type::named("Error")),
            ) else {
                unreachable!()
            };
            signature.callable = Some(Callable::Builtin("std.bytes.fromList".into()));
            return Type::Function(signature);
        }
        if let Some(definition) = self.environment.definitions.get(name).cloned() {
            if definition.kind != K::Class {
                self.error(
                    node,
                    "E_NOT_CONSTRUCTOR",
                    format!("`{name}` is not a class constructor."),
                    "Call an enum case or a class with a new declaration.",
                );
                return Type::Invalid;
            }
            if definition.constructors.is_empty() {
                self.error(
                    node,
                    "E_CONSTRUCTOR_REQUIRED",
                    format!("`{name}` has no explicit new constructor."),
                    "Write new(...); field constructors are not generated.",
                );
                return Type::Invalid;
            }
            if !args.is_empty() && args.len() != definition.parameters.len() {
                self.error(
                    node,
                    "E_TYPE_ARITY",
                    "The constructor has the wrong number of type arguments.",
                    "Supply one type for each declared parameter.",
                );
                return Type::Invalid;
            }
            let replacements: BTreeMap<_, _> =
                definition.parameters.iter().cloned().zip(args).collect();
            for (parameter, ty) in &replacements {
                for requirement in definition.bounds.get(parameter).into_iter().flatten() {
                    if !self.implements(ty, requirement, &mut BTreeSet::new()) {
                        self.error(
                            node,
                            "E_TRAIT_BOUND",
                            format!("Expected {ty} to provide {requirement}."),
                            "Use a type that implements the declared class bound.",
                        );
                    }
                }
            }
            let constructors = definition
                .constructors
                .iter()
                .map(
                    |f| match Type::Function(f.clone()).replace_parameters(&replacements) {
                        Type::Function(f) => f,
                        _ => unreachable!(),
                    },
                )
                .collect::<Vec<_>>();
            return if constructors.len() == 1 {
                Type::Function(constructors[0].clone())
            } else {
                Type::Overloaded(constructors)
            };
        }
        let arity = builtin_arity(name);
        let args = if args.is_empty() {
            (0..arity.unwrap_or(0))
                .map(|_| self.inference.fresh())
                .collect::<Vec<_>>()
        } else {
            args
        };
        if arity.is_some_and(|n| n != args.len()) {
            self.error(
                node,
                "E_TYPE_ARITY",
                format!("`{name}` expects {} type arguments.", arity.unwrap()),
                "Use the declared number of type arguments.",
            );
            return Type::Invalid;
        }
        let result = Type::applied(name, args.clone());
        let mut constructed = match name {
            "ArenaRef" => {
                self.error(
                    node,
                    "E_ARENA_HANDLE",
                    "ArenaRef handles have no public constructor.",
                    "Call insert(value) on an Arena to obtain a handle.",
                );
                Type::Invalid
            }
            // `Error(message)` builds the standard RuntimeError.Message case, boxed as Error.
            "Error" => Type::function(vec![Type::named("String")], Type::named("Error")),
            "Mutex" | "Atomic" | "Box" => Type::function(args, result),
            // Q37: building a boundary number checks its range before any C call.
            "CFloat32" => Type::function(
                vec![Type::named("double")],
                Type::union(result, Type::named("Error")),
            ),
            name if crate::interop::C_TYPES.contains(&name) => Type::function(
                vec![Type::named("Int")],
                Type::union(result, Type::named("Error")),
            ),
            "Channel" | "MutMap" | "MutSet" | "Arena" => Type::function(vec![], result),
            // Each slot starts as a copy of the value (Q53).
            "Array" => Type::function(vec![Type::named("Int"), args[0].clone()], result),
            "Map" if node.children.len() == 1 => Type::function(vec![], result),
            "Set" | "SortedSet" => {
                let count = node.children.len().saturating_sub(1);
                let Type::Function(mut signature) =
                    Type::function(vec![args[0].clone(); count], result)
                else {
                    unreachable!()
                };
                if count > 0 {
                    signature.requirements.push((
                        args[0].clone(),
                        Type::named(if name == "SortedSet" {
                            "Comparable"
                        } else {
                            "Equals"
                        }),
                    ));
                }
                Type::Function(signature)
            }
            _ => {
                self.error(
                    node,
                    "E_CONSTRUCTOR_UNSPECIFIED",
                    format!("The constructor contract for `{name}` is not implemented."),
                    "See Q06 for missing standard-library contracts.",
                );
                Type::Invalid
            }
        };
        if let Type::Function(function) = &mut constructed {
            function.callable = Some(Callable::Builtin(format!("std.core.{name}")));
        }
        constructed
    }

    fn apply(
        &mut self,
        node: &Node,
        callable: Type,
        arguments: &[(Option<String>, Type, &Node, bool)],
        expected: Option<&Type>,
    ) -> Type {
        let callable = self.inference.resolve(&callable);
        if let Type::Variable(_) = callable {
            let result = self.inference.fresh();
            let function = Type::function(
                arguments.iter().map(|a| a.1.clone()).collect(),
                result.clone(),
            );
            if !self.inference.unify(&callable, &function) {
                self.mismatch(node, &function, &callable);
            }
            if let Type::Function(signature) = function {
                self.resolved_calls.insert(
                    node.span,
                    ResolvedCall {
                        target: Callable::Indirect,
                        callee: match node.kind {
                            K::Call => node.children.first().map(|node| node.span),
                            K::Pipe => node.children.get(2).map(|node| node.span),
                            _ => None,
                        },
                        overload: 0,
                        signature,
                        type_arguments: BTreeMap::new(),
                        arguments: arguments
                            .iter()
                            .map(|argument| ResolvedArgument::Supplied {
                                expression: argument.2.span,
                                hole: argument.3,
                            })
                            .collect(),
                        evaluation_order: arguments
                            .iter()
                            .filter(|argument| !argument.3)
                            .map(|argument| argument.2.span)
                            .collect(),
                        receiver: None,
                        partial: arguments.iter().any(|argument| argument.3),
                    },
                );
            }
            return result;
        }
        let functions = match callable {
            Type::Function(f) => vec![f],
            Type::Overloaded(fs) => fs,
            Type::Invalid => return Type::Invalid,
            found => {
                self.error(
                    node,
                    "E_NOT_CALLABLE",
                    format!("Expected a function, found {found}."),
                    "Call a function, constructor or partial application.",
                );
                return Type::Invalid;
            }
        };
        let saved = self.inference.clone();
        let mut matches = vec![];
        let mut wrong_results = vec![];
        for (overload, function) in functions.iter().enumerate() {
            self.inference = saved.clone();
            let (instantiated, substitutions) = self
                .inference
                .instantiate_with_parameters(&Type::Function(function.clone()));
            let function = match instantiated {
                Type::Function(f) => f,
                _ => unreachable!(),
            };
            let Some(order) = argument_order(&function, arguments) else {
                continue;
            };
            let holes = order
                .iter()
                .enumerate()
                .filter_map(|(i, argument)| {
                    argument
                        .filter(|arg| arguments[*arg].3)
                        .map(|_| function.parameters[i].clone())
                })
                .collect::<Vec<_>>();
            let candidate_result = if holes.is_empty() {
                *function.result.clone()
            } else {
                Type::Function(FunctionType {
                    parameters: holes,
                    ..function.clone()
                })
            };
            // A later argument can determine the parts of an earlier n + m.
            // Retry only indexed equalities; evaluation still follows source order.
            let mut pending = order
                .iter()
                .enumerate()
                .filter_map(|(i, arg)| {
                    let arg = (*arg)?;
                    matches!(
                        (
                            self.inference.resolve(&function.parameters[i].ty),
                            self.inference.resolve(&arguments[arg].1)
                        ),
                        (Type::Indexed(..), Type::Indexed(..))
                    )
                    .then_some((i, arg))
                })
                .collect::<Vec<_>>();
            loop {
                let before = pending.len();
                pending.retain(|(i, arg)| {
                    let saved = self.inference.clone();
                    if self
                        .inference
                        .unify(&function.parameters[*i].ty, &arguments[*arg].1)
                    {
                        false
                    } else {
                        self.inference = saved;
                        true
                    }
                });
                if pending.len() == before {
                    break;
                }
            }
            let mut callback_obligations = vec![];
            let mut boxed = vec![];
            let mut stand_ins = 0;
            if order.iter().enumerate().all(|(i, arg)| {
                arg.is_none_or(|arg| {
                    let parameter = &function.parameters[i];
                    let saved = self.inference.clone();
                    let count = callback_obligations.len();
                    if self.constrain_argument(
                        &parameter.ty,
                        &arguments[arg].1,
                        &mut callback_obligations,
                    ) {
                        return true;
                    }
                    self.inference = saved.clone();
                    callback_obligations.truncate(count);
                    // Keep a nominal match so access checking can explain a lost
                    // capture permission, including the arena-specific diagnostic.
                    if !capture_permissions_fit(
                        &self.inference.resolve(&parameter.ty),
                        &self.inference.resolve(&arguments[arg].1),
                    ) && self.constrain_argument(
                        &without_capture_permissions(&parameter.ty),
                        &without_capture_permissions(&arguments[arg].1),
                        &mut callback_obligations,
                    ) {
                        return true;
                    }
                    self.inference = saved.clone();
                    callback_obligations.truncate(count);
                    // A borrowing parameter reaches the value inside a Box with no marker.
                    if let Type::Named(name, inner) = self.inference.resolve(&arguments[arg].1)
                        && parameter.passing != Passing::Move
                        && name == "Box"
                        && inner.len() == 1
                        && self.constrain_argument(
                            &parameter.ty,
                            &inner[0],
                            &mut callback_obligations,
                        )
                    {
                        return true;
                    }
                    self.inference = saved;
                    callback_obligations.truncate(count);
                    // A subclass stands in where a borrowed superclass is expected (Q85).
                    if parameter.passing != Passing::Move
                        && let Some(steps) = self.superclass_steps(
                            &self.inference.resolve(&arguments[arg].1),
                            &self.inference.resolve(&parameter.ty),
                        )
                    {
                        stand_ins += steps;
                        return true;
                    }
                    // An Int argument widens where a BigInt parameter is expected (Q125).
                    if self.widens(&parameter.ty, &arguments[arg].1) {
                        boxed.push((arguments[arg].2.span, parameter.ty.clone(), true));
                        return true;
                    }
                    // A plain value goes into a new box where a Box is expected (Q86).
                    if self.box_content(&parameter.ty, &arguments[arg].1, &mut callback_obligations)
                    {
                        boxed.push((arguments[arg].2.span, parameter.ty.clone(), false));
                        return true;
                    }
                    false
                })
            }) {
                if let Some(ty) = expected {
                    let before = self.inference.clone();
                    let count = callback_obligations.len();
                    if !self.constrain_argument(ty, &candidate_result, &mut callback_obligations) {
                        self.inference = before;
                        callback_obligations.truncate(count);
                        // The caller may put this result into a new box (Q86), or
                        // lend a subclass result where it borrows a superclass (Q85).
                        if !self.box_content(ty, &candidate_result, &mut callback_obligations)
                            && self
                                .superclass_steps(
                                    &self.inference.resolve(&candidate_result),
                                    &self.inference.resolve(ty),
                                )
                                .is_none()
                        {
                            wrong_results.push(self.inference.resolve(&candidate_result));
                            continue;
                        }
                    }
                }
                matches.push((
                    overload,
                    function,
                    order,
                    self.inference.clone(),
                    substitutions,
                    callback_obligations,
                    boxed,
                    stand_ins,
                ));
            }
        }
        self.inference = saved;
        // An overload that takes the value as it is wins over one that boxes it,
        // and a nearer superclass wins over a farther one.
        if matches.iter().any(|m| m.6.is_empty()) {
            matches.retain(|m| m.6.is_empty());
        }
        if let Some(nearest) = matches.iter().map(|m| m.7).min() {
            matches.retain(|m| m.7 == nearest);
        }
        if matches.is_empty()
            && let (Some(expected), Some(found)) = (expected, wrong_results.first())
        {
            self.mismatch(node, expected, found);
            return Type::Invalid;
        }
        if matches.len() != 1 {
            // With one candidate, name the first argument that fails to fit so
            // the caller sees the expected and actual types instead of only the
            // full signature list.
            if matches.is_empty() && functions.len() == 1 {
                let function = &functions[0];
                let callable = match node.kind {
                    K::Call => node.children.first(),
                    K::Pipe => node.children.get(2),
                    _ => None,
                }
                .map(ungroup)
                .map(|callable| self.source.slice(callable.span).trim().to_owned())
                .unwrap_or_else(|| "call".into());
                let parameters = parameter_list(function);
                if let Some(name) = arguments
                    .iter()
                    .filter_map(|argument| argument.0.as_deref())
                    .find(|name| {
                        !function
                            .parameters
                            .iter()
                            .any(|parameter| parameter.name.as_deref() == Some(*name))
                    })
                {
                    self.error(
                        node,
                        "E_CALL_ARGUMENTS",
                        format!(
                            "Function `{callable}` has no parameter named `{name}`. Available parameters: ({parameters})."
                        ),
                        "Check the parameter name or remove the argument name.",
                    );
                    return Type::Invalid;
                }
                let mut named = BTreeSet::new();
                if let Some(name) = arguments
                    .iter()
                    .filter_map(|argument| argument.0.as_deref())
                    .find(|name| !named.insert((*name).to_owned()))
                {
                    self.error(
                        node,
                        "E_CALL_ARGUMENTS",
                        format!("Function `{callable}` gets parameter `{name}` more than once."),
                        "Pass each parameter exactly once.",
                    );
                    return Type::Invalid;
                }
                let required = function
                    .parameters
                    .iter()
                    .filter(|parameter| !parameter.default)
                    .count();
                let total = function.parameters.len();
                if arguments.len() < required || arguments.len() > total {
                    let expected = if arguments.len() < required {
                        if required == total {
                            format!("{required}")
                        } else {
                            format!("at least {required}")
                        }
                    } else if required == total {
                        format!("{total}")
                    } else {
                        format!("at most {total}")
                    };
                    let noun = if arguments.len() < required {
                        if required == 1 {
                            "argument"
                        } else {
                            "arguments"
                        }
                    } else if total == 1 {
                        "argument"
                    } else {
                        "arguments"
                    };
                    self.error(
                        node,
                        "E_CALL_ARGUMENTS",
                        format!(
                            "Function `{callable}` expects {expected} {noun} ({parameters}), but {} {} supplied.",
                            arguments.len(),
                            if arguments.len() == 1 { "was" } else { "were" }
                        ),
                        "Add or remove arguments to match the declared parameters.",
                    );
                    return Type::Invalid;
                }
                if let Some(order) = argument_order(function, arguments) {
                    let saved_inference = self.inference.clone();
                    let mut mismatch = None;
                    for (slot, argument) in order.iter().enumerate() {
                        let Some(argument) = *argument else {
                            continue;
                        };
                        let parameter = &function.parameters[slot];
                        let (_, found, argument_node, _) = &arguments[argument];
                        let expected = self.inference.resolve(&parameter.ty);
                        let found = self.inference.resolve(found);
                        if matches!(expected, Type::Variable(_) | Type::Parameter(_))
                            || matches!(expected, Type::Indexed(..))
                            || matches!(found, Type::Indexed(..))
                            || expected == Type::Invalid
                            || expected == found
                        {
                            continue;
                        }
                        let before = self.inference.clone();
                        let mut obligations = vec![];
                        let compatible =
                            self.constrain_argument(&expected, &found, &mut obligations)
                                || self.box_content(&expected, &found, &mut obligations)
                                || self.superclass_steps(&found, &expected).is_some();
                        self.inference = before;
                        if !compatible {
                            mismatch = Some((slot, argument, argument_node, expected, found));
                            break;
                        }
                    }
                    self.inference = saved_inference;
                    if let Some((slot, argument, argument_node, expected, found)) = mismatch {
                        let parameter = &function.parameters[slot];
                        let label = parameter
                            .name
                            .as_deref()
                            .map(|name| format!(" (`{name}`)"))
                            .unwrap_or_default();
                        let expression = self.source.slice(argument_node.span).trim();
                        self.error(
                        node,
                        "E_CALL_ARGUMENTS",
                        format!(
                            "Argument {} of `{callable}`{label} expects type `{expected}`, but `{expression}` has type `{found}`.",
                            argument + 1
                        ),
                        "Pass a value of the expected type or change the parameter type.",
                    );
                        return Type::Invalid;
                    }
                }
            }
            // Several overloads would fit if a numeral became a double: say which.
            if matches.is_empty() && functions.len() > 1 {
                let fits: Vec<&FunctionType> = functions
                    .iter()
                    .filter(|function| {
                        argument_order(function, arguments).is_some_and(|order| {
                            let mut probe = self.inference.clone();
                            order.iter().enumerate().all(|(slot, argument)| {
                                argument.is_none_or(|argument| {
                                    let (_, found, value, _) = &arguments[argument];
                                    let expected = probe.resolve(&function.parameters[slot].ty);
                                    probe.unify(&expected, found)
                                        || int_literal_expression(value, self.source)
                                            && expected == Type::named("double")
                                })
                            })
                        })
                    })
                    .collect();
                if fits.len() > 1 {
                    self.error(
                        node,
                        "E_AMBIGUOUS_CALL",
                        format!(
                            "This call fits more than one overload: {}.",
                            fits.iter()
                                .map(|f| Type::Function((*f).clone()).to_string())
                                .collect::<Vec<_>>()
                                .join("; ")
                        ),
                        "Write a numeral as a double, such as 1.0, or annotate an argument to choose one.",
                    );
                    return Type::Invalid;
                }
            }
            let mut code = if matches.is_empty() {
                "E_CALL_ARGUMENTS"
            } else {
                "E_AMBIGUOUS_CALL"
            };
            if matches.is_empty() {
                for function in &functions {
                    if let Some(order) = argument_order(function, arguments) {
                        for (i, argument) in order.iter().enumerate() {
                            if let Some(argument) = argument {
                                let found = self.inference.resolve(&arguments[*argument].1);
                                if impossible_indices(&function.parameters[i].ty, &found) {
                                    code = "E_INDEX_LENGTH";
                                } else if matches!((&function.parameters[i].ty, &found), (Type::Indexed(_, a), Type::Indexed(_, b)) if a != b)
                                    && code != "E_INDEX_LENGTH"
                                {
                                    code = "E_INDEX_UNSPECIFIED";
                                }
                            }
                        }
                    }
                }
            }
            let found = arguments
                .iter()
                .map(|a| self.inference.resolve(&a.1).to_string())
                .collect::<Vec<_>>()
                .join(", ");
            self.error(node, code, format!("Expected one matching call signature, found {} for arguments ({found}). Available: {}.", matches.len(), functions.iter().map(|f| Type::Function(f.clone()).to_string()).collect::<Vec<_>>().join("; ")), "Check argument names, counts and types. Positional arguments come first.");
            return Type::Invalid;
        }
        let (overload, function, order, inference, substitutions, callback_obligations, boxed, _) =
            matches.pop().unwrap();
        self.inference = inference;
        for (span, ty, widen) in boxed {
            if widen {
                self.widened.insert(span, ty);
            } else {
                self.autoboxed.insert(span, ty);
            }
        }
        self.check_call_access(node, &function, &order, arguments);
        self.check_obligations(node, &callback_obligations);
        if !order
            .iter()
            .flatten()
            .any(|argument| arguments[*argument].3)
        {
            self.check_obligations(node, &function.requirements);
        }
        let holes: Vec<_> = order
            .iter()
            .enumerate()
            .filter_map(|(i, argument)| {
                argument
                    .filter(|arg| arguments[*arg].3)
                    .map(|_| function.parameters[i].clone())
            })
            .collect();
        let mut remaining_bounds = BTreeMap::new();
        let mut partial_parameters = BTreeMap::new();
        for (parameter, requirements) in &function.bounds {
            let resolved = substitutions
                .get(parameter)
                .map(|ty| self.inference.resolve(ty));
            if let Some(ty) = resolved {
                if let Type::Variable(id) = ty
                    && !holes.is_empty()
                {
                    remaining_bounds.insert(parameter.clone(), requirements.clone());
                    partial_parameters.insert(id, Type::Parameter(parameter.clone()));
                    continue;
                }
                for requirement in requirements {
                    if !self.satisfy_trait(&ty, requirement) {
                        self.error(
                            node,
                            "E_TRAIT_BOUND",
                            format!("Expected {ty} to provide {requirement}, but it does not."),
                            "Use a type that declares and implements the required trait.",
                        );
                    }
                }
            }
        }
        let callable_node = match node.kind {
            K::Call => node.children.first(),
            K::Pipe => node.children.get(2),
            _ => None,
        }
        .map(ungroup);
        let receiver = function.receiver.as_ref().and_then(|_| {
            callable_node
                .filter(|node| node.kind == K::Member)
                .map(|node| node.children[0].span)
        });
        self.resolved_calls.insert(
            node.span,
            ResolvedCall {
                target: if function.receiver.is_some() && receiver.is_none() {
                    Callable::Indirect
                } else {
                    function.callable.clone().unwrap_or(Callable::Indirect)
                },
                callee: callable_node.map(|node| node.span),
                overload,
                signature: function.clone(),
                type_arguments: substitutions
                    .iter()
                    .map(|(name, ty)| (name.clone(), self.inference.resolve(ty)))
                    .collect(),
                arguments: order
                    .iter()
                    .map(|argument| {
                        argument
                            .map(|index| ResolvedArgument::Supplied {
                                expression: arguments[index].2.span,
                                hole: arguments[index].3,
                            })
                            .unwrap_or(ResolvedArgument::Default)
                    })
                    .collect(),
                evaluation_order: arguments
                    .iter()
                    .filter(|argument| !argument.3)
                    .map(|argument| argument.2.span)
                    .collect(),
                receiver,
                partial: !holes.is_empty(),
            },
        );
        let result = self.inference.resolve(&function.result);
        if holes.is_empty() {
            if function.effects {
                self.require_effect(node, "Calling this function");
            }
            if function.receiver_mutates
                && function.receiver.is_none()
                && let Some(callee) = match node.kind {
                    K::Call => node.children.first(),
                    K::Pipe => node.children.get(2),
                    _ => None,
                }
                && let Some(root) = place_root(ungroup(callee), self.source)
            {
                let root = root.to_owned();
                self.note_mutation(&root);
            }
            self.validate_type(node, &result);
            result
        } else {
            self.inference
                .resolve(&Type::Function(FunctionType {
                    parameters: holes,
                    result: Box::new(result),
                    bounds: remaining_bounds,
                    requirements: function.requirements.clone(),
                    effects: function.effects,
                    receiver_mutates: function.receiver_mutates,
                    callable: Some(Callable::Closure(DeclarationId {
                        file: self.source.name.clone(),
                        span: node.span,
                    })),
                    receiver: None,
                }))
                .map(&mut |ty| {
                    if let Type::Variable(id) = ty {
                        partial_parameters.get(id).cloned()
                    } else {
                        None
                    }
                })
        }
    }

    fn member(&mut self, node: &Node) -> Type {
        let name = self.text(&node.children[1]).to_owned();
        let receiver = if node.children[0].kind == K::Name
            && !self
                .scopes
                .iter()
                .rev()
                .any(|scope| scope.contains_key(self.text(&node.children[0])))
            && !self
                .environment
                .values
                .contains_key(self.text(&node.children[0]))
            && let Some(id) = self.environment.names.get(self.text(&node.children[0]))
        {
            let ty = self
                .expand(&Type::named(id), &mut BTreeSet::new())
                .unwrap_or(Type::Invalid);
            if let Type::Named(id, args) = ty {
                Type::Constructor(id, args)
            } else {
                Type::Invalid
            }
        } else {
            let previous = self
                .member_receiver
                .replace(ungroup(&node.children[0]).span);
            let ty = self.infer(&node.children[0], None);
            self.member_receiver = previous;
            ty
        };
        let receiver = self.inference.resolve(&receiver);
        if let Type::Namespace(namespace) = &receiver {
            let qualified = format!("{namespace}.{name}");
            if self.environment.values.contains_key(&qualified)
                || self.environment.names.contains_key(&qualified)
            {
                return self.lookup(node, &qualified, self.member_expected.clone().as_ref());
            }
            self.error(
                node,
                "E_NAMESPACE_MEMBER",
                format!("Module `{namespace}` has no accessible export `{name}`."),
                "Check the member name and its public or package visibility.",
            );
            return Type::Invalid;
        }
        if receiver == Type::named("PythonValue") {
            return self.python_member(node, &name);
        }
        let mut unwrapped = &receiver;
        while let Type::Borrow(inner) | Type::StoredView(inner) = unwrapped {
            unwrapped = inner;
        }
        // A Mutex is reached only through a using block.
        if let Type::Named(id, args) = unwrapped
            && id == "Mutex"
            && args.len() == 1
        {
            if name == "lock" && self.lock_call == Some(node.span) {
                let Type::Function(mut signature) = Type::function(
                    vec![],
                    Type::union(Type::Guard(Box::new(args[0].clone())), Type::named("Error")),
                ) else {
                    unreachable!()
                };
                // Locking reads what other tasks did (confirmed 2026-11-03).
                signature.effects = true;
                signature.callable = Some(Callable::Builtin("std.concurrent.Mutex.lock".into()));
                signature.receiver = Some(Box::new(receiver.clone()));
                return Type::Function(signature);
            }
            self.error(
                node,
                "E_MUTEX_GUARD",
                "A Mutex is read and changed only inside `using g := m.lock()? { ... }`.",
                "Lock it in a using block, then read or assign g.value inside the block.",
            );
            return Type::Invalid;
        }
        if let Type::Guard(inner) = unwrapped {
            if name == "value" {
                return (**inner).clone();
            }
            self.error(
                node,
                "E_UNKNOWN_MEMBER",
                format!("A Mutex lock has `value`, not `{name}`."),
                "Read or assign g.value inside the using block.",
            );
            return Type::Invalid;
        }
        if let Type::Constructor(id, args) = &receiver
            && id == "Array"
            && name == "generate"
        {
            let element = args
                .first()
                .cloned()
                .unwrap_or_else(|| self.inference.fresh());
            let Type::Function(mut factory) =
                Type::function(vec![Type::named("Int")], element.clone())
            else {
                unreachable!()
            };
            factory.effects = true;
            factory.receiver_mutates = true;
            let Type::Function(mut function) = Type::function(
                vec![Type::named("Int"), Type::Function(factory)],
                Type::applied("Array", vec![element]),
            ) else {
                unreachable!()
            };
            function.effects = true;
            function.callable = Some(Callable::Builtin("std.array.generate".into()));
            return Type::Function(function);
        }
        // Array.empty() makes a zero-slot array, so a generic class can start an empty array without a value.
        if let Type::Constructor(id, args) = &receiver
            && id == "Array"
            && name == "empty"
        {
            let element = args
                .first()
                .cloned()
                .unwrap_or_else(|| self.inference.fresh());
            let Type::Function(mut function) =
                Type::function(vec![], Type::applied("Array", vec![element]))
            else {
                unreachable!()
            };
            function.callable = Some(Callable::Builtin("std.array.empty".into()));
            return Type::Function(function);
        }
        if let Type::Constructor(id, args) = &receiver
            && id == "Option"
            && matches!(name.as_str(), "Some" | "None")
        {
            let item = args
                .first()
                .cloned()
                .unwrap_or_else(|| self.inference.fresh());
            let option = Type::applied("Option", vec![item.clone()]);
            return if name == "Some" {
                let Type::Function(mut function) = Type::function(vec![item], option) else {
                    unreachable!()
                };
                function.callable = Some(Callable::Builtin("std.core.Some".into()));
                Type::Function(function)
            } else {
                option
            };
        }
        if let Type::Constructor(id, args) = &receiver
            && let Some(definition) = self.environment.definitions.get(id)
        {
            if let Some(variant) = definition.variants.iter().find(|v| v.name == name).cloned() {
                let ty = match variant.function() {
                    Some(mut function) => {
                        function.callable = Some(Callable::Declaration {
                            id: variant.declaration,
                            name: variant.name,
                            owner: Some(id.clone()),
                            kind: DeclarationKind::Variant,
                        });
                        Type::Function(function)
                    }
                    None => variant.result,
                };
                return self.inference.instantiate(&ty);
            }
            let static_field = definition.static_fields.get(&name).cloned();
            let static_var = definition.static_vars.contains(&name);
            let visible = self.can_access(id, definition, &name);
            if let Some(ty) = static_field {
                if !visible {
                    self.error(
                        node,
                        "E_MEMBER_PRIVATE",
                        format!("Static field `{name}` is not visible here."),
                        "Make the field public or package, or read it inside its class.",
                    );
                } else {
                    self.static_references
                        .insert(node.span, (id.clone(), name.clone()));
                    if static_var {
                        if self.constant_initializer {
                            self.error(
                                node,
                                "E_CONST_DEPENDENCY",
                                format!("A static or constant initializer cannot read static var `{name}`, because its value can change."),
                                "Read a static final field or a constant, or pass the value as an argument.",
                            );
                        } else if self.assignment_target_span != Some(node.span) {
                            self.require_effect(node, "Reading a static var");
                        }
                    }
                }
                return ty;
            }
            if definition.static_methods.contains(&name) {
                let methods = self.methods(
                    &Type::Named(id.clone(), args.clone()),
                    &name,
                    &mut BTreeSet::new(),
                );
                return overload(methods);
            }
        }
        let dispatch_receiver = receiver.clone();
        let mut receiver = receiver;
        loop {
            receiver = match receiver {
                // A stored view, such as an item from first(), reads its methods (REVIEW2 4).
                Type::Borrow(inner) | Type::Dynamic(inner) | Type::StoredView(inner) => *inner,
                Type::Named(ref name, ref args) if name == "Box" && args.len() == 1 => {
                    args[0].clone()
                }
                other => {
                    receiver = other;
                    break;
                }
            };
        }
        if receiver == Type::named("Text") && matches!(name.as_str(), "text" | "debug") {
            let mut function = match Type::function(vec![], Type::named("String")) {
                Type::Function(function) => function,
                _ => unreachable!(),
            };
            function.callable = Some(Callable::Builtin(format!("std.core.Text.{name}")));
            return self.bind_receiver(Type::Function(function), &dispatch_receiver);
        }
        if let Type::Parameter(parameter) | Type::Rigid(parameter) = &receiver {
            for bound in self.bounds.get(parameter).cloned().unwrap_or_default() {
                if let Type::Named(trait_name, _) = &bound
                    && trait_name == "Text"
                    && matches!(name.as_str(), "text" | "debug")
                {
                    let mut function = match Type::function(vec![], Type::named("String")) {
                        Type::Function(function) => function,
                        _ => unreachable!(),
                    };
                    function.callable = Some(Callable::Builtin(format!("std.core.Text.{name}")));
                    return self.bind_receiver(Type::Function(function), &dispatch_receiver);
                }
                let methods = self.methods(&bound, &name, &mut BTreeSet::new());
                if !methods.is_empty() {
                    if self.direct_callee != Some(node.span)
                        && methods.iter().any(|method| method.receiver_mutates)
                    {
                        self.require_mutable_receiver(node, &node.children[0]);
                    }
                    // A trait's `Self` is the bounded type here, so `b.reset()`
                    // has type T and its chain keeps type T.
                    let methods = methods
                        .into_iter()
                        .map(|method| {
                            match Type::Function(method).replace_parameters(&BTreeMap::from([(
                                "Self".to_owned(),
                                receiver.clone(),
                            )])) {
                                Type::Function(method) => method,
                                _ => unreachable!(),
                            }
                        })
                        .collect();
                    return self.bind_receiver(overload(methods), &dispatch_receiver);
                }
            }
        }
        if let Some((owner, field, _)) = self.field(&receiver, &name, &mut BTreeSet::new()) {
            let definition = &self.environment.definitions[&owner];
            if !self.can_access(&owner, definition, &name) {
                self.error(
                    node,
                    "E_MEMBER_PRIVATE",
                    format!("Field `{name}` is not visible here."),
                    "Use a public field, or package visibility within the same folder.",
                );
                return Type::Invalid;
            }
            if self
                .initializing_fields
                .as_ref()
                .is_some_and(|fields| !fields.contains(&name))
                && self.text(ungroup(&node.children[0])) == "this"
                && self.assignment_target_span != Some(node.span)
            {
                self.error(
                    node,
                    "E_FIELD_UNINITIALIZED",
                    format!("Field `{name}` is read before initialization."),
                    "Initialize the field before reading it.",
                );
            }
            return erase_callable_identities(&field);
        }
        if self.text(ungroup(&node.children[0])) == "this" {
            self.require_initialized_this(node);
        }
        if let Some(id) = base_name(&receiver)
            && let Some(definition) = self.environment.definitions.get(id)
        {
            if definition.member_visibility.contains_key(&name)
                && !self.can_access(id, definition, &name)
            {
                self.error(
                    node,
                    "E_MEMBER_PRIVATE",
                    format!("Member `{name}` is not visible here."),
                    "Use a public member, or package visibility within the same folder.",
                );
                return Type::Invalid;
            }
            let methods = self.methods(&receiver, &name, &mut BTreeSet::new());
            if !methods.is_empty() {
                if self.direct_callee != Some(node.span)
                    && methods.iter().any(|method| method.receiver_mutates)
                {
                    self.require_mutable_receiver(node, &node.children[0]);
                }
                return self.bind_receiver(overload(methods), &dispatch_receiver);
            }
        }
        if matches!(name.as_str(), "text" | "debug")
            && self.implements(&receiver, &Type::named("Text"), &mut BTreeSet::new())
        {
            let mut function = match Type::function(vec![], Type::named("String")) {
                Type::Function(function) => function,
                _ => unreachable!(),
            };
            function.callable = Some(Callable::Builtin(format!("std.core.Text.{name}")));
            return self.bind_receiver(Type::Function(function), &dispatch_receiver);
        }
        // A named error enum, as in `Int or AppError`, takes these too; the result holds the
        // root Error, since a context or a mapped error is no longer an AppError case.
        if (contains_error(&receiver) || self.returns_some_error(&receiver))
            && matches!(name.as_str(), "mapError" | "withContext" | "context")
        {
            let is_root = self.is_error_type(&receiver);
            if name != "context" || is_root {
                let mut parameters = vec![Parameter {
                    name: Some("result".into()),
                    ty: receiver.clone(),
                    default: false,
                    passing: Passing::Move,
                }];
                let argument = if name == "mapError" {
                    let Type::Function(mut transform) =
                        Type::function(vec![Type::named("Error")], Type::named("Error"))
                    else {
                        unreachable!()
                    };
                    transform.parameters[0].passing = Passing::Move;
                    Type::Function(transform)
                } else {
                    Type::named("String")
                };
                parameters.push(Parameter {
                    name: Some(
                        if name == "mapError" {
                            "transform"
                        } else {
                            "message"
                        }
                        .into(),
                    ),
                    ty: argument,
                    default: false,
                    passing: Passing::Borrow,
                });
                let widened = match self.inference.resolve(&receiver) {
                    Type::Union(parts) => parts.iter().fold(Type::Never, |result, part| {
                        let part = if self.is_error_type(part) {
                            Type::named("Error")
                        } else {
                            part.clone()
                        };
                        if result == Type::Never {
                            part
                        } else {
                            Type::union(result, part)
                        }
                    }),
                    other => other,
                };
                let Type::Function(mut function) = Type::function(
                    vec![],
                    if is_root {
                        Type::named("Error")
                    } else {
                        widened
                    },
                ) else {
                    unreachable!()
                };
                function.parameters = parameters;
                function.callable = Some(Callable::Builtin(format!("std.error.{name}")));
                return self.extension_member(node, &receiver, Type::Function(function));
            }
        }
        if let Some(ty) = self.builtin_member(&receiver, &name) {
            if self.direct_callee != Some(node.span)
                && matches!(&ty, Type::Function(function) if function.receiver_mutates)
            {
                self.require_mutable_receiver(node, &node.children[0]);
            }
            let mut ty = self.bind_receiver(ty, &dispatch_receiver);
            let owner = base_name(&receiver).unwrap_or("protocol");
            let callable = Callable::Builtin(format!("std.core.{owner}.{name}"));
            match &mut ty {
                Type::Function(function) => function.callable = Some(callable),
                Type::Overloaded(functions) => {
                    for function in functions {
                        function.callable = Some(callable.clone());
                    }
                }
                _ => {}
            }
            return ty;
        }
        if let Some(base) = base_name(&receiver)
            .map(str::to_owned)
            .or_else(|| contains_error(&receiver).then(|| "Error".to_owned()))
            && let Some(symbol) = self
                .environment
                .values
                .get(&format!("{base}.{name}"))
                .cloned()
        {
            return self.extension_member(node, &receiver, symbol.ty);
        }
        if receiver != Type::Invalid {
            self.error(
                node,
                "E_UNKNOWN_MEMBER",
                format!("{receiver} has no accessible member `{name}`."),
                "Check the member name and the receiver's declared traits.",
            );
        }
        Type::Invalid
    }

    /// `xs.take(2)` where `take` comes from `extend List`: the member's type leaves out the
    /// receiver parameter, and the call passes the receiver first.
    fn extension_member(&mut self, node: &Node, receiver: &Type, ty: Type) -> Type {
        if self.direct_callee != Some(node.span) {
            self.error(
                node,
                "E_EXTEND_VALUE",
                "A function from an extend block is called as a method here.",
                "Call it, as in xs.take(2), or write a lambda: n => xs.take(n).",
            );
            return Type::Invalid;
        }
        let full = self.inference.instantiate(&ty);
        let without_receiver = |function: &FunctionType| {
            let mut function = function.clone();
            if !function.parameters.is_empty() {
                function.parameters.remove(0);
            }
            function
        };
        let bound = match &full {
            Type::Function(function) => {
                if let Some(first) = function.parameters.first() {
                    let first = first.ty.clone();
                    let mut obligations = vec![];
                    self.constrain_argument(&first, receiver, &mut obligations);
                }
                Type::Function(without_receiver(function))
            }
            Type::Overloaded(functions) => {
                Type::Overloaded(functions.iter().map(without_receiver).collect())
            }
            other => other.clone(),
        };
        self.extension_members
            .insert(node.span, (full, receiver.clone()));
        bound
    }

    fn builtin_member(&mut self, receiver: &Type, name: &str) -> Option<Type> {
        // Tuples, records and enums that order by their parts offer compare (REVIEW2 section 3).
        if name == "compare" {
            let resolved = self.inference.resolve(receiver);
            let plain = match &resolved {
                Type::Borrow(inner) | Type::StoredView(inner) => inner.as_ref().clone(),
                other => other.clone(),
            };
            let structural = matches!(&plain, Type::Tuple(_))
                || matches!(&plain, Type::Named(n, _) if self.environment.definitions.get(n).is_some_and(|d| {
                    (d.kind == K::Enum && d.parent.is_none() || d.kind == K::Class && d.record)
                        && !d.methods.contains_key("compare")
                }));
            if structural
                && self.implements(&plain, &Type::named("Comparable"), &mut BTreeSet::new())
            {
                return Some(Type::function(vec![plain], Type::named("Int")));
            }
        }
        if crate::types::is_variable(receiver) {
            return self.variable_member(receiver, name);
        }
        if crate::types::is_tensor(receiver) {
            return self.tensor_member(receiver, name);
        }
        let function = |params, result| Some(Type::function(params, result));
        // at(i) reads one List item without materializing the whole List as an Array.
        // It copies the item, so restrict it to the value types whose copies are independent.
        if name == "at"
            && let Type::Named(base, arguments) = receiver
            && base == "List"
            && let [element] = arguments.as_slice()
            && matches!(element,
                Type::Named(name, args)
                    if args.is_empty()
                        && matches!(name.as_str(), "Int" | "Nat" | "double" | "Bool" | "String"))
        {
            return function(vec![Type::named("Int")], element.clone());
        }
        if name == "compare"
            && self.implements(receiver, &Type::named("Comparable"), &mut BTreeSet::new())
        {
            return function(vec![receiver.clone()], Type::named("Int"));
        }
        // fold, sum and product read the items of any Iterable in order (Q133). sum and product take numbers.
        if matches!(name, "fold" | "sum" | "product")
            && let Some(instance) =
                self.trait_instance(receiver, "Iterable", &[], &mut BTreeSet::new())
            && let Some((_, _, Some(bindings))) = trait_parts(&instance)
            && let Some(item) = bindings.get("Item").cloned()
        {
            if name == "fold" {
                let accumulator = self.inference.fresh();
                let step = Type::function(vec![accumulator.clone(), item], accumulator.clone());
                return function(vec![accumulator.clone(), step], accumulator);
            }
            if matches!(&item, Type::Named(n, a) if a.is_empty() && matches!(n.as_str(), "Int" | "Nat" | "double"))
            {
                return function(vec![], item);
            }
        }
        // enumerate pairs each item with its position, and zip pairs the items of two Iterables (Q134).
        if matches!(name, "enumerate" | "zip")
            && let Some(instance) =
                self.trait_instance(receiver, "Iterable", &[], &mut BTreeSet::new())
            && let Some((_, _, Some(bindings))) = trait_parts(&instance)
            && let Some(item) = bindings.get("Item").cloned()
        {
            if name == "enumerate" {
                return function(
                    vec![],
                    Type::applied("List", vec![Type::Tuple(vec![Type::named("Int"), item])]),
                );
            }
            let other = self.inference.fresh();
            let other_item = self.inference.fresh();
            let Type::Function(mut signature) = Type::function(
                vec![other.clone()],
                Type::applied("List", vec![Type::Tuple(vec![item, other_item.clone()])]),
            ) else {
                unreachable!()
            };
            signature.requirements.push((
                other,
                associated(Type::named("Iterable"), [("Item", other_item)]),
            ));
            return Some(Type::Function(signature));
        }
        // sorted() gives a new List of the items in ascending order, for any Sortable receiver (Q123).
        // sorted() orders the items; sorted(key = f) orders them by f(item), which needs only Comparable keys (Q135).
        // sorted(key) needs only a Comparable key, so it works for any List or Array;
        // sorted() needs Comparable items (REVIEW2 1.9).
        let keyed_item = match self.inference.resolve(receiver) {
            Type::Named(name, args)
                if matches!(name.as_str(), "List" | "Array") && args.len() == 1 =>
            {
                Some(args[0].clone())
            }
            Type::Borrow(inner) | Type::StoredView(inner) => match *inner {
                Type::Named(name, args)
                    if matches!(name.as_str(), "List" | "Array") && args.len() == 1 =>
                {
                    Some(args[0].clone())
                }
                _ => None,
            },
            _ => None,
        };
        let sortable_item = self
            .trait_instance(receiver, "Sortable", &[], &mut BTreeSet::new())
            .and_then(|instance| {
                trait_parts(&instance).and_then(|(_, _, b)| b.and_then(|b| b.get("Item").cloned()))
            });
        if name == "sorted"
            && let Some(item) = sortable_item.clone().or(keyed_item)
        {
            let item_for_requirement = item.clone();
            let Type::Function(plain) =
                Type::function(vec![], Type::applied("List", vec![item.clone()]))
            else {
                unreachable!()
            };
            let key_result = self.inference.fresh();
            let Type::Function(mut keyed) = Type::function(
                vec![Type::function(vec![item.clone()], key_result.clone())],
                Type::applied("List", vec![item]),
            ) else {
                unreachable!()
            };
            keyed.parameters[0].name = Some("key".into());
            keyed
                .requirements
                .push((key_result, Type::named("Comparable")));
            // Without Comparable items, sorted() stays visible so its error names the
            // missing trait instead of a missing key.
            let mut plain = plain;
            if sortable_item.is_none() {
                plain
                    .requirements
                    .push((item_for_requirement, Type::named("Comparable")));
            }
            return Some(Type::Overloaded(vec![plain, keyed]));
        }
        let protocol = match name {
            "items" => Some(("Iterable", "Iterator")),
            "next" => Some(("Iterator", "Item")),
            other => method_operator_trait(other).map(|name| (name, "Output")),
        };
        if let Some((trait_name, slot)) = protocol {
            let wanted = if operator_trait(trait_name) {
                vec![self.inference.fresh()]
            } else {
                vec![]
            };
            if let Some(instance) =
                self.trait_instance(receiver, trait_name, &wanted, &mut BTreeSet::new())
                && let Some((_, args, Some(bindings))) = trait_parts(&instance)
                && let Some(result) = bindings.get(slot).cloned().or_else(|| {
                    (trait_name == "Iterable")
                        .then(|| {
                            bindings
                                .get("Item")
                                .map(|item| Type::Cursor(Box::new(item.clone()), true))
                        })
                        .flatten()
                })
            {
                let Type::Function(mut signature) = Type::function(
                    args.to_vec(),
                    if name == "next" {
                        Type::applied("Option", vec![Type::StoredView(Box::new(result.clone()))])
                    } else {
                        result.clone()
                    },
                ) else {
                    unreachable!()
                };
                signature.effects = matches!(trait_name, "Iterable" | "Iterator")
                    && matches!(
                        receiver,
                        Type::Rigid(_) | Type::Parameter(_) | Type::Associated(_, _)
                    );
                signature.receiver_mutates = signature.effects;
                if let Type::Lazy(_, traversal) = receiver {
                    signature.receiver_mutates = traversal.mutates;
                }
                if let Type::Cursor(_, effects) = receiver {
                    signature.effects = *effects;
                    signature.receiver_mutates = true;
                }
                return Some(Type::Function(signature));
            }
        }
        if matches!(
            name,
            "map"
                | "filter"
                | "toList"
                | "toArray"
                | "join"
                | "first"
                | "last"
                | "min"
                | "max"
                | "minBy"
                | "maxBy"
                | "any"
                | "all"
                | "isEmpty"
        ) && let Some(instance) =
            self.trait_instance(receiver, "Iterable", &[], &mut BTreeSet::new())
            && let Some((_, _, Some(bindings))) = trait_parts(&instance)
            && let Some(element) = bindings.get("Item")
        {
            let traversal = self.traversal(receiver, bindings);
            let mut key_requirement = None;
            let (parameters, result) = match name {
                "map" => {
                    let output = self.inference.fresh();
                    (
                        vec![Type::function(vec![element.clone()], output.clone())],
                        Type::Lazy(Box::new(output), traversal),
                    )
                }
                "filter" => (
                    vec![Type::function(vec![element.clone()], Type::named("Bool"))],
                    Type::Lazy(Box::new(element.clone()), traversal),
                ),
                // first and last give the outer items, min and max the least and greatest, as Options.
                // An item that copies comes out as a value. Any other item comes out as a stored view,
                // as peek does, so a class that implements Comparable can be compared in place (Q114).
                "first" | "last" | "min" | "max" => {
                    let item = match self.inference.resolve(element) {
                        Type::Variable(_) => element.clone(),
                        ty if crate::types::copies(&ty) => element.clone(),
                        _ => Type::StoredView(Box::new(element.clone())),
                    };
                    (vec![], Type::applied("Option", vec![item]))
                }
                // minBy and maxBy choose by a Comparable key, as min and max choose by the item.
                "minBy" | "maxBy" => {
                    let item = match self.inference.resolve(element) {
                        Type::Variable(_) => element.clone(),
                        ty if crate::types::copies(&ty) => element.clone(),
                        _ => Type::StoredView(Box::new(element.clone())),
                    };
                    let key = self.inference.fresh();
                    key_requirement = Some(key.clone());
                    (
                        vec![Type::function(vec![element.clone()], key)],
                        Type::applied("Option", vec![item]),
                    )
                }
                // any and all test each item with a predicate.
                "any" | "all" => (
                    vec![Type::function(vec![element.clone()], Type::named("Bool"))],
                    Type::named("Bool"),
                ),
                "isEmpty" => (vec![], Type::named("Bool")),
                // join takes the separator; each item is converted with its Text implementation.
                "join" => (vec![Type::named("String")], Type::named("String")),
                "toArray" => (vec![], Type::applied("Array", vec![element.clone()])),
                _ => (vec![], Type::applied("List", vec![element.clone()])),
            };
            let Type::Function(mut signature) = Type::function(parameters, result) else {
                unreachable!()
            };
            signature.receiver_mutates = traversal.mutates;
            signature.effects = matches!(
                name,
                "toList"
                    | "toArray"
                    | "join"
                    | "first"
                    | "last"
                    | "min"
                    | "max"
                    | "minBy"
                    | "maxBy"
                    | "any"
                    | "all"
                    | "isEmpty"
            ) && traversal.effects;
            if let Some(key) = key_requirement {
                signature
                    .requirements
                    .push((key, Type::named("Comparable")));
            }
            if matches!(name, "min" | "max") {
                signature
                    .requirements
                    .push((element.clone(), Type::named("Comparable")));
            }
            if name == "join" {
                signature
                    .requirements
                    .push((element.clone(), Type::named("Text")));
            }
            return Some(Type::Function(signature));
        }
        if self.is_error_type(receiver) {
            if name == "context" {
                return function(vec![Type::named("String")], Type::named("Error"));
            }
            if name == "cause" {
                return function(vec![], Type::applied("Option", vec![Type::named("Error")]));
            }
        }
        let Type::Named(ty, args) = receiver else {
            return None;
        };
        if ty == "Bytes" && args.is_empty() {
            let int = || Type::named("Int");
            return match name {
                "length" => function(vec![], int()),
                "isEmpty" => function(vec![], Type::named("Bool")),
                "get" => function(vec![int()], Type::applied("Option", vec![int()])),
                "at" => function(vec![int()], int()),
                "slice" => function(vec![int(), int()], Type::named("Bytes")),
                "concat" => function(vec![Type::named("Bytes")], Type::named("Bytes")),
                "toList" => function(vec![], Type::applied("List", vec![int()])),
                "toHex" => function(vec![], Type::named("String")),
                "decodeUtf8" => function(
                    vec![],
                    Type::union(Type::named("String"), Type::named("Error")),
                ),
                _ => None,
            };
        }
        // Basic methods of the named types. Each one lowers to a builtin of the same name.
        if ty == "String" && args.is_empty() {
            match name {
                "bytes" => return function(vec![], Type::named("Bytes")),
                "nextCharBoundary" | "nextGraphemeBoundary" => {
                    return function(vec![Type::named("Int")], Type::named("Int"));
                }
                "trim" | "toUpper" | "toLower" => return function(vec![], Type::named("String")),
                "isEmpty" => return function(vec![], Type::named("Bool")),
                // Python's str methods (Q136). A name with two forms, such as strip() and strip(chars), is overloaded.
                "isalpha" | "isdecimal" | "isdigit" | "isnumeric" | "isalnum" | "isspace"
                | "isupper" | "islower" | "istitle" | "isascii" | "isprintable"
                | "isidentifier" => {
                    return function(vec![], Type::named("Bool"));
                }
                "capitalize" | "title" | "swapcase" | "casefold" | "lower" | "upper" => {
                    return function(vec![], Type::named("String"));
                }
                "zfill" => return function(vec![Type::named("Int")], Type::named("String")),
                // Parts of a String by character position, and characters as numbers (REVIEW2 4).
                "substring" | "slice" => {
                    return function(
                        vec![Type::named("Int"), Type::named("Int")],
                        Type::named("String"),
                    );
                }
                "reversed" => return function(vec![], Type::named("String")),
                "ord" | "byteLength" => return function(vec![], Type::named("Int")),
                "byteAt" => return function(vec![Type::named("Int")], Type::named("Int")),
                "byteSlice" => {
                    return function(
                        vec![Type::named("Int"), Type::named("Int")],
                        Type::named("String"),
                    );
                }
                "lines" | "words" => {
                    return function(vec![], Type::applied("List", vec![Type::named("String")]));
                }
                "padStart" | "padEnd" => {
                    return string_overloads(
                        vec![
                            vec![Type::named("Int")],
                            vec![Type::named("Int"), Type::named("String")],
                        ],
                        Type::named("String"),
                    );
                }
                "removeprefix" | "removesuffix" => {
                    return function(vec![Type::named("String")], Type::named("String"));
                }
                "find" | "rfind" | "count" => {
                    return function(vec![Type::named("String")], Type::named("Int"));
                }
                "index" | "rindex" => {
                    return function(
                        vec![Type::named("String")],
                        Type::applied("Option", vec![Type::named("Int")]),
                    );
                }
                "replace" => {
                    return function(
                        vec![Type::named("String"), Type::named("String")],
                        Type::named("String"),
                    );
                }
                "partition" | "rpartition" => {
                    return function(
                        vec![Type::named("String")],
                        Type::Tuple(vec![
                            Type::named("String"),
                            Type::named("String"),
                            Type::named("String"),
                        ]),
                    );
                }
                "splitlines" => {
                    return function(vec![], Type::applied("List", vec![Type::named("String")]));
                }
                "center" | "ljust" | "rjust" => {
                    return string_overloads(
                        vec![
                            vec![Type::named("Int")],
                            vec![Type::named("Int"), Type::named("String")],
                        ],
                        Type::named("String"),
                    );
                }
                "strip" | "lstrip" | "rstrip" => {
                    return string_overloads(
                        vec![vec![], vec![Type::named("String")]],
                        Type::named("String"),
                    );
                }
                "expandtabs" => {
                    return string_overloads(
                        vec![vec![], vec![Type::named("Int")]],
                        Type::named("String"),
                    );
                }
                "contains" | "startsWith" | "endsWith" => {
                    return function(vec![Type::named("String")], Type::named("Bool"));
                }
                "split" => {
                    return string_overloads(
                        vec![vec![], vec![Type::named("String")]],
                        Type::applied("List", vec![Type::named("String")]),
                    );
                }
                _ => {}
            }
        }
        if ty == "Int" && args.is_empty() {
            match name {
                "abs" => return function(vec![], Type::named("Int")),
                "chr" => return function(vec![], Type::named("String")),
                "sign" | "isqrt" | "popCount" => return function(vec![], Type::named("Int")),
                "isPrime" => return function(vec![], Type::named("Bool")),
                "gcd" | "lcm" | "mod" => {
                    return function(vec![Type::named("Int")], Type::named("Int"));
                }
                "clamp" => {
                    return function(
                        vec![Type::named("Int"), Type::named("Int")],
                        Type::named("Int"),
                    );
                }
                "min" | "max" => return function(vec![Type::named("Int")], Type::named("Int")),
                "isEven" | "isOdd" => return function(vec![], Type::named("Bool")),
                _ => {}
            }
        }
        if ty == "double" && args.is_empty() {
            match name {
                "abs" | "sqrt" | "floor" | "ceil" | "round" => {
                    return function(vec![], Type::named("double"));
                }
                "min" | "max" | "pow" => {
                    return function(vec![Type::named("double")], Type::named("double"));
                }
                "isFinite" | "isNaN" => return function(vec![], Type::named("Bool")),
                "sign" | "trunc" | "cbrt" => return function(vec![], Type::named("double")),
                "hypot" | "atan2" => {
                    return function(vec![Type::named("double")], Type::named("double"));
                }
                "clamp" => {
                    return function(
                        vec![Type::named("double"), Type::named("double")],
                        Type::named("double"),
                    );
                }
                _ => {}
            }
        }
        if ty == "Option" && args.len() == 1 && matches!(name, "isSome" | "isNone") {
            return function(vec![], Type::named("Bool"));
        }
        if matches!(ty.as_str(), "CPointer" | "CConstPointer")
            && args.len() == 1
            && name == "isNull"
        {
            return function(vec![], Type::named("Bool"));
        }
        if ty == "Int" && name == "toDouble" {
            return function(vec![], Type::named("double"));
        }
        if ty == "Int" && name == "toBigInt" {
            return function(vec![], Type::named("BigInt"));
        }
        if ty == "BigInt" {
            match name {
                "checkedToInt" => {
                    return function(
                        vec![],
                        Type::union(Type::named("Int"), Type::named("Error")),
                    );
                }
                "checkedToDouble" => {
                    return function(
                        vec![],
                        Type::union(Type::named("double"), Type::named("Error")),
                    );
                }
                "checkedDivide" => {
                    return function(
                        vec![receiver.clone()],
                        Type::union(receiver.clone(), Type::named("Error")),
                    );
                }
                _ => {}
            }
        }
        // Q45: conversion back from C widths says whether it can fail.
        if crate::interop::C_TYPES.contains(&ty.as_str()) {
            return match name {
                "toDouble" => function(vec![], Type::named("double")),
                "toInt" if !matches!(ty.as_str(), "CUInt64" | "CFloat32") => {
                    function(vec![], Type::named("Int"))
                }
                "checkedToInt" if ty == "CUInt64" => function(
                    vec![],
                    Type::union(Type::named("Int"), Type::named("Error")),
                ),
                _ => None,
            };
        }
        if ty == "double" && name == "checkedToInt" {
            return function(
                vec![],
                Type::union(Type::named("Int"), Type::named("Error")),
            );
        }
        if ty == "double" && name == "isFinite" {
            return function(vec![], Type::named("Bool"));
        }
        if ty == "Nat" && matches!(name, "checkedAdd" | "checkedSubtract") {
            return function(
                vec![receiver.clone()],
                Type::union(receiver.clone(), Type::named("Error")),
            );
        }
        if matches!(ty.as_str(), "Int" | "double")
            && matches!(
                name,
                "checkedAdd" | "checkedSubtract" | "checkedMultiply" | "checkedDivide"
            )
        {
            return function(
                vec![receiver.clone()],
                Type::union(receiver.clone(), Type::named("Error")),
            );
        }
        // get(i) gives the i-th character as a one-character String, or None (chosen 2026-11-08).
        if ty == "String" && args.is_empty() && name == "splitArray" {
            return function(
                vec![Type::named("String")],
                Type::applied("Array", vec![Type::named("String")]),
            );
        }
        if ty == "String" && args.is_empty() && name == "get" {
            return function(
                vec![Type::named("Int")],
                Type::applied("Option", vec![Type::named("String")]),
            );
        }
        // `xs.slice(a, b)` and `xs.reversed()` share the List's cells (REVIEW2 section 4).
        if ty == "List" && args.len() == 1 && name == "slice" {
            return function(
                vec![Type::named("Int"), Type::named("Int")],
                receiver.clone(),
            );
        }
        // `xs.pick([2, 0])` gathers items by position into a new List that shares their cells.
        if ty == "List" && args.len() == 1 && name == "pick" {
            return function(
                vec![Type::applied("List", vec![Type::named("Int")])],
                receiver.clone(),
            );
        }
        if ty == "List" && args.len() == 1 && name == "reversed" {
            return function(vec![], receiver.clone());
        }
        if name == "length"
            && (ty == "String" && args.is_empty()
                || matches!(ty.as_str(), "List" | "Set" | "SortedSet" | "MutSet")
                    && args.len() == 1
                || matches!(ty.as_str(), "Map" | "MutMap") && args.len() == 2)
        {
            return function(vec![], Type::named("Int"));
        }
        if matches!(ty.as_str(), "List" | "Set" | "SortedSet")
            && args.len() == 1
            && (name == "contains" || name == "add" && ty != "List")
        {
            let Type::Function(mut signature) = Type::function(
                vec![args[0].clone()],
                if name == "contains" {
                    Type::named("Bool")
                } else {
                    receiver.clone()
                },
            ) else {
                unreachable!()
            };
            signature.requirements.push((
                args[0].clone(),
                Type::named(if ty == "SortedSet" {
                    "Comparable"
                } else {
                    "Equals"
                }),
            ));
            return Some(Type::Function(signature));
        }
        // index finds a position, so it applies to ordered collections only; the result is an Option.
        if matches!(ty.as_str(), "List" | "Array") && args.len() == 1 && name == "index" {
            let Type::Function(mut signature) = Type::function(
                vec![args[0].clone()],
                Type::applied("Option", vec![Type::named("Int")]),
            ) else {
                unreachable!()
            };
            signature
                .requirements
                .push((args[0].clone(), Type::named("Equals")));
            return Some(Type::Function(signature));
        }
        if ty == "Arena" && args.len() == 1 {
            let handle = Type::applied("ArenaRef", args.clone());
            let signature = match name {
                "insert" => Type::function(vec![args[0].clone()], handle),
                "get" => Type::function(
                    vec![handle],
                    Type::applied("Option", vec![Type::StoredView(Box::new(args[0].clone()))]),
                ),
                "replace" => Type::function(
                    vec![handle, args[0].clone()],
                    Type::union(Type::named("void"), Type::named("Error")),
                ),
                _ => return None,
            };
            let Type::Function(mut signature) = signature else {
                unreachable!()
            };
            signature.receiver_mutates = matches!(name, "insert" | "replace");
            if signature.receiver_mutates {
                signature.parameters[usize::from(name == "replace")].passing = Passing::Move;
            }
            return Some(Type::Function(signature));
        }
        if ty == "Array" && args.len() == 1 {
            match name {
                "get" => {
                    return function(
                        vec![Type::named("Int")],
                        Type::applied("Option", vec![Type::StoredView(Box::new(args[0].clone()))]),
                    );
                }
                "length" => return function(vec![], Type::named("Int")),
                // pop removes the last item and gives it back as an Option; swap exchanges two slots.
                "pop" => {
                    let Type::Function(mut signature) =
                        Type::function(vec![], Type::applied("Option", vec![args[0].clone()]))
                    else {
                        unreachable!()
                    };
                    signature.receiver_mutates = true;
                    return Some(Type::Function(signature));
                }
                "swap" => {
                    let Type::Function(mut signature) = Type::function(
                        vec![Type::named("Int"), Type::named("Int")],
                        Type::named("void"),
                    ) else {
                        unreachable!()
                    };
                    signature.receiver_mutates = true;
                    return Some(Type::Function(signature));
                }
                // sort() orders the items in place, so it needs a mutable array and Comparable items. sort(key = f)
                // orders them by f(item) instead (Q123, Q135).
                "sort" => {
                    let Type::Function(mut plain) = Type::function(vec![], Type::named("void"))
                    else {
                        unreachable!()
                    };
                    plain.receiver_mutates = true;
                    plain
                        .requirements
                        .push((args[0].clone(), Type::named("Comparable")));
                    let key_result = self.inference.fresh();
                    let Type::Function(mut keyed) = Type::function(
                        vec![Type::function(vec![args[0].clone()], key_result.clone())],
                        Type::named("void"),
                    ) else {
                        unreachable!()
                    };
                    keyed.receiver_mutates = true;
                    keyed.parameters[0].name = Some("key".into());
                    keyed
                        .requirements
                        .push((key_result, Type::named("Comparable")));
                    return Some(Type::Overloaded(vec![plain, keyed]));
                }
                "append" | "prepend" => {
                    let Type::Function(mut signature) =
                        Type::function(vec![args[0].clone()], Type::named("void"))
                    else {
                        unreachable!()
                    };
                    signature.receiver_mutates = true;
                    signature.parameters[0].passing = Passing::Move;
                    return Some(Type::Function(signature));
                }
                _ => {}
            }
        }
        // get(key) finds an Option. get(key, fallback) gives the stored value, or the fallback, so it needs no
        // Option. For now only scalar values are covered: an owned fallback that is not used would need a drop
        // here, and owned values such as String come next (Q127).
        if matches!(ty.as_str(), "Map" | "MutMap") && args.len() == 2 && name == "get" {
            let Type::Function(mut lookup) = Type::function(
                vec![args[0].clone()],
                Type::applied("Option", vec![Type::StoredView(Box::new(args[1].clone()))]),
            ) else {
                unreachable!()
            };
            lookup
                .requirements
                .push((args[0].clone(), Type::named("Equals")));
            let mut overloads = vec![lookup];
            if matches!(&args[1], Type::Named(name, a) if a.is_empty() && matches!(name.as_str(), "Int" | "Nat" | "Bool" | "double"))
            {
                let Type::Function(mut fallback) =
                    Type::function(vec![args[0].clone(), args[1].clone()], args[1].clone())
                else {
                    unreachable!()
                };
                fallback
                    .requirements
                    .push((args[0].clone(), Type::named("Equals")));
                overloads.push(fallback);
            }
            return Some(if overloads.len() == 1 {
                Type::Function(overloads.remove(0))
            } else {
                Type::Overloaded(overloads)
            });
        }
        if matches!(ty.as_str(), "Map" | "MutMap") && args.len() == 2 {
            let signature = match name {
                "get" => Some(Type::function(
                    vec![args[0].clone()],
                    Type::applied("Option", vec![Type::StoredView(Box::new(args[1].clone()))]),
                )),
                "contains" => Some(Type::function(vec![args[0].clone()], Type::named("Bool"))),
                // update(key, change) changes a stored value in place and tells whether
                // the key was present (REVIEW 1.6).
                // A copyable value is replaced by the change's result; an object is
                // changed in place through a mut parameter.
                "update" if ty == "MutMap" => {
                    // An object with var fields changes in place; any other value, such as
                    // a number or a record, is replaced by the change's result.
                    let in_place =
                        !crate::types::copies(&args[1])
                            && match &args[1] {
                                Type::Named(name, _) => {
                                    self.environment.definitions.get(name).is_some_and(
                                        |definition| !definition.mutable_fields.is_empty(),
                                    ) || matches!(name.as_str(), "Array" | "MutMap" | "MutSet")
                                }
                                _ => false,
                            };
                    let copies = !in_place;
                    let change = Type::Function(FunctionType {
                        parameters: vec![Parameter {
                            name: Some("value".into()),
                            ty: args[1].clone(),
                            default: false,
                            passing: if copies {
                                Passing::Borrow
                            } else {
                                Passing::Mut
                            },
                        }],
                        result: Box::new(if copies {
                            args[1].clone()
                        } else {
                            Type::named("void")
                        }),
                        bounds: BTreeMap::new(),
                        requirements: vec![],
                        effects: false,
                        receiver_mutates: false,
                        callable: None,
                        receiver: None,
                    });
                    Some(Type::function(
                        vec![args[0].clone(), change],
                        Type::named("Bool"),
                    ))
                }
                "put" => Some(Type::function(
                    args.clone(),
                    if ty == "Map" {
                        receiver.clone()
                    } else {
                        Type::named("void")
                    },
                )),
                // remove(key): a Map gives a new Map without the key; a MutMap drops the
                // entry and tells whether the key was present, as update does (Q164).
                "remove" => Some(Type::function(
                    vec![args[0].clone()],
                    if ty == "Map" {
                        receiver.clone()
                    } else {
                        Type::named("Bool")
                    },
                )),
                _ => None,
            };
            if let Some(Type::Function(mut signature)) = signature {
                signature
                    .requirements
                    .push((args[0].clone(), Type::named("Equals")));
                signature.receiver_mutates =
                    ty == "MutMap" && matches!(name, "put" | "update" | "remove");
                return Some(Type::Function(signature));
            }
        }
        if ty == "Atomic"
            && args.len() == 1
            && matches!(args[0], Type::Named(ref n, ref a) if a.is_empty() && matches!(n.as_str(), "Int" | "double"))
        {
            let value = args[0].clone();
            let result = match name {
                "load" => Some(Type::function(vec![], value.clone())),
                "store" => Some(Type::function(vec![value.clone()], Type::named("void"))),
                "fetchAdd" => Some(Type::function(
                    vec![value.clone()],
                    if value == Type::named("Int") {
                        Type::union(value.clone(), Type::named("Error"))
                    } else {
                        value.clone()
                    },
                )),
                "compareExchange" => Some(Type::function(
                    vec![value.clone(), value.clone()],
                    Type::named("Bool"),
                )),
                _ => None,
            };
            if let Some(Type::Function(mut signature)) = result {
                signature.effects = true;
                return Some(Type::Function(signature));
            }
        }
        if ty == "Channel" && args.len() == 1 {
            let signature = match name {
                "send" => Some(Type::function(
                    vec![args[0].clone()],
                    Type::union(Type::named("void"), Type::named("Error")),
                )),
                "close" => Some(Type::function(vec![], Type::named("void"))),
                "receive" => Some(Type::function(
                    vec![],
                    Type::applied("Option", vec![args[0].clone()]),
                )),
                _ => None,
            };
            if let Some(Type::Function(mut signature)) = signature {
                // Talking to other tasks is outside work (confirmed 2026-11-03).
                signature.effects = true;
                // The channel keeps what it is sent.
                for parameter in &mut signature.parameters {
                    parameter.passing = Passing::Move;
                }
                return Some(Type::Function(signature));
            }
        }
        if matches!(ty.as_str(), "Reader" | "Writer") && args.is_empty() {
            let failing = |ty: &str| Type::union(Type::named(ty), Type::named("Error"));
            let signature = match (ty.as_str(), name) {
                ("Reader", "readLine") => Some(Type::function(vec![], failing("String"))),
                ("Writer", "writeLine") => {
                    Some(Type::function(vec![Type::named("String")], failing("void")))
                }
                _ => None,
            };
            if let Some(Type::Function(mut signature)) = signature {
                signature.effects = true;
                // Reading or writing moves the file position.
                signature.receiver_mutates = true;
                return Some(Type::Function(signature));
            }
        }
        if ty == "MutSet" && args.len() == 1 {
            if name == "add" {
                let Type::Function(mut signature) =
                    Type::function(vec![args[0].clone()], Type::named("void"))
                else {
                    unreachable!()
                };
                signature.receiver_mutates = true;
                signature
                    .requirements
                    .push((args[0].clone(), Type::named("Equals")));
                return Some(Type::Function(signature));
            }
            if name == "contains" {
                let Type::Function(mut signature) =
                    Type::function(vec![args[0].clone()], Type::named("Bool"))
                else {
                    unreachable!()
                };
                signature
                    .requirements
                    .push((args[0].clone(), Type::named("Equals")));
                return Some(Type::Function(signature));
            }
        }
        None
    }

    fn traversal(
        &self,
        collection: &Type,
        bindings: &BTreeMap<String, Type>,
    ) -> crate::types::Traversal {
        if let Type::Lazy(_, traversal) = collection {
            return *traversal;
        }
        let generic = abstract_protocol(&self.inference.resolve(collection));
        let items = self.methods(collection, "items", &mut BTreeSet::new());
        let next_effectful = bindings.get("Iterator").is_some_and(|cursor| {
            self.methods(cursor, "next", &mut BTreeSet::new())
                .iter()
                .any(|method| method.effects)
        });
        crate::types::Traversal {
            effects: generic || items.iter().any(|method| method.effects) || next_effectful,
            mutates: generic || items.iter().any(|method| method.receiver_mutates),
        }
    }

    fn iteration_item(&mut self, node: &Node, collection: &Type, expression: &Node) -> Type {
        let item = self.inference.fresh();
        if let Some(instance) =
            self.trait_instance(collection, "Iterable", &[], &mut BTreeSet::new())
            && let Some((_, _, Some(bindings))) = trait_parts(&instance)
        {
            let traversal = self.traversal(collection, bindings);
            if traversal.mutates {
                self.require_mutable_receiver(node, expression);
            }
            if traversal.effects {
                self.require_effect(node, "Iterating this cursor");
            }
        }
        let requirement = associated(Type::named("Iterable"), [("Item", item.clone())]);
        if self.satisfy_trait(collection, &requirement) {
            return self.inference.resolve(&item);
        }
        self.error(node, "E_ITERABLE", format!("{collection} does not supply an Iterable item type."), "Declare has Iterable and write items returning a cursor whose next returns Option[Item]. Generic bounds must supply Item.");
        Type::Invalid
    }

    fn loop_statement(&mut self, node: &Node) -> Type {
        let before_initialization = self.initializing_fields.clone();
        self.scopes.push(BTreeMap::new());
        self.loop_depth += 1;
        if node.kind == K::While {
            let condition = self.infer(&node.children[0], Some(&Type::named("Bool")));
            self.require(&node.children[0], &Type::named("Bool"), &condition);
        } else if node.children[0].kind == K::ForBinding {
            let header = &node.children[0];
            let collection = self.infer(header.children.last().unwrap(), None);
            let item = self.iteration_item(header, &collection, header.children.last().unwrap());
            if let Some(annotation) = header.children.iter().find(|n| is_type(n.kind)) {
                let expected = self.annotation(annotation, false);
                self.require(header, &expected, &item);
            }
            if let Some(pattern) = header.children.iter().find(|n| {
                matches!(
                    n.kind,
                    K::PatternName | K::PatternTuple | K::PatternWildcard
                )
            }) {
                self.pattern(pattern, &item, false, false);
            }
        } else {
            // The C-style loop variable is assigned by its step, as in the syntax examples.
            let header = &node.children[0];
            let name = binding_name(header, self.source);
            let expected = header
                .children
                .iter()
                .find(|n| is_type(n.kind))
                .map(|n| self.annotation(n, false));
            let value = self.infer(header.children.last().unwrap(), expected.as_ref());
            if let Some(expected) = &expected {
                self.require(header, expected, &value);
            }
            self.bind(header, &name, expected.unwrap_or(value), true, false);
            let condition = self.infer(&node.children[1], Some(&Type::named("Bool")));
            self.require(&node.children[1], &Type::named("Bool"), &condition);
            self.infer(&node.children[2], None);
        }
        self.infer(node.children.last().unwrap(), None);
        self.loop_depth -= 1;
        self.scopes.pop();
        self.initializing_fields = before_initialization;
        Type::named("void")
    }
}

impl Checker<'_> {
    /// An expression whose value is thrown away cannot hide a failure: a result that
    /// includes Error must be matched, propagated with `?`, or discarded with `let _ :=`.
    fn unused_errors(&mut self, root: &Node, types: &[(Span, Type)]) {
        let types: BTreeMap<Span, &Type> = types.iter().map(|(span, ty)| (*span, ty)).collect();
        let mut discarded = vec![];
        // A session or notebook shows each entry's result, and it rechecks its whole
        // history as one file, so only a script's top-level statements are discarded.
        let interactive = matches!(self.source.name.as_str(), "<session>" | "<cell>");
        if !interactive {
            discarded.extend(
                root.children
                    .iter()
                    .filter(|child| !is_declaration(child.kind)),
            );
        }
        fn visit<'a>(
            node: &'a Node,
            discards_last: bool,
            source: &SourceFile,
            out: &mut Vec<&'a Node>,
        ) {
            if node.kind == K::Block {
                let count = node.children.len();
                for (i, child) in node.children.iter().enumerate() {
                    if i + 1 < count || discards_last {
                        out.push(child);
                    }
                }
            }
            let void_body = node.kind == K::Constructor
                || node.kind == K::Function
                    && node
                        .child(K::ReturnType)
                        .is_some_and(|r| source.slice(r.span).trim() == "void");
            for (i, child) in node.children.iter().enumerate() {
                let discards = child.kind == K::Block
                    && (matches!(node.kind, K::For | K::While)
                        || node.kind == K::If && node.children.len() == 2 && i == 1
                        || void_body);
                visit(child, discards, source, out);
            }
        }
        visit(root, false, self.source, &mut discarded);
        for node in discarded {
            if matches!(
                node.kind,
                K::Binding
                    | K::FieldDeclaration
                    | K::Assignment
                    | K::For
                    | K::While
                    | K::Return
                    | K::Break
                    | K::Continue
                    | K::Spawn
            ) || is_declaration(node.kind)
            {
                continue;
            }
            let Some(ty) = types.get(&node.span) else {
                continue;
            };
            let Type::Union(parts) = ty else { continue };
            if !parts.iter().any(|part| self.is_error_type(part)) {
                continue;
            }
            self.diagnostics.push(
                Diagnostic::new(
                    "E_UNUSED_ERROR",
                    format!("This `{ty}` result is ignored, so a failure would be lost."),
                    node.span,
                )
                .with_hint("Handle it with `match`, pass it on with `?`, or discard it explicitly with `let _ := ...`."),
            );
        }
    }
    /// Whether a function's declared result can carry an Error, including a named
    /// error enum such as `Int or AppError`.
    fn returns_some_error(&self, ty: &Type) -> bool {
        match self.inference.resolve(ty) {
            Type::Union(parts) => parts.iter().any(|part| self.is_error_type(part)),
            other => contains_error(&other) || self.is_error_type(&other),
        }
    }
    fn is_error_type(&self, ty: &Type) -> bool {
        match ty {
            Type::Named(name, _) if name == "Error" => true,
            Type::Named(name, _) => {
                self.environment
                    .definitions
                    .get(name)
                    .is_some_and(|definition| {
                        definition.kind == K::Enum
                            && definition.parent.as_ref() == Some(&Type::named("Error"))
                    })
            }
            Type::Borrow(inner) => self.is_error_type(inner),
            _ => false,
        }
    }

    fn case_expected(&self, expected: &Type, found: &Type) -> bool {
        match expected {
            Type::Union(parts) => parts.iter().any(|part| self.case_expected(part, found)),
            Type::Named(name, _) if name == "Error" => self.is_error_type(found),
            Type::Named(_, _) | Type::Indexed(_, _) => base_name(expected) == base_name(found),
            _ => true,
        }
    }

    fn map_literal(&mut self, node: &Node, constructor: Type, expected: Option<&Type>) -> Type {
        let Type::Constructor(_, arguments) = constructor else {
            unreachable!()
        };
        let (key, value) = if arguments.len() == 2 {
            (arguments[0].clone(), arguments[1].clone())
        } else if let Some(Type::Named(name, args)) = self.value_expected(expected)
            && name == "Map"
            && args.len() == 2
        {
            (args[0].clone(), args[1].clone())
        } else {
            (self.inference.fresh(), self.inference.fresh())
        };
        for entry in node.children.iter().skip(1) {
            if entry.kind != K::ArrowArgument || entry.children.len() != 2 {
                self.error(
                    entry,
                    "E_MAP_ENTRY",
                    "Map arguments must be key => value entries.",
                    "Write Map(key => value), including for variable keys.",
                );
                continue;
            }
            let found_key = self.infer(&entry.children[0], Some(&key));
            let found_value = self.infer(&entry.children[1], Some(&value));
            self.require(&entry.children[0], &key, &found_key);
            self.require(&entry.children[1], &value, &found_value);
        }
        if node.children.len() > 1 {
            self.check_obligations(node, &[(key.clone(), Type::named("Equals"))]);
        }
        let key = self.inference.resolve(&key);
        let value = self.inference.resolve(&value);
        let result = Type::applied("Map", vec![key.clone(), value.clone()]);
        let entries = node
            .children
            .iter()
            .skip(1)
            .filter(|entry| entry.kind == K::ArrowArgument && entry.children.len() == 2)
            .collect::<Vec<_>>();
        let inputs = entries
            .iter()
            .flat_map(|entry| entry.children.iter().map(|node| node.span))
            .collect::<Vec<_>>();
        let Type::Function(mut signature) = Type::function(
            entries
                .iter()
                .flat_map(|_| [key.clone(), value.clone()])
                .collect(),
            result.clone(),
        ) else {
            unreachable!()
        };
        signature.callable = Some(Callable::Builtin("std.collections.Map".into()));
        self.resolved_calls.insert(
            node.span,
            ResolvedCall {
                target: Callable::Builtin("std.collections.Map".into()),
                callee: Some(node.children[0].span),
                overload: 0,
                signature,
                type_arguments: BTreeMap::from([("K".into(), key), ("V".into(), value)]),
                arguments: inputs
                    .iter()
                    .map(|span| ResolvedArgument::Supplied {
                        expression: *span,
                        hole: false,
                    })
                    .collect(),
                evaluation_order: inputs,
                receiver: None,
                partial: false,
            },
        );
        result
    }
}

fn parameter_list(function: &FunctionType) -> String {
    function
        .parameters
        .iter()
        .map(|parameter| {
            format!(
                "{} : {}",
                parameter.name.as_deref().unwrap_or("_"),
                parameter.ty
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn argument_order(
    function: &FunctionType,
    arguments: &[(Option<String>, Type, &Node, bool)],
) -> Option<Vec<Option<usize>>> {
    let mut order = vec![None; function.parameters.len()];
    let mut positional = 0;
    for (index, (name, _, _, _)) in arguments.iter().enumerate() {
        let slot = if let Some(name) = name {
            function
                .parameters
                .iter()
                .position(|p| p.name.as_ref() == Some(name))?
        } else {
            let slot = positional;
            positional += 1;
            slot
        };
        if slot >= order.len() || order[slot].replace(index).is_some() {
            return None;
        }
    }
    if order
        .iter()
        .zip(&function.parameters)
        .any(|(arg, parameter)| arg.is_none() && !parameter.default)
    {
        return None;
    }
    Some(order)
}
fn base_name(ty: &Type) -> Option<&str> {
    match ty {
        Type::Named(name, _) => Some(name),
        Type::Indexed(inner, _) | Type::Borrow(inner) | Type::Associated(inner, _) => {
            base_name(inner)
        }
        _ => None,
    }
}
fn intersect_initialization(
    left: Option<BTreeSet<String>>,
    right: Option<BTreeSet<String>>,
) -> Option<BTreeSet<String>> {
    left.zip(right)
        .map(|(left, right)| left.intersection(&right).cloned().collect())
}
fn abstract_protocol(ty: &Type) -> bool {
    match ty {
        Type::Rigid(_) | Type::Parameter(_) | Type::Dynamic(_) | Type::Associated(_, _) => true,
        Type::Borrow(inner) => abstract_protocol(inner),
        _ => false,
    }
}
fn ungroup(mut node: &Node) -> &Node {
    while node.kind == K::Group {
        node = &node.children[0];
    }
    node
}
fn place_path(node: &Node, source: &SourceFile) -> Option<Vec<String>> {
    match node.kind {
        K::Name => Some(vec![source.slice(node.span).into()]),
        K::Group => place_path(&node.children[0], source),
        K::Member => {
            let mut path = place_path(&node.children[0], source)?;
            path.push(source.slice(node.children[1].span).into());
            Some(path)
        }
        // A literal index names one element; any other index may be any element, so it
        // stands for the whole Array (REVIEW2 1.11).
        K::Index => {
            let mut path = place_path(&node.children[0], source)?;
            if let Some(index) = node.children.get(1)
                && index.kind == K::Literal
            {
                path.push(format!("[{}]", source.slice(index.span)));
            }
            Some(path)
        }
        _ => None,
    }
}
/// A place path as written: `xs[0].n`, not `xs.[0].n`.
fn render_place(path: &[String]) -> String {
    let mut text = String::new();
    for (i, part) in path.iter().enumerate() {
        if i > 0 && !part.starts_with('[') {
            text.push('.');
        }
        text.push_str(part);
    }
    text
}
fn place_root<'a>(node: &Node, source: &'a SourceFile) -> Option<&'a str> {
    match node.kind {
        K::Name => Some(source.slice(node.span)),
        // An Array slot belongs to its Array: `this.items[i]` is rooted at `this`.
        K::Member | K::Group | K::Index => place_root(&node.children[0], source),
        _ => None,
    }
}
fn contains_error(ty: &Type) -> bool {
    match ty {
        Type::Named(name, _) => name == "Error",
        Type::Union(parts) => parts.iter().any(contains_error),
        _ => false,
    }
}

fn function_body(node: &Node) -> Option<&Node> {
    let parameter_end = node
        .child(K::ParameterList)
        .map(|parameters| parameters.span.end)
        .unwrap_or(node.span.end);
    node.children.iter().rev().find(|child| {
        child.span.start >= parameter_end
            && !matches!(
                child.kind,
                K::Modifier
                    | K::Attribute
                    | K::TypeParameters
                    | K::ParameterList
                    | K::ReturnType
                    | K::Effects
                    | K::Requires
                    | K::Ensures
            )
    })
}

/// `Option[T]` for some T, the type that `?` can unwrap or return None for (Q115).
fn is_option_type(ty: &Type) -> bool {
    matches!(ty, Type::Named(name, args) if name == "Option" && args.len() == 1)
}
/// Arguments as they are inferred, before the call resolves: optional name,
/// inferred type, source node, and whether the source wrote `_`.
type InferredArguments<'a> = [Option<(Option<String>, Type, &'a Node, bool)>];
type TraitParts<'a> = (&'a str, &'a [Type], Option<&'a BTreeMap<String, Type>>);
fn trait_parts(ty: &Type) -> Option<TraitParts<'_>> {
    match ty {
        Type::Named(name, args) => Some((name, args, None)),
        Type::Associated(base, bindings) => {
            let (name, args, _) = trait_parts(base)?;
            Some((name, args, Some(bindings)))
        }
        _ => None,
    }
}
/// The forms of one String method, which share a name and differ in their parameters (Q136).
fn string_overloads(forms: Vec<Vec<Type>>, result: Type) -> Option<Type> {
    let functions = forms
        .into_iter()
        .map(
            |parameters| match Type::function(parameters, result.clone()) {
                Type::Function(function) => function,
                _ => unreachable!(),
            },
        )
        .collect::<Vec<_>>();
    Some(if functions.len() == 1 {
        Type::Function(functions.into_iter().next().unwrap())
    } else {
        Type::Overloaded(functions)
    })
}
fn associated<const N: usize>(base: Type, bindings: [(&str, Type); N]) -> Type {
    Type::Associated(
        Box::new(base),
        bindings
            .into_iter()
            .map(|(name, ty)| (name.into(), ty))
            .collect(),
    )
}
fn match_trait(inference: &mut Inference, instance: &Type, requirement: &Type) -> bool {
    let (Some((a, xs, bindings)), Some((b, ys, expected))) =
        (trait_parts(instance), trait_parts(requirement))
    else {
        return false;
    };
    let saved = inference.clone();
    let valid = a == b
        && inference.unify(&Type::Tuple(xs.to_vec()), &Type::Tuple(ys.to_vec()))
        && expected.is_none_or(|expected| {
            expected.iter().all(|(name, ty)| {
                bindings
                    .and_then(|bindings| bindings.get(name))
                    .is_some_and(|found| inference.unify(ty, found))
            })
        });
    if !valid {
        *inference = saved;
    }
    valid
}
fn builtin_value(ty: &Type) -> bool {
    matches!(ty, Type::Tuple(_))
        || matches!(ty, Type::Named(name, _) if builtin_arity(name).is_some())
}
fn bigint_context(ty: &Type) -> bool {
    *ty == Type::named("BigInt")
        || matches!(ty,Type::Union(parts) if parts.contains(&Type::named("BigInt")) && !parts.contains(&Type::named("Int")) && !parts.contains(&Type::named("Nat")))
}
fn int_literal_expression(node: &Node, source: &SourceFile) -> bool {
    match node.kind {
        K::Literal => {
            let text = source.slice(node.span);
            text.as_bytes().first().is_some_and(u8::is_ascii_digit)
                && !crate::lexer::is_double_literal(text)
        }
        K::One => true,
        K::Group => int_literal_expression(&node.children[0], source),
        K::Unary if matches!(source.slice(node.children[0].span), "+" | "-") => {
            int_literal_expression(&node.children[1], source)
        }
        K::Binary
            if matches!(
                source.slice(node.children[1].span),
                "+" | "-" | "*" | "/" | "%" | "**"
            ) =>
        {
            int_literal_expression(&node.children[0], source)
                && int_literal_expression(&node.children[2], source)
        }
        _ => false,
    }
}

fn bigint_literal_expression(node: &Node, source: &SourceFile) -> bool {
    match node.kind {
        K::Literal => {
            source
                .slice(node.span)
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_digit)
                && !crate::lexer::is_double_literal(source.slice(node.span))
        }
        K::Group => bigint_literal_expression(&node.children[0], source),
        K::Unary if matches!(source.slice(node.children[0].span), "+" | "-") => {
            bigint_literal_expression(&node.children[1], source)
        }
        K::Binary
            if matches!(
                source.slice(node.children[1].span),
                "+" | "-" | "*" | "/" | "%" | "**"
            ) =>
        {
            bigint_literal_expression(&node.children[0], source)
                && bigint_literal_expression(&node.children[2], source)
        }
        _ => false,
    }
}
fn numeric(ty: &Type) -> bool {
    matches!(ty, Type::Named(name, args) if args.is_empty() && matches!(name.as_str(), "Int" | "double" | "BigInt"))
}
fn capture_permissions_fit(expected: &Type, found: &Type) -> bool {
    if crate::types::same_capture_permissions(expected, found) {
        return true;
    }
    if let (Type::Function(wanted), Type::Function(given)) = (expected, found)
        && wanted.receiver_mutates
        && !given.receiver_mutates
    {
        let mut wanted = wanted.clone();
        wanted.receiver_mutates = false;
        return crate::types::same_capture_permissions(&Type::Function(wanted), found);
    }
    false
}

fn without_capture_permissions(ty: &Type) -> Type {
    ty.map(&mut |ty| {
        if let Type::Function(function) = ty {
            let mut function = function.clone();
            function.receiver_mutates = false;
            function = function.map(&mut |ty| Some(without_capture_permissions(ty)));
            Some(Type::Function(function))
        } else {
            None
        }
    })
}

fn overload(mut methods: Vec<FunctionType>) -> Type {
    if methods.len() == 1 {
        Type::Function(methods.remove(0))
    } else {
        Type::Overloaded(methods)
    }
}
fn operator_family(op: &str) -> Option<u8> {
    match op {
        "+" | "-" | "*" | "/" | "%" | "**" | "@" => Some(0),
        "&" | "|" | "^" | "<<" | ">>" => Some(1),
        "==" | "!=" | "<" | "<=" | ">" | ">=" => Some(2),
        "&&" | "||" => Some(3),
        _ => None,
    }
}
fn irrefutable(node: &Node) -> bool {
    matches!(node.kind, K::Name | K::PatternName | K::PatternWildcard)
        || node.kind == K::PatternTuple && node.children.iter().all(irrefutable)
        || node.kind == K::PatternList
            && node.children.len() == 1
            && node.children[0].kind == K::PatternRest
}
/// The value of an integer pattern literal, as a range bound needs it.
fn integer_literal(text: &str) -> Option<i64> {
    let text = text.replace('_', "");
    let (negative, unsigned) = match text.as_bytes().first() {
        Some(b'-') => (true, &text[1..]),
        _ => (false, text.as_str()),
    };
    let magnitude = if let Some(hex) = unsigned
        .strip_prefix("0x")
        .or_else(|| unsigned.strip_prefix("0X"))
    {
        u64::from_str_radix(hex, 16)
    } else {
        unsigned.parse::<u64>()
    }
    .ok()?;
    if negative && magnitude == 1u64 << 63 {
        Some(i64::MIN)
    } else {
        i64::try_from(magnitude)
            .ok()
            .map(|value| if negative { -value } else { value })
    }
}

#[derive(Clone, Debug)]
enum Pattern {
    Any,
    Case(String, Vec<Pattern>),
}

impl Checker<'_> {
    fn pattern(&mut self, node: &Node, ty: &Type, mutable: bool, generalize: bool) -> Pattern {
        let ty = self.inference.resolve(ty);
        match node.kind {
            K::PatternWildcard => Pattern::Any,
            K::Name | K::PatternName => {
                let name = if node.kind == K::Name {
                    self.text(node).to_owned()
                } else {
                    self.named_child(node)
                };
                if let Type::Union(parts) = &ty
                    && self.narrow_unions
                {
                    let values: Vec<_> = parts
                        .iter()
                        .enumerate()
                        .filter(|(_, ty)| !self.is_error_type(ty))
                        .collect();
                    if values.len() == 1 {
                        let (index, value) = values[0];
                        self.bind(node, &name, value.clone(), mutable, generalize);
                        return Pattern::Case(format!("union:{index}"), vec![Pattern::Any]);
                    }
                    if parts.iter().any(|ty| self.is_error_type(ty)) {
                        self.error(node, "E_UNION_PATTERN_UNSUPPORTED", "Binding several value alternatives while excluding Error is not implemented yet.", "Use separate typed cases, or an unguarded wildcard that handles every alternative.");
                    }
                }
                self.bind(node, &name, ty, mutable, generalize);
                Pattern::Any
            }
            K::PatternLiteral => {
                let text = self.text(node).to_owned();
                let found = if matches!(text.as_str(), "true" | "false") {
                    Type::named("Bool")
                } else if text.starts_with('"') {
                    Type::named("String")
                } else if crate::lexer::is_double_literal(&text)
                    || self.inference.resolve(&ty) == Type::named("double")
                {
                    Type::named("double")
                } else if bigint_context(&ty) {
                    Type::named("BigInt")
                } else if ty == Type::named("Nat") {
                    // Q71: a numeral is a Nat where a Nat is expected, including a pattern.
                    Type::named("Nat")
                } else {
                    Type::named("Int")
                };
                self.check_integer_literal(node, &found);
                self.require(node, &ty, &found);
                Pattern::Case(text, vec![])
            }
            K::PatternRange | K::PatternRangeInclusive => {
                let resolved = self.inference.resolve(&ty);
                let supported = match &resolved {
                    Type::Named(name, _) => name == "Int" || name == "Nat",
                    Type::Union(parts) => parts.iter().any(
                        |part| matches!(part, Type::Named(name, _) if name == "Int" || name == "Nat"),
                    ),
                    Type::Variable(_) => true,
                    _ => false,
                };
                let found = match &resolved {
                    Type::Named(name, _) if name == "Nat" => Type::named("Nat"),
                    Type::Union(parts)
                        if !parts.iter().any(|part| *part == Type::named("Int"))
                            && parts.iter().any(|part| *part == Type::named("Nat")) =>
                    {
                        Type::named("Nat")
                    }
                    _ => Type::named("Int"),
                };
                for bound in &node.children {
                    if integer_literal(self.text(bound)).is_none() {
                        self.error(
                            bound,
                            "E_PATTERN_RANGE_BOUND",
                            "A range bound must be an integer literal.",
                            "Write a whole number such as 0, 9 or -5.",
                        );
                    }
                    self.check_integer_literal(bound, &found);
                }
                if supported {
                    self.require(node, &ty, &found);
                } else {
                    self.error(
                        node,
                        "E_PATTERN_RANGE_TYPE",
                        format!("A range matches Int or Nat values, not {resolved}."),
                        "Use a guard for another type.",
                    );
                }
                // A range is refutable for every integer type, so coverage never
                // counts it as a case.
                Pattern::Case("range".into(), vec![])
            }
            K::PatternAlt => {
                self.error(
                    node,
                    "E_PATTERN_ALT",
                    "An alternative belongs only at the top of a match arm pattern.",
                    "Write the alternatives as separate arms, or move this one out of the nested pattern.",
                );
                self.pattern(&node.children[0], &ty, mutable, generalize)
            }
            K::PatternTuple => {
                let elements = match &ty {
                    Type::Tuple(elements) => elements.clone(),
                    _ => {
                        let elements = node
                            .children
                            .iter()
                            .map(|_| self.inference.fresh())
                            .collect::<Vec<_>>();
                        self.require(node, &Type::Tuple(elements.clone()), &ty);
                        elements
                    }
                };
                if elements.len() != node.children.len() {
                    self.error(
                        node,
                        "E_PATTERN_ARITY",
                        format!(
                            "Expected {} tuple parts, found {}.",
                            elements.len(),
                            node.children.len()
                        ),
                        "Use a pattern with the tuple's fixed number of parts.",
                    );
                }
                Pattern::Case(
                    "tuple".into(),
                    node.children
                        .iter()
                        .zip(&elements)
                        .map(|(n, t)| self.pattern(n, t, mutable, generalize))
                        .collect(),
                )
            }
            K::PatternList => {
                let element = match &ty {
                    Type::Named(name, args) if name == "List" && args.len() == 1 => args[0].clone(),
                    _ => {
                        let element = self.inference.fresh();
                        self.require(node, &Type::applied("List", vec![element.clone()]), &ty);
                        element
                    }
                };
                let mut patterns = vec![];
                let mut tail = Pattern::Case("list:nil".into(), vec![]);
                // `..rest` may stand anywhere once: `[..init, last]` and `[a, ..mid, z]` too.
                let rest_at = node.children.iter().position(|n| n.kind == K::PatternRest);
                let mut suffix_refutable = false;
                for (i, child) in node.children.iter().enumerate() {
                    if child.kind == K::PatternRest {
                        if rest_at != Some(i) {
                            self.error(
                                child,
                                "E_PATTERN_REST",
                                "A list pattern can hold only one rest pattern.",
                                "Keep one ..rest in this list pattern.",
                            );
                        }
                        if let Some(name) = child.children.first() {
                            self.bind(
                                name,
                                self.text(name).to_owned().as_str(),
                                Type::applied("List", vec![element.clone()]),
                                mutable,
                                generalize,
                            );
                        }
                        tail = Pattern::Any;
                    } else {
                        let pattern = self.pattern(child, &element, mutable, generalize);
                        if rest_at.is_some_and(|at| i > at) {
                            // A part after the rest is matched from the end. For coverage it
                            // counts only as a length when it accepts every item.
                            suffix_refutable |= !irrefutable(child);
                            patterns.push(Pattern::Any);
                        } else {
                            patterns.push(pattern);
                        }
                    }
                }
                if suffix_refutable {
                    // Coverage treats this pattern as matching nothing it can prove.
                    return Pattern::Case("list:from-end".into(), vec![]);
                }
                for pattern in patterns.into_iter().rev() {
                    tail = Pattern::Case("list:cons".into(), vec![pattern, tail]);
                }
                tail
            }
            K::PatternConstructor => {
                let name = self.named_child(node);
                if name == "Error" {
                    let index = if let Type::Union(parts) = &ty {
                        parts.iter().position(|part| self.is_error_type(part))
                    } else {
                        None
                    };
                    let error_ty = index
                        .and_then(|i| {
                            if let Type::Union(parts) = &ty {
                                Some(parts[i].clone())
                            } else {
                                None
                            }
                        })
                        .unwrap_or(ty.clone());
                    if !self.is_error_type(&error_ty) {
                        self.error(
                            node,
                            "E_PATTERN_CASE",
                            "Error(e) needs an error alternative.",
                            "Match Error(e) against Error or a fallible result.",
                        );
                    }
                    let arguments = node
                        .children
                        .iter()
                        .filter(|n| n.kind != K::Name)
                        .collect::<Vec<_>>();
                    if arguments.len() != 1 {
                        self.error(
                            node,
                            "E_PATTERN_ARITY",
                            "Error(e) binds exactly one error.",
                            "Write Error(e) or Error(_).",
                        );
                    }
                    let inner = arguments
                        .first()
                        .map(|arg| self.pattern(arg, &Type::named("Error"), mutable, generalize))
                        .unwrap_or(Pattern::Any);
                    if !matches!(inner, Pattern::Any) {
                        self.error(
                            node,
                            "E_PATTERN_CASE",
                            "Error(e) needs a name or wildcard.",
                            "Use a separate match for a specific error case.",
                        );
                    }
                    return index
                        .map(|i| Pattern::Case(format!("union:{i}"), vec![Pattern::Any]))
                        .unwrap_or(Pattern::Any);
                }
                if let Type::Union(parts) = &ty {
                    for (index, part) in parts.iter().enumerate() {
                        if self.constructors(part).is_some_and(|constructors| {
                            constructors
                                .iter()
                                .any(|(case, _)| self.pattern_case_matches(&name, case))
                        }) {
                            return Pattern::Case(
                                format!("union:{index}"),
                                vec![self.pattern(node, part, mutable, generalize)],
                            );
                        }
                    }
                }
                if let Some(constructors) = self.constructors(&ty)
                    && let Some((key, fields)) = constructors
                        .into_iter()
                        .find(|(key, _)| self.pattern_case_matches(&name, key))
                {
                    // A length-indexed case refines the matched length in this arm.
                    let fields = self.indexed_case(&ty, &key).unwrap_or(fields);
                    let patterns =
                        self.case_patterns(node, &name, &key, &fields, mutable, generalize);
                    return Pattern::Case(key, patterns);
                }
                self.error(
                    node,
                    "E_PATTERN_CASE",
                    format!("Case `{name}` does not belong to {ty}, or its index is impossible."),
                    "Use a case of the matched type; an open Error root also needs a fallback.",
                );
                Pattern::Any
            }
            _ => {
                self.error(
                    node,
                    "E_UNSUPPORTED_CHECK",
                    "This pattern is not checked yet.",
                    "Use a specified pattern form.",
                );
                Pattern::Any
            }
        }
    }

    /// The payload names a case declares, when it declares any.
    fn case_field_names(&self, key: &str) -> Option<Vec<Option<String>>> {
        let (owner, case) = key.rsplit_once('.')?;
        let variant = self
            .environment
            .definitions
            .get(owner)?
            .variants
            .iter()
            .find(|variant| variant.name == case)?;
        variant
            .field_names
            .iter()
            .any(Option::is_some)
            .then(|| variant.field_names.clone())
    }

    /// Checks a case pattern's payloads and returns one pattern per field.
    /// Positional patterns fill the fields in order; named ones follow in any
    /// order, the same rule calls use.
    fn case_patterns(
        &mut self,
        node: &Node,
        case: &str,
        key: &str,
        fields: &[Type],
        mutable: bool,
        generalize: bool,
    ) -> Vec<Pattern> {
        let children = node.children.iter().skip(1).collect::<Vec<_>>();
        if !children.iter().any(|child| child.kind == K::NamedPattern) {
            if children.len() != fields.len() {
                self.error(
                    node,
                    "E_PATTERN_ARITY",
                    format!(
                        "Case `{case}` expects {} fields, found {}.",
                        fields.len(),
                        children.len()
                    ),
                    "Match each positional field once.",
                );
            }
            return children
                .iter()
                .zip(fields)
                .map(|(n, t)| self.pattern(n, t, mutable, generalize))
                .collect();
        }
        let Some(labels) = self.case_field_names(key) else {
            self.error(
                node,
                "E_PATTERN_CASE",
                format!("Case `{case}` declares positional payloads and takes no names."),
                "Write the payloads as positional patterns.",
            );
            return children
                .iter()
                .map(|child| child.children.last().unwrap_or(child))
                .zip(fields)
                .map(|(n, t)| self.pattern(n, t, mutable, generalize))
                .collect();
        };
        let mut slots: Vec<Option<&Node>> = vec![None; fields.len()];
        let mut next = 0;
        let mut extra = 0;
        for child in &children {
            if child.kind == K::NamedPattern {
                let label = child
                    .children
                    .first()
                    .map(|n| self.text(n).to_owned())
                    .unwrap_or_default();
                let value = child.children.last().unwrap_or(child);
                match labels
                    .iter()
                    .position(|name| name.as_deref() == Some(label.as_str()))
                {
                    Some(slot) => {
                        if slots[slot].replace(value).is_some() {
                            self.error(
                                child,
                                "E_PATTERN_CASE",
                                format!("Case `{case}` names payload `{label}` twice."),
                                "Match each payload exactly once.",
                            );
                        }
                    }
                    None => self.error(
                        child,
                        "E_PATTERN_CASE",
                        format!("Case `{case}` has no payload named `{label}`."),
                        "Use a payload name the case declares.",
                    ),
                }
            } else if next < fields.len() {
                slots[next] = Some(child);
                next += 1;
            } else {
                extra += 1;
            }
        }
        if extra > 0 {
            self.error(
                node,
                "E_PATTERN_ARITY",
                format!(
                    "Case `{case}` expects {} fields, found {}.",
                    fields.len(),
                    fields.len() + extra
                ),
                "Match each positional field once.",
            );
        }
        slots
            .iter()
            .enumerate()
            .map(|(slot, argument)| match argument {
                Some(argument) => self.pattern(argument, &fields[slot], mutable, generalize),
                None => {
                    let label = labels[slot].as_deref().unwrap_or("_");
                    self.error(
                        node,
                        "E_PATTERN_ARITY",
                        format!("Case `{case}` is missing payload `{label}`."),
                        "Match each payload exactly once.",
                    );
                    Pattern::Any
                }
            })
            .collect()
    }

    /// Every alternative of one arm binds the same names with the same types.
    fn check_pattern_alternatives(
        &mut self,
        first: &BTreeMap<String, Symbol>,
        other: &BTreeMap<String, Symbol>,
        node: &Node,
    ) {
        for (name, symbol) in first {
            let Some(other) = other.get(name) else {
                self.error(
                    node,
                    "E_PATTERN_ALT_BINDINGS",
                    format!("This alternative does not bind `{name}`, and the first one does."),
                    "Every alternative in an arm binds the same names, with the same types.",
                );
                continue;
            };
            let expected = self.inference.resolve(&symbol.ty);
            let found = self.inference.resolve(&other.ty);
            if expected != found {
                self.error(
                    node,
                    "E_PATTERN_ALT_BINDINGS",
                    format!("`{name}` is {expected} in one alternative and {found} in another."),
                    "Every alternative in an arm binds the same names, with the same types.",
                );
            }
        }
        for name in other.keys().filter(|name| !first.contains_key(*name)) {
            self.error(
                node,
                "E_PATTERN_ALT_BINDINGS",
                format!("Only one alternative binds `{name}`."),
                "Every alternative in an arm binds the same names, with the same types.",
            );
        }
    }

    /// The fields of a length-indexed case, instantiated in the checker's own
    /// inference so the arm keeps what the case says about the matched length.
    fn indexed_case(&mut self, ty: &Type, key: &str) -> Option<Vec<Type>> {
        let ty = self.inference.resolve(ty);
        let Type::Indexed(inner, scrutinee) = &ty else {
            return None;
        };
        let Type::Named(name, args) = inner.as_ref() else {
            return None;
        };
        let definition = self.environment.definitions.get(name)?;
        let variant = definition
            .variants
            .iter()
            .find(|v| format!("{name}.{}", v.name) == key)?;
        let substitutions = definition
            .parameters
            .iter()
            .cloned()
            .zip(args.iter().cloned())
            .collect();
        let combined = Type::Tuple(
            variant
                .fields
                .iter()
                .cloned()
                .chain(std::iter::once(variant.result.clone()))
                .collect(),
        )
        .replace_parameters(&substitutions);
        let Type::Tuple(mut specialized) = self.inference.instantiate(&combined) else {
            return None;
        };
        let Some(Type::Indexed(case_inner, case_index)) = specialized.pop() else {
            return None;
        };
        self.inference.unify(inner, &case_inner);
        self.inference.refine_index(scrutinee, &case_index);
        Some(
            specialized
                .iter()
                .map(|field| self.inference.resolve(field))
                .collect(),
        )
    }

    fn pattern_case_matches(&self, name: &str, key: &str) -> bool {
        let name = crate::lexer::path_name(name);
        if let Some((owner, case)) = name.rsplit_once('.') {
            let Some(owner) = self.environment.names.get(owner).map(String::as_str) else {
                return false;
            };
            if owner == "Option" {
                return case == key;
            }
            format!("{owner}.{case}") == key
        } else {
            key.rsplit('.').next() == Some(name.as_str())
        }
    }

    fn constructors(&self, ty: &Type) -> Option<Vec<(String, Vec<Type>)>> {
        let ty = self.inference.resolve(ty);
        match &ty {
            Type::Tuple(parts) => Some(vec![("tuple".into(), parts.clone())]),
            Type::Union(parts) => Some(
                parts
                    .iter()
                    .enumerate()
                    .map(|(i, ty)| (format!("union:{i}"), vec![ty.clone()]))
                    .collect(),
            ),
            Type::Named(name, _) if name == "Bool" => {
                Some(vec![("true".into(), vec![]), ("false".into(), vec![])])
            }
            Type::Named(name, args) if name == "Option" && args.len() == 1 => Some(vec![
                ("Some".into(), vec![args[0].clone()]),
                ("None".into(), vec![]),
            ]),
            Type::Named(name, args) if name == "List" && args.len() == 1 => Some(vec![
                ("list:nil".into(), vec![]),
                ("list:cons".into(), vec![args[0].clone(), ty.clone()]),
            ]),
            Type::Indexed(inner, _) => {
                let Type::Named(name, args) = inner.as_ref() else {
                    return None;
                };
                let definition = self.environment.definitions.get(name)?;
                let substitutions = definition
                    .parameters
                    .iter()
                    .cloned()
                    .zip(args.iter().cloned())
                    .collect();
                Some(
                    definition
                        .variants
                        .iter()
                        .filter_map(|variant| {
                            let mut inference = self.inference.clone();
                            let combined = Type::Tuple(
                                variant
                                    .fields
                                    .iter()
                                    .cloned()
                                    .chain(std::iter::once(variant.result.clone()))
                                    .collect(),
                            );
                            let Type::Tuple(mut specialized) =
                                inference.instantiate(&combined.replace_parameters(&substitutions))
                            else {
                                unreachable!()
                            };
                            let result = specialized.pop().unwrap();
                            if let (Type::Indexed(_, a), Type::Indexed(_, b)) = (&ty, &result)
                                && inference.impossible_lengths(a, b)
                            {
                                return None;
                            }
                            inference.unify(&ty, &result);
                            Some((
                                format!("{name}.{}", variant.name),
                                specialized
                                    .iter()
                                    .map(|field| inference.resolve(field))
                                    .collect(),
                            ))
                        })
                        .collect(),
                )
            }
            Type::Named(name, _) if name == "Error" => {
                let mut cases = vec![("error:other".into(), vec![])];
                for (id, definition) in &self.environment.definitions {
                    if definition.kind == K::Enum
                        && definition.parent.as_ref() == Some(&Type::named("Error"))
                    {
                        cases.extend(definition.variants.iter().map(|variant| {
                            (format!("{id}.{}", variant.name), variant.fields.clone())
                        }));
                    }
                }
                Some(cases)
            }
            Type::Named(name, args) => {
                let definition = self.environment.definitions.get(name)?;
                if definition.kind != K::Enum {
                    return None;
                }
                let substitutions = definition
                    .parameters
                    .iter()
                    .cloned()
                    .zip(args.iter().cloned())
                    .collect();
                Some(
                    definition
                        .variants
                        .iter()
                        .map(|variant| {
                            (
                                format!("{name}.{}", variant.name),
                                variant
                                    .fields
                                    .iter()
                                    .map(|t| t.replace_parameters(&substitutions))
                                    .collect(),
                            )
                        })
                        .collect(),
                )
            }
            _ => None,
        }
    }

    fn match_expression(&mut self, node: &Node, expected: Option<&Type>) -> Type {
        let value = self.infer(&node.children[0], None);
        let mut rows = vec![];
        let mut result = Type::Never;
        let before_initialization = self.initializing_fields.clone();
        let mut completed_initialization: Option<BTreeSet<String>> = None;
        for arm in node
            .children
            .iter()
            .skip(1)
            .filter(|n| n.kind == K::MatchArm)
        {
            self.initializing_fields = before_initialization.clone();
            self.scopes.push(BTreeMap::new());
            // A case can fix the matched length, as Nil fixes n at 0; that holds
            // only inside its arm.
            let equations = self.inference.equations();
            let pattern_node = &arm.children[0];
            let patterns = if pattern_node.kind == K::PatternAlt {
                let mut patterns = vec![];
                let mut first_scope = None;
                for alternative in &pattern_node.children {
                    let alternative_equations = self.inference.equations();
                    self.scopes.push(BTreeMap::new());
                    patterns.push(self.pattern(alternative, &value, false, false));
                    let scope = self.scopes.pop().unwrap();
                    self.inference.restore_equations(alternative_equations);
                    match &first_scope {
                        Some(first) => self.check_pattern_alternatives(first, &scope, alternative),
                        None => first_scope = Some(scope),
                    }
                }
                // The guard and the body read the first alternative's bindings; every
                // alternative binds the same names with the same types.
                self.scopes
                    .last_mut()
                    .unwrap()
                    .extend(first_scope.unwrap_or_default());
                patterns
            } else {
                vec![self.pattern(pattern_node, &value, false, false)]
            };
            if let Some(guard) = arm.child(K::Guard) {
                let ty = self.infer(&guard.children[0], Some(&Type::named("Bool")));
                self.require(guard, &Type::named("Bool"), &ty);
            } else {
                rows.extend(patterns.into_iter().map(|pattern| vec![pattern]));
            }
            let expression = arm.children.last().unwrap();
            let found = self.infer(expression, expected);
            if found != Type::Never {
                completed_initialization = match completed_initialization {
                    Some(previous) => {
                        intersect_initialization(Some(previous), self.initializing_fields.clone())
                    }
                    None => self.initializing_fields.clone(),
                };
            }
            if let Some(expected) = expected {
                self.require(expression, expected, &found);
                result = expected.clone();
            } else if result == Type::Never {
                result = found;
            } else if !self.inference.unify(&result, &found) {
                self.mismatch(expression, &result, &found);
            }
            self.scopes.pop();
            self.inference.restore_equations(equations);
        }
        self.initializing_fields = if before_initialization.is_some() {
            completed_initialization.or(before_initialization)
        } else {
            None
        };
        match self.useful(
            &rows,
            &[Pattern::Any],
            std::slice::from_ref(&value),
            &mut 10_000,
            0,
        ) {
            Ok(true) => {
                let missing = self.missing_top_level_cases(&rows, &value);
                let fixes = crate::actions::match_fixes(self.source, node, &value, &missing);
                self.error_with_fixes(
                    node,
                    "E_MATCH_EXHAUSTIVE",
                    "This match leaves at least one case uncovered.",
                    "Add the missing cases or an unguarded _ arm. Guards do not cover a case by themselves.",
                    fixes,
                );
            }
            Err(()) => self.error(
                node,
                "E_MATCH_COMPLEX",
                "The match-coverage check exceeded its work limit.",
                "Simplify the nested patterns or add an unguarded _ arm.",
            ),
            Ok(false) => {}
        }
        erase_callable_identities(&result)
    }

    /// The cases a match does not fully cover. A case that is only partly
    /// covered still counts: a bare skeleton arm placed after the existing arms
    /// catches exactly the remaining values.
    fn missing_top_level_cases(
        &self,
        rows: &[Vec<Pattern>],
        ty: &Type,
    ) -> Vec<(String, Vec<Type>)> {
        let Some(constructors) = self.constructors(ty) else {
            return vec![];
        };
        let mut missing = vec![];
        for (case, fields) in constructors {
            if case == "error:other" {
                continue;
            }
            let query = vec![Pattern::Case(
                case.clone(),
                std::iter::repeat_n(Pattern::Any, fields.len()).collect(),
            )];
            let mut budget = 10_000;
            if matches!(
                self.useful(rows, &query, std::slice::from_ref(ty), &mut budget, 0),
                Ok(true)
            ) {
                missing.push((case, fields));
            }
        }
        missing
    }

    fn useful(
        &self,
        rows: &[Vec<Pattern>],
        query: &[Pattern],
        types: &[Type],
        budget: &mut usize,
        depth: usize,
    ) -> Result<bool, ()> {
        if *budget == 0 || depth >= 96 {
            return Err(());
        }
        *budget -= 1;
        if query.is_empty() {
            return Ok(rows.is_empty());
        }
        if rows
            .iter()
            .any(|row| row.iter().all(|pattern| matches!(pattern, Pattern::Any)))
        {
            return Ok(false);
        }
        match &query[0] {
            Pattern::Case(key, fields) => {
                let constructors = self.constructors(&types[0]).unwrap_or_default();
                let arguments = constructors
                    .iter()
                    .find(|(name, _)| name == key)
                    .map(|(_, fields)| fields.clone())
                    .unwrap_or_default();
                let specialized = specialize(rows, key, fields.len());
                let query = fields
                    .iter()
                    .cloned()
                    .chain(query[1..].iter().cloned())
                    .collect::<Vec<_>>();
                let types = arguments
                    .into_iter()
                    .chain(types[1..].iter().cloned())
                    .collect::<Vec<_>>();
                self.useful(&specialized, &query, &types, budget, depth + 1)
            }
            Pattern::Any => {
                if let Some(constructors) = self.constructors(&types[0]) {
                    for (key, arguments) in constructors {
                        let specialized = specialize(rows, &key, arguments.len());
                        let query = std::iter::repeat_n(Pattern::Any, arguments.len())
                            .chain(query[1..].iter().cloned())
                            .collect::<Vec<_>>();
                        let types = arguments
                            .into_iter()
                            .chain(types[1..].iter().cloned())
                            .collect::<Vec<_>>();
                        if self.useful(&specialized, &query, &types, budget, depth + 1)? {
                            return Ok(true);
                        }
                    }
                    Ok(false)
                } else {
                    let defaults = rows
                        .iter()
                        .filter(|row| matches!(row.first(), Some(Pattern::Any)))
                        .map(|row| row[1..].to_vec())
                        .collect::<Vec<_>>();
                    self.useful(&defaults, &query[1..], &types[1..], budget, depth + 1)
                }
            }
        }
    }
}

fn specialize(rows: &[Vec<Pattern>], key: &str, arity: usize) -> Vec<Vec<Pattern>> {
    rows.iter()
        .filter_map(|row| match row.first()? {
            Pattern::Any => Some(
                std::iter::repeat_n(Pattern::Any, arity)
                    .chain(row[1..].iter().cloned())
                    .collect(),
            ),
            Pattern::Case(name, fields) if name == key => Some(
                fields
                    .iter()
                    .cloned()
                    .chain(row[1..].iter().cloned())
                    .collect(),
            ),
            _ => None,
        })
        .collect()
}
fn default_exits(node: &Node) -> bool {
    match node.kind {
        K::Lambda | K::Function => false,
        K::Return | K::Propagate => true,
        _ => node.children.iter().any(default_exits),
    }
}

fn impossible_indices(left: &Type, right: &Type) -> bool {
    match (left, right) {
        (Type::Indexed(_, a), Type::Indexed(_, b)) => crate::types::impossible_lengths(a, b),
        _ => false,
    }
}

fn is_type(kind: K) -> bool {
    matches!(
        kind,
        K::TypeName
            | K::TypeApplication
            | K::IndexedType
            | K::TupleType
                | K::ShapeType
            | K::FunctionType
            | K::UnionType
            | K::BorrowType
            | K::MutableType
            | K::DynType
            // A proof parameter's claim stands where its type would (Q115).
            | K::Proposition
    )
}
fn is_declaration(kind: K) -> bool {
    matches!(
        kind,
        K::Import
            | K::Function
            | K::Opaque
            | K::Class
            | K::Trait
            | K::Enum
            | K::Alias
            | K::OperatorDeclaration
            | K::Const
            | K::Theorem
            | K::Axiom
            | K::Example
    )
}
fn binding_name(node: &Node, source: &SourceFile) -> String {
    node.children
        .iter()
        .find(|n| matches!(n.kind, K::Name | K::PatternName | K::PatternConstructor))
        .and_then(|n| {
            n.descendants()
                .find(|n| n.kind == K::Name && n.children.is_empty())
        })
        .map(|n| source.slice(n.span).to_owned())
        .unwrap_or_default()
}
/// Arithmetic operators on user types desugar to these traits: the left
/// operand's trait and method, then the right-side form used when the left
/// operand is a built-in value (Q87, following Multipliable).
pub(crate) const OPERATOR_TRAITS: &[(&str, &str, &str, &str, &str)] = &[
    (
        "*",
        "Multipliable",
        "multiply",
        "RightMultipliable",
        "rightMultiply",
    ),
    ("+", "Addable", "add", "RightAddable", "rightAdd"),
    (
        "-",
        "Subtractable",
        "subtract",
        "RightSubtractable",
        "rightSubtract",
    ),
    ("/", "Divisible", "divide", "RightDivisible", "rightDivide"),
    ("**", "Powerable", "power", "RightPowerable", "rightPower"),
];

/// The unary operator trait and its method.
pub(crate) fn unary_trait(name: &str) -> Option<&'static str> {
    (name == "Negatable").then_some("negate")
}

/// Whether `name` is one of the arithmetic operator traits.
pub(crate) fn operator_trait(name: &str) -> bool {
    OPERATOR_TRAITS
        .iter()
        .any(|(_, left, _, right, _)| name == *left || name == *right)
}

/// The method an arithmetic operator trait requires.
pub(crate) fn operator_method(name: &str) -> Option<&'static str> {
    OPERATOR_TRAITS
        .iter()
        .find_map(|(_, left, left_method, right, right_method)| {
            if name == *left {
                Some(*left_method)
            } else if name == *right {
                Some(*right_method)
            } else {
                None
            }
        })
}

fn builtin_power_output(base: &Type, exponent: &Type) -> Option<Type> {
    if base == &Type::named("Int") && exponent == &Type::named("Int") {
        Some(Type::named("Int"))
    } else if base == &Type::named("double")
        && (exponent == &Type::named("Int") || exponent == &Type::named("double"))
    {
        Some(Type::named("double"))
    } else {
        None
    }
}

/// The operator trait a method name belongs to.
fn method_operator_trait(method: &str) -> Option<&'static str> {
    if method == "negate" {
        return Some("Negatable");
    }
    OPERATOR_TRAITS
        .iter()
        .find_map(|(_, left, left_method, right, right_method)| {
            if method == *left_method {
                Some(*left)
            } else if method == *right_method {
                Some(*right)
            } else {
                None
            }
        })
}

pub(crate) fn builtin_arity(name: &str) -> Option<usize> {
    if operator_trait(name) {
        return Some(1);
    }
    if unary_trait(name).is_some() {
        return Some(0);
    }
    match name {
        "Map" | "MutMap" | "Tensor" => Some(2),
        "DynTensor" => Some(1),
        "List" | "Array" | "Set" | "MutSet" | "SortedSet" | "Option" | "Box" | "Mutex"
        | "Atomic" | "Channel" | "CPointer" | "CConstPointer" | "Arena" | "ArenaRef" => Some(1),
        "Int" | "double" | "Bool" | "String" | "void" | "Proof" | "BigInt" | "Object" | "Error"
        | "Text" | "Equals" | "Comparable" | "Sortable" | "Iterator" | "Iterable" | "Reader"
        | "Writer" => Some(0),
        name if crate::interop::C_TYPES.contains(&name) => Some(0),
        "PythonValue" | "CVoid" => Some(0),
        _ => None,
    }
}

include!("checker_variable.rs");
include!("checker_tensor.rs");

include!("checker_conversions.rs");

include!("checker_numerical.rs");

/// Whether an integer literal fits a C boundary integer type.
fn c_literal_fits(name: &str, text: &str) -> bool {
    let digits: String = text.chars().filter(|c| *c != '_').collect();
    let value = if let Some(hex) = digits
        .strip_prefix("0x")
        .or_else(|| digits.strip_prefix("0X"))
    {
        i128::from_str_radix(hex, 16).ok()
    } else {
        digits.parse::<i128>().ok()
    };
    let Some(value) = value else { return false };
    let (low, high): (i128, i128) = match name {
        "CInt8" => (i8::MIN.into(), i8::MAX.into()),
        "CInt16" => (i16::MIN.into(), i16::MAX.into()),
        "CInt32" => (i32::MIN.into(), i32::MAX.into()),
        "CInt64" => (i64::MIN.into(), i64::MAX.into()),
        "CUInt8" => (0, u8::MAX.into()),
        "CUInt16" => (0, u16::MAX.into()),
        "CUInt32" => (0, u32::MAX.into()),
        "CUInt64" => (0, u64::MAX.into()),
        _ => return false,
    };
    (low..=high).contains(&value)
}
