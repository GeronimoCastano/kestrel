//! Typed native core. Every operation reads already evaluated values.
use crate::{
    checker::{Checked, Visibility},
    diagnostic::Diagnostic,
    modules::ProjectCheck,
    resolution::{Argument, Callable, DeclarationId, DeclarationKind, Reference, ResolvedCall},
    source::Span,
    syntax::{Node, SyntaxKind as K},
    types::{FunctionType, Passing, Type},
};
use std::collections::{BTreeMap, BTreeSet};
#[path = "core_views.rs"]
mod views;

fn layout_key(ty: &Type) -> String {
    let key = ty.without_parameter_names().to_string();
    if !ty.mentions("Arena") && !ty.mentions("ArenaRef") {
        return key;
    }
    let mut permissions = vec![];
    ty.map(&mut |part| {
        if let Type::Function(function) = part {
            permissions.push(function.receiver_mutates);
        }
        None
    });
    format!("{key} captures={permissions:?}")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Ty {
    Int,
    /// The standard Nat (Q71): an unsigned 64-bit number. `Zero` is 0 and
    /// `Succ(n)` adds one, stopping on overflow.
    Nat,
    Double,
    Bool,
    String,
    Unit,
    Record(usize),
    Ref(usize),
    Boxed(usize),
    Error,
    Tasks,
    /// A borrowed trait object: a data pointer and the number of its implementation.
    Dyn(usize),
    /// An owned trait object, `Box[dyn Trait]`, with the same two words.
    DynBox(usize),
    /// A std.c boundary number, by its index in `interop::C_TYPES`.
    C(usize),
    /// An opaque foreign address; constness survives its native representation.
    Pointer(bool),
    /// An arena identity and slot; the layout records its phantom item type.
    ArenaRef(usize),
    /// One owned reference to a Python object.
    Python,
    /// A counted owner retained by an erased iteration step or a List entry.
    Owner,
    /// A KesNet differentiable tensor: a counted handle to a node that holds
    /// data, a gradient and (for non-leaves) the operation that produced it.
    Variable,
}
impl Ty {
    pub fn owns(self) -> bool {
        matches!(
            self,
            Self::String
                | Self::Record(_)
                | Self::Boxed(_)
                | Self::Error
                | Self::Tasks
                | Self::DynBox(_)
                | Self::Python
                | Self::Owner
                | Self::Variable
        )
    }
}
#[derive(Clone, Debug)]
pub struct Field {
    pub name: String,
    pub ty: Ty,
}
#[derive(Clone, Debug)]
pub struct Layout {
    pub is_error: bool,
    pub source: Type,
    pub fields: Vec<Field>,
    pub kind: LayoutKind,
}
#[derive(Clone, Debug)]
pub enum LayoutKind {
    BigInt,
    Tensor,
    ArenaRef,
    Class,
    Tuple,
    Enum(Vec<Case>),
    /// An Option of a borrowed item, with an optional hidden retained owner.
    Step(Vec<Case>),
    Union(Vec<Type>, Vec<Case>),
    /// A superclass borrow with a known finite set of concrete subclasses.
    ClassView(Vec<Case>),
    Resource(Resource),
    List(Ty),
    /// A mutable Array: a length and a heap block of elements that copy (Q53).
    Array(Ty),
    Map(MapLayout),
    /// The methods of a trait used through `dyn`, and every type converted to it.
    Trait(TraitLayout),
    /// The heap value inside `Box[T]` when T is not a class or enum.
    Cell,
    /// A function value: a shared, counted environment and a code pointer.
    Closure {
        parameters: Vec<(Ty, Passing)>,
        result: Ty,
    },
    /// The values a closure captured, after a counted header. `true` marks a pointer
    /// to a captured variable that the closure assigns.
    Env(Vec<bool>),
}
/// One Python call or attribute read; the generated helper converts at the boundary.
#[derive(Clone, Debug)]
pub struct PythonSite {
    /// Index into `Program::python_modules`, or None when the receiver is a PythonValue.
    pub module: Option<usize>,
    pub attribute: Option<String>,
    pub call: bool,
    /// Argument layouts in source order; a name marks a keyword argument.
    pub arguments: Vec<(Ty, Option<String>)>,
    /// The `T or Error` union.
    pub result: Ty,
}
#[derive(Clone, Debug)]
pub struct ClosureCode {
    pub layout: usize,
    pub function: usize,
    pub env: Option<usize>,
}
#[derive(Clone, Debug)]
pub struct TraitLayout {
    pub sealed: bool,
    pub methods: Vec<DynMethod>,
    pub impls: Vec<DynImpl>,
}
#[derive(Clone, Debug)]
pub struct DynMethod {
    pub name: String,
    pub parameters: Vec<(Ty, Passing)>,
    pub result: Ty,
}
#[derive(Clone, Debug)]
pub struct DynImpl {
    pub concrete: usize,
    pub methods: Vec<usize>,
}
#[derive(Clone, Copy, Debug)]
pub struct MapLayout {
    pub key: Ty,
    pub value: Ty,
    pub item: usize,
    pub equality: Option<usize>,
    /// A generated hash helper for tuple, enum and record keys (REVIEW 1.3).
    pub hash: Option<usize>,
    pub mutable: bool,
    pub style: MapStyle,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapStyle {
    Map,
    Set,
    SortedSet,
}
#[derive(Clone, Copy, Debug)]
pub enum Resource {
    Arena(Ty),
    Reader,
    Writer,
    Mutex(Ty),
    Guard(Ty),
    Channel(Ty),
    Atomic(Ty),
}
#[derive(Clone, Debug)]
pub enum Cleanup {
    Drop(Slot),
    File(Atom),
    Join(Slot, bool),
}
impl LayoutKind {
    pub fn cases(&self) -> Option<&[Case]> {
        match self {
            Self::Enum(cases)
            | Self::Step(cases)
            | Self::Union(_, cases)
            | Self::ClassView(cases) => Some(cases),
            _ => None,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Case {
    pub name: String,
    pub fields: Vec<Ty>,
    /// The payload name each field declares, in the same order as `fields`;
    /// empty when the case stays positional.
    pub names: Vec<Option<String>>,
}
#[derive(Clone, Debug)]
pub enum Test {
    Yes,
    Equal(Ty, Atom, Atom),
    /// `a < b` on Int or Nat, used by a range pattern.
    Less(Ty, Atom, Atom),
    BigIntLiteral(Atom, String),
    ListLength(Atom, usize, bool),
    And(Box<Test>, Box<Test>),
    Or(Box<Test>, Box<Test>),
    Not(Box<Test>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Slot {
    pub id: usize,
    pub global: bool,
}

#[derive(Clone, Debug)]
pub enum Atom {
    Slot(Slot),
    Int(i64),
    Nat(u64),
    /// The Nat one below a nonzero Nat: the payload of `Succ(k)`.
    NatPred(Box<Atom>),
    Double(f64),
    Bool(bool),
    Text(String),
    Unit,
    Field(Box<Atom>, usize, Ty),
    Address(Box<Atom>, usize),
    Payload(Box<Atom>, usize, usize, Ty),
    Tag(Box<Atom>),
    ErrorType(Box<Atom>),
    ErrorPayload(Box<Atom>, usize),
    ResourceValue(Box<Atom>, Ty),
    ListItem(Box<Atom>, Box<Atom>, Ty),
    /// An Array slot whose index was already checked.
    ArrayItem(Box<Atom>, Box<Atom>, Ty),
    ArrayLength(Box<Atom>),
    CollectionLength(Box<Atom>),
    /// A list's length less a constant, the index of an item counted from the end.
    LengthLess(Box<Atom>, usize),
    MapCapacity(Box<Atom>),
    MapOccupied(Box<Atom>, Box<Atom>),
    MapItem(Box<Atom>, Box<Atom>, usize),
    SetItem(Box<Atom>, Box<Atom>, Ty, bool),
    StepOwner(Box<Atom>),
    /// Read a value, or its address, from counted owner storage.
    OwnerValue(Box<Atom>, Ty, Ty),
    /// A captured value in a closure environment; `true` reads through a pointer.
    EnvField(Box<Atom>, usize, Ty, bool),
}

#[derive(Clone, Debug)]
pub struct Location {
    pub span: Span,
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub expression: String,
}

/// An environment layout and each captured value: its atom, layout, and whether the
/// closure keeps a pointer to the variable instead of a copy.
pub type Captured = (usize, Vec<(Atom, Ty, bool)>);

#[derive(Clone, Debug)]
pub enum Rhs {
    Copy(Atom),
    /// Lazily initialized static final value.
    StaticRead(usize),
    /// Store a new value in a synchronized static var.
    StaticWrite(usize, Atom),
    Take(Atom),
    Unary(String, Atom),
    Binary(String, Ty, Atom, Atom),
    Call(usize, Vec<(Atom, Passing)>),
    Builtin(String, Ty, Vec<Atom>),
    Text(Ty, Atom, Option<u32>),
    Join(Vec<Atom>),
    Aggregate(usize, Option<usize>, Vec<(Atom, Ty)>),
    Test(Test),
    ErrorBox(usize, Atom, bool),
    FileClose(Atom),
    JoinTasks(Slot),
    ListNew(usize, Vec<(Atom, Ty)>),
    /// The items of a list after the first `skip` and before the last `keep`.
    ListSlice(usize, Atom, usize, usize),
    /// A new list with the items of the first list and then the second.
    ListConcat(usize, Atom, Atom),
    ListAppend(usize, Atom, Atom, Ty),
    ListAppendFrom(usize, Atom, Atom, Atom),
    ListAppendSet(usize, usize, Atom, Atom, Atom),
    ListAppendOwner(usize, Atom, Atom, Ty, Atom),
    OwnerNew(Atom, Ty),
    StepSome(usize, Atom, Ty, Atom),
    MapNew(usize, Vec<(Atom, Ty)>),
    MapPut(usize, Atom, Atom, Ty, Atom, Ty),
    MapGet(usize, Atom, Atom, bool),
    /// Trait, implementation, and a pointer to the concrete value.
    MakeDyn(usize, usize, Atom),
    /// Trait, implementation, and an owned record (false) or Box pointer (true) to move.
    DynBoxNew(usize, usize, Atom, bool),
    /// Trait, method, receiver and arguments.
    DynCall(usize, usize, Atom, Vec<Atom>),
    /// A Box layout and the owned value moved into it.
    BoxNew(usize, Atom, Ty),
    /// Move a boxed record out of its storage and release only the empty box.
    BoxUnwrap(usize, Atom),
    /// A closure layout, its code, and the environment layout with captured values.
    ClosureNew(usize, usize, Option<Captured>),
    /// A closure layout, the closure, and its arguments.
    ClosureCall(usize, Atom, Vec<Atom>),
    /// A Python site, its receiver (when not a module) and its arguments.
    Python(usize, Vec<Atom>),
    /// Appends a String to a String variable, in place when it holds the only reference.
    StringAppend(Slot, Atom),
    /// Appends a scalar's text directly, without a temporary String.
    StringAppendText(Slot, Ty, Atom),
}

#[derive(Clone, Debug)]
pub enum Statement {
    Set(Slot, Rhs, Location),
    Store(Atom, Ty, Rhs, Location),
    Evaluate(Rhs, Location),
    Print(Ty, Atom),
    If(Atom, Vec<Statement>, Vec<Statement>),
    Match(usize, Vec<MatchArm>),
    Loop {
        id: usize,
        condition: Vec<Statement>,
        value: Atom,
        body: Vec<Statement>,
        step: Vec<Statement>,
    },
    Break(usize, Vec<Cleanup>),
    Continue(usize, Vec<Cleanup>),
    Return(Atom, Vec<Cleanup>),
    TailCall(usize, Vec<(Atom, Passing)>),
    Drop(Vec<Slot>),
    Spawn {
        id: usize,
        scope: Slot,
        call: Rhs,
        result: Ty,
        location: Location,
    },
}
#[derive(Clone, Debug)]
pub struct MatchArm {
    pub test: Vec<Statement>,
    pub condition: Atom,
    pub bindings: Vec<Statement>,
    pub guard: Option<(Vec<Statement>, Atom)>,
    pub body: Vec<Statement>,
    pub cleanup: Vec<Statement>,
}

#[derive(Clone, Debug)]
pub struct Local {
    pub scope: usize,
    pub ty: Ty,
    pub parameter: bool,
    pub owned: bool,
}
#[derive(Clone, Debug)]
pub struct Function {
    pub name: String,
    pub parameters: Vec<(Slot, Ty, Passing)>,
    pub result: Ty,
    pub locals: Vec<Local>,
    pub body: Vec<Statement>,
}
/// A lazy static final value: its native type and the function computing it.
#[derive(Clone, Debug)]
pub struct StaticField {
    /// Stable identity, `class path.field`, shared across session libraries.
    pub key: String,
    pub ty: Ty,
    pub function: usize,
    pub mutable: bool,
}

/// The Python-style String methods that the native backend implements (Q136).
const STRING_METHODS: &[&str] = &[
    "isalpha",
    "isdecimal",
    "isdigit",
    "isnumeric",
    "isalnum",
    "isspace",
    "isupper",
    "islower",
    "istitle",
    "isascii",
    "isprintable",
    "isidentifier",
    "capitalize",
    "title",
    "swapcase",
    "casefold",
    "lower",
    "upper",
    "zfill",
    "removeprefix",
    "removesuffix",
    "find",
    "rfind",
    "count",
    "index",
    "rindex",
    "replace",
    "partition",
    "rpartition",
    "splitlines",
    "center",
    "ljust",
    "rjust",
    "strip",
    "lstrip",
    "rstrip",
    "expandtabs",
    "split",
    "substring",
    "slice",
    "reversed",
    "ord",
    "byteLength",
    "byteAt",
    "byteSlice",
    "lines",
    "words",
    "padStart",
    "padEnd",
];

#[derive(Clone, Debug)]
pub struct Program {
    pub copyable_layouts: BTreeSet<usize>,
    /// The immutable std.process.args List, initialized before the script runs.
    pub process_args: Option<Slot>,
    /// Headers named by `import c`, and the folders searched for them.
    pub c_headers: Vec<String>,
    pub c_include_dirs: Vec<std::path::PathBuf>,
    /// C files beside project headers, and system libraries named by `--link` (Q51).
    pub c_sources: Vec<std::path::PathBuf>,
    pub c_libraries: Vec<String>,
    /// C functions that take text; each gets a wrapper copying its Strings with a final NUL.
    pub c_text_calls: BTreeMap<String, (Vec<Ty>, Ty)>,
    pub python_modules: Vec<String>,
    pub python_sites: Vec<PythonSite>,
    pub closures: Vec<ClosureCode>,
    pub layouts: Vec<Layout>,
    pub functions: Vec<Function>,
    pub statics: Vec<StaticField>,
    pub globals: Vec<Ty>,
    /// Tail-call cycles with different signatures or aggregate arguments. Each member
    /// also gets a twin taking one shared argument block outside the replaced frames.
    pub tail_groups: Vec<Vec<usize>>,
    /// The compare helper function of each record element layout that is sorted (Q123).
    pub sort_comparators: BTreeMap<usize, usize>,
    /// A test program runs the test whose index is its first argument (Stage 6).
    pub tests: bool,
}
#[derive(Clone, Debug)]
pub struct LocatedDiagnostic {
    pub module: usize,
    pub diagnostic: Diagnostic,
}

type Result<T> = std::result::Result<T, LocatedDiagnostic>;

#[derive(Clone, Debug)]
struct Value {
    keepalive: BTreeSet<Slot>,
    native: Ty,
    atom: Atom,
    ty: Type,
    temporary: bool,
    callable: Option<Box<FunctionValue>>,
}
#[derive(Clone, Debug)]
enum FunctionValue {
    Sequence {
        source: Box<Value>,
        steps: Vec<SequenceStep>,
    },
    Range {
        start: Atom,
        stop: Atom,
        step: Atom,
    },
    Named,
    /// A call with some arguments left open. A bound method value such as `c.increment`
    /// is a partial with every argument open and the receiver kept.
    Partial {
        module: usize,
        call: Box<ResolvedCall>,
        captures: Vec<Option<Value>>,
        receiver: Option<Box<Value>>,
    },
    Lambda {
        module: usize,
        node: Node,
        captures: BTreeMap<String, Value>,
    },
}
#[derive(Clone, Debug)]
enum SequenceStep {
    Map(Value),
    Filter(Value),
}
#[derive(Clone, Debug)]
enum IterationOwner {
    Collection(usize, Atom, Atom),
    Step(Atom),
}
impl Value {
    fn unit() -> Self {
        Self {
            keepalive: BTreeSet::new(),
            native: Ty::Unit,
            atom: Atom::Unit,
            ty: Type::named("void"),
            temporary: false,
            callable: None,
        }
    }
}

#[derive(Clone)]
struct Definition {
    module: usize,
    node: Node,
    signature: FunctionType,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Key {
    checked_contract: bool,
    /// This body is reached from an invariant, so its own exits skip the check.
    skip_invariants: bool,
    declaration: DeclarationId,
    parameters: Vec<Ty>,
    result: Ty,
}
#[derive(Clone)]
struct Pending {
    checked_contract: bool,
    skip_invariants: bool,
    /// Enclosing locals a local function reads: name, native layout, source type and
    /// whether the function assigns it (then it is passed as a mutable borrow).
    captures: Vec<(String, Ty, Type, bool)>,
    native_parameters: Vec<Ty>,
    native_result: Ty,
    receiver: Option<(Type, Ty)>,
    index: usize,
    definition: Definition,
    signature: FunctionType,
    mono: Mono,
}

#[derive(Clone, Default)]
struct Mono {
    names: BTreeMap<String, Type>,
    variables: BTreeMap<usize, Type>,
}
impl Mono {
    fn apply(&self, ty: &Type) -> Type {
        ty.map(&mut |ty| match ty {
            Type::Parameter(name) | Type::Rigid(name) => self.names.get(name).cloned(),
            Type::Variable(id) => self.variables.get(id).cloned(),
            _ => None,
        })
    }
    fn bind(&mut self, schema: &Type, concrete: &Type) {
        match (schema, concrete) {
            (Type::Indexed(inner, _), concrete) => self.bind(
                inner,
                if let Type::Indexed(inner, _) = concrete {
                    inner
                } else {
                    concrete
                },
            ),
            (schema, Type::Indexed(inner, _)) => self.bind(schema, inner),

            (Type::Parameter(name) | Type::Rigid(name), concrete) => {
                self.names.insert(name.clone(), concrete.clone());
            }
            (Type::Variable(id), concrete) => {
                self.variables.insert(*id, concrete.clone());
            }
            (Type::Named(_, a), Type::Named(_, b)) | (Type::Tuple(a), Type::Tuple(b)) => {
                for (a, b) in a.iter().zip(b) {
                    self.bind(a, b);
                }
            }
            (Type::Function(a), Type::Function(b)) => {
                for (a, b) in a.parameters.iter().zip(&b.parameters) {
                    self.bind(&a.ty, &b.ty);
                }
                self.bind(&a.result, &b.result);
            }
            // A borrowed or stored view names the same type as its content, so bind through it.
            (Type::StoredView(schema) | Type::Borrow(schema), concrete) => {
                let concrete = match concrete {
                    Type::StoredView(inner) | Type::Borrow(inner) => inner.as_ref(),
                    other => other,
                };
                self.bind(schema, concrete);
            }
            (schema, Type::StoredView(inner) | Type::Borrow(inner)) => self.bind(schema, inner),
            _ => {}
        }
    }
}

/// A function declared inside another: its module, its declaration, and each enclosing
/// local it reads with whether it assigns that local.
type LocalFunction = (usize, Node, Vec<(String, bool)>);

struct Compiler<'a> {
    view_escapes: Vec<(usize, Span, Vec<usize>)>,
    session: bool,
    /// Notebook mode: only expressions from this offset display, as marked result blocks.
    notebook: bool,
    display_from: usize,
    process_args: Option<Slot>,
    c_text_calls: BTreeMap<String, (Vec<Ty>, Ty)>,
    python_modules: Vec<String>,
    python_sites: Vec<PythonSite>,
    closures: Vec<ClosureCode>,
    closure_codes: BTreeMap<String, usize>,
    layouts: Vec<Layout>,
    layout_ids: BTreeMap<String, usize>,
    /// The compare helper of each record element layout that is sorted (Q123).
    sort_comparators: BTreeMap<usize, usize>,
    /// Generated equality (0), text (1) and debug (2) helpers of enums and records
    /// that compare and print by their fields (REVIEW 2.2).
    structural: BTreeMap<(usize, u8), usize>,
    /// Functions declared inside another function, by declaration: the module, the
    /// declaration and the enclosing locals it reads, passed as hidden borrowed
    /// arguments after the declared ones (REVIEW 1.4).
    local_functions: BTreeMap<DeclarationId, LocalFunction>,
    building_layouts: BTreeSet<usize>,
    project: &'a ProjectCheck,
    nodes: Vec<BTreeMap<Span, &'a Node>>,
    types: Vec<BTreeMap<Span, Type>>,
    definitions: BTreeMap<DeclarationId, Definition>,
    instances: BTreeMap<Key, usize>,
    pending: Vec<Pending>,
    functions: Vec<Option<Function>>,
    statics: Vec<StaticField>,
    static_ids: BTreeMap<(String, String), usize>,
    static_visiting: BTreeSet<usize>,
    static_infos: BTreeMap<usize, crate::checker::StaticFieldInfo>,
    globals: Vec<Ty>,
    next_task: usize,
    global_slots: BTreeMap<DeclarationId, Slot>,
    global_bindings: BTreeMap<DeclarationId, Value>,
}

/// Saved native identities and live bindings between successful session entries.
#[derive(Clone, Default)]
pub struct SessionState {
    layouts: Vec<Layout>,
    layout_ids: BTreeMap<String, usize>,
    functions: Vec<Option<Function>>,
    statics: Vec<StaticField>,
    static_ids: BTreeMap<(String, String), usize>,
    globals: Vec<Ty>,
    global_slots: BTreeMap<DeclarationId, Slot>,
    global_bindings: BTreeMap<DeclarationId, Value>,
    process_args: Option<Slot>,
    c_text_calls: BTreeMap<String, (Vec<Ty>, Ty)>,
    python_modules: Vec<String>,
    python_sites: Vec<PythonSite>,
    closures: Vec<ClosureCode>,
    closure_codes: BTreeMap<String, usize>,
    instances: BTreeMap<Key, usize>,
    next_task: usize,
    names: BTreeMap<String, Value>,
    owned: Vec<Slot>,
}

impl SessionState {
    /// After an entry stopped part-way, keep what the worker has allocated (slots, layouts,
    /// functions) but show the names and owners of the earlier state.
    pub fn forget_names(&mut self, earlier: &SessionState) {
        self.names = earlier.names.clone();
        self.owned = earlier.owned.clone();
    }
}

pub fn lower(project: &ProjectCheck) -> std::result::Result<Program, Vec<LocatedDiagnostic>> {
    lower_inner(
        project,
        project.modules.len().saturating_sub(1),
        0,
        None,
        &[],
        (0, false),
    )
}

/// A program for one test file: its entry runs the `@test` function chosen by index,
/// so the runner can start each test in its own process (Q60).
pub fn lower_tests(
    project: &ProjectCheck,
    tests: &[Span],
) -> std::result::Result<Program, Vec<LocatedDiagnostic>> {
    let mut program = lower_inner(
        project,
        project.modules.len().saturating_sub(1),
        0,
        None,
        tests,
        (0, false),
    )?;
    program.tests = true;
    Ok(program)
}

/// Check source history, but lower only the new entry. The caller commits this state
/// only after native compilation succeeds.
pub fn lower_session(
    project: &ProjectCheck,
    start: usize,
    state: &mut SessionState,
) -> std::result::Result<Program, Vec<LocatedDiagnostic>> {
    lower_inner(project, 0, start, Some(state), &[], (start, false))
}

/// Like `lower_session` for a notebook cell: only an expression at or after `display_from`
/// displays, as a marked result block.
pub fn lower_notebook_cell(
    project: &ProjectCheck,
    start: usize,
    display_from: usize,
    state: &mut SessionState,
) -> std::result::Result<Program, Vec<LocatedDiagnostic>> {
    lower_inner(project, 0, start, Some(state), &[], (display_from, true))
}

fn lower_inner(
    project: &ProjectCheck,
    entry: usize,
    start: usize,
    mut session: Option<&mut SessionState>,
    tests: &[Span],
    (display_from, notebook): (usize, bool),
) -> std::result::Result<Program, Vec<LocatedDiagnostic>> {
    let definitions = project
        .modules
        .iter()
        .flat_map(|m| m.checked.environment.definitions.clone())
        .collect();
    let _copy_definitions = crate::types::copy_definitions(&definitions);
    let existing = project
        .modules
        .iter()
        .enumerate()
        .flat_map(|(module, checked)| {
            checked
                .checked
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.severity == crate::diagnostic::Severity::Error)
                .cloned()
                .map(move |diagnostic| LocatedDiagnostic { module, diagnostic })
        })
        .collect::<Vec<_>>();
    if !existing.is_empty() {
        return Err(existing);
    }
    let mut compiler = Compiler::new(project);
    compiler.session = session.is_some();
    compiler.notebook = notebook;
    compiler.display_from = display_from;
    if let Some(state) = session.as_ref() {
        compiler.layouts = state.layouts.clone();
        compiler.layout_ids = state.layout_ids.clone();
        if !state.functions.is_empty() {
            compiler.functions = state.functions.clone();
        }
        compiler.statics = state.statics.clone();
        compiler.static_ids = state.static_ids.clone();
        compiler.globals = state.globals.clone();
        compiler.global_slots = state.global_slots.clone();
        compiler.global_bindings = state.global_bindings.clone();
        compiler.process_args = state.process_args;
        compiler.c_text_calls = state.c_text_calls.clone();
        compiler.python_modules = state.python_modules.clone();
        compiler.python_sites = state.python_sites.clone();
        compiler.closures = state.closures.clone();
        compiler.closure_codes = state.closure_codes.clone();
        compiler.instances = state.instances.clone();
        compiler.next_task = state.next_task;
    }
    compiler
        .layout_type(&Type::named("std.core.RuntimeError"))
        .expect("standard runtime error layout");
    if project.modules.is_empty() {
        return Err(vec![]);
    }
    let root = &project.modules[entry].parsed.root;
    let mut builder = Builder::new(
        &mut compiler,
        entry,
        Mono::default(),
        "entry".into(),
        Ty::Unit,
        true,
    );
    if let Some(state) = session.as_ref() {
        builder.scopes[0].names = state.names.clone();
        builder.scopes[0].owned = state.owned.clone();
    }
    for node in &root.children {
        if node.span.start < start {
            continue;
        }
        if matches!(
            node.kind,
            K::Import
                | K::Function
                | K::Class
                | K::Trait
                | K::Enum
                | K::Alias
                | K::OperatorDeclaration
                | K::Const
                | K::Theorem
                | K::Axiom
                | K::Example
        ) {
            continue;
        }
        let before = builder.scopes[0].owned.len();
        let value = match builder.expression(node) {
            Ok(value) => value,
            Err(error) => return Err(vec![error]),
        };
        if session.is_some()
            && value.native != Ty::Unit
            && !matches!(value.ty, Type::Function(_) | Type::Overloaded(_))
            && node.span.start >= display_from
        {
            if notebook {
                if let Err(error) = builder.notebook_display(value, node) {
                    return Err(vec![error]);
                }
            } else {
                let text = match builder.display_render(value, node) {
                    Ok(text) => text,
                    Err(error) => return Err(vec![error]),
                };
                builder.code.push(Statement::Print(text.native, text.atom));
            }
        }
        builder.drop_statement_temporaries(before);
    }
    for (index, span) in tests.iter().enumerate() {
        if let Err(error) = builder.test_case(index, *span) {
            return Err(vec![error]);
        }
    }
    if let Some(state) = session.as_deref_mut() {
        state.names = builder.scopes[0].names.clone();
        state.owned = builder.scopes[0].owned.clone();
    } else {
        builder.finish_scope();
    }
    builder.code.push(Statement::Return(Atom::Unit, vec![]));
    let entry_function = builder.finish();
    compiler.functions[0] = Some(entry_function);
    while let Some(pending) = compiler.pending.pop() {
        let result = compiler.build_function(&pending);
        match result {
            Ok(function) => compiler.functions[pending.index] = Some(function),
            Err(error) => return Err(vec![error]),
        }
    }
    compiler
        .validate_view_escapes()
        .map_err(|error| vec![error])?;
    if let Some(state) = session {
        state.layouts = compiler.layouts.clone();
        state.layout_ids = compiler.layout_ids.clone();
        state.functions = compiler.functions.clone();
        state.statics = compiler.statics.clone();
        state.static_ids = compiler.static_ids.clone();
        state.globals = compiler.globals.clone();
        state.global_slots = compiler.global_slots.clone();
        state.global_bindings = compiler.global_bindings.clone();
        state.process_args = compiler.process_args;
        state.c_text_calls = compiler.c_text_calls.clone();
        state.python_modules = compiler.python_modules.clone();
        state.python_sites = compiler.python_sites.clone();
        state.closures = compiler.closures.clone();
        state.closure_codes = compiler.closure_codes.clone();
        state.instances = compiler.instances.clone();
        state.next_task = compiler.next_task;
    }
    let mut c_headers = vec![];
    let mut c_include_dirs = std::env::current_dir().into_iter().collect::<Vec<_>>();
    for module in &project.modules {
        for node in module
            .parsed
            .root
            .children
            .iter()
            .filter(|n| n.kind == K::Import)
        {
            let head = node
                .children
                .first()
                .map(|h| module.parsed.source.slice(h.span));
            if head == Some("c")
                && let Some(header) = crate::modules::foreign_name(node, &module.parsed)
            {
                let header = module
                    .checked
                    .environment
                    .c_headers
                    .get(&header)
                    .map(|path| path.to_string_lossy().into_owned())
                    .unwrap_or(header);
                if !c_headers.contains(&header) {
                    c_headers.push(header);
                }
            }
        }
        if let Some(folder) = std::path::Path::new(&module.parsed.source.name).parent()
            && folder.is_dir()
            && !c_include_dirs.iter().any(|d| d == folder)
        {
            c_include_dirs.push(folder.to_owned());
        }
    }
    let mut c_sources = vec![];
    for header in &c_headers {
        for folder in &c_include_dirs {
            let source = folder.join(header).with_extension("c");
            if header.ends_with(".h") && source.is_file() && !c_sources.contains(&source) {
                c_sources.push(source);
            }
        }
    }
    let mut program = Program {
        copyable_layouts: compiler
            .layouts
            .iter()
            .enumerate()
            .filter_map(|(id, layout)| {
                (!matches!(layout.kind, LayoutKind::Env(_)) && crate::types::copies(&layout.source))
                    .then_some(id)
            })
            .collect(),
        process_args: compiler.process_args,
        c_headers,
        c_include_dirs,
        c_sources,
        c_libraries: if cfg!(target_os = "linux") {
            vec!["dl".into()]
        } else {
            vec![]
        },
        c_text_calls: compiler.c_text_calls,
        sort_comparators: compiler.sort_comparators,
        python_modules: compiler.python_modules,
        python_sites: compiler.python_sites,
        closures: compiler.closures,
        layouts: compiler.layouts,
        functions: compiler.functions.into_iter().map(Option::unwrap).collect(),
        statics: compiler.statics,
        globals: compiler.globals,
        tail_groups: vec![],
        tests: false,
    };
    prepare_tail_calls(&mut program, project)?;
    Ok(program)
}

impl<'a> Compiler<'a> {
    fn is_trait_default(&self, declaration: &DeclarationId) -> bool {
        self.project.modules.iter().any(|module| {
            module
                .checked
                .environment
                .definitions
                .values()
                .any(|definition| {
                    definition.kind == K::Trait
                        && definition.file == declaration.file
                        && definition.default_methods.contains(&declaration.span)
                })
        })
    }

    fn bigint_layout(&self, ty: Ty) -> Option<usize> {
        match ty {
            Ty::Record(id) | Ty::Ref(id) if matches!(self.layouts[id].kind, LayoutKind::BigInt) => {
                Some(id)
            }
            _ => None,
        }
    }
    fn returns_borrow(&self, target: &Callable) -> bool {
        self.returns_borrow_within(target, &mut BTreeSet::new())
    }
    fn returns_borrow_within(&self, target: &Callable, seen: &mut BTreeSet<DeclarationId>) -> bool {
        let Callable::Declaration { id, kind, .. } = target else {
            return false;
        };
        if matches!(
            kind,
            DeclarationKind::Constructor | DeclarationKind::Variant
        ) {
            return false;
        }
        if !seen.insert(id.clone()) {
            return false;
        }
        let Some(definition) = self.definitions.get(id) else {
            return true;
        };
        let mut names = definition
            .signature
            .parameters
            .iter()
            .filter_map(|p| {
                p.name
                    .clone()
                    .map(|name| (name, p.passing != Passing::Move))
            })
            .collect::<BTreeMap<_, _>>();
        names.insert("this".into(), true);
        if let Some((_, _, captures)) = self.local_functions.get(id) {
            names.extend(captures.iter().map(|(name, _)| (name.clone(), true)));
        }
        let mut returned = false;
        let body = definition.node.children.last().unwrap();
        let result =
            self.borrow_expression(definition.module, body, &mut names, &mut returned, seen);
        seen.remove(id);
        result || returned
    }
    fn borrow_expression(
        &self,
        module: usize,
        node: &Node,
        names: &mut BTreeMap<String, bool>,
        returned: &mut bool,
        seen: &mut BTreeSet<DeclarationId>,
    ) -> bool {
        let source = &self.project.modules[module].parsed.source;
        if matches!(
            node.kind,
            K::Name | K::Member | K::Index | K::Call | K::Pipe
        ) && self.types[module]
            .get(&node.span)
            .is_some_and(crate::types::copies)
        {
            return false;
        }
        match node.kind {
            K::Name => names
                .get(source.slice(node.span))
                .copied()
                .unwrap_or_else(|| {
                    matches!(
                        self.project.modules[module]
                            .checked
                            .references
                            .get(&node.span),
                        Some(Reference::ScriptBinding(_))
                    )
                }),
            // An element read with `a[i]` is a view of its Array, as a field is of its object.
            // A tensor index or any `a:b` slice builds a new value instead.
            K::Index => {
                let base = self.types[module].get(&node.children[0].span);
                let sliced = node.children[1].kind == K::Slice
                    || node.children[1]
                        .descendants()
                        .any(|part| part.kind == K::Slice);
                if sliced || base.is_some_and(crate::types::is_tensor) {
                    return false;
                }
                self.borrow_expression(module, &node.children[0], names, returned, seen)
            }
            K::Group | K::Member => {
                self.borrow_expression(module, &node.children[0], names, returned, seen)
            }
            K::Call | K::Pipe => self.project.modules[module]
                .checked
                .resolved_calls
                .get(&node.span)
                .is_some_and(|call| self.returns_borrow_within(&call.target, seen)),
            K::Binding => {
                let initializer = node
                    .children
                    .iter()
                    .rev()
                    .find(|n| n.kind != K::Otherwise)
                    .unwrap();
                let borrow = self.borrow_expression(module, initializer, names, returned, seen);
                for pattern in node.children.iter().filter(|n| {
                    matches!(
                        n.kind,
                        K::PatternName | K::PatternTuple | K::PatternConstructor
                    )
                }) {
                    for name in pattern.descendants().filter(|n| n.kind == K::PatternName) {
                        let text = source
                            .slice(name.child(K::Name).unwrap_or(name).span)
                            .to_owned();
                        names.insert(text, borrow);
                    }
                }
                false
            }
            K::Block => {
                let mut local = names.clone();
                let mut result = false;
                for child in &node.children {
                    result = self.borrow_expression(module, child, &mut local, returned, seen);
                }
                result
            }
            K::Return => {
                let borrow = node
                    .children
                    .first()
                    .is_some_and(|n| self.borrow_expression(module, n, names, returned, seen));
                *returned |= borrow;
                borrow
            }
            K::For if node.child(K::ForBinding).is_some() => {
                let binding = node.child(K::ForBinding).unwrap();
                let mut local = names.clone();
                for name in binding.descendants().filter(|n| n.kind == K::PatternName) {
                    local.insert(
                        source
                            .slice(name.child(K::Name).unwrap_or(name).span)
                            .to_owned(),
                        true,
                    );
                }
                self.borrow_expression(
                    module,
                    node.children.last().unwrap(),
                    &mut local,
                    returned,
                    seen,
                );
                false
            }
            K::If => node.children.iter().skip(1).any(|child| {
                self.borrow_expression(module, child, &mut names.clone(), returned, seen)
            }),
            K::Match => {
                let borrowed = matches!(self.types[module].get(&node.children[0].span),Some(Type::Named(name,_)) if matches!(name.as_str(),"List"|"Map"|"Set"|"SortedSet"))
                    || self.borrow_expression(module, &node.children[0], names, returned, seen);
                let mut result = false;
                for arm in node.children.iter().skip(1) {
                    let mut local = names.clone();
                    for name in arm.children[0]
                        .descendants()
                        .filter(|n| n.kind == K::PatternName)
                    {
                        local.insert(
                            source
                                .slice(name.child(K::Name).unwrap_or(name).span)
                                .to_owned(),
                            borrowed,
                        );
                    }
                    result |= self.borrow_expression(
                        module,
                        arm.children.last().unwrap(),
                        &mut local,
                        returned,
                        seen,
                    );
                }
                result
            }
            K::Lambda | K::Function | K::Class => false,
            _ => {
                for child in &node.children {
                    self.borrow_expression(module, child, &mut names.clone(), returned, seen);
                }
                false
            }
        }
    }
    fn is_error_type(&self, ty: &Type) -> bool {
        ty == &Type::named("Error")
            || matches!(ty, Type::Named(id, _) if self.definition(id).is_some_and(|d| d.parent == Some(Type::named("Error"))))
    }
    fn error_case(&mut self, name: &str) -> Option<(usize, usize)> {
        let (owner, case) = name
            .rsplit_once('.')
            .map(|(owner, case)| (Some(owner), case))
            .unwrap_or((None, name));
        let definitions = self
            .project
            .modules
            .iter()
            .flat_map(|m| m.checked.environment.definitions.iter())
            .filter(|(id, d)| {
                d.parent == Some(Type::named("Error"))
                    && owner
                        .is_none_or(|owner| *id == owner || id.rsplit('.').next() == Some(owner))
            })
            .map(|(id, d)| (id.clone(), d.clone()))
            .collect::<BTreeMap<_, _>>();
        for (id, definition) in definitions {
            if let Some(case) = definition.variants.iter().position(|v| v.name == case)
                && let Ok(Ty::Record(layout)) = self.layout_type(&Type::named(&id))
            {
                return Some((layout, case));
            }
        }
        None
    }
    fn default_fields(&self, id: &str) -> Vec<(usize, Node)> {
        let Some(definition) = self.definition(id) else {
            return vec![];
        };
        let mut fields = if let Some(Type::Named(parent, _)) = &definition.parent {
            self.default_fields(parent)
        } else {
            vec![]
        };
        if let Some((module, checked)) = self
            .project
            .modules
            .iter()
            .enumerate()
            .find(|(_, m)| m.parsed.source.name == definition.file)
            && let Some(class) = checked.parsed.root.children.iter().find(|n| {
                n.kind == K::Class
                    && n.child(K::Name).is_some_and(|name| {
                        checked.parsed.source.slice(name.span) == id.rsplit('.').next().unwrap()
                    })
            })
        {
            fields.extend(
                class
                    .children
                    .iter()
                    .filter(|n| n.kind == K::Binding)
                    .cloned()
                    .map(|n| (module, n)),
            );
        }
        fields
    }
    /// Constructor-declared private fields, in constructor and source order. They
    /// join the layout after the class-body fields so public state keeps its
    /// existing offsets.
    fn constructor_field_nodes(&self, id: &str) -> Vec<(usize, Node)> {
        let Some(definition) = self.definition(id) else {
            return vec![];
        };
        let Some((module, checked)) = self
            .project
            .modules
            .iter()
            .enumerate()
            .find(|(_, m)| m.parsed.source.name == definition.file)
        else {
            return vec![];
        };
        let Some(class) = checked.parsed.root.children.iter().find(|n| {
            n.kind == K::Class
                && n.child(K::Name).is_some_and(|name| {
                    checked.parsed.source.slice(name.span) == id.rsplit('.').next().unwrap()
                })
        }) else {
            return vec![];
        };
        class
            .children
            .iter()
            .filter(|n| n.kind == K::Constructor)
            .flat_map(|constructor| {
                constructor
                    .children
                    .iter()
                    .filter(|n| n.kind == K::Block)
                    .flat_map(|body| body.children.iter())
                    .filter(|n| n.kind == K::FieldDeclaration)
            })
            .cloned()
            .map(|node| (module, node))
            .collect()
    }
    /// Adds the methods that a trait gains from the traits its `has` list names, so a trait object of
    /// `trait Expr { has Text }` has a text() slot. Built-in Text supplies text(); user traits supply
    /// their own methods, and the traits they name are followed in turn.
    fn with_inherited_methods(
        &self,
        mut methods: Vec<(String, FunctionType)>,
        declared: &[Type],
        substitutions: &BTreeMap<String, Type>,
    ) -> Vec<(String, FunctionType)> {
        let mut pending = vec![];
        for item in declared.iter().rev() {
            pending.extend(trait_name_and_args(&item.replace_parameters(substitutions)));
        }
        let mut visited = BTreeSet::new();
        while let Some((name, args, bindings)) = pending.pop() {
            if !visited.insert(name.clone()) {
                continue;
            }
            if name == "Text" && args.is_empty() {
                let Type::Function(text) = Type::function(vec![], Type::named("String")) else {
                    unreachable!()
                };
                if !methods.iter().any(|(known, _)| known == "text") {
                    methods.push(("text".to_owned(), text));
                }
                continue;
            }
            // Iterator offers next(), with the Item binding that the trait declares as
            // `has Iterator(Item = Int)`.
            if let Some(item) = bindings.get("Item").cloned()
                && name == "Iterator"
            {
                let Type::Function(mut next) = Type::function(
                    vec![],
                    Type::applied("Option", vec![Type::StoredView(Box::new(item))]),
                ) else {
                    unreachable!()
                };
                next.receiver_mutates = true;
                if !methods.iter().any(|(known, _)| known == "next") {
                    methods.push(("next".to_owned(), next));
                }
                continue;
            }
            let Some(definition) = self
                .definition(&name)
                .filter(|definition| definition.kind == K::Trait)
            else {
                continue;
            };
            let inner: BTreeMap<String, Type> = definition
                .parameters
                .iter()
                .cloned()
                .zip(args.iter().cloned())
                .collect();
            for (method, functions) in &definition.methods {
                if definition.static_methods.contains(method) {
                    continue;
                }
                for function in functions {
                    let Type::Function(function) =
                        Type::Function(function.clone()).replace_parameters(&inner)
                    else {
                        unreachable!()
                    };
                    if !methods.iter().any(|(known, known_function)| {
                        known == method && known_function.same_parameters(&function)
                    }) {
                        methods.push((method.clone(), function));
                    }
                }
            }
            for item in definition.traits.iter().rev() {
                pending.extend(trait_name_and_args(&item.replace_parameters(&inner)));
            }
        }
        methods
    }

    fn definition(&self, id: &str) -> Option<crate::checker::Definition> {
        self.project
            .modules
            .iter()
            .find_map(|module| module.checked.environment.definitions.get(id).cloned())
    }

    /// The invariants a method or constructor must check on a normal exit: its
    /// class's clauses and every ancestor's. A private method is exempt, because
    /// its caller checks the state; static methods have no receiver to check.
    fn invariant_checks(&self, pending: &Pending) -> Vec<InvariantCheck> {
        let Some(Callable::Declaration {
            id,
            name,
            owner: Some(owner),
            kind,
        }) = &pending.signature.callable
        else {
            return vec![];
        };
        if !matches!(kind, DeclarationKind::Method | DeclarationKind::Constructor) {
            return vec![];
        }
        let Some(class) = self.definition(owner) else {
            return vec![];
        };
        if class.kind != K::Class || class.record || id.file != class.file {
            return vec![];
        }
        if *kind == DeclarationKind::Method
            && class.member_visibility.get(name) == Some(&Visibility::Private)
        {
            return vec![];
        }
        let owner_name = owner.rsplit('.').next().unwrap_or(owner).to_owned();
        let method = name.clone();
        let mut checks = vec![];
        let mut current = owner.clone();
        let mut seen = BTreeSet::new();
        while seen.insert(current.clone()) {
            let Some(definition) = self.definition(&current) else {
                break;
            };
            if definition.kind != K::Class {
                break;
            }
            let Some(module) = self
                .project
                .modules
                .iter()
                .position(|module| module.parsed.source.name == definition.file)
            else {
                break;
            };
            let class_name = current.rsplit('.').next().unwrap_or(&current).to_owned();
            for invariant in &definition.invariants {
                let Some(condition) = invariant.children.first().cloned() else {
                    continue;
                };
                checks.push(InvariantCheck {
                    module,
                    class: class_name.clone(),
                    owner: owner_name.clone(),
                    method: method.clone(),
                    condition,
                });
            }
            match &definition.parent {
                Some(Type::Named(parent, _)) if parent != "Object" => current = parent.clone(),
                _ => break,
            }
        }
        checks
    }

    /// The id a module's environment gives a type or trait name, such as the prelude's Indexable.
    fn trait_id(&self, module: usize, name: &str) -> String {
        self.project.modules[module]
            .checked
            .environment
            .names
            .get(name)
            .cloned()
            .unwrap_or_else(|| name.to_owned())
    }
    fn trait_instance_for(&self, ty: &Type, target: &str) -> Option<Type> {
        fn base(ty: &Type) -> (&Type, Option<&BTreeMap<String, Type>>) {
            match ty {
                Type::Associated(base, bindings) => (base, Some(bindings)),
                _ => (ty, None),
            }
        }
        fn visit(
            compiler: &Compiler<'_>,
            ty: &Type,
            target: &str,
            seen: &mut BTreeSet<String>,
        ) -> Option<Type> {
            let ty = match ty {
                Type::Borrow(inner) | Type::StoredView(inner) | Type::Dynamic(inner) => {
                    inner.as_ref()
                }
                _ => ty,
            };
            let (base_ty, bindings) = base(ty);
            let Type::Named(id, arguments) = base_ty else {
                return None;
            };
            if id == target {
                return Some(ty.clone());
            }
            if !seen.insert(ty.to_string()) {
                return None;
            }
            let definition = compiler.definition(id)?;
            let mut substitutions = definition
                .parameters
                .iter()
                .cloned()
                .zip(arguments.iter().cloned())
                .collect::<BTreeMap<_, _>>();
            if let Some(bindings) = bindings {
                substitutions.extend(bindings.clone());
            }
            for required in &definition.traits {
                let required = required.replace_parameters(&substitutions);
                let required_base = match &required {
                    Type::Associated(base, _) => base.as_ref(),
                    _ => &required,
                };
                if matches!(required_base, Type::Named(name, _) if name == target) {
                    return Some(required);
                }
                if let Some(found) = visit(compiler, &required, target, seen) {
                    return Some(found);
                }
            }
            definition.parent.as_ref().and_then(|parent| {
                visit(
                    compiler,
                    &parent.replace_parameters(&substitutions),
                    target,
                    seen,
                )
            })
        }
        let instance = visit(self, ty, target, &mut BTreeSet::new())?;
        Some(self.infer_trait_bindings(ty, &instance).unwrap_or(instance))
    }
    fn infer_trait_bindings(&self, concrete: &Type, instance: &Type) -> Option<Type> {
        let (trait_id, arguments, explicit) = trait_name_and_args(instance)?;
        let definition = self
            .definition(&trait_id)
            .filter(|definition| definition.kind == K::Trait)?;
        if definition.associated_types.is_empty() {
            return Some(instance.clone());
        }
        let plain = Type::Named(trait_id.clone(), arguments.clone());
        let mut inference = crate::types::Inference::default();
        let mut substitutions = definition
            .parameters
            .iter()
            .cloned()
            .zip(arguments)
            .collect::<BTreeMap<_, _>>();
        let mut slots = BTreeMap::new();
        for name in &definition.associated_types {
            let variable = inference.fresh();
            if let Some(value) = explicit.get(name)
                && !inference.unify(&variable, value)
            {
                return None;
            }
            substitutions.insert(name.clone(), variable.clone());
            slots.insert(name.clone(), variable);
        }
        let mut states = vec![inference];
        for (name, contracts) in &definition.methods {
            let methods = self.methods(concrete, name);
            for contract in contracts {
                let Type::Function(contract) =
                    Type::Function(contract.clone()).replace_parameters(&substitutions)
                else {
                    unreachable!()
                };
                let mut next = vec![];
                for state in &states {
                    for method in &methods {
                        if method.effects != contract.effects
                            || method.receiver_mutates != contract.receiver_mutates
                        {
                            continue;
                        }
                        let mut candidate = state.clone();
                        if candidate.unify(
                            &Type::Function(contract.clone()),
                            &Type::Function(method.clone()),
                        ) {
                            next.push(candidate);
                        }
                    }
                }
                if next.is_empty() {
                    let is_default = matches!(
                        &contract.callable,
                        Some(Callable::Declaration { id, .. })
                            if id.file == definition.file
                                && definition.default_methods.contains(&id.span)
                    );
                    let matching_parameters = methods.iter().any(|method| {
                        states.iter().any(|state| {
                            let mut candidate = state.clone();
                            candidate.unify(
                                &Type::Tuple(
                                    contract
                                        .parameters
                                        .iter()
                                        .map(|parameter| parameter.ty.clone())
                                        .collect(),
                                ),
                                &Type::Tuple(
                                    method
                                        .parameters
                                        .iter()
                                        .map(|parameter| parameter.ty.clone())
                                        .collect(),
                                ),
                            )
                        })
                    });
                    if is_default && !matching_parameters {
                        continue;
                    }
                    return None;
                }
                states = next;
            }
        }
        let mut solutions = vec![];
        for state in states {
            let bindings = slots
                .iter()
                .map(|(name, variable)| (name.clone(), state.resolve(variable)))
                .collect::<BTreeMap<_, _>>();
            if bindings.values().any(|value| !value.variables().is_empty()) {
                return None;
            }
            if !solutions.contains(&bindings) {
                solutions.push(bindings);
            }
        }
        if solutions.len() != 1 {
            return None;
        }
        let mut bindings = explicit;
        bindings.extend(solutions.pop().unwrap());
        Some(Type::Associated(Box::new(plain), bindings))
    }
    fn holds_borrow(&self, ty: Ty, seen: &mut BTreeSet<usize>) -> bool {
        self.borrow_layout(ty, seen, true)
    }
    fn borrow_layout(&self, ty: Ty, seen: &mut BTreeSet<usize>, callbacks: bool) -> bool {
        match ty {
            Ty::Ref(_) | Ty::Error | Ty::Dyn(_) | Ty::DynBox(_) => true,
            Ty::Record(id) | Ty::Boxed(id) if seen.insert(id) => {
                let layout = &self.layouts[id];
                (callbacks
                    && match layout.kind {
                        LayoutKind::Closure { .. } => true,
                        LayoutKind::List(item) | LayoutKind::Array(item) => {
                            self.borrow_layout(item, seen, callbacks)
                        }
                        LayoutKind::Map(info) => {
                            self.borrow_layout(info.key, seen, callbacks)
                                || self.borrow_layout(info.value, seen, callbacks)
                        }
                        _ => false,
                    })
                    || layout
                        .fields
                        .iter()
                        .any(|f| self.borrow_layout(f.ty, seen, callbacks))
                    || layout.kind.cases().is_some_and(|cases| {
                        cases.iter().any(|case| {
                            case.fields
                                .iter()
                                .any(|ty| self.borrow_layout(*ty, seen, callbacks))
                        })
                    })
            }
            _ => false,
        }
    }
    fn source_type(&self, ty: Ty) -> Type {
        match ty {
            Ty::ArenaRef(id) => self.layouts[id].source.clone(),
            Ty::Nat => Type::named("Nat"),
            Ty::Dyn(id) => self.layouts[id].source.clone(),
            Ty::C(index) => Type::named(crate::interop::C_TYPES[index]),
            Ty::Pointer(constant) => Type::applied(
                if constant {
                    "CConstPointer"
                } else {
                    "CPointer"
                },
                vec![Type::named("CVoid")],
            ),
            Ty::Python => Type::named("PythonValue"),
            Ty::DynBox(id) => match &self.layouts[id].source {
                Type::Borrow(inner) => Type::applied("Box", vec![inner.as_ref().clone()]),
                other => other.clone(),
            },
            Ty::Record(id) | Ty::Ref(id) | Ty::Boxed(id) => self.layouts[id].source.clone(),
            _ => Type::named(match ty {
                Ty::Int => "Int",
                Ty::Double => "double",
                Ty::Bool => "Bool",
                Ty::String => "String",
                Ty::Error => "Error",
                Ty::Variable => "Variable",
                _ => "void",
            }),
        }
    }
    fn by_value_reaches(&self, from: usize, target: usize, seen: &mut BTreeSet<usize>) -> bool {
        if from == target {
            return true;
        }
        if !seen.insert(from) {
            return false;
        }
        let layout = &self.layouts[from];
        layout
            .fields
            .iter()
            .map(|f| f.ty)
            .chain(
                layout
                    .kind
                    .cases()
                    .into_iter()
                    .flatten()
                    .flat_map(|c| c.fields.iter().copied()),
            )
            .any(|ty| matches!(ty,Ty::Record(child) if self.by_value_reaches(child,target,seen)))
    }
    fn trait_layout(&mut self, requirement: &Type) -> std::result::Result<usize, String> {
        let key = format!("dyn {requirement}");
        if let Some(id) = self.layout_ids.get(&key) {
            return Ok(*id);
        }
        let unsupported = || format!("Native layout for dyn {requirement} is not implemented yet.");
        // `Iterator(Item = T)` names its one method's result through the associated type.
        let iterator_item = match requirement {
            Type::Associated(base, bindings) if **base == Type::named("Iterator") => bindings
                .get("Item")
                .filter(|_| bindings.len() == 1)
                .cloned(),
            _ => None,
        };
        let iterable_item = match requirement {
            Type::Associated(base, bindings)
                if **base == Type::named("Iterable") && bindings.len() == 1 =>
            {
                bindings.get("Item").cloned()
            }
            _ => None,
        };
        let operation = match requirement {
            Type::Associated(base, bindings) if bindings.len() == 1 => match base.as_ref() {
                Type::Named(name, args)
                    if crate::checker::operator_trait(name) && args.len() == 1 =>
                {
                    bindings
                        .get("Output")
                        .map(|result| (name.clone(), args[0].clone(), result.clone()))
                }
                _ => None,
            },
            _ => None,
        };
        let empty = vec![];
        let (name, args) = match requirement {
            Type::Named(name, args) => (name.as_str(), args),
            Type::Associated(base, _) if matches!(base.as_ref(), Type::Named(name, _) if self.definition(name).is_some_and(|d| d.kind == K::Trait)) =>
            {
                let Type::Named(name, args) = base.as_ref() else {
                    unreachable!()
                };
                (name.as_str(), args)
            }
            _ if iterator_item.is_some() => ("Iterator", &empty),
            _ if iterable_item.is_some() => ("Iterable", &empty),
            _ if operation.is_some() => ("operator", &empty),
            _ => return Err(unsupported()),
        };
        let (sealed, methods) = if let Some((name, operand, result)) = operation {
            let Type::Function(function) = Type::function(vec![operand], result) else {
                unreachable!()
            };
            (
                false,
                vec![(
                    crate::checker::operator_method(&name)
                        .unwrap_or("multiply")
                        .into(),
                    function,
                )],
            )
        } else if let Some(item) = iterable_item {
            let cursor = Type::applied(
                "Box",
                vec![Type::Dynamic(Box::new(Type::Associated(
                    Box::new(Type::named("Iterator")),
                    BTreeMap::from([("Item".to_owned(), item)]),
                )))],
            );
            let Type::Function(mut items) = Type::function(vec![], cursor) else {
                unreachable!()
            };
            items.effects = true;
            items.receiver_mutates = true;
            (false, vec![("items".to_owned(), items)])
        } else if let Some(item) = iterator_item {
            let Type::Function(mut next) = Type::function(
                vec![],
                Type::applied("Option", vec![Type::StoredView(Box::new(item))]),
            ) else {
                unreachable!()
            };
            next.receiver_mutates = true;
            (false, vec![("next".to_owned(), next)])
        } else if name == "Text" && args.is_empty() {
            let Type::Function(text) = Type::function(vec![], Type::named("String")) else {
                unreachable!()
            };
            (false, vec![("text".to_owned(), text)])
        } else {
            let definition = self
                .definition(name)
                .filter(|definition| definition.kind == K::Trait)
                .ok_or_else(unsupported)?;
            let mut substitutions: BTreeMap<_, _> = definition
                .parameters
                .iter()
                .cloned()
                .zip(args.iter().cloned())
                .collect();
            if let Type::Associated(_, bindings) = requirement {
                substitutions.extend(bindings.clone());
            }
            let methods = definition
                .methods
                .iter()
                .filter(|(name, _)| !definition.static_methods.contains(*name))
                .flat_map(|(name, functions)| {
                    functions.iter().map(|function| {
                        let Type::Function(function) =
                            Type::Function(function.clone()).replace_parameters(&substitutions)
                        else {
                            unreachable!()
                        };
                        (name.clone(), function)
                    })
                })
                .collect::<Vec<_>>();
            let methods = self.with_inherited_methods(methods, &definition.traits, &substitutions);
            (definition.sealed, methods)
        };
        let id = self.layouts.len();
        self.layout_ids.insert(key, id);
        self.layouts.push(Layout {
            is_error: false,
            source: Type::Borrow(Box::new(Type::Dynamic(Box::new(requirement.clone())))),
            fields: vec![],
            kind: LayoutKind::Trait(TraitLayout {
                sealed,
                methods: vec![],
                impls: vec![],
            }),
        });
        let mut lowered = vec![];
        for (name, function) in methods {
            let mut parameters = vec![];
            for parameter in &function.parameters {
                let native = match self.layout_type(&parameter.ty)? {
                    Ty::Record(id)
                        if parameter.passing != Passing::Move
                            && !crate::types::copies(&parameter.ty) =>
                    {
                        Ty::Ref(id)
                    }
                    other => other,
                };
                parameters.push((native, parameter.passing));
            }
            let result = self.layout_type(&function.result)?;
            lowered.push(DynMethod {
                name,
                parameters,
                result,
            });
        }
        let LayoutKind::Trait(info) = &mut self.layouts[id].kind else {
            unreachable!()
        };
        info.methods = lowered;
        Ok(id)
    }
    fn layout_type(&mut self, ty: &Type) -> std::result::Result<Ty, String> {
        if matches!(ty, Type::Named(name, args) if name == "Variable" && args.is_empty()) {
            return Ok(Ty::Variable);
        }
        let resolved = ty.map(&mut |part| {
            if let Type::StoredView(inner) = part {
                if crate::types::is_tensor(inner) {
                    None
                } else {
                    Some(if crate::types::copies(inner) {
                        *inner.clone()
                    } else {
                        Type::Borrow(inner.clone())
                    })
                }
            } else if let Type::Borrow(inner) = part
                && crate::types::copies(inner)
            {
                // A borrow of a copyable value is the value itself, so Option[&T] at Int is Option[Int].
                Some(*inner.clone())
            } else {
                None
            }
        });
        if &resolved != ty {
            return self.layout_type(&resolved);
        }
        if let Some(primitive) = primitive(ty) {
            return Ok(primitive);
        }
        if let Type::Cursor(item, _) = ty {
            let requirement = Type::Associated(
                Box::new(Type::named("Iterator")),
                BTreeMap::from([("Item".into(), *item.clone())]),
            );
            return Ok(Ty::DynBox(self.trait_layout(&requirement)?));
        }
        if let Type::Borrow(inner) = ty
            && let Type::Dynamic(requirement) = inner.as_ref()
        {
            return Ok(Ty::Dyn(self.trait_layout(requirement)?));
        }
        if let Type::Named(name, args) = ty
            && name == "Box"
            && args.len() == 1
        {
            if let Type::Dynamic(requirement) = &args[0] {
                return Ok(Ty::DynBox(self.trait_layout(requirement)?));
            }
            let inner = self.layout_type(&args[0])?;
            if let Ty::Record(id) = inner
                && matches!(
                    self.layouts[id].kind,
                    LayoutKind::Class | LayoutKind::Enum(_)
                )
            {
                return Ok(Ty::Boxed(id));
            }
            let key = format!("cell {ty}");
            if let Some(id) = self.layout_ids.get(&key) {
                return Ok(Ty::Boxed(*id));
            }
            let id = self.layouts.len();
            self.layout_ids.insert(key, id);
            self.layouts.push(Layout {
                is_error: false,
                source: ty.clone(),
                fields: vec![Field {
                    name: "value".into(),
                    ty: inner,
                }],
                kind: LayoutKind::Cell,
            });
            return Ok(Ty::Boxed(id));
        }
        if let Type::Borrow(inner) = ty {
            if crate::types::copies(inner) {
                return self.layout_type(inner);
            }
            return match self.layout_type(inner)? {
                Ty::Record(id) | Ty::Ref(id) | Ty::Boxed(id) => Ok(Ty::Ref(id)),
                Ty::DynBox(id) => Ok(Ty::Dyn(id)),
                other => Ok(other),
            };
        }
        if let Type::Indexed(inner, _) = ty {
            return self.layout_type(inner);
        }
        if ty == &Type::named("Error") {
            return Ok(Ty::Error);
        }
        if let Type::Union(parts) = ty {
            let key = layout_key(ty);
            if let Some(id) = self.layout_ids.get(&key) {
                return Ok(Ty::Record(*id));
            }
            let id = self.layouts.len();
            self.layout_ids.insert(key, id);
            self.layouts.push(Layout {
                is_error: false,
                source: ty.clone(),
                fields: vec![],
                kind: LayoutKind::Union(parts.clone(), vec![]),
            });
            let mut cases = vec![];
            for (i, part) in parts.iter().enumerate() {
                let native = self.layout_type(part)?;
                let fields = if native == Ty::Unit {
                    vec![]
                } else {
                    vec![native]
                };
                cases.push(Case {
                    name: format!("union:{i}"),
                    names: vec![None; fields.len()],
                    fields,
                });
            }
            self.layouts[id].kind = LayoutKind::Union(parts.clone(), cases);
            return Ok(Ty::Record(id));
        }
        if let Type::Tuple(parts) = ty {
            let key = layout_key(ty);
            if let Some(id) = self.layout_ids.get(&key) {
                return Ok(Ty::Record(*id));
            }
            let id = self.layouts.len();
            self.layout_ids.insert(key, id);
            self.layouts.push(Layout {
                is_error: false,
                source: ty.clone(),
                fields: vec![],
                kind: LayoutKind::Tuple,
            });
            let fields = parts
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    self.layout_type(t).map(|ty| Field {
                        name: i.to_string(),
                        ty,
                    })
                })
                .collect::<std::result::Result<Vec<_>, _>>()?;
            self.layouts[id].fields = fields;
            return Ok(Ty::Record(id));
        }
        if let Type::StoredView(inner) = ty {
            let native = self.layout_type(inner)?;
            // A tensor view is a full tensor header that borrows its storage; unlike an item
            // view, it is not a pointer to a record owned elsewhere.
            if crate::types::is_tensor(inner) {
                return Ok(native);
            }
            if crate::types::copies(inner) {
                return Ok(native);
            }
            return Ok(match native {
                Ty::Record(id) | Ty::Boxed(id) => Ty::Ref(id),
                Ty::DynBox(id) => Ty::Dyn(id),
                other => other,
            });
        }
        if let Type::Guard(inner) = ty {
            let key = layout_key(ty);
            if let Some(id) = self.layout_ids.get(&key) {
                return Ok(Ty::Record(*id));
            }
            let payload = self.layout_type(inner)?;
            let mutex = self.layout_type(&Type::applied("Mutex", vec![inner.as_ref().clone()]))?;
            let _ = mutex;
            let id = self.layouts.len();
            self.layout_ids.insert(key, id);
            self.layouts.push(Layout {
                is_error: false,
                source: ty.clone(),
                fields: vec![],
                kind: LayoutKind::Resource(Resource::Guard(payload)),
            });
            return Ok(Ty::Record(id));
        }
        if let Type::Function(function) = ty {
            let mut parameters = vec![];
            for parameter in &function.parameters {
                let native = match self.layout_type(&parameter.ty)? {
                    Ty::Record(id)
                        if parameter.passing != Passing::Move
                            && !crate::types::copies(&parameter.ty) =>
                    {
                        Ty::Ref(id)
                    }
                    other => other,
                };
                parameters.push((native, parameter.passing));
            }
            let result = self.layout_type(&function.result)?;
            let key = format!("closure {parameters:?} {result:?}");
            if let Some(id) = self.layout_ids.get(&key) {
                return Ok(Ty::Record(*id));
            }
            let id = self.layouts.len();
            self.layout_ids.insert(key, id);
            let mut source = function.clone();
            source.callable = None;
            source.receiver = None;
            self.layouts.push(Layout {
                is_error: false,
                source: Type::Function(source),
                fields: vec![],
                kind: LayoutKind::Closure { parameters, result },
            });
            return Ok(Ty::Record(id));
        }
        let Type::Named(name, args) = ty else {
            return Err(format!("Native layout for {ty} is not implemented yet."));
        };
        if matches!(name.as_str(), "CPointer" | "CConstPointer") && args.len() == 1 {
            return Ok(Ty::Pointer(name == "CConstPointer"));
        }
        let key = if crate::types::is_tensor(ty) {
            "native double tensor".into()
        } else {
            layout_key(ty)
        };
        if let Some(id) = self.layout_ids.get(&key) {
            if matches!(self.layouts[*id].kind, LayoutKind::ArenaRef) {
                return Ok(Ty::ArenaRef(*id));
            }
            return Ok(Ty::Record(*id));
        }
        if crate::types::is_tensor(ty) {
            let id = self.layouts.len();
            self.layout_ids.insert(key, id);
            self.layouts.push(Layout {
                is_error: false,
                source: crate::types::tensor_type(None),
                fields: vec![],
                kind: LayoutKind::Tensor,
            });
            return Ok(Ty::Record(id));
        }
        if name == "BigInt" && args.is_empty() {
            let id = self.layouts.len();
            self.layout_ids.insert(key, id);
            self.layouts.push(Layout {
                is_error: false,
                source: ty.clone(),
                fields: vec![],
                kind: LayoutKind::BigInt,
            });
            return Ok(Ty::Record(id));
        }
        if name == "ArenaRef" && args.len() == 1 {
            // Laying out the element may already have laid out this type, as when
            // a class holds a List of itself; reuse that layout.
            if let Some(id) = self.layout_ids.get(&key) {
                return Ok(Ty::Record(*id));
            }
            let id = self.layouts.len();
            self.layout_ids.insert(key, id);
            self.layouts.push(Layout {
                is_error: false,
                source: ty.clone(),
                fields: vec![],
                kind: LayoutKind::ArenaRef,
            });
            return Ok(Ty::ArenaRef(id));
        }
        if matches!(
            name.as_str(),
            "Reader" | "Writer" | "Mutex" | "Channel" | "Atomic" | "Arena"
        ) {
            let payload = args
                .first()
                .map(|t| self.layout_type(t))
                .transpose()?
                .unwrap_or(Ty::Unit);
            if name == "Atomic" && !matches!(payload, Ty::Int | Ty::Double) {
                return Err(
                    "Atomic supports Int and double; use Mutex for other value types.".into(),
                );
            }
            if name == "Arena" && payload == Ty::Unit {
                return Err("Arena storage needs a value type; void has no stored value.".into());
            }
            let resource = match name.as_str() {
                "Reader" => Resource::Reader,
                "Writer" => Resource::Writer,
                "Mutex" => Resource::Mutex(payload),
                "Channel" => Resource::Channel(payload),
                "Atomic" => Resource::Atomic(payload),
                "Arena" => Resource::Arena(payload),
                _ => unreachable!(),
            };
            // Laying out the element may already have laid out this type, as when
            // a class holds a List of itself; reuse that layout.
            if let Some(id) = self.layout_ids.get(&key) {
                return Ok(Ty::Record(*id));
            }
            let id = self.layouts.len();
            self.layout_ids.insert(key, id);
            self.layouts.push(Layout {
                is_error: false,
                source: ty.clone(),
                fields: vec![],
                kind: LayoutKind::Resource(resource),
            });
            return Ok(Ty::Record(id));
        }
        if matches!(
            name.as_str(),
            "Map" | "MutMap" | "Set" | "MutSet" | "SortedSet"
        ) && !args.is_empty()
        {
            let style = if name == "SortedSet" {
                MapStyle::SortedSet
            } else if matches!(name.as_str(), "Set" | "MutSet") {
                MapStyle::Set
            } else {
                MapStyle::Map
            };
            let mutable = matches!(name.as_str(), "MutMap" | "MutSet");
            // Laying out the element may already have laid out this type, as when
            // a class holds a List of itself; reuse that layout.
            if let Some(id) = self.layout_ids.get(&key) {
                return Ok(Ty::Record(*id));
            }
            let id = self.layouts.len();
            self.layout_ids.insert(key, id);
            self.layouts.push(Layout {
                is_error: false,
                source: ty.clone(),
                fields: vec![],
                kind: LayoutKind::Map(MapLayout {
                    key: Ty::Unit,
                    value: Ty::Unit,
                    item: 0,
                    equality: None,
                    hash: None,
                    mutable,
                    style,
                }),
            });
            let key = self.layout_type(&args[0])?;
            let value = if style == MapStyle::Map {
                self.layout_type(&args[1])?
            } else {
                Ty::Bool
            };
            let parts = if style == MapStyle::Map {
                args.clone()
            } else {
                vec![args[0].clone(), Type::named("Bool")]
            };
            let Ty::Record(item) = self.layout_type(&Type::Tuple(parts))? else {
                unreachable!()
            };
            self.layouts[id].kind = LayoutKind::Map(MapLayout {
                key,
                value,
                item,
                equality: None,
                hash: None,
                mutable,
                style,
            });
            return Ok(Ty::Record(id));
        }
        if name == "Array" && args.len() == 1 {
            let element = self.layout_type(&args[0])?;
            // Laying out the element may already have laid out this type, as when
            // a class holds a List of itself; reuse that layout.
            if let Some(id) = self.layout_ids.get(&key) {
                return Ok(Ty::Record(*id));
            }
            let id = self.layouts.len();
            self.layout_ids.insert(key, id);
            self.layouts.push(Layout {
                is_error: false,
                source: ty.clone(),
                fields: vec![],
                kind: LayoutKind::Array(element),
            });
            return Ok(Ty::Record(id));
        }
        if name == "List" && args.len() == 1 {
            let element = self.layout_type(&args[0])?;
            // Laying out the element may already have laid out this type, as when
            // a class holds a List of itself; reuse that layout.
            if let Some(id) = self.layout_ids.get(&key) {
                return Ok(Ty::Record(*id));
            }
            let id = self.layouts.len();
            self.layout_ids.insert(key, id);
            self.layouts.push(Layout {
                is_error: false,
                source: ty.clone(),
                fields: vec![],
                kind: LayoutKind::List(element),
            });
            return Ok(Ty::Record(id));
        }
        if name == "Option" && args.len() == 1 {
            // Laying out the element may already have laid out this type, as when
            // a class holds a List of itself; reuse that layout.
            if let Some(id) = self.layout_ids.get(&key) {
                return Ok(Ty::Record(*id));
            }
            let id = self.layouts.len();
            self.layout_ids.insert(key, id);
            self.layouts.push(Layout {
                is_error: false,
                source: ty.clone(),
                fields: vec![],
                kind: LayoutKind::Enum(vec![]),
            });
            let item = self.layout_type(&args[0])?;
            let item = if let Ty::Record(child) = item
                && (self.building_layouts.contains(&child)
                    || self.by_value_reaches(child, id, &mut BTreeSet::new()))
            {
                Ty::Boxed(child)
            } else {
                item
            };
            let cases = vec![
                Case {
                    name: "Some".into(),
                    fields: vec![item],
                    names: vec![None],
                },
                Case {
                    name: "None".into(),
                    fields: vec![],
                    names: vec![],
                },
            ];
            self.layouts[id].kind = if matches!(item, Ty::Ref(_) | Ty::Dyn(_)) {
                LayoutKind::Step(cases)
            } else {
                LayoutKind::Enum(cases)
            };
            return Ok(Ty::Record(id));
        }
        let Some(definition) = self.definition(name) else {
            return Err(format!("Native layout for {ty} is not implemented yet."));
        };
        if !matches!(definition.kind, K::Class | K::Enum) {
            return Err(format!("Native layout for {ty} is not implemented yet."));
        }
        if self.building_layouts.len() >= 96 || self.layouts.len() >= 4096 {
            return Err("Native type specialization exceeded the layout nesting limit.".into());
        }
        let id = self.layouts.len();
        self.building_layouts.insert(id);
        self.layout_ids.insert(key, id);
        self.layouts.push(Layout {
            is_error: false,
            source: ty.clone(),
            fields: vec![],
            kind: LayoutKind::Class,
        });
        let substitutions = definition
            .parameters
            .iter()
            .cloned()
            .zip(args.iter().cloned())
            .collect();
        if definition.kind == K::Enum {
            self.layouts[id].is_error = self.is_error_type(ty);
            self.layouts[id].kind = LayoutKind::Enum(vec![]);
            let mut cases = vec![];
            for variant in &definition.variants {
                let mut fields = vec![];
                for ty in &variant.fields {
                    let native = self.layout_type(&ty.replace_parameters(&substitutions))?;
                    fields.push(
                        if let Ty::Record(child) = native
                            && self.building_layouts.contains(&child)
                        {
                            Ty::Boxed(child)
                        } else {
                            native
                        },
                    );
                }
                cases.push(Case {
                    name: variant.name.clone(),
                    names: variant.field_names.clone(),
                    fields,
                });
            }
            self.layouts[id].kind = LayoutKind::Enum(cases);
            self.building_layouts.remove(&id);
            return Ok(Ty::Record(id));
        }
        let mut fields = vec![];
        if let Some(parent) = &definition.parent
            && parent != &Type::named("Object")
            && let Ty::Record(parent_id) =
                self.layout_type(&parent.replace_parameters(&substitutions))?
        {
            fields.extend(self.layouts[parent_id].fields.clone());
        }
        // Class-body fields keep their old source order. Constructor-declared
        // private fields follow them in their own source order.
        let mut field_positions = BTreeMap::new();
        let body_fields = self
            .default_fields(name)
            .into_iter()
            .filter_map(|(module, node)| {
                node.child(K::Name).map(|field| {
                    (
                        self.project.modules[module]
                            .parsed
                            .source
                            .slice(field.span)
                            .to_owned(),
                        node.span.start,
                    )
                })
            })
            .filter(|(field, _)| definition.fields.contains_key(field))
            .collect::<Vec<_>>();
        let mut ordered = body_fields;
        ordered.sort_by_key(|(_, start)| *start);
        for (index, (field, _)) in ordered.into_iter().enumerate() {
            field_positions.entry(field).or_insert(index);
        }
        let constructor_fields = self
            .constructor_field_nodes(name)
            .into_iter()
            .filter_map(|(module, node)| {
                let member = node.child(K::Member)?;
                let field = member.children.get(1)?;
                Some((
                    self.project.modules[module]
                        .parsed
                        .source
                        .slice(field.span)
                        .to_owned(),
                    node.span.start,
                ))
            })
            .collect::<Vec<_>>();
        let mut ordered = constructor_fields;
        ordered.sort_by_key(|(_, start)| *start);
        let offset = field_positions.len();
        for (index, (field, _)) in ordered.into_iter().enumerate() {
            field_positions.entry(field).or_insert(offset + index);
        }
        let mut own_fields = definition.fields.iter().collect::<Vec<_>>();
        own_fields
            .sort_by_key(|(name, _)| field_positions.get(*name).copied().unwrap_or(usize::MAX));
        for (name, ty) in own_fields {
            if fields.iter().any(|field| field.name == *name) {
                return Err("Shadowing inherited fields needs a language rule before it can have a native layout.".into());
            }
            let ty = self.layout_type(&ty.replace_parameters(&substitutions))?;
            if matches!(ty, Ty::Record(child) if self.building_layouts.contains(&child)) {
                return Err(
                    "Recursive owned class fields need the unresolved cycle-layout rules in Q20."
                        .into(),
                );
            }
            fields.push(Field {
                name: name.clone(),
                ty,
            });
        }
        self.layouts[id].fields = fields;
        self.building_layouts.remove(&id);
        Ok(Ty::Record(id))
    }
    fn ancestor(&self, ty: &Type, owner: &str) -> Option<Type> {
        let Type::Named(id, args) = ty else {
            return None;
        };
        if id == owner {
            return Some(ty.clone());
        }
        let definition = self.definition(id)?;
        let substitutions = definition
            .parameters
            .iter()
            .cloned()
            .zip(args.iter().cloned())
            .collect();
        self.ancestor(
            &definition.parent?.replace_parameters(&substitutions),
            owner,
        )
    }
    fn methods(&self, ty: &Type, name: &str) -> Vec<FunctionType> {
        let ty = match ty {
            Type::Borrow(inner) => inner.as_ref(),
            _ => ty,
        };
        let Type::Named(id, args) = ty else {
            return vec![];
        };
        let Some(definition) = self.definition(id) else {
            return vec![];
        };
        let substitutions = definition
            .parameters
            .iter()
            .cloned()
            .zip(args.iter().cloned())
            .collect();
        let mut methods = definition
            .methods
            .get(name)
            .into_iter()
            .flatten()
            .map(
                |f| match Type::Function(f.clone()).replace_parameters(&substitutions) {
                    Type::Function(f) => f,
                    _ => unreachable!(),
                },
            )
            .collect::<Vec<_>>();
        if let Some(parent) = definition.parent {
            for inherited in self.methods(&parent.replace_parameters(&substitutions), name) {
                if !methods.iter().any(|own| own.same_parameters(&inherited)) {
                    methods.push(inherited);
                }
            }
        }
        methods
    }

    fn new(project: &'a ProjectCheck) -> Self {
        let nodes = project
            .modules
            .iter()
            .map(|module| {
                module
                    .parsed
                    .root
                    .descendants()
                    .map(|node| (node.span, node))
                    .collect()
            })
            .collect();
        let types = project
            .modules
            .iter()
            .map(|module| module.checked.expression_types.iter().cloned().collect())
            .collect();
        let mut definitions = BTreeMap::new();
        for (module, checked) in project.modules.iter().enumerate() {
            for node in checked
                .parsed
                .root
                .children
                .iter()
                .filter(|node| node.kind == K::Function)
            {
                let mut name = node
                    .child(K::Name)
                    .map(|node| checked.parsed.source.slice(node.span))
                    .unwrap_or("")
                    .to_owned();
                // A function from `extend List` is stored as `List.take`.
                if let Some(target) = node.child(K::ExtendTarget) {
                    name = format!(
                        "{}.{name}",
                        checked.parsed.source.slice(target.children[0].span)
                    );
                }
                if let Some(symbol) = checked.checked.environment.values.get(&name) {
                    let signatures = match &symbol.ty {
                        Type::Function(f) => vec![f.clone()],
                        Type::Overloaded(fs) => fs.clone(),
                        _ => vec![],
                    };
                    for signature in signatures {
                        if let Some(Callable::Declaration { id, .. }) = &signature.callable
                            && id.span == node.span
                            && id.file == checked.parsed.source.name
                        {
                            definitions.insert(
                                id.clone(),
                                Definition {
                                    module,
                                    node: node.clone(),
                                    signature,
                                },
                            );
                        }
                    }
                }
            }
        }
        for (module, checked) in project.modules.iter().enumerate() {
            for definition in checked.checked.environment.definitions.values() {
                for signature in definition
                    .constructors
                    .iter()
                    .chain(definition.methods.values().flatten())
                {
                    if let Some(Callable::Declaration { id, .. }) = &signature.callable
                        && id.file == checked.parsed.source.name
                        && !matches!(
                            &signature.callable,
                            Some(Callable::Declaration {
                                owner: Some(owner),
                                kind:
                                    DeclarationKind::Method
                                    | DeclarationKind::StaticMethod,
                                ..
                            }) if checked.checked.environment.definitions.get(owner)
                                .is_some_and(|owner| owner.kind == K::Class)
                                && checked.checked.environment.definitions.values().any(|trait_def| {
                                    trait_def.kind == K::Trait
                                        && trait_def.file == id.file
                                        && trait_def.default_methods.contains(&id.span)
                                })
                        )
                        && let Some(node) = checked
                            .parsed
                            .root
                            .descendants()
                            .find(|node| node.span == id.span)
                    {
                        definitions.insert(
                            id.clone(),
                            Definition {
                                module,
                                node: node.clone(),
                                signature: signature.clone(),
                            },
                        );
                    }
                }
            }
        }
        Self {
            view_escapes: vec![],
            session: false,
            notebook: false,
            display_from: 0,
            process_args: None,
            c_text_calls: BTreeMap::new(),
            python_modules: vec![],
            python_sites: vec![],
            closures: vec![],
            closure_codes: BTreeMap::new(),
            layouts: vec![],
            layout_ids: BTreeMap::new(),
            sort_comparators: BTreeMap::new(),
            structural: BTreeMap::new(),
            local_functions: BTreeMap::new(),
            building_layouts: BTreeSet::new(),
            project,
            nodes,
            types,
            definitions,
            instances: BTreeMap::new(),
            pending: vec![],
            functions: vec![None],
            statics: vec![],
            static_ids: BTreeMap::new(),
            static_visiting: BTreeSet::new(),
            static_infos: BTreeMap::new(),
            globals: vec![],
            global_slots: BTreeMap::new(),
            global_bindings: BTreeMap::new(),
            next_task: 0,
        }
    }
    fn build_function(&mut self, pending: &Pending) -> Result<Function> {
        let definition = &pending.definition;
        let name = definition
            .node
            .child(K::Name)
            .map(|node| {
                self.project.modules[definition.module]
                    .parsed
                    .source
                    .slice(node.span)
            })
            .unwrap_or("function")
            .to_owned();
        let result = pending.native_result;
        let invariants = if pending.skip_invariants {
            vec![]
        } else {
            self.invariant_checks(pending)
        };
        let mut builder = Builder::new(
            self,
            definition.module,
            pending.mono.clone(),
            name,
            result,
            false,
        );
        let constructor = definition.node.kind == K::Constructor;
        builder.constructor = constructor;
        builder.invariants = invariants;
        builder.checking_invariants = pending.skip_invariants;
        let mut this_value = None;
        if constructor {
            let slot = builder.local(result, false, result.owns());
            let value = Value {
                keepalive: BTreeSet::new(),
                atom: Atom::Slot(slot),
                ty: (*pending.signature.result).clone(),
                native: result,
                temporary: false,
                callable: None,
            };
            builder
                .scopes
                .last_mut()
                .unwrap()
                .names
                .insert("this".into(), value.clone());
            this_value = Some(value);
        } else if let Some((source, ty)) = &pending.receiver {
            // A copyable receiver, such as a record of Strings, arrives as the caller's copy,
            // like any copyable argument, so the method releases it on return.
            let slot = builder.local(*ty, true, ty.owns() && crate::types::copies(source));
            builder.parameters.push((slot, *ty, Passing::Borrow));
            builder.scopes.last_mut().unwrap().names.insert(
                "this".into(),
                Value {
                    keepalive: BTreeSet::new(),
                    atom: Atom::Slot(slot),
                    ty: source.clone(),
                    native: *ty,
                    temporary: false,
                    callable: None,
                },
            );
        }
        // An invariant reads `this` and top-level names only, never a parameter or
        // local that a predicate could shadow.
        builder.invariant_names = builder.scopes[0].names.clone();
        // A proof parameter takes no argument. Its name is a unit local, so the body can pass it on (Q115).
        let mut natives = pending.native_parameters.iter();
        for parameter in pending.signature.parameters.iter() {
            if is_proof_type(&parameter.ty) {
                if let Some(name) = &parameter.name {
                    let slot = builder.local(Ty::Unit, true, false);
                    builder.scopes.last_mut().unwrap().names.insert(
                        name.clone(),
                        Value {
                            keepalive: BTreeSet::new(),
                            native: Ty::Unit,
                            atom: Atom::Slot(slot),
                            ty: parameter.ty.clone(),
                            temporary: false,
                            callable: None,
                        },
                    );
                }
                continue;
            }
            let Some(ty) = natives.next() else {
                continue;
            };
            let ty = *ty;
            let slot = builder.local(ty, true, ty.owns() && parameter.passing != Passing::Mut);
            builder.parameters.push((
                slot,
                ty,
                if matches!(ty, Ty::Ref(_)) {
                    Passing::Borrow
                } else {
                    parameter.passing
                },
            ));
            if let Some(name) = &parameter.name {
                builder.scopes.last_mut().unwrap().names.insert(
                    name.clone(),
                    Value {
                        keepalive: BTreeSet::new(),
                        native: ty,
                        atom: Atom::Slot(slot),
                        ty: parameter.ty.clone(),
                        temporary: false,
                        callable: None,
                    },
                );
            }
        }
        for (name, ty, source, assigned) in &pending.captures {
            let slot = builder.local(*ty, true, false);
            builder.parameters.push((
                slot,
                *ty,
                if *assigned {
                    Passing::Mut
                } else {
                    Passing::Borrow
                },
            ));
            builder.scopes.last_mut().unwrap().names.insert(
                name.clone(),
                Value {
                    keepalive: BTreeSet::new(),
                    native: *ty,
                    atom: Atom::Slot(slot),
                    ty: source.clone(),
                    temporary: false,
                    callable: None,
                },
            );
        }
        if !constructor && let Ty::Ref(id) = result {
            let slot = builder.local(Ty::Boxed(id), true, false);
            builder.parameters.push((slot, Ty::Boxed(id), Passing::Mut));
            builder.owned_result = Some(slot);
        }
        if constructor && let Some(this) = &this_value {
            let Type::Named(owner, _) = &this.ty else {
                unreachable!()
            };
            let explicit_super = definition.node.descendants().any(|n| {
                n.kind == K::Call
                    && n.children.first().is_some_and(|callee| {
                        callee.kind == K::Name && builder.text(callee) == "super"
                    })
            });
            let inherited = if explicit_super {
                builder
                    .compiler
                    .definition(owner)
                    .and_then(|definition| definition.parent)
                    .and_then(|parent| {
                        if let Type::Named(name, _) = parent {
                            Some(name)
                        } else {
                            None
                        }
                    })
                    .map(|parent| {
                        builder
                            .compiler
                            .default_fields(&parent)
                            .into_iter()
                            .map(|(module, field)| (module, field.span))
                            .collect::<BTreeSet<_>>()
                    })
                    .unwrap_or_default()
            } else {
                BTreeSet::new()
            };
            let field_nodes = builder
                .compiler
                .default_fields(owner)
                .into_iter()
                .filter(|(module, field)| !inherited.contains(&(*module, field.span)))
                .collect::<Vec<_>>();
            let Ty::Record(layout) = result else {
                unreachable!()
            };
            for (module, field) in field_nodes {
                let Some(annotation) = field.children.iter().find(|n| {
                    matches!(
                        n.kind,
                        K::TypeName
                            | K::TypeApplication
                            | K::BorrowType
                            | K::MutableType
                            | K::TupleType
                            | K::ShapeType
                            | K::UnionType
                            | K::FunctionType
                    )
                }) else {
                    continue;
                };
                let Some(initializer) = field
                    .children
                    .last()
                    .filter(|n| n.span.start >= annotation.span.end)
                else {
                    continue;
                };
                let source = &builder.compiler.project.modules[module].parsed.source;
                let name = field
                    .child(K::Name)
                    .map(|n| source.slice(n.span))
                    .unwrap_or("");
                let Some((index, field)) = builder.compiler.layouts[layout]
                    .fields
                    .iter()
                    .enumerate()
                    .find(|(_, f)| f.name == name)
                else {
                    continue;
                };
                let native = field.ty;
                let previous = builder.module;
                builder.module = module;
                let value = builder.expression(initializer)?;
                let rhs = builder.transfer(value, native, initializer)?;
                builder.code.push(Statement::Store(
                    Atom::Field(Box::new(this.atom.clone()), index, native),
                    native,
                    rhs,
                    builder.location(initializer),
                ));
                builder.module = previous;
            }
        }
        if pending.checked_contract {
            for requirement in definition
                .node
                .children
                .iter()
                .filter(|n| n.kind == K::Requires)
            {
                if builder
                    .checked()
                    .theorem_requirements
                    .contains(&requirement.span)
                {
                    continue;
                }
                let predicate = &requirement.children[0];
                let condition = builder.expression(predicate)?;
                let message = builder.text(predicate).to_owned();
                let (_, failure) = builder.isolated(|builder| {
                    let error =
                        builder.runtime_error("Contract", Atom::Text(message), predicate)?;
                    builder.return_value(error, predicate)?;
                    Ok(Value::unit())
                })?;
                builder
                    .code
                    .push(Statement::If(condition.atom, vec![], failure));
            }
        }
        let parameter_end = definition
            .node
            .child(K::ParameterList)
            .map(|node| node.span.end)
            .unwrap_or(0);
        let ensures = definition
            .node
            .children
            .iter()
            .filter(|n| n.kind == K::Ensures)
            .cloned()
            .collect::<Vec<_>>();
        let ensures_names = builder.scopes[0].names.clone();
        // The checks belong after the body, so a failed close or task join still
        // returns Error and stays exempt. The wrapper below runs both contracts.
        let invariants = std::mem::take(&mut builder.invariants);
        let invariant_names = builder.invariant_names.clone();
        let Some(body) = definition.node.children.iter().rev().find(|node| {
            node.span.start >= parameter_end
                && !matches!(
                    node.kind,
                    K::ReturnType | K::Effects | K::Requires | K::Ensures | K::Modifier
                )
        }) else {
            return builder.unsupported(&definition.node, "a function without a body");
        };
        let value = builder.expression(body)?;
        if let Some(this) = this_value {
            if !ends_with_return(&builder.code) {
                builder.return_value(this, body)?;
            }
        } else if body.kind != K::Return && !ends_with_return(&builder.code) {
            builder.return_value(value, body)?;
        }
        if ensures.is_empty() && invariants.is_empty() {
            return Ok(builder.finish());
        }
        // The body completes resource cleanup before its success value is
        // checked. In particular, a failed close or task join returns Error.
        let mono = builder.mono.clone();
        let owned_result = builder.owned_result;
        let (compiler, mut worker) = builder.finish_function();
        let parameters = worker.parameters.clone();
        let name = worker.name.clone();
        worker.name = format!("{name} body");
        let target = compiler.functions.len();
        compiler.functions.push(Some(worker));
        let mut wrapper = Builder::new(compiler, definition.module, mono, name, result, false);
        let mut args = vec![];
        for (slot, ty, mode) in &parameters {
            let created = wrapper.local(*ty, true, ty.owns() && *mode != Passing::Mut);
            // A constructor keeps the object in slot 0, so only a method's receiver
            // and a function's parameters keep their body slots.
            if !constructor {
                debug_assert_eq!(created, *slot);
            }
            wrapper.parameters.push((created, *ty, *mode));
            args.push((Atom::Slot(created), *mode));
        }
        wrapper.owned_result = owned_result;
        wrapper.ensures = ensures;
        wrapper.ensures_names = ensures_names.clone();
        if !wrapper.ensures.is_empty() {
            wrapper.scopes[0].names = ensures_names;
        }
        let returned = wrapper.temporary(result, Rhs::Call(target, args), body);
        wrapper.invariants = invariants;
        if constructor {
            // The constructed object arrives as the body's result, not as a parameter.
            let mut names = BTreeMap::new();
            names.insert("this".into(), wrapper.borrow_value(returned.clone()));
            wrapper.invariant_names = names;
        } else {
            wrapper.invariant_names = invariant_names;
        }
        wrapper.return_value(returned, body)?;
        Ok(wrapper.finish())
    }
}

struct Scope {
    finalizers: Vec<Cleanup>,
    names: BTreeMap<String, Value>,
    owned: Vec<Slot>,
}
struct Builder<'a, 'b> {
    compiler: &'b mut Compiler<'a>,
    /// Set while rendering a debug view, so Strings inside containers show quoted.
    debug_render: bool,
    /// Set while a session or notebook shows a result: Strings there stay unquoted.
    plain_strings: bool,
    module: usize,
    mono: Mono,
    name: String,
    result: Ty,
    entry: bool,
    parameters: Vec<(Slot, Ty, Passing)>,
    locals: Vec<Local>,
    scopes: Vec<Scope>,
    code: Vec<Statement>,
    loops: Vec<(usize, usize)>,
    next_loop: usize,
    inline_lambda: bool,
    task_scopes: Vec<(Slot, bool)>,
    consume_receiver: Option<Span>,
    /// Expressions being lowered before they go into a new box (Q86).
    boxing: BTreeSet<Span>,
    constructor: bool,
    /// A function that returns a borrowed argument on one path may build a new value on
    /// another. That value goes into a box the caller passes and frees after the borrow.
    owned_result: Option<Slot>,
    /// Boxes this function passed for such calls; returning their view hands the box on.
    result_boxes: BTreeSet<Slot>,
    result_box_calls: BTreeMap<Slot, usize>,
    active_defaults: BTreeSet<(DeclarationId, usize)>,
    ensures: Vec<Node>,
    ensures_names: BTreeMap<String, Value>,
    checking_ensures: bool,
    /// Class invariants checked after the body finishes, in its wrapper.
    invariants: Vec<InvariantCheck>,
    /// The names an invariant may read: `this` and nothing from the body's scope.
    invariant_names: BTreeMap<String, Value>,
    checking_invariants: bool,
}

/// One `invariant` clause reached through the receiver's class and its ancestors.
#[derive(Clone)]
struct InvariantCheck {
    module: usize,
    /// The class that declares the clause, for the failure message.
    class: String,
    /// The class whose method runs the check, for the failure message.
    owner: String,
    /// The method or constructor that runs the check, for the failure message.
    method: String,
    condition: Node,
}

fn primitive(ty: &Type) -> Option<Ty> {
    match ty {
        Type::Named(name, args) if args.is_empty() => match name.as_str() {
            "Int" => Some(Ty::Int),
            "Nat" => Some(Ty::Nat),
            "double" => Some(Ty::Double),
            "Bool" => Some(Ty::Bool),
            "String" | "Bytes" => Some(Ty::String),
            // A proof has no run-time value, so it has the layout of void (Q115).
            "void" | "Proof" => Some(Ty::Unit),
            "PythonValue" => Some(Ty::Python),
            name => crate::interop::C_TYPES
                .iter()
                .position(|t| *t == name)
                .map(Ty::C),
        },
        Type::Never => Some(Ty::Unit),
        _ => None,
    }
}

fn plain_default_type(ty: &Type) -> bool {
    primitive(ty).is_some_and(|ty| {
        matches!(
            ty,
            Ty::Int | Ty::Nat | Ty::Double | Ty::Bool | Ty::String | Ty::C(_)
        )
    }) || matches!(ty, Type::Named(name, _) if matches!(name.as_str(), "CPointer" | "CConstPointer" | "ArenaRef"))
}

/// Points the call that received `slot` for its owned result at `out` instead.
fn forward_box(code: &mut [Statement], slot: Slot, out: Slot) -> bool {
    code.iter_mut().rev().any(|statement| match statement {
        Statement::Set(_, Rhs::Call(_, arguments), _)
        | Statement::Evaluate(Rhs::Call(_, arguments), _) => {
            let argument = arguments.iter_mut().find(|(atom, mode)| {
                *mode == Passing::Mut && matches!(atom, Atom::Slot(found) if *found == slot)
            });
            argument
                .map(|argument| argument.0 = Atom::Slot(out))
                .is_some()
        }
        Statement::If(_, left, right) => {
            forward_box(left, slot, out) || forward_box(right, slot, out)
        }
        _ => false,
    })
}
/// A borrowed and an owned value of the same record: they merge into a view.
fn mixed_view(a: Ty, b: Ty) -> Option<usize> {
    match (a, b) {
        (Ty::Ref(x), Ty::Record(y)) | (Ty::Record(x), Ty::Ref(y)) if x == y => Some(x),
        _ => None,
    }
}
fn ends_with_return(code: &[Statement]) -> bool {
    match code.iter().rev().find(|s| !matches!(s, Statement::Drop(_))) {
        Some(Statement::Return(..) | Statement::TailCall(..)) => true,
        Some(Statement::If(_, a, b)) => ends_with_return(a) && ends_with_return(b),
        Some(Statement::Match(_, arms)) => arms.iter().all(|arm| ends_with_return(&arm.body)),
        _ => false,
    }
}

impl<'a, 'b> Builder<'a, 'b> {
    fn new(
        compiler: &'b mut Compiler<'a>,
        module: usize,
        mono: Mono,
        name: String,
        result: Ty,
        entry: bool,
    ) -> Self {
        Self {
            compiler,
            module,
            mono,
            name,
            result,
            entry,
            parameters: vec![],
            locals: vec![],
            scopes: vec![Scope {
                finalizers: vec![],
                names: BTreeMap::new(),
                owned: vec![],
            }],
            code: vec![],
            loops: vec![],
            next_loop: 0,
            inline_lambda: false,
            task_scopes: vec![],
            consume_receiver: None,
            boxing: BTreeSet::new(),
            constructor: false,
            owned_result: None,
            result_boxes: BTreeSet::new(),
            result_box_calls: BTreeMap::new(),
            debug_render: false,
            plain_strings: false,
            active_defaults: BTreeSet::new(),
            ensures: vec![],
            ensures_names: BTreeMap::new(),
            checking_ensures: false,
            invariants: vec![],
            invariant_names: BTreeMap::new(),
            checking_invariants: false,
        }
    }
    fn finish_function(self) -> (&'b mut Compiler<'a>, Function) {
        (
            self.compiler,
            Function {
                name: self.name,
                parameters: self.parameters,
                result: self.result,
                locals: self.locals,
                body: self.code,
            },
        )
    }
    fn finish(self) -> Function {
        Function {
            name: self.name,
            parameters: self.parameters,
            result: self.result,
            locals: self.locals,
            body: self.code,
        }
    }
    fn checked(&self) -> &Checked {
        &self.compiler.project.modules[self.module].checked
    }
    fn text<'n>(&self, node: &'n Node) -> &'a str {
        self.compiler.project.modules[self.module]
            .parsed
            .source
            .slice(node.span)
    }
    fn location(&self, node: &Node) -> Location {
        let source = &self.compiler.project.modules[self.module].parsed.source;
        let (line, column) = source.position(node.span.start);
        Location {
            span: node.span,
            file: source.name.clone(),
            line,
            column,
            expression: source.slice(node.span).to_owned(),
        }
    }
    fn error<T>(&self, node: &Node, code: &'static str, message: impl Into<String>) -> Result<T> {
        Err(LocatedDiagnostic {
            module: self.module,
            diagnostic: Diagnostic::new(code, message, node.span).with_hint(
                "This form checks, but native lowering does not support it yet. See the limits section of README.md.",
            ),
        })
    }
    /// In a branch whose sibling gives a borrow, a new value moves into the shared box.
    fn box_branch(&mut self, value: Value, mixed: Option<Slot>, node: &Node) -> Result<Value> {
        let (Some(slot), Ty::Record(id)) = (mixed, value.native) else {
            return Ok(value);
        };
        let ty = value.ty.clone();
        let value = self.freeze_owned(value, node)?;
        self.code.push(Statement::Set(
            slot,
            Rhs::BoxNew(id, value.atom, value.native),
            self.location(node),
        ));
        Ok(Value {
            native: Ty::Ref(id),
            atom: Atom::Slot(slot),
            ty,
            ..Value::unit()
        })
    }
    /// Runs the test at `index` when the program was started for it; an Error it
    /// returns is shown and fails the run.
    fn test_case(&mut self, index: usize, span: Span) -> Result<()> {
        let module = self.module;
        let found = self
            .compiler
            .definitions
            .iter()
            .find(|(id, definition)| id.span == span && definition.module == module)
            .map(|(_, definition)| definition.clone());
        let Some(definition) = found else {
            return Err(LocatedDiagnostic {
                module,
                diagnostic: Diagnostic::new(
                    "E_NATIVE_DECLARATION",
                    "This test has no native declaration.",
                    span,
                ),
            });
        };
        let node = definition.node.clone();
        let chosen = self.temporary(
            Ty::Bool,
            Rhs::Builtin(
                "testSelected".into(),
                Ty::Unit,
                vec![Atom::Int(index as i64)],
            ),
            &node,
        );
        let (_, body) = self.isolated(|builder| {
            let call = ResolvedCall {
                target: definition.signature.callable.clone().unwrap(),
                callee: None,
                overload: 0,
                signature: definition.signature.clone(),
                type_arguments: BTreeMap::new(),
                arguments: vec![],
                evaluation_order: vec![],
                receiver: None,
                partial: false,
            };
            let result = builder.invoke(&call, vec![], None, &node, None)?;
            if result.native != Ty::Unit {
                builder.code.push(Statement::Evaluate(
                    Rhs::Builtin("testResult".into(), result.native, vec![result.atom]),
                    builder.location(&node),
                ));
            }
            Ok(Value::unit())
        })?;
        self.code.push(Statement::If(chosen.atom, body, vec![]));
        Ok(())
    }
    /// The slot `a[i]` names, after a check that stops the program on a bad index (Q53).
    fn array_item(&mut self, node: &Node) -> Result<(Atom, Ty)> {
        let owner = self.expression(&node.children[0])?;
        let (Ty::Record(id) | Ty::Ref(id)) = owner.native else {
            return self.unsupported(node, "brackets on this value");
        };
        let LayoutKind::Array(element) = self.compiler.layouts[id].kind else {
            return self.unsupported(node, "brackets on this value");
        };
        let index = self.expression(&node.children[1])?;
        let length = Atom::ArrayLength(Box::new(owner.atom.clone()));
        let checked = self.temporary(
            Ty::Int,
            Rhs::Builtin("arrayIndex".into(), Ty::Unit, vec![index.atom, length]),
            node,
        );
        Ok((
            Atom::ArrayItem(Box::new(owner.atom), Box::new(checked.atom), element),
            element,
        ))
    }
    fn unsupported<T>(&self, node: &Node, what: &str) -> Result<T> {
        self.error(
            node,
            "E_NATIVE_UNSUPPORTED",
            format!("Native compilation does not support {what} yet."),
        )
    }
    fn native_type(&mut self, ty: &Type, node: &Node) -> Result<Ty> {
        let resolved = self.mono.apply(ty);
        self.compiler
            .layout_type(&resolved)
            .map_err(|message| LocatedDiagnostic {
                module: self.module,
                diagnostic: Diagnostic::new("E_NATIVE_TYPE", message, node.span),
            })
    }

    fn ty(&self, node: &Node) -> Type {
        self.mono.apply(
            self.compiler.types[self.module]
                .get(&node.span)
                .unwrap_or(&Type::named("void")),
        )
    }
    fn local(&mut self, ty: Ty, parameter: bool, owned: bool) -> Slot {
        if self.entry && self.compiler.session && !parameter {
            let slot = Slot {
                id: self.compiler.globals.len(),
                global: true,
            };
            self.compiler.globals.push(ty);
            if owned {
                self.scopes.last_mut().unwrap().owned.push(slot);
            }
            return slot;
        }
        let slot = Slot {
            id: self.locals.len(),
            global: false,
        };
        self.locals.push(Local {
            scope: self.scopes.len() - 1,
            ty,
            parameter,
            owned,
        });
        if owned {
            self.scopes.last_mut().unwrap().owned.push(slot);
        }
        slot
    }
    fn temporary(&mut self, ty: Ty, rhs: Rhs, node: &Node) -> Value {
        if ty == Ty::Unit {
            self.code
                .push(Statement::Evaluate(rhs, self.location(node)));
            return Value::unit();
        }
        let slot = self.local(ty, false, ty.owns());
        self.code
            .push(Statement::Set(slot, rhs, self.location(node)));
        Value {
            keepalive: BTreeSet::new(),
            native: ty,
            atom: Atom::Slot(slot),
            ty: self.compiler.source_type(ty),
            temporary: true,
            callable: None,
        }
    }
    fn slot_type(&self, slot: Slot) -> Ty {
        if slot.global {
            self.compiler.globals[slot.id]
        } else {
            self.locals[slot.id].ty
        }
    }
    fn borrow_value(&self, mut value: Value) -> Value {
        if crate::types::copies(&value.ty) && !matches!(value.native, Ty::Boxed(_)) {
            return value;
        }
        // Lists and persistent maps share counted storage, so even a view passes them by value.
        if let Ty::Record(id) = value.native
            && matches!(
                self.compiler.layouts[id].kind,
                LayoutKind::List(_)
                    | LayoutKind::Map(MapLayout { mutable: false, .. })
                    | LayoutKind::ClassView(_)
            )
        {
            return value;
        }
        if let Ty::DynBox(id) = value.native {
            if value.temporary
                && let Atom::Slot(slot) = value.atom
            {
                value.keepalive.insert(slot);
            }
            value.native = Ty::Dyn(id);
            value.temporary = true;
            return value;
        }
        if let Ty::Boxed(id) = value.native {
            if let Atom::Slot(slot) = value.atom {
                value.keepalive.insert(slot);
            }
            value.native = Ty::Ref(id);
            value.temporary = true;
        } else if let Ty::Record(id) = value.native {
            if let Atom::Slot(slot) = value.atom {
                value.keepalive.insert(slot);
            }
            value.atom = Atom::Address(Box::new(value.atom), id);
            value.native = Ty::Ref(id);
            value.temporary = true;
        }
        value
    }
    /// A place holding a value that a trait-object view will point at. A stored value is
    /// used where it is; a computed one goes into storage that lives as long as the frame.
    fn dyn_place(&mut self, value: Value, node: &Node) -> Result<Atom> {
        if matches!(
            value.atom,
            Atom::Slot(_)
                | Atom::Field(..)
                | Atom::Payload(..)
                | Atom::ListItem(..)
                | Atom::EnvField(..)
        ) {
            return Ok(value.atom);
        }
        let native = value.native;
        let slot = self.local(native, false, false);
        self.locals[slot.id].scope = 0;
        if native.owns() {
            if !slot.global {
                self.locals[slot.id].owned = true;
            }
            self.scopes[0].owned.push(slot);
        }
        let rhs = self.transfer(value, native, node)?;
        self.code
            .push(Statement::Set(slot, rhs, self.location(node)));
        Ok(Atom::Slot(slot))
    }
    fn freeze_owned(&mut self, value: Value, node: &Node) -> Result<Value> {
        if value.native.owns() && !value.temporary {
            let rhs = if crate::types::copies(&value.ty) {
                Rhs::Copy(value.atom)
            } else {
                Rhs::Take(value.atom)
            };
            return Ok(self.temporary(value.native, rhs, node));
        }
        Ok(value)
    }
    fn boxed_view_layout(&self, value: &Value) -> Option<usize> {
        let Ty::Ref(id) = value.native else {
            return None;
        };
        let stored = match &value.atom {
            Atom::Slot(slot) => self.slot_type(*slot),
            Atom::Field(_, _, ty) | Atom::Payload(_, _, _, ty) => *ty,
            _ => return None,
        };
        (stored == Ty::Boxed(id)).then_some(id)
    }
    /// Ownership checking has approved this handover. A pattern may expose an
    /// owned recursive field as a view; take its actual box rather than passing
    /// a pointer into the enclosing frame.
    fn move_boxed_payload(&mut self, value: Value, node: &Node) -> Value {
        let Some(id) = self.boxed_view_layout(&value) else {
            return value;
        };
        let source = self.ty(node);
        let explicit_box = matches!(&source, Type::Named(name,_) if name == "Box")
            || matches!(&value.ty, Type::Named(name,_) if name == "Box");
        let mut moved = if explicit_box {
            // A programmer's Box stays a Box. Recursive enum storage is the
            // representation that needs unwrapping to recover an owned record.
            self.temporary(Ty::Boxed(id), Rhs::Take(value.atom), node)
        } else {
            self.temporary(Ty::Record(id), Rhs::BoxUnwrap(id, value.atom), node)
        };
        moved.ty = source;
        moved
    }
    fn freeze(&mut self, value: Value, node: &Node) -> Result<Value> {
        if matches!(value.native, Ty::Record(_) | Ty::Boxed(_) | Ty::DynBox(_))
            && !crate::types::copies(&value.ty)
        {
            return Ok(self.borrow_value(value));
        }
        // Counted values read straight out of a collection, such as a List item in
        // a loop, are places too: a later move must not empty the collection.
        let collection_place = value.native.owns()
            && matches!(
                value.atom,
                Atom::ListItem(..)
                    | Atom::ArrayItem(..)
                    | Atom::SetItem(..)
                    | Atom::OwnerValue(..)
                    | Atom::ResourceValue(..)
                    | Atom::ErrorPayload(..)
            );
        if (collection_place
            || matches!(
                value.atom,
                Atom::Slot(_) | Atom::Field(..) | Atom::Payload(..) | Atom::EnvField(..)
            ))
            && value.callable.is_none()
            && !value.temporary
        {
            let mut copied = self.temporary(value.native, Rhs::Copy(value.atom), node);
            copied.ty = value.ty;
            copied.keepalive = value.keepalive;
            Ok(copied)
        } else {
            Ok(value)
        }
    }
    fn transfer(&mut self, value: Value, target: Ty, node: &Node) -> Result<Rhs> {
        if let Ty::Record(id) = target
            && let LayoutKind::ClassView(cases) = self.compiler.layouts[id].kind.clone()
        {
            if value.native == target {
                return Ok(Rhs::Copy(value.atom));
            }
            if let Some(case) = cases.iter().position(|case| case.fields == [value.native]) {
                return Ok(Rhs::Aggregate(
                    id,
                    Some(case),
                    vec![(value.atom, value.native)],
                ));
            }
        }
        if let Ty::Record(source) = value.native
            && let LayoutKind::ClassView(cases) = self.compiler.layouts[source].kind.clone()
            && (matches!(target, Ty::Dyn(_))
                || matches!(target, Ty::Record(id) if matches!(self.compiler.layouts[id].kind,LayoutKind::ClassView(_))))
        {
            let output = self.local(target, false, target.owns());
            for (case, info) in cases.iter().enumerate() {
                let native = info.fields[0];
                let actual = Value {
                    native,
                    atom: Atom::Payload(Box::new(value.atom.clone()), case, 0, native),
                    ty: self.compiler.source_type(native),
                    keepalive: value.keepalive.clone(),
                    ..Value::unit()
                };
                let (_, code) = self.isolated(|builder| {
                    builder.assign(output, actual, node).map(|()| Value::unit())
                })?;
                let condition = self.temporary(
                    Ty::Bool,
                    Rhs::Test(Test::Equal(
                        Ty::Int,
                        Atom::Tag(Box::new(value.atom.clone())),
                        Atom::Int((case + 1) as i64),
                    )),
                    node,
                );
                self.code.push(Statement::If(condition.atom, code, vec![]));
            }
            return Ok(Rhs::Copy(Atom::Slot(output)));
        }
        if value.callable.is_some()
            && let Some(layout) = self.is_closure(target)
        {
            let source = self.compiler.layouts[layout].source.clone();
            let value = self.materialize(value, &source, node)?;
            return Ok(Rhs::Take(value.atom));
        }
        if let Ty::Record(id) = target
            && let LayoutKind::Union(parts, cases) = self.compiler.layouts[id].kind.clone()
        {
            if value.native == target {
                return Ok(Rhs::Take(value.atom));
            }
            if let Ty::Record(source) | Ty::Ref(source) = value.native
                && let LayoutKind::Union(source_parts, source_cases) =
                    self.compiler.layouts[source].kind.clone()
            {
                let output = self.local(target, false, true);
                for (case, part) in source_parts.iter().enumerate() {
                    if *part == Type::Never {
                        continue;
                    }
                    let native = source_cases[case]
                        .fields
                        .first()
                        .copied()
                        .unwrap_or(Ty::Unit);
                    let payload = Value {
                        keepalive: value.keepalive.clone(),
                        atom: if native == Ty::Unit {
                            Atom::Unit
                        } else {
                            Atom::Payload(Box::new(value.atom.clone()), case, 0, native)
                        },
                        native,
                        ty: part.clone(),
                        temporary: false,
                        callable: None,
                    };
                    let payload = if matches!(value.native, Ty::Ref(_)) {
                        self.borrow_value(payload)
                    } else {
                        payload
                    };
                    let condition = self.temporary(
                        Ty::Bool,
                        Rhs::Test(Test::Equal(
                            Ty::Int,
                            Atom::Tag(Box::new(value.atom.clone())),
                            Atom::Int(case as i64 + 1),
                        )),
                        node,
                    );
                    let (_, code) = self.isolated(|builder| {
                        let rhs = builder.transfer(payload, target, node)?;
                        builder
                            .code
                            .push(Statement::Set(output, rhs, builder.location(node)));
                        Ok(Value::unit())
                    })?;
                    self.code.push(Statement::If(condition.atom, code, vec![]));
                }
                return Ok(Rhs::Take(Atom::Slot(output)));
            }
            if let Some(case) = parts.iter().position(|part| {
                *part == value.ty
                    || *part == Type::named("Error") && self.compiler.is_error_type(&value.ty)
            }) {
                let fields = cases[case].fields.clone();
                if fields.is_empty() {
                    return Ok(Rhs::Aggregate(id, Some(case), vec![]));
                }
                let native = fields[0];
                let value = if value.native != native {
                    let rhs = self.transfer(value, native, node)?;
                    self.temporary(native, rhs, node)
                } else {
                    value
                };
                return Ok(Rhs::Aggregate(
                    id,
                    Some(case),
                    vec![(value.atom, value.native)],
                ));
            }
        }
        if target == Ty::Error
            && self.compiler.is_error_type(&value.ty)
            && let Ty::Record(id) | Ty::Ref(id) = value.native
        {
            return Ok(Rhs::ErrorBox(
                id,
                value.atom,
                matches!(value.native, Ty::Ref(_)),
            ));
        }
        if let Ty::Dyn(trait_id) = target {
            match value.native {
                Ty::Dyn(id) | Ty::DynBox(id) if id == trait_id => {
                    return Ok(Rhs::Copy(value.atom));
                }
                Ty::Record(id)
                    if matches!(
                        self.compiler.layouts[id].kind,
                        LayoutKind::List(_) | LayoutKind::Map(MapLayout { mutable: false, .. })
                    ) =>
                {
                    let cell = self.builtin_cell(value.native, node)?.unwrap();
                    let implementation = self.dyn_impl(trait_id, cell, node)?;
                    let slot = self.local(Ty::Record(cell), false, false);
                    if !slot.global {
                        self.locals[slot.id].owned = true;
                        self.locals[slot.id].scope = 0;
                    }
                    self.scopes[0].owned.push(slot);
                    self.code.push(Statement::Set(
                        slot,
                        Rhs::Aggregate(cell, None, vec![(value.atom, value.native)]),
                        self.location(node),
                    ));
                    return Ok(Rhs::MakeDyn(
                        trait_id,
                        implementation,
                        Atom::Address(Box::new(Atom::Slot(slot)), cell),
                    ));
                }
                Ty::Record(id) | Ty::Ref(id) | Ty::Boxed(id) => {
                    let implementation = self.dyn_impl(trait_id, id, node)?;
                    let pointer = if matches!(value.native, Ty::Record(_)) {
                        let borrowed = self.borrow_value(value);
                        if matches!(borrowed.native, Ty::Record(_)) {
                            // A copyable record stays a value when borrowed; the view
                            // points at its storage, which must outlive the view.
                            Atom::Address(Box::new(self.dyn_place(borrowed, node)?), id)
                        } else {
                            borrowed.atom
                        }
                    } else {
                        value.atom
                    };
                    return Ok(Rhs::MakeDyn(trait_id, implementation, pointer));
                }
                native => {
                    let Some(cell) = self.builtin_cell(native, node)? else {
                        return self.unsupported(node, "a trait object over this value");
                    };
                    let implementation = self.dyn_impl(trait_id, cell, node)?;
                    let place = self.dyn_place(value, node)?;
                    return Ok(Rhs::MakeDyn(
                        trait_id,
                        implementation,
                        Atom::Address(Box::new(place), cell),
                    ));
                }
            }
        }
        // A plain value placed where a Box is expected moves into a new box (Q86).
        if matches!(target, Ty::DynBox(_) | Ty::Boxed(_))
            && matches!(value.native, Ty::Record(_))
            && !value.temporary
        {
            let value = self.freeze_owned(value, node)?;
            return self.transfer(value, target, node);
        }
        if let (Ty::Boxed(id), Ty::Record(source)) = (target, value.native)
            && id == source
        {
            return Ok(Rhs::BoxNew(id, value.atom, value.native));
        }
        if let Ty::DynBox(trait_id) = target {
            match value.native {
                Ty::Record(id)
                    if matches!(
                        self.compiler.layouts[id].kind,
                        LayoutKind::List(_) | LayoutKind::Map(MapLayout { mutable: false, .. })
                    ) =>
                {
                    let cell = self.builtin_cell(value.native, node)?.unwrap();
                    let implementation = self.dyn_impl(trait_id, cell, node)?;
                    let value = self.temporary(
                        Ty::Record(cell),
                        Rhs::Aggregate(cell, None, vec![(value.atom, value.native)]),
                        node,
                    );
                    return Ok(Rhs::DynBoxNew(trait_id, implementation, value.atom, false));
                }
                Ty::Record(id) | Ty::Boxed(id) => {
                    let implementation = self.dyn_impl(trait_id, id, node)?;
                    let boxed = matches!(value.native, Ty::Boxed(_));
                    return Ok(Rhs::DynBoxNew(trait_id, implementation, value.atom, boxed));
                }
                native => {
                    if let Some(cell) = self.builtin_cell(native, node)? {
                        let implementation = self.dyn_impl(trait_id, cell, node)?;
                        let value = self.freeze_owned(value, node)?;
                        return Ok(Rhs::DynBoxNew(trait_id, implementation, value.atom, false));
                    }
                }
            }
        }
        if matches!((target, value.native), (Ty::Ref(a), Ty::Record(b)) if a == b) {
            return Ok(Rhs::Copy(self.borrow_value(value).atom));
        }
        // A view of a copyable value, such as the rest of an immutable list, becomes an owned
        // copy that shares its counted parts.
        if matches!((target, value.native), (Ty::Record(a), Ty::Ref(b)) if a == b)
            && crate::types::copies(&value.ty)
        {
            return Ok(Rhs::Copy(value.atom));
        }
        if target != value.native {
            if matches!(
                (target, value.native),
                (Ty::Pointer(true), Ty::Pointer(false))
            ) {
                return Ok(Rhs::Copy(value.atom));
            }
            return self.error(node, "E_NATIVE_OWNERSHIP", "This expression mixes an owned value and a returned borrow; its native result layout is not implemented yet.");
        }
        if matches!(target, Ty::Record(_) | Ty::Boxed(_)) && !crate::types::copies(&value.ty)
            || matches!(target, Ty::Python | Ty::DynBox(_) | Ty::Tasks)
            || target.owns() && value.temporary
        {
            return Ok(Rhs::Take(value.atom));
        }
        Ok(Rhs::Copy(value.atom))
    }
    fn env_layout(&mut self, fields: &[(Ty, bool)]) -> usize {
        let key = format!("env {fields:?}");
        if let Some(id) = self.compiler.layout_ids.get(&key) {
            return *id;
        }
        let id = self.compiler.layouts.len();
        self.compiler.layout_ids.insert(key, id);
        self.compiler.layouts.push(Layout {
            is_error: false,
            source: Type::named("void"),
            fields: fields
                .iter()
                .enumerate()
                .map(|(i, (ty, _))| Field {
                    name: i.to_string(),
                    ty: *ty,
                })
                .collect(),
            kind: LayoutKind::Env(fields.iter().map(|(_, by_ref)| *by_ref).collect()),
        });
        id
    }
    /// Adds parameters for a closure's code and returns their values.
    fn closure_parameters(
        &mut self,
        parameters: &[(Ty, Passing)],
        signature: &FunctionType,
    ) -> Vec<Value> {
        let mut values = vec![];
        for ((ty, passing), parameter) in parameters.iter().zip(&signature.parameters) {
            let slot = self.local(*ty, true, ty.owns() && *passing != Passing::Mut);
            let mode = if matches!(ty, Ty::Ref(_)) {
                Passing::Borrow
            } else {
                *passing
            };
            self.parameters.push((slot, *ty, mode));
            values.push(Value {
                native: *ty,
                atom: Atom::Slot(slot),
                ty: parameter.ty.clone(),
                ..Value::unit()
            });
        }
        values
    }
    fn finish_closure_code(builder: Builder, layout: usize, env: Option<usize>) -> usize {
        let function = builder.finish_function();
        let compiler = function.0;
        let index = compiler.functions.len();
        compiler.functions.push(Some(function.1));
        compiler.closures.push(ClosureCode {
            layout,
            function: index,
            env,
        });
        compiler.closures.len() - 1
    }
    /// Turns a function value known at compile time into a run-time closure.
    fn materialize(&mut self, value: Value, function_type: &Type, node: &Node) -> Result<Value> {
        let Some(callable) = value.callable.clone() else {
            return Ok(value);
        };
        let mut owners = BTreeSet::new();
        protect(&value, &mut owners);
        let function_type = self.mono.apply(function_type);
        let native = self.native_type(&function_type, node)?;
        let Ty::Record(layout) = native else {
            return self.unsupported(node, "this function value layout");
        };
        let LayoutKind::Closure { parameters, result } = self.compiler.layouts[layout].kind.clone()
        else {
            return self.unsupported(node, "this function value layout");
        };
        let Type::Function(signature) = function_type else {
            unreachable!()
        };
        let module = self.module;
        let (code, env) = match *callable {
            FunctionValue::Named => {
                let functions = match &value.ty {
                    Type::Function(f) => vec![f.clone()],
                    Type::Overloaded(fs) => fs.clone(),
                    _ => vec![],
                };
                let candidates = functions
                    .into_iter()
                    .filter(|f| f.parameters.len() == parameters.len())
                    .collect::<Vec<_>>();
                let chosen = candidates
                    .iter()
                    .find(|f| {
                        f.parameters
                            .iter()
                            .zip(&signature.parameters)
                            .all(|(a, b)| a.ty.same_type(&b.ty))
                    })
                    .or(candidates.first())
                    .cloned();
                let Some(chosen) = chosen else {
                    return self.unsupported(node, "this function value");
                };
                let Some(target) = chosen.callable.clone() else {
                    return self.unsupported(node, "a function value without a known target");
                };
                let captures = if let Callable::Declaration { id, .. } = &target {
                    self.compiler
                        .local_functions
                        .get(id)
                        .map(|(_, _, names)| names.clone())
                        .unwrap_or_default()
                } else {
                    vec![]
                };
                let mut captured = vec![];
                for (name, assigned) in captures {
                    let Some(value) = self.lookup(&name) else {
                        return self
                            .unsupported(node, "a local function whose captures are out of scope");
                    };
                    let value = if value.callable.is_some() {
                        self.materialize(value.clone(), &value.ty.clone(), node)?
                    } else {
                        value
                    };
                    let value = if assigned {
                        value
                    } else {
                        self.borrow_value(value)
                    };
                    protect(&value, &mut owners);
                    captured.push((name, value, assigned));
                }
                let env = if captured.is_empty() {
                    None
                } else {
                    Some(
                        self.env_layout(
                            &captured
                                .iter()
                                .map(|(_, value, assigned)| (value.native, *assigned))
                                .collect::<Vec<_>>(),
                        ),
                    )
                };
                let key = format!("named {target:?} {layout} {env:?}");
                let code = if let Some(code) = self.compiler.closure_codes.get(&key) {
                    *code
                } else {
                    let mono = self.mono.clone();
                    let mut builder = Builder::new(
                        &mut *self.compiler,
                        module,
                        mono,
                        "closure".into(),
                        result,
                        false,
                    );
                    if let Some(env) = env {
                        let env_slot = builder.local(Ty::Ref(env), true, false);
                        builder
                            .parameters
                            .push((env_slot, Ty::Ref(env), Passing::Borrow));
                        for (i, (name, value, by_ref)) in captured.iter().enumerate() {
                            builder.scopes[0].names.insert(
                                name.clone(),
                                Value {
                                    native: value.native,
                                    atom: Atom::EnvField(
                                        Box::new(Atom::Slot(env_slot)),
                                        i,
                                        value.native,
                                        *by_ref,
                                    ),
                                    ty: value.ty.clone(),
                                    ..Value::unit()
                                },
                            );
                        }
                    }
                    let arguments = builder.closure_parameters(&parameters, &signature);
                    let selected = ResolvedCall {
                        target,
                        callee: None,
                        overload: 0,
                        signature: chosen.clone(),
                        type_arguments: BTreeMap::new(),
                        arguments: vec![],
                        evaluation_order: vec![],
                        receiver: None,
                        partial: false,
                    };
                    let value = builder.invoke(
                        &selected,
                        arguments,
                        None,
                        node,
                        Some((*signature.result).clone()),
                    )?;
                    builder.return_value(value, node)?;
                    let code = Self::finish_closure_code(builder, layout, env);
                    self.compiler.closure_codes.insert(key, code);
                    code
                };
                (
                    code,
                    env.map(|env| {
                        (
                            env,
                            captured
                                .into_iter()
                                .map(|(_, value, assigned)| (value.atom, value.native, assigned))
                                .collect(),
                        )
                    }),
                )
            }
            FunctionValue::Partial {
                module: origin,
                call,
                captures,
                receiver,
            } => {
                let mut captured = captures.iter().flatten().cloned().collect::<Vec<_>>();
                if captured.iter().any(|value| value.callable.is_some()) {
                    return self.unsupported(node, "a partial call that captures a function value");
                }
                // The closure reaches the receiver through a pointer; it never owns it.
                let receiver = receiver.map(|receiver| self.borrow_value(*receiver));
                captured.extend(receiver.clone());
                let fields = captured
                    .iter()
                    .map(|value| (value.native, false))
                    .collect::<Vec<_>>();
                let env = self.env_layout(&fields);
                let key = format!(
                    "partial {:?} {:?} {layout} {env}",
                    call.callee, call.evaluation_order
                );
                let code = if let Some(code) = self.compiler.closure_codes.get(&key) {
                    *code
                } else {
                    let mono = self.mono.clone();
                    let mut builder = Builder::new(
                        &mut *self.compiler,
                        origin,
                        mono,
                        "closure".into(),
                        result,
                        false,
                    );
                    let env_slot = builder.local(Ty::Ref(env), true, false);
                    builder
                        .parameters
                        .push((env_slot, Ty::Ref(env), Passing::Borrow));
                    let mut provided = builder
                        .closure_parameters(&parameters, &signature)
                        .into_iter();
                    let mut field = 0;
                    let mut complete = vec![];
                    let this = receiver.as_ref().map(|receiver| {
                        let field = fields.len() - 1;
                        let (ty, _) = fields[field];
                        Value {
                            native: ty,
                            atom: Atom::EnvField(Box::new(Atom::Slot(env_slot)), field, ty, false),
                            ty: receiver.ty.clone(),
                            ..Value::unit()
                        }
                    });
                    for (index, argument) in call.arguments.iter().enumerate() {
                        let value = match argument {
                            Argument::Supplied { hole: true, .. } => provided.next().unwrap(),
                            Argument::Supplied { hole: false, .. } => {
                                let (ty, _) = fields[field];
                                let source = captured[field].ty.clone();
                                field += 1;
                                Value {
                                    native: ty,
                                    atom: Atom::EnvField(
                                        Box::new(Atom::Slot(env_slot)),
                                        field - 1,
                                        ty,
                                        false,
                                    ),
                                    ty: source,
                                    ..Value::unit()
                                }
                            }
                            Argument::Default => builder.default_argument(
                                &call,
                                index,
                                node,
                                &mut complete,
                                this.as_ref(),
                            )?,
                        };
                        complete.push(value);
                    }
                    let value = builder.invoke(
                        &call,
                        complete,
                        this,
                        node,
                        Some((*signature.result).clone()),
                    )?;
                    builder.return_value(value, node)?;
                    let code = Self::finish_closure_code(builder, layout, Some(env));
                    self.compiler.closure_codes.insert(key, code);
                    code
                };
                let values = captured
                    .into_iter()
                    .map(|value| (value.atom, value.native, false))
                    .collect();
                (code, Some((env, values)))
            }
            FunctionValue::Lambda {
                module: origin,
                node: lambda,
                captures,
            } => {
                let moves = lambda
                    .children
                    .iter()
                    .any(|child| child.kind == K::Modifier && self.text(child) == "move");
                let body = lambda.children.last().unwrap();
                let source = &self.compiler.project.modules[origin].parsed.source;
                let assigned = body
                    .descendants()
                    .filter(|n| n.kind == K::Assignment && n.children[0].kind == K::Name)
                    .map(|n| source.slice(n.children[0].span).to_owned())
                    .collect::<BTreeSet<_>>();
                let mut names = vec![];
                let mut fields = vec![];
                let mut values = vec![];
                for (name, value) in &captures {
                    // Captured function values travel as closures too.
                    let value = if value.callable.is_some() {
                        self.materialize(value.clone(), &value.ty.clone(), node)?
                    } else {
                        value.clone()
                    };
                    let by_ref = !moves && assigned.contains(name);
                    let value = if by_ref || moves {
                        value
                    } else {
                        self.borrow_value(value)
                    };
                    if by_ref && !matches!(value.atom, Atom::Slot(_) | Atom::Field(..)) {
                        return self
                            .unsupported(node, "a closure that assigns this captured value");
                    }
                    names.push((name.clone(), value.ty.clone()));
                    fields.push((value.native, by_ref));
                    values.push((value.atom, value.native, by_ref));
                }
                let env = self.env_layout(&fields);
                let key = format!("lambda {origin} {:?} {layout} {env}", lambda.span);
                let code = if let Some(code) = self.compiler.closure_codes.get(&key) {
                    *code
                } else {
                    let mut mono = self.mono.clone();
                    if let Some(Type::Function(schema)) =
                        self.compiler.types[origin].get(&lambda.span)
                    {
                        for (parameter, actual) in
                            schema.parameters.iter().zip(&signature.parameters)
                        {
                            mono.bind(&parameter.ty, &actual.ty);
                        }
                        mono.bind(&schema.result, &signature.result);
                    }
                    let mut builder = Builder::new(
                        &mut *self.compiler,
                        origin,
                        mono,
                        "lambda".into(),
                        result,
                        false,
                    );
                    let env_slot = builder.local(Ty::Ref(env), true, false);
                    builder
                        .parameters
                        .push((env_slot, Ty::Ref(env), Passing::Borrow));
                    for (i, ((name, ty), (native, by_ref))) in names.iter().zip(&fields).enumerate()
                    {
                        let field = Value {
                            native: *native,
                            atom: Atom::EnvField(
                                Box::new(Atom::Slot(env_slot)),
                                i,
                                *native,
                                *by_ref,
                            ),
                            ty: ty.clone(),
                            ..Value::unit()
                        };
                        builder.scopes[0].names.insert(name.clone(), field);
                    }
                    let arguments = builder.closure_parameters(&parameters, &signature);
                    if let Some(list) = lambda.child(K::LambdaParameters) {
                        for (parameter, value) in list.children.iter().zip(arguments) {
                            let name = parameter
                                .child(K::Name)
                                .map(|node| builder.text(node))
                                .unwrap_or("")
                                .to_owned();
                            builder.scopes[0].names.insert(name, value);
                        }
                    }
                    let value = builder.expression(body)?;
                    if !ends_with_return(&builder.code) {
                        builder.return_value(value, body)?;
                    }
                    let code = Self::finish_closure_code(builder, layout, Some(env));
                    self.compiler.closure_codes.insert(key, code);
                    code
                };
                (code, Some((env, values)))
            }
            FunctionValue::Sequence { .. } | FunctionValue::Range { .. } => {
                return self.unsupported(node, "a lazy sequence used as a function");
            }
        };
        let mut closure = self.temporary(native, Rhs::ClosureNew(layout, code, env), node);
        closure.keepalive = owners;
        Ok(closure)
    }
    fn python_operation(
        &mut self,
        node: &Node,
        operation: crate::checker::PythonOperation,
    ) -> Result<Value> {
        let module = operation.module.as_ref().map(|name| {
            let modules = &mut self.compiler.python_modules;
            modules.iter().position(|m| m == name).unwrap_or_else(|| {
                modules.push(name.clone());
                modules.len() - 1
            })
        });
        let mut atoms = vec![];
        if module.is_none() {
            let span = operation.receiver.expect("a Python receiver");
            let expression = self.compiler.nodes[self.module][&span];
            let value = self.expression(expression)?;
            if value.native != Ty::Python {
                return self.unsupported(node, "this Python receiver");
            }
            atoms.push(value.atom);
        }
        let mut arguments = vec![];
        for (name, span) in &operation.arguments {
            let expression = self.compiler.nodes[self.module][span];
            let value = self.expression(expression)?;
            // A PythonValue argument is lent to the call; copying it would release it twice.
            let value = if value.native == Ty::Python {
                value
            } else if value.callable.is_some() {
                // A callback crosses as a counted closure that Python may keep.
                let ty = self.ty(expression);
                self.materialize(value, &ty, expression)?
            } else {
                self.freeze(value, expression)?
            };
            arguments.push((value.native, name.clone()));
            atoms.push(value.atom);
        }
        let result = self.native_type(&self.mono.apply(&operation.result), node)?;
        let site = self.compiler.python_sites.len();
        self.compiler.python_sites.push(PythonSite {
            module,
            attribute: operation.attribute.clone(),
            call: operation.call,
            arguments,
            result,
        });
        Ok(self.temporary(result, Rhs::Python(site, atoms), node))
    }
    /// For `name = name + a + b`, the pieces a and b, when none can see or change `name`.
    /// A piece spelled `value.text()` where value is a built-in scalar: its
    /// digits can go straight into the target String.
    fn scalar_text_piece(&mut self, piece: &Node) -> Result<Option<(Ty, Atom)>> {
        if piece.kind != K::Call || piece.children.len() != 1 || piece.children[0].kind != K::Member
        {
            return Ok(None);
        }
        let callee = &piece.children[0];
        if self.text(&callee.children[1]) != "text" {
            return Ok(None);
        }
        let receiver = &callee.children[0];
        let Type::Named(name, arguments) = self.ty(receiver) else {
            return Ok(None);
        };
        if !arguments.is_empty() {
            return Ok(None);
        }
        let native = match name.as_str() {
            "Int" => Ty::Int,
            "Nat" => Ty::Nat,
            "double" => Ty::Double,
            "Bool" => Ty::Bool,
            _ => return Ok(None),
        };
        let value = self.expression(receiver)?;
        let value = self.freeze(value, receiver)?;
        Ok(Some((native, value.atom)))
    }
    fn appended_pieces<'n>(&self, name: &str, value: &'n Node) -> Option<Vec<&'n Node>> {
        let mut pieces = vec![];
        let mut current = ungroup_node(value);
        while current.kind == K::Binary && self.text(&current.children[1]) == "+" {
            pieces.push(&current.children[2]);
            current = ungroup_node(&current.children[0]);
        }
        if pieces.is_empty() || current.kind != K::Name || self.text(current) != name {
            return None;
        }
        pieces.reverse();
        let unsafe_piece = |node: &Node| {
            let reads_target = node.kind == K::Name && self.text(node) == name;
            // Only a closure call could change the variable midway.
            let runs_closure = matches!(node.kind, K::Call | K::Pipe)
                && self
                    .checked()
                    .resolved_calls
                    .get(&node.span)
                    .is_none_or(|call| {
                        matches!(call.target, Callable::Indirect | Callable::Closure(_))
                    });
            reads_target || runs_closure
        };
        let safe = pieces
            .iter()
            .all(|piece| !piece.descendants().any(unsafe_piece));
        safe.then_some(pieces)
    }
    fn is_closure(&self, ty: Ty) -> Option<usize> {
        match ty {
            Ty::Record(id) | Ty::Ref(id)
                if matches!(self.compiler.layouts[id].kind, LayoutKind::Closure { .. }) =>
            {
                Some(id)
            }
            _ => None,
        }
    }
    fn closure_call(
        &mut self,
        closure: Value,
        arguments: Vec<Value>,
        node: &Node,
    ) -> Result<Value> {
        let Some(layout) = self.is_closure(closure.native) else {
            return self.unsupported(node, "this call target");
        };
        let LayoutKind::Closure { parameters, result } = self.compiler.layouts[layout].kind.clone()
        else {
            unreachable!()
        };
        let mut atoms = vec![];
        let mut owners = BTreeSet::new();
        protect(&closure, &mut owners);
        for (value, (ty, passing)) in arguments.into_iter().zip(&parameters) {
            protect(&value, &mut owners);
            let value = if value.callable.is_some() {
                let source = self.compiler.source_type(*ty);
                self.materialize(value, &source, node)?
            } else if *passing == Passing::Move {
                self.freeze_owned(value, node)?
            } else {
                self.borrow_value(value)
            };
            let value = if value.native == *ty {
                value
            } else {
                let rhs = self.transfer(value, *ty, node)?;
                self.temporary(*ty, rhs, node)
            };
            atoms.push(value.atom);
        }
        let mut value = self.temporary(result, Rhs::ClosureCall(layout, closure.atom, atoms), node);
        if self.compiler.holds_borrow(result, &mut BTreeSet::new()) {
            value.keepalive = owners;
        }
        Ok(value)
    }
    fn dyn_impl(&mut self, trait_id: usize, concrete: usize, node: &Node) -> Result<usize> {
        let LayoutKind::Trait(info) = self.compiler.layouts[trait_id].kind.clone() else {
            unreachable!()
        };
        if let Some(index) = info.impls.iter().position(|i| i.concrete == concrete) {
            return Ok(index);
        }
        if matches!(self.compiler.layouts[concrete].kind, LayoutKind::Cell) {
            return self.builtin_impl(trait_id, concrete, node);
        }
        if !matches!(
            self.compiler.layouts[concrete].kind,
            LayoutKind::Class | LayoutKind::Enum(_) | LayoutKind::BigInt
        ) {
            return self.unsupported(node, "a trait object over a built-in value");
        }
        let actual = self.compiler.layouts[concrete].source.clone();
        let mut methods = vec![];
        for method in &info.methods {
            let mut builder = Builder::new(
                &mut *self.compiler,
                self.module,
                Mono::default(),
                "vtable".into(),
                method.result,
                false,
            );
            let receiver = builder.local(Ty::Ref(concrete), true, false);
            builder
                .parameters
                .push((receiver, Ty::Ref(concrete), Passing::Borrow));
            let receiver = Value {
                native: Ty::Ref(concrete),
                atom: Atom::Slot(receiver),
                ty: actual.clone(),
                ..Value::unit()
            };
            let mut arguments = vec![];
            for (ty, passing) in &method.parameters {
                let slot = builder.local(*ty, true, false);
                builder.parameters.push((slot, *ty, *passing));
                arguments.push(Value {
                    native: *ty,
                    atom: Atom::Slot(slot),
                    ty: builder.compiler.source_type(*ty),
                    ..Value::unit()
                });
            }
            let result_type = if (method.name == "items" && matches!(method.result, Ty::DynBox(_)))
                || (method.name == "next"
                    && matches!(method.result,Ty::Record(id) if matches!(builder.compiler.layouts[id].kind,LayoutKind::Step(_))))
            {
                builder
                    .compiler
                    .methods(&actual, &method.name)
                    .into_iter()
                    .find(|f| f.parameters.is_empty())
                    .map(|f| *f.result)
                    .unwrap_or_else(|| builder.compiler.source_type(method.result))
            } else {
                builder.compiler.source_type(method.result)
            };
            let value =
                builder.method_call(receiver, &method.name, arguments, result_type, node)?;
            if matches!(builder.compiler.layouts[concrete].kind, LayoutKind::BigInt) {
                builder.return_value(value, node)?;
                let (compiler, function) = builder.finish_function();
                methods.push(compiler.functions.len());
                compiler.functions.push(Some(function));
                continue;
            }
            if method.name == "next"
                && let Ty::Record(option) = method.result
                && (matches!(builder.compiler.layouts[option].kind, LayoutKind::Step(_))
                    || matches!(&builder.compiler.layouts[option].source, Type::Named(name, _) if name == "Option"))
            {
                let value = builder.erased_step(value, option, node)?;
                builder.return_value(value, node)?;
                let (compiler, function) = builder.finish_function();
                methods.push(compiler.functions.len());
                compiler.functions.push(Some(function));
                continue;
            }
            if method.name == "items"
                && let Ty::DynBox(iterator) = method.result
            {
                let value = builder.scoped_cursor(value, iterator, node)?;
                builder.return_value(value, node)?;
                let (compiler, function) = builder.finish_function();
                methods.push(compiler.functions.len());
                compiler.functions.push(Some(function));
                continue;
            }
            let index = builder
                .code
                .iter()
                .rev()
                .find_map(|statement| match statement {
                    Statement::Set(_, Rhs::Call(index, _), _)
                    | Statement::Evaluate(Rhs::Call(index, _), _) => Some(*index),
                    _ => None,
                })
                .ok_or_else(|| LocatedDiagnostic {
                    module: self.module,
                    diagnostic: Diagnostic::new(
                        "E_NATIVE_CALL",
                        format!(
                            "No concrete {} method was found for this trait object.",
                            method.name
                        ),
                        node.span,
                    ),
                })?;
            if value.native != method.result {
                return self.error(node, "E_NATIVE_OWNERSHIP", format!("{actual}.{} returns a borrow where the trait promises an owned value; a trait object cannot carry that.", method.name));
            }
            methods.push(index);
        }
        let LayoutKind::Trait(info) = &mut self.compiler.layouts[trait_id].kind else {
            unreachable!()
        };
        info.impls.push(DynImpl { concrete, methods });
        Ok(info.impls.len() - 1)
    }
    /// Copy a stored copyable item, or retain an owned noncopyable step's owner.
    fn erased_step(&mut self, item: Value, option: usize, node: &Node) -> Result<Value> {
        if item.native == Ty::Record(option) {
            return Ok(item);
        }
        if item.native == Ty::Ref(option)
            && !matches!(self.compiler.layouts[option].kind, LayoutKind::Step(_))
            && let Type::Named(_, arguments) = &self.compiler.layouts[option].source
            && arguments.first().is_some_and(crate::types::copies)
        {
            let payload = self.compiler.layouts[option].kind.cases().unwrap()[0].fields[0];
            let output = self.temporary(
                Ty::Record(option),
                Rhs::Aggregate(option, Some(1), vec![]),
                node,
            );
            let present = self.temporary(
                Ty::Bool,
                Rhs::Test(Test::Equal(
                    Ty::Int,
                    Atom::Tag(Box::new(item.atom.clone())),
                    Atom::Int(1),
                )),
                node,
            );
            let Atom::Slot(slot) = output.atom else {
                unreachable!()
            };
            self.code.push(Statement::If(
                present.atom,
                vec![Statement::Set(
                    slot,
                    Rhs::Aggregate(
                        option,
                        Some(0),
                        vec![(Atom::Payload(Box::new(item.atom), 0, 0, payload), payload)],
                    ),
                    self.location(node),
                )],
                vec![],
            ));
            return Ok(output);
        }
        if let Ty::Ref(concrete) = item.native
            && let Type::Named(_, arguments) = &self.compiler.layouts[concrete].source
            && arguments
                .first()
                .is_some_and(|item| !crate::types::copies(item))
            && !matches!(self.compiler.layouts[concrete].kind, LayoutKind::Step(_))
        {
            let payload = self.compiler.layouts[concrete].kind.cases().unwrap()[0].fields[0];
            let view = self.compiler.layouts[option].kind.cases().unwrap()[0].fields[0];
            if let (Ty::Record(id), Ty::Ref(view_id)) = (payload, view)
                && id == view_id
            {
                // A stored Option of noncopyable items is lent for one step; the
                // returned Step points into the cursor's storage and has no owner.
                let cases = self.compiler.layouts[option].kind.cases().unwrap();
                let some = cases.iter().position(|case| case.name == "Some");
                let none = cases.iter().position(|case| case.name == "None");
                let (Some(some), Some(none)) = (some, none) else {
                    return self.unsupported(node, "this cursor's Option cases");
                };
                let output = self.temporary(
                    Ty::Record(option),
                    Rhs::Aggregate(option, Some(none), vec![]),
                    node,
                );
                let present = self.temporary(
                    Ty::Bool,
                    Rhs::Test(Test::Equal(
                        Ty::Int,
                        Atom::Tag(Box::new(item.atom.clone())),
                        Atom::Int(1),
                    )),
                    node,
                );
                let address = Atom::Address(
                    Box::new(Atom::Payload(Box::new(item.atom.clone()), 0, 0, payload)),
                    id,
                );
                let (_, code) = self.isolated(|builder| {
                    let Atom::Slot(slot) = output.atom else {
                        unreachable!()
                    };
                    builder.code.push(Statement::Set(
                        slot,
                        Rhs::Aggregate(option, Some(some), vec![(address, view)]),
                        builder.location(node),
                    ));
                    Ok(Value::unit())
                })?;
                self.code.push(Statement::If(present.atom, code, vec![]));
                return Ok(output);
            }
        }
        let Ty::Record(source) = item.native else {
            return self.error(node,"E_NATIVE_ITERATOR_ITEM","An erased cursor returning a stored Option needs copyable items; a noncopyable item has no retained owner. Return an owned Option instead.");
        };
        let payload = self.compiler.layouts[source].kind.cases().unwrap()[0].fields[0];
        let view = self.compiler.layouts[option].kind.cases().unwrap()[0].fields[0];
        let compatible = matches!((payload,view), (Ty::Record(a) | Ty::Boxed(a),Ty::Ref(b)) | (Ty::DynBox(a),Ty::Dyn(b)) if a == b);
        if !compatible {
            return self.unsupported(node, "this cursor's borrowed item representation");
        }
        let output = self.temporary(
            Ty::Record(option),
            Rhs::Aggregate(option, Some(1), vec![]),
            node,
        );
        let present = self.temporary(
            Ty::Bool,
            Rhs::Test(Test::Equal(
                Ty::Int,
                Atom::Tag(Box::new(item.atom.clone())),
                Atom::Int(1),
            )),
            node,
        );
        let (_, code) = self.isolated(|builder| {
            let owner = builder.temporary(
                Ty::Owner,
                Rhs::OwnerNew(
                    Atom::Payload(Box::new(item.atom.clone()), 0, 0, payload),
                    payload,
                ),
                node,
            );
            let value = Atom::OwnerValue(Box::new(owner.atom.clone()), payload, view);
            let Atom::Slot(slot) = output.atom else {
                unreachable!()
            };
            builder.code.push(Statement::Set(
                slot,
                Rhs::StepSome(option, value, view, owner.atom),
                builder.location(node),
            ));
            Ok(Value::unit())
        })?;
        self.code.push(Statement::If(present.atom, code, vec![]));
        Ok(output)
    }
    /// Keep a borrowed cursor and any storage supplied for a newly created result
    /// together until its traversal ends. The iterable itself remains borrowed.
    fn scoped_cursor(&mut self, mut cursor: Value, iterator: usize, node: &Node) -> Result<Value> {
        if let Ty::Ref(concrete) = cursor.native {
            let key = format!("cursor view {concrete}");
            let cell = if let Some(id) = self.compiler.layout_ids.get(&key) {
                *id
            } else {
                let id = self.compiler.layouts.len();
                self.compiler.layout_ids.insert(key, id);
                self.compiler.layouts.push(Layout {
                    source: Type::named("hidden.cursor"),
                    is_error: false,
                    fields: vec![Field {
                        name: "cursor".into(),
                        ty: Ty::Ref(concrete),
                    }],
                    kind: LayoutKind::Cell,
                });
                id
            };
            let owner = cursor
                .keepalive
                .iter()
                .find(|slot| {
                    self.result_boxes.contains(slot)
                        && self.slot_type(**slot) == Ty::Boxed(concrete)
                })
                .copied();
            if let Some(owner) = owner {
                let view = self.dyn_impl(iterator, cell, node)?;
                let owned = self.dyn_impl(iterator, concrete, node)?;
                return Ok(self.temporary(
                    Ty::DynBox(iterator),
                    Rhs::Builtin(
                        format!("cursorBox.{iterator}.{owned}.{view}.{cell}"),
                        Ty::Boxed(concrete),
                        vec![cursor.atom, Atom::Slot(owner)],
                    ),
                    node,
                ));
            }
            cursor = self.temporary(
                Ty::Record(cell),
                Rhs::Aggregate(cell, None, vec![(cursor.atom, Ty::Ref(concrete))]),
                node,
            );
        }
        let target = Ty::DynBox(iterator);
        let rhs = self.transfer(cursor, target, node)?;
        Ok(self.temporary(target, rhs, node))
    }

    /// A built-in value behind `dyn Text` is seen as the one-field cell that holds it; each
    /// table entry is a small function that renders the field.
    fn builtin_impl(&mut self, trait_id: usize, cell: usize, node: &Node) -> Result<usize> {
        let LayoutKind::Trait(info) = self.compiler.layouts[trait_id].kind.clone() else {
            unreachable!()
        };
        let inner = self.compiler.layouts[cell].fields[0].ty;
        let mut methods = vec![];
        for method in &info.methods {
            if !method.parameters.is_empty()
                && !matches!(method.name.as_str(), "multiply" | "rightMultiply")
            {
                return self
                    .unsupported(node, &format!("{} on a built-in trait object", method.name));
            }
            let mut builder = Builder::new(
                &mut *self.compiler,
                self.module,
                Mono::default(),
                "vtable".into(),
                method.result,
                false,
            );
            let receiver = builder.local(Ty::Ref(cell), true, false);
            builder
                .parameters
                .push((receiver, Ty::Ref(cell), Passing::Borrow));
            let mut arguments = vec![];
            for (ty, passing) in &method.parameters {
                let slot = builder.local(*ty, true, ty.owns() && *passing != Passing::Mut);
                builder.parameters.push((slot, *ty, *passing));
                arguments.push(Value {
                    atom: Atom::Slot(slot),
                    native: *ty,
                    ty: builder.compiler.source_type(*ty),
                    ..Value::unit()
                });
            }
            let value = Value {
                native: inner,
                atom: Atom::Field(Box::new(Atom::Slot(receiver)), 0, inner),
                ty: builder.compiler.source_type(inner),
                ..Value::unit()
            };
            let result = if matches!(method.name.as_str(), "multiply" | "rightMultiply")
                && matches!(inner, Ty::Int | Ty::Double)
                && arguments.len() == 1
                && arguments[0].native == inner
                && method.result == inner
            {
                let other = arguments.pop().unwrap();
                let (left, right) = if method.name == "multiply" {
                    (value.atom, other.atom)
                } else {
                    (other.atom, value.atom)
                };
                builder.temporary(inner, Rhs::Binary("*".into(), inner, left, right), node)
            } else if method.name == "items"
                && let Ty::DynBox(iterator) = method.result
            {
                builder.collection_cursor(value, iterator, node)?
            } else if method.name == "next"
                && matches!(builder.compiler.layouts[cell].source, Type::Cursor(..))
            {
                builder.collection_next(Atom::Slot(receiver), cell, method.result, node)?
            } else if matches!(method.name.as_str(), "text" | "debug") {
                builder.render(value, method.name == "debug", node)?
            } else {
                builder.method_call(
                    value,
                    &method.name,
                    vec![],
                    builder.compiler.source_type(method.result),
                    node,
                )?
            };
            builder.return_value(result, node)?;
            let (compiler, function) = builder.finish_function();
            methods.push(compiler.functions.len());
            compiler.functions.push(Some(function));
        }
        let LayoutKind::Trait(info) = &mut self.compiler.layouts[trait_id].kind else {
            unreachable!()
        };
        info.impls.push(DynImpl {
            concrete: cell,
            methods,
        });
        Ok(info.impls.len() - 1)
    }
    /// The cell layout for a built-in value that a trait object can point at.
    /// sorted(key = f) and sort(key = f) pair each item with its key, sort the pairs by key, and then read the
    /// items back in that order. Items must be copyable, since each pair holds its item by value (Q135).
    fn sorted_pairs(
        &mut self,
        sequence: Value,
        key: Value,
        item: Ty,
        node: &Node,
    ) -> Result<(Value, usize, Ty)> {
        self.require_copyable_pair_item(item, node)?;
        let Type::Function(signature) = key.ty.clone() else {
            return self.unsupported(node, "a sort key that is not a function");
        };
        let key_type = self.mono.apply(&signature.result);
        let key_native = self.native_type(&key_type, node)?;
        self.ensure_sort_comparator(key_native, node)?;
        let item_type = self.compiler.source_type(item);
        let pair_type = Type::Tuple(vec![key_type.clone(), item_type]);
        let pair = self.native_type(&pair_type, node)?;
        let Ty::Record(tuple) = pair else {
            return self.unsupported(node, "this sort key");
        };
        let pairs_type = Type::applied("List", vec![pair_type]);
        let pairs_native = self.native_type(&pairs_type, node)?;
        let Ty::Record(pairs_list) = pairs_native else {
            return self.unsupported(node, "this sort key");
        };
        let mut pairs = self.temporary(pairs_native, Rhs::ListNew(pairs_list, vec![]), node);
        self.for_each(sequence, node, |builder, value, cell| {
            let value = builder.freeze(value, node)?;
            let key_value = builder.callback(&key, vec![value.clone()], node)?;
            let key_value = builder.freeze(key_value, node)?;
            let pair_value = builder.temporary(
                pair,
                Rhs::Aggregate(
                    tuple,
                    None,
                    vec![(key_value.atom, key_native), (value.atom, item)],
                ),
                node,
            );
            builder.append_list(pairs_list, &mut pairs, pair_value, cell, node)
        })?;
        let sorted = self.temporary(
            pairs_native,
            Rhs::Builtin(format!("sortedBy.{pairs_list}"), Ty::Unit, vec![pairs.atom]),
            node,
        );
        Ok((sorted, tuple, item))
    }
    /// sorted(key) over items that do not copy: the keys sort with each item's position,
    /// then the result shares the source List's cells in that order.
    fn sorted_by_position(
        &mut self,
        sequence: Value,
        key: Value,
        list: usize,
        element: Ty,
        node: &Node,
    ) -> Result<Value> {
        let result = Ty::Record(list);
        let source = if sequence.native == result {
            sequence
        } else {
            self.collect_list(sequence, element, node)?
        };
        let source = self.freeze(source, node)?;
        let Type::Function(signature) = key.ty.clone() else {
            return self.unsupported(node, "a sort key that is not a function");
        };
        let key_type = self.mono.apply(&signature.result);
        let key_native = self.native_type(&key_type, node)?;
        self.ensure_sort_comparator(key_native, node)?;
        let pair_type = Type::Tuple(vec![key_type, Type::named("Int")]);
        let pair = self.native_type(&pair_type, node)?;
        let Ty::Record(tuple) = pair else {
            return self.unsupported(node, "this sort key");
        };
        let pairs_native = self.native_type(&Type::applied("List", vec![pair_type]), node)?;
        let Ty::Record(pairs_list) = pairs_native else {
            return self.unsupported(node, "this sort key");
        };
        let mut pairs = self.temporary(pairs_native, Rhs::ListNew(pairs_list, vec![]), node);
        self.for_each(source.clone(), node, |builder, value, cell| {
            let Some(IterationOwner::Collection(_, _, index)) = cell else {
                return builder.unsupported(node, "sorting this sequence by a key");
            };
            let value = builder.borrow_value(value);
            let key_value = builder.callback(&key, vec![value], node)?;
            let key_value = builder.freeze(key_value, node)?;
            let pair_value = builder.temporary(
                pair,
                Rhs::Aggregate(
                    tuple,
                    None,
                    vec![(key_value.atom, key_native), (index, Ty::Int)],
                ),
                node,
            );
            builder.append_list(pairs_list, &mut pairs, pair_value, None, node)
        })?;
        let sorted = self.temporary(
            pairs_native,
            Rhs::Builtin(format!("sortedBy.{pairs_list}"), Ty::Unit, vec![pairs.atom]),
            node,
        );
        let mut output = self.temporary(result, Rhs::ListNew(list, vec![]), node);
        self.for_each(sorted, node, |builder, value, _| {
            let position = builder.temporary(
                Ty::Int,
                Rhs::Copy(Atom::Field(Box::new(value.atom.clone()), 1, Ty::Int)),
                node,
            );
            let item = Value {
                keepalive: BTreeSet::new(),
                atom: Atom::ListItem(
                    Box::new(source.atom.clone()),
                    Box::new(position.atom.clone()),
                    element,
                ),
                native: element,
                ty: builder.compiler.source_type(element),
                temporary: false,
                callable: None,
            };
            let item = builder.borrow_value(item);
            let cell = IterationOwner::Collection(list, source.atom.clone(), position.atom);
            builder.append_list(list, &mut output, item, Some(cell), node)
        })?;
        Ok(output)
    }
    /// The List, pair and tuple layouts of a List of pairs, as enumerate and zip build them (Q134).
    fn pair_list_layout(&mut self, result: Ty, node: &Node) -> Result<(usize, Ty, usize)> {
        let Ty::Record(list) = result else {
            return self.unsupported(node, "this pair list");
        };
        let LayoutKind::List(pair) = self.compiler.layouts[list].kind else {
            return self.unsupported(node, "this pair list");
        };
        let Ty::Record(tuple) = pair else {
            return self.unsupported(node, "this pair list");
        };
        Ok((list, pair, tuple))
    }
    fn require_copyable_pair_item(&mut self, item: Ty, node: &Node) -> Result<()> {
        if crate::types::copies(&self.compiler.source_type(item)) {
            Ok(())
        } else {
            self.unsupported(node, "a pair of owned items")
        }
    }
    /// Collects a sequence into a new List of its items, as toList does (Q134).
    fn collect_list(&mut self, sequence: Value, element: Ty, node: &Node) -> Result<Value> {
        let list_type = Type::applied("List", vec![self.compiler.source_type(element)]);
        let list = self.native_type(&list_type, node)?;
        let Ty::Record(id) = list else {
            return self.unsupported(node, "this List");
        };
        let mut output = self.temporary(list, Rhs::ListNew(id, vec![]), node);
        self.for_each(sequence, node, |builder, value, cell| {
            builder.append_list(id, &mut output, value, cell, node)
        })?;
        Ok(output)
    }
    /// The start of a sum (zero) or a product (one) for a number type (Q133).
    fn zero_or_one(&mut self, ty: Ty, zero: bool, node: &Node) -> Result<Rhs> {
        let atom = match (ty, zero) {
            (Ty::Int, true) => Atom::Int(0),
            (Ty::Int, false) => Atom::Int(1),
            (Ty::Nat, true) => Atom::Nat(0),
            (Ty::Nat, false) => Atom::Nat(1),
            (Ty::Double, true) => Atom::Double(0.0),
            (Ty::Double, false) => Atom::Double(1.0),
            _ => return self.unsupported(node, "a sum or product of this type"),
        };
        Ok(Rhs::Copy(atom))
    }
    /// The characters of a String, each a one-character String, as a List made by split("").
    fn string_characters(&mut self, text: Atom, node: &Node) -> Result<Value> {
        let list = self.native_type(&Type::applied("List", vec![Type::named("String")]), node)?;
        Ok(self.temporary(
            list,
            Rhs::Builtin(
                "split".into(),
                Ty::String,
                vec![text, Atom::Text(String::new())],
            ),
            node,
        ))
    }
    fn collection_cursor(
        &mut self,
        collection: Value,
        iterator: usize,
        node: &Node,
    ) -> Result<Value> {
        let Ty::Record(source) = collection.native else {
            return self.unsupported(node, "this collection cursor");
        };
        let item = match self.compiler.layouts[source].kind {
            LayoutKind::List(item) => self.compiler.source_type(item),
            LayoutKind::Map(info) if info.style == MapStyle::Map => {
                self.compiler.layouts[info.item].source.clone()
            }
            LayoutKind::Map(info) => self.compiler.source_type(info.key),
            _ => return self.unsupported(node, "this collection cursor"),
        };
        let key = format!("collection cursor {source}");
        let cursor = if let Some(id) = self.compiler.layout_ids.get(&key) {
            *id
        } else {
            let id = self.compiler.layouts.len();
            self.compiler.layout_ids.insert(key, id);
            self.compiler.layouts.push(Layout {
                source: Type::Cursor(Box::new(item), false),
                is_error: false,
                fields: vec![
                    Field {
                        name: "collection".into(),
                        ty: collection.native,
                    },
                    Field {
                        name: "position".into(),
                        ty: Ty::Int,
                    },
                ],
                kind: LayoutKind::Cell,
            });
            id
        };
        let value = self.temporary(
            Ty::Record(cursor),
            Rhs::Aggregate(
                cursor,
                None,
                vec![
                    (collection.atom, collection.native),
                    (Atom::Int(0), Ty::Int),
                ],
            ),
            node,
        );
        let rhs = self.transfer(value, Ty::DynBox(iterator), node)?;
        Ok(self.temporary(Ty::DynBox(iterator), rhs, node))
    }

    fn collection_next(
        &mut self,
        receiver: Atom,
        cursor: usize,
        result: Ty,
        node: &Node,
    ) -> Result<Value> {
        let source_ty = self.compiler.layouts[cursor].fields[0].ty;
        let Ty::Record(source) = source_ty else {
            unreachable!()
        };
        let collection = Atom::Field(Box::new(receiver.clone()), 0, source_ty);
        let position = Atom::Field(Box::new(receiver), 1, Ty::Int);
        let Ty::Record(option) = result else {
            unreachable!()
        };
        let output = self.temporary(result, Rhs::Aggregate(option, Some(1), vec![]), node);
        let unordered = matches!(self.compiler.layouts[source].kind,LayoutKind::Map(info) if info.style!=MapStyle::SortedSet);
        let bound = if unordered {
            Atom::MapCapacity(Box::new(collection.clone()))
        } else {
            Atom::CollectionLength(Box::new(collection.clone()))
        };
        let (condition, condition_code) = self.isolated(|builder| {
            Ok(builder.temporary(
                Ty::Bool,
                Rhs::Binary("<".into(), Ty::Int, position.clone(), bound),
                node,
            ))
        })?;
        let id = self.next_loop;
        self.next_loop += 1;
        let (_, body) = self.isolated(|builder| {
            let index = builder.temporary(Ty::Int, Rhs::Copy(position.clone()), node);
            let next = builder.temporary(
                Ty::Int,
                Rhs::Binary("+".into(), Ty::Int, index.atom.clone(), Atom::Int(1)),
                node,
            );
            builder.code.push(Statement::Store(
                position.clone(),
                Ty::Int,
                Rhs::Copy(next.atom),
                builder.location(node),
            ));
            if unordered {
                builder.code.push(Statement::If(
                    Atom::MapOccupied(Box::new(collection.clone()), Box::new(index.atom.clone())),
                    vec![],
                    vec![Statement::Continue(id, vec![])],
                ));
            }
            let (atom, native) = match builder.compiler.layouts[source].kind {
                LayoutKind::List(item) => (
                    Atom::ListItem(
                        Box::new(collection.clone()),
                        Box::new(index.atom.clone()),
                        item,
                    ),
                    item,
                ),
                LayoutKind::Map(info) if info.style == MapStyle::Map => (
                    Atom::MapItem(
                        Box::new(collection.clone()),
                        Box::new(index.atom.clone()),
                        info.item,
                    ),
                    Ty::Record(info.item),
                ),
                LayoutKind::Map(info) => (
                    Atom::SetItem(
                        Box::new(collection.clone()),
                        Box::new(index.atom.clone()),
                        info.key,
                        info.style == MapStyle::SortedSet,
                    ),
                    info.key,
                ),
                _ => unreachable!(),
            };
            let step = matches!(builder.compiler.layouts[option].kind, LayoutKind::Step(_));
            let value = if !step
                && let Ty::Record(tuple) = native
                && matches!(builder.compiler.layouts[tuple].kind, LayoutKind::Tuple)
            {
                let fields = builder.compiler.layouts[tuple]
                    .fields
                    .iter()
                    .enumerate()
                    .map(|(i, field)| (Atom::Field(Box::new(atom.clone()), i, field.ty), field.ty))
                    .collect();
                builder.temporary(native, Rhs::Aggregate(tuple, None, fields), node)
            } else {
                Value {
                    atom,
                    native,
                    ty: builder.compiler.source_type(native),
                    ..Value::unit()
                }
            };
            let rhs = if step {
                let view = builder.borrow_value(value);
                let owner = builder.temporary(
                    Ty::Owner,
                    Rhs::OwnerNew(collection.clone(), source_ty),
                    node,
                );
                Rhs::StepSome(option, view.atom, view.native, owner.atom)
            } else {
                Rhs::Aggregate(option, Some(0), vec![(value.atom, value.native)])
            };
            let Atom::Slot(slot) = output.atom else {
                unreachable!()
            };
            builder
                .code
                .push(Statement::Set(slot, rhs, builder.location(node)));
            builder.code.push(Statement::Break(id, vec![]));
            Ok(Value::unit())
        })?;
        self.code.push(Statement::Loop {
            id,
            condition: condition_code,
            value: condition.atom,
            body,
            step: vec![],
        });
        Ok(output)
    }

    fn builtin_cell(&mut self, native: Ty, node: &Node) -> Result<Option<usize>> {
        let builtin = match native {
            Ty::Int | Ty::Double | Ty::Bool | Ty::String | Ty::C(_) => true,
            Ty::Record(id) => matches!(
                self.compiler.layouts[id].kind,
                LayoutKind::List(_) | LayoutKind::Map(MapLayout { mutable: false, .. })
            ),
            _ => false,
        };
        if !builtin {
            return Ok(None);
        }
        let source = self.compiler.source_type(native);
        match self.native_type(&Type::applied("Box", vec![source]), node)? {
            Ty::Boxed(cell) => Ok(Some(cell)),
            _ => Ok(None),
        }
    }
    /// A `Box[T]` holding a built-in value is read through its single field.
    fn unbox_cell(&self, value: Value) -> Value {
        if let Ty::Boxed(id) | Ty::Ref(id) = value.native
            && matches!(self.compiler.layouts[id].kind, LayoutKind::Cell)
        {
            let inner = self.compiler.layouts[id].fields[0].ty;
            return Value {
                keepalive: value.keepalive,
                native: inner,
                atom: Atom::Field(Box::new(value.atom), 0, inner),
                ty: self.compiler.source_type(inner),
                temporary: false,
                callable: None,
            };
        }
        value
    }
    fn dyn_call(
        &mut self,
        trait_id: usize,
        name: &str,
        receiver: Value,
        arguments: Vec<Value>,
        node: &Node,
    ) -> Result<Value> {
        let LayoutKind::Trait(info) = self.compiler.layouts[trait_id].kind.clone() else {
            unreachable!()
        };
        let Some(index) = info
            .methods
            .iter()
            .position(|m| m.name == name && m.parameters.len() == arguments.len())
        else {
            return self.unsupported(node, &format!("trait-object method {name}"));
        };
        let method = info.methods[index].clone();
        let mut owners = receiver.keepalive.clone();
        let mut atoms = vec![];
        for (value, (ty, passing)) in arguments.into_iter().zip(&method.parameters) {
            owners.extend(value.keepalive.iter().copied());
            let value = if *passing == Passing::Move {
                self.freeze_owned(value, node)?
            } else {
                self.borrow_value(value)
            };
            let value = if value.native == *ty {
                value
            } else {
                let rhs = self.transfer(value, *ty, node)?;
                self.temporary(*ty, rhs, node)
            };
            atoms.push(value.atom);
        }
        let mut value = self.temporary(
            method.result,
            Rhs::DynCall(trait_id, index, receiver.atom, atoms),
            node,
        );
        if self
            .compiler
            .holds_borrow(value.native, &mut BTreeSet::new())
        {
            value.keepalive = owners;
        }
        Ok(value)
    }
    fn promote_owners(&mut self, owners: &BTreeSet<Slot>, depth: usize) {
        for slot in owners {
            if slot.global {
                if let Some(original) = self
                    .scopes
                    .iter()
                    .position(|scope| scope.owned.contains(slot))
                    && original > depth
                {
                    self.scopes[original].owned.retain(|s| s != slot);
                    self.scopes[depth].owned.push(*slot);
                }
                continue;
            }
            let original = self.locals[slot.id].scope;
            if original > depth
                && original < self.scopes.len()
                && self.scopes[original].owned.contains(slot)
            {
                self.scopes[original].owned.retain(|s| s != slot);
                if !self.scopes[depth].owned.contains(slot) {
                    self.scopes[depth].owned.push(*slot);
                }
                self.locals[slot.id].scope = depth;
            }
        }
    }
    /// Stores an evaluated copyable value in a name, a field or an array slot (Q130).
    fn store_into(&mut self, target: &Node, value: Value, node: &Node) -> Result<()> {
        match target.kind {
            K::Name => {
                let Some(Value {
                    atom: Atom::Slot(slot),
                    ..
                }) = self.lookup(self.text(target))
                else {
                    return self.unsupported(node, "a tuple target that captures a name");
                };
                self.assign(slot, value, node)
            }
            K::Member => {
                let place = self.member(target, true)?;
                let rhs = self.transfer(value, place.native, node)?;
                self.code.push(Statement::Store(
                    place.atom,
                    place.native,
                    rhs,
                    self.location(node),
                ));
                Ok(())
            }
            K::Index if crate::types::is_tensor(&self.ty(&target.children[0])) => {
                self.tensor_store(target, value, node)
            }
            // A class that has MutableIndexable[Key, Item] writes through setAt(key, value).
            K::Index if self.has_trait(&target.children[0], "MutableIndexable") => {
                self.indexable_store(node, target, value)
            }
            K::Index => {
                let (item, element) = self.array_item(target)?;
                let rhs = self.transfer(value, element, node)?;
                self.code
                    .push(Statement::Store(item, element, rhs, self.location(node)));
                Ok(())
            }
            _ => self.unsupported(node, "this tuple target"),
        }
    }
    fn assign(&mut self, slot: Slot, value: Value, node: &Node) -> Result<()> {
        let depth = if slot.global {
            0
        } else {
            self.locals[slot.id].scope
        };
        self.promote_owners(&value.keepalive, depth);
        let rhs = self.transfer(value, self.slot_type(slot), node)?;
        self.code
            .push(Statement::Set(slot, rhs, self.location(node)));
        Ok(())
    }
    fn return_value(&mut self, mut value: Value, node: &Node) -> Result<()> {
        if self.owned_result.is_some()
            && self.task_scopes.is_empty()
            && let Ty::Ref(id) = value.native
            && let Atom::Address(place, _) = &value.atom
            && let Atom::Slot(slot) = place.as_ref()
            && !slot.global
            && self.locals[slot.id].owned
            && self.locals[slot.id].ty == Ty::Record(id)
            && !value.keepalive.iter().any(|owner| {
                owner != slot
                    && !owner.global
                    && self.locals[owner.id].owned
                    && !crate::types::copies(&self.compiler.source_type(self.locals[owner.id].ty))
                    && !self.result_boxes.contains(owner)
            })
        {
            // The exact local record can move into this caller's result box.
            let slot = *slot;
            value.native = Ty::Record(id);
            value.atom = Atom::Slot(slot);
            value.keepalive.remove(&slot);
            value.temporary = false;
        }
        if matches!(value.native, Ty::Ref(_))
            && value.keepalive.iter().any(|slot| {
                !slot.global
                    && self.locals[slot.id].owned
                    && !crate::types::copies(&self.compiler.source_type(self.locals[slot.id].ty))
                    && !self.result_boxes.contains(slot)
            })
        {
            return self.error(node, "E_NATIVE_OWNERSHIP", "This returned reference may point into an owned local value that is released when the function returns.");
        }
        if matches!(value.native, Ty::Dyn(_))
            || matches!(value.native, Ty::Record(_))
                && self
                    .compiler
                    .holds_borrow(value.native, &mut BTreeSet::new())
        {
            let mut calls = vec![];
            for slot in value.keepalive.intersection(&self.result_boxes) {
                let Some(call) = self.result_box_calls.get(slot) else {
                    return self.error(node,"E_NATIVE_OWNERSHIP","This view retains a newly owned result; returning its storage through an erased view is not implemented yet.");
                };
                calls.push(*call);
            }
            if !calls.is_empty() {
                self.compiler
                    .view_escapes
                    .push((self.module, node.span, calls));
            }
        }
        let value = if value.callable.is_some() && self.is_closure(self.result).is_some() {
            let rhs = self.transfer(value, self.result, node)?;
            self.temporary(self.result, rhs, node)
        } else {
            value
        };
        let value = if let Ty::Ref(id) = self.result
            && matches!(self.compiler.layouts[id].kind, LayoutKind::Union(_, _))
            && !matches!(value.native, Ty::Ref(source) | Ty::Record(source) if source == id)
        {
            let owners = value.keepalive.clone();
            let rhs = self.transfer(value, Ty::Record(id), node)?;
            let mut value = self.temporary(Ty::Record(id), rhs, node);
            value.keepalive = owners;
            value
        } else {
            value
        };
        let value = match (self.owned_result, self.result, value.native) {
            (Some(out), Ty::Ref(a), Ty::Record(b)) if a == b => {
                // A new value on a borrowing function's path lives in the caller's box.
                let ty = value.ty.clone();
                let value = self.freeze_owned(value, node)?;
                self.code.push(Statement::Set(
                    out,
                    Rhs::BoxNew(a, value.atom, value.native),
                    self.location(node),
                ));
                Value {
                    native: Ty::Ref(a),
                    atom: Atom::Slot(out),
                    ty,
                    ..Value::unit()
                }
            }
            (Some(out), Ty::Ref(a), Ty::Ref(b)) if a == b => {
                // A view into a box from a nested call: hand the box to this caller.
                let boxes = value
                    .keepalive
                    .intersection(&self.result_boxes)
                    .copied()
                    .collect::<Vec<_>>();
                for slot in boxes {
                    // A call whose view is returned writes straight into this caller's box,
                    // which keeps a tail call in tail position.
                    if !forward_box(&mut self.code, slot, out) {
                        self.code.push(Statement::Set(
                            out,
                            Rhs::Take(Atom::Slot(slot)),
                            self.location(node),
                        ));
                    }
                }
                value
            }
            (None, Ty::Ref(a), Ty::Record(b)) if a == b => {
                return self.error(node,"E_NATIVE_OWNERSHIP","This function mixes owned and borrowed returns. Its native result layout is not implemented yet.");
            }
            _ => value,
        };
        let value = if value.native != self.result && value.native != Ty::Unit
            || self.result != Ty::Unit
                && value.native == Ty::Unit
                && matches!(self.result, Ty::Record(id) if matches!(self.compiler.layouts[id].kind, LayoutKind::Union(_, _)))
        {
            let rhs = self.transfer(value, self.result, node)?;
            self.temporary(self.result, rhs, node)
        } else {
            value
        };
        self.check_invariants(&value, node)?;
        self.check_ensures(&value, node)?;
        self.code
            .push(Statement::Return(value.atom, self.cleanups(0)));
        Ok(())
    }
    /// Runs the receiver's invariants just before a normal return. An Error return,
    /// including one from closing a file or joining a task, is exempt as it is for
    /// ensures. A direct write to a public field from outside the class is not
    /// checked: invariants hold at method and constructor boundaries.
    fn check_invariants(&mut self, value: &Value, node: &Node) -> Result<()> {
        if self.invariants.is_empty() || self.checking_invariants {
            return Ok(());
        }
        let mut present = None;
        match value.native {
            Ty::Record(id) | Ty::Ref(id)
                if matches!(self.compiler.layouts[id].kind, LayoutKind::Union(_, _)) =>
            {
                let LayoutKind::Union(parts, _) = self.compiler.layouts[id].kind.clone() else {
                    unreachable!()
                };
                let mut test = None;
                for (index, part) in parts.iter().enumerate() {
                    if self.compiler.is_error_type(part) {
                        continue;
                    }
                    let tag = Test::Equal(
                        Ty::Int,
                        Atom::Tag(Box::new(value.atom.clone())),
                        Atom::Int(index as i64 + 1),
                    );
                    test = Some(match test {
                        Some(previous) => Test::Or(Box::new(previous), Box::new(tag)),
                        None => tag,
                    });
                }
                match test {
                    Some(test) => {
                        present = Some(self.temporary(Ty::Bool, Rhs::Test(test), node).atom);
                    }
                    None => return Ok(()),
                }
            }
            _ if self.compiler.is_error_type(&value.ty) => return Ok(()),
            _ => {}
        }
        let (_, checks) = self.isolated(|builder| {
            builder.invariant_predicates()?;
            Ok(Value::unit())
        })?;
        match present {
            Some(condition) => self.code.push(Statement::If(condition, checks, vec![])),
            None => self.code.extend(checks),
        }
        Ok(())
    }
    fn invariant_predicates(&mut self) -> Result<()> {
        let names = self
            .scopes
            .iter_mut()
            .map(|scope| std::mem::take(&mut scope.names))
            .collect::<Vec<_>>();
        self.scopes.push(Scope {
            names: self.invariant_names.clone(),
            owned: vec![],
            finalizers: vec![],
        });
        let checking = std::mem::replace(&mut self.checking_invariants, true);
        let checked = (|| {
            for invariant in self.invariants.clone() {
                let previous = self.module;
                self.module = invariant.module;
                let value = self.expression(&invariant.condition);
                let text = self.text(&invariant.condition).to_owned();
                let failure = Statement::Evaluate(
                    Rhs::Builtin(
                        "invariantFailed".into(),
                        Ty::Unit,
                        vec![Atom::Text(format!(
                            "Invariant of {} failed in {}.{}: {}",
                            invariant.class, invariant.owner, invariant.method, text
                        ))],
                    ),
                    self.location(&invariant.condition),
                );
                self.module = previous;
                let value = value?;
                self.code
                    .push(Statement::If(value.atom, vec![], vec![failure]));
            }
            Ok(())
        })();
        self.checking_invariants = checking;
        self.finish_scope();
        for (scope, names) in self.scopes.iter_mut().zip(names) {
            scope.names = names;
        }
        checked
    }
    fn ensures_predicates(&mut self, value: Value) -> Result<()> {
        let value = self.borrow_value(value);
        self.scopes
            .last_mut()
            .unwrap()
            .names
            .insert("result".into(), value);
        for condition in self.ensures.clone() {
            let predicate = &condition.children[0];
            let value = self.expression(predicate)?;
            let failure = Statement::Evaluate(
                Rhs::Builtin("ensuresFailed".into(), Ty::Unit, vec![]),
                self.location(predicate),
            );
            self.code
                .push(Statement::If(value.atom, vec![], vec![failure]));
        }
        Ok(())
    }
    fn check_ensures(&mut self, value: &Value, node: &Node) -> Result<()> {
        if self.ensures.is_empty()
            || self.checking_ensures
            || self.compiler.is_error_type(&value.ty)
        {
            return Ok(());
        }
        // Only signature names are visible in the predicate, including when an
        // early return sits inside a block with same-named local declarations.
        let names = self
            .scopes
            .iter_mut()
            .map(|scope| std::mem::take(&mut scope.names))
            .collect::<Vec<_>>();
        self.scopes.push(Scope {
            names: self.ensures_names.clone(),
            owned: vec![],
            finalizers: vec![],
        });
        self.checking_ensures = true;
        let checked = (|| {
            if let Ty::Record(id) | Ty::Ref(id) = value.native
                && let LayoutKind::Union(parts, cases) = self.compiler.layouts[id].kind.clone()
                && parts.iter().any(|p| self.compiler.is_error_type(p))
            {
                let successes = parts
                    .iter()
                    .filter(|p| !self.compiler.is_error_type(p))
                    .count();
                for (case, part) in parts.iter().enumerate() {
                    if self.compiler.is_error_type(part) {
                        continue;
                    }
                    let present = self.temporary(
                        Ty::Bool,
                        Rhs::Test(Test::Equal(
                            Ty::Int,
                            Atom::Tag(Box::new(value.atom.clone())),
                            Atom::Int(case as i64 + 1),
                        )),
                        node,
                    );
                    let (_, code) = self.isolated(|builder| {
                        let result = if successes == 1 {
                            let native = cases[case].fields.first().copied().unwrap_or(Ty::Unit);
                            Value {
                                native,
                                atom: if native == Ty::Unit {
                                    Atom::Unit
                                } else {
                                    Atom::Payload(Box::new(value.atom.clone()), case, 0, native)
                                },
                                ty: part.clone(),
                                ..Value::unit()
                            }
                        } else {
                            value.clone()
                        };
                        builder.ensures_predicates(result)?;
                        Ok(Value::unit())
                    })?;
                    self.code.push(Statement::If(present.atom, code, vec![]));
                }
                Ok(())
            } else {
                self.ensures_predicates(value.clone())
            }
        })();
        self.finish_scope();
        for (scope, names) in self.scopes.iter_mut().zip(names) {
            scope.names = names;
        }
        self.checking_ensures = false;
        checked
    }
    fn class_choices(&self, native: Ty) -> Option<Vec<usize>> {
        match native {
            Ty::Ref(id) if matches!(self.compiler.layouts[id].kind, LayoutKind::Class) => {
                Some(vec![id])
            }
            Ty::Record(id) => match &self.compiler.layouts[id].kind {
                LayoutKind::ClassView(cases) => Some(
                    cases
                        .iter()
                        .filter_map(|case| match case.fields[0] {
                            Ty::Ref(id) => Some(id),
                            _ => None,
                        })
                        .collect(),
                ),
                _ => None,
            },
            _ => None,
        }
    }

    fn class_view(&mut self, base: &Type, classes: BTreeSet<usize>, node: &Node) -> Result<Ty> {
        let key = format!("class view {base} {classes:?}");
        if let Some(id) = self.compiler.layout_ids.get(&key) {
            return Ok(Ty::Record(*id));
        }
        let Ty::Record(base_id) = self.native_type(base, node)? else {
            return self.unsupported(node, "a superclass view over this type");
        };
        let fields = self.compiler.layouts[base_id].fields.clone();
        let id = self.compiler.layouts.len();
        self.compiler.layout_ids.insert(key, id);
        self.compiler.layouts.push(Layout {
            source: base.clone(),
            is_error: false,
            fields,
            kind: LayoutKind::ClassView(
                classes
                    .into_iter()
                    .map(|class| Case {
                        name: format!("class{class}"),
                        fields: vec![Ty::Ref(class)],
                        names: vec![None],
                    })
                    .collect(),
            ),
        });
        Ok(Ty::Record(id))
    }

    fn merge_class_views(&mut self, a: Ty, b: Ty, base: &Type, node: &Node) -> Result<Option<Ty>> {
        if let (Some(a), Some(b)) = (self.class_choices(a), self.class_choices(b)) {
            return self
                .class_view(base, a.into_iter().chain(b).collect(), node)
                .map(Some);
        }
        Ok(None)
    }

    fn borrowed_class_result(
        &mut self,
        declared: usize,
        sources: Vec<(Ty, Type)>,
        node: &Node,
    ) -> Result<Ty> {
        let base = self.compiler.source_type(Ty::Ref(declared));
        let mut classes = BTreeSet::new();
        for (native, formal) in sources {
            if !matches!(&base,Type::Named(name,_) if self.compiler.ancestor(&formal,name).is_some_and(|ancestor| ancestor==base))
            {
                continue;
            }
            let native = match native {
                Ty::Record(id) if matches!(self.compiler.layouts[id].kind, LayoutKind::Class) => {
                    Ty::Ref(id)
                }
                other => other,
            };
            if let Some(choices) = self.class_choices(native) {
                classes.extend(choices);
            }
        }
        if classes.len() > 1 {
            self.class_view(&base, classes, node)
        } else {
            Ok(Ty::Ref(classes.first().copied().unwrap_or(declared)))
        }
    }

    fn node_native(&mut self, node: &Node) -> Result<Ty> {
        if let Some(ty) = self.checked().autoboxed.get(&node.span).cloned() {
            return self.native_type(&self.mono.apply(&ty), node);
        }
        if let Some(ty) = self.checked().widened.get(&node.span).cloned() {
            return self.native_type(&self.mono.apply(&ty), node);
        }
        if matches!(self.ty(node), Type::Union(_)) {
            return self.native_type(&self.ty(node), node);
        }

        if node.kind == K::Name
            && let Some(value) = self.lookup(self.text(node))
        {
            if matches!(value.native, Ty::Ref(_))
                && self.checked().owned_returns.contains(&node.span)
            {
                return self.native_type(&self.ty(node), node);
            }
            return Ok(value.native);
        }
        if node.kind == K::Group {
            return self.node_native(&node.children[0]);
        }
        if node.kind == K::Block {
            self.scopes.push(Scope {
                finalizers: vec![],
                names: BTreeMap::new(),
                owned: vec![],
            });
            let result = (|| {
                let mut native = Ty::Unit;
                for child in &node.children {
                    if child.kind == K::Binding {
                        if let Some(pattern) = child.child(K::PatternName) {
                            let init = child
                                .children
                                .iter()
                                .rev()
                                .find(|n| n.kind != K::Otherwise)
                                .unwrap();
                            let native = self.node_native(init)?;
                            let mut value = Value::unit();
                            value.native = native;
                            value.ty = self.ty(init);
                            let name = self.text(pattern).to_owned();
                            self.scopes.last_mut().unwrap().names.insert(name, value);
                        }
                        native = Ty::Unit;
                    } else {
                        native = self.node_native(child)?;
                    }
                }
                Ok(native)
            })();
            self.scopes.pop();
            return result;
        }
        if node.kind == K::Member {
            let owner = self.node_native(&node.children[0])?;
            if let Ty::Record(id) | Ty::Ref(id) = owner {
                let name = self.text(&node.children[1]);
                if let Some(field) = self.compiler.layouts[id]
                    .fields
                    .iter()
                    .find(|f| f.name == name)
                {
                    let field = Value {
                        native: field.ty,
                        ty: self.compiler.source_type(field.ty),
                        ..Value::unit()
                    };
                    return Ok(
                        if matches!(owner, Ty::Ref(_))
                            || matches!(owner, Ty::Record(id) if matches!(self.compiler.layouts[id].kind, LayoutKind::ClassView(_)))
                        {
                            self.borrow_value(field).native
                        } else {
                            field.native
                        },
                    );
                }
            }
        }
        if node.kind == K::If && node.children.len() == 3 {
            let a = self.node_native(&node.children[1])?;
            let b = self.node_native(&node.children[2])?;
            return if a == Ty::Unit {
                Ok(b)
            } else if b == Ty::Unit || a == b {
                Ok(a)
            } else if let Some(id) = mixed_view(a, b) {
                // The owned branch's value moves into a box; both branches give a view.
                Ok(Ty::Ref(id))
            } else if let Some(view) = self.merge_class_views(a, b, &self.ty(node), node)? {
                Ok(view)
            } else {
                self.error(
                    node,
                    "E_NATIVE_OWNERSHIP",
                    "These branches need a mixed owned/borrowed native layout.",
                )
            };
        }
        let ty = self.ty(node);
        let result = self.native_type(&ty, node)?;
        if let Ty::Record(id) = result
            && let Some(call) = self.checked().resolved_calls.get(&node.span)
            && !crate::types::copies(&self.ty(node))
            && self.compiler.returns_borrow(&call.target)
        {
            let call = self.checked().resolved_calls[&node.span].clone();
            let mut sources = vec![];
            for (argument, parameter) in call.arguments.iter().zip(&call.signature.parameters) {
                if let Argument::Supplied { expression, .. } = argument
                    && let Some(argument) = self.compiler.nodes[self.module]
                        .get(expression)
                        .copied()
                        .cloned()
                {
                    sources.push((self.node_native(&argument)?, self.mono.apply(&parameter.ty)));
                }
            }
            if let Some(receiver) = call.receiver
                && let Some(receiver) = self.compiler.nodes[self.module]
                    .get(&receiver)
                    .copied()
                    .cloned()
            {
                let formal =
                    call.signature
                        .receiver
                        .as_deref()
                        .cloned()
                        .or_else(|| match &call.target {
                            Callable::Declaration {
                                owner: Some(owner), ..
                            } => Some(Type::named(owner)),
                            _ => None,
                        });
                if let Some(formal) = formal {
                    sources.push((self.node_native(&receiver)?, self.mono.apply(&formal)));
                }
            }
            return self.borrowed_class_result(id, sources, node);
        }
        Ok(result)
    }
    /// Owned temporaries made by a finished statement die at once, unless a binding in this
    /// scope still reaches them: as its value, a borrowed owner, or something it captured.
    fn drop_statement_temporaries(&mut self, before: usize) {
        let scope = self.scopes.last().unwrap();
        if scope.owned.len() <= before {
            return;
        }
        let mut protected = BTreeSet::new();
        for value in scope.names.values() {
            protect(value, &mut protected);
        }
        let (keep, release): (Vec<Slot>, Vec<Slot>) = scope.owned[before..]
            .iter()
            .partition(|slot| slot.global || protected.contains(slot));
        if release.is_empty() {
            return;
        }
        let scope = self.scopes.last_mut().unwrap();
        scope.owned.truncate(before);
        scope.owned.extend(keep);
        self.code.push(Statement::Drop(release));
    }
    fn cleanups(&self, depth: usize) -> Vec<Cleanup> {
        self.scopes[depth..]
            .iter()
            .rev()
            .flat_map(|scope| {
                scope
                    .finalizers
                    .iter()
                    .rev()
                    .cloned()
                    .chain(scope.owned.iter().rev().copied().map(Cleanup::Drop))
            })
            .collect()
    }
    fn finish_scope(&mut self) {
        let scope = self.scopes.pop().unwrap();
        if !scope.owned.is_empty() {
            self.code.push(Statement::Drop(scope.owned));
        }
    }
    fn isolated<F>(&mut self, build: F) -> Result<(Value, Vec<Statement>)>
    where
        F: FnOnce(&mut Self) -> Result<Value>,
    {
        let outer = std::mem::take(&mut self.code);
        self.scopes.push(Scope {
            finalizers: vec![],
            names: BTreeMap::new(),
            owned: vec![],
        });
        let result = build(self);
        self.finish_scope();
        let code = std::mem::replace(&mut self.code, outer);
        result.map(|value| (value, code))
    }
    fn lookup(&self, name: &str) -> Option<Value> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.names.get(name))
            .cloned()
    }

    fn one(&mut self, node: &Node) -> Result<Value> {
        let ty = self.ty(node);
        let native = self.native_type(&ty, node)?;
        if ty == Type::named("BigInt") {
            return Ok(self.temporary(
                native,
                Rhs::Builtin(
                    "bigint.literal".into(),
                    native,
                    vec![Atom::Text("1".into())],
                ),
                node,
            ));
        }
        let atom = match native {
            Ty::Nat => Atom::Nat(1),
            Ty::Double => Atom::Double(1.0),
            _ => Atom::Int(1),
        };
        Ok(Value {
            keepalive: BTreeSet::new(),
            native,
            atom,
            ty,
            temporary: false,
            callable: None,
        })
    }

    fn literal(&mut self, node: &Node) -> Result<Value> {
        let text = self.text(node);
        let ty = self.ty(node);
        if ty == Type::named("BigInt") {
            let native = self.native_type(&ty, node)?;
            let literal = Atom::Text(text.replace('_', ""));
            return Ok(self.temporary(
                native,
                Rhs::Builtin("bigint.literal".into(), native, vec![literal]),
                node,
            ));
        }
        let atom = if text.starts_with('"') {
            Atom::Text(
                crate::strings::decode(&text[1..text.len() - 1], node.span.start + 1, false)
                    .map_err(|diagnostic| LocatedDiagnostic {
                        module: self.module,
                        diagnostic,
                    })?,
            )
        } else if matches!(text, "true" | "false") {
            Atom::Bool(text == "true")
        } else if self.native_type(&ty, node)? == Ty::Nat {
            Atom::Nat(
                text.replace('_', "")
                    .parse()
                    .map_err(|_| LocatedDiagnostic {
                        module: self.module,
                        diagnostic: Diagnostic::new(
                            "E_INTEGER_LITERAL_RANGE",
                            "This literal does not fit in a 64-bit Nat.",
                            node.span,
                        ),
                    })?,
            )
        } else if self.native_type(&ty, node)? == Ty::Double {
            let text = text.replace('_', "");
            let parsed: std::result::Result<f64, ()> =
                if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
                    u128::from_str_radix(hex, 16)
                        .map(|value| value as f64)
                        .map_err(|_| ())
                } else {
                    text.parse::<f64>().map_err(|_| ())
                };
            Atom::Double(parsed.map_err(|_| LocatedDiagnostic {
                module: self.module,
                diagnostic: Diagnostic::new(
                    "E_NATIVE_LITERAL",
                    "Cannot read this double literal.",
                    node.span,
                ),
            })?)
        } else {
            let text = text.replace('_', "");
            let parsed =
                if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
                    i64::from_str_radix(hex, 16)
                } else {
                    text.parse()
                };
            Atom::Int(parsed.map_err(|_| {
                LocatedDiagnostic {
                    module: self.module,
                    diagnostic: Diagnostic::new(
                        "E_INTEGER_LITERAL_RANGE",
                        "This literal does not fit in a 64-bit Int.",
                        node.span,
                    )
                    .with_hint("Use an explicit BigInt annotation for larger values."),
                }
            })?)
        };
        Ok(Value {
            keepalive: BTreeSet::new(),
            native: self.native_type(&ty, node)?,
            atom,
            ty,
            temporary: false,
            callable: None,
        })
    }

    fn expression(&mut self, node: &Node) -> Result<Value> {
        if self.checked().theorem_evidence.contains(&node.span) {
            return Ok(Value::unit());
        }
        if let Some(ty) = self.checked().borrowed_views.get(&node.span).cloned()
            && self.boxing.insert(node.span)
        {
            let source = self.expression(node);
            self.boxing.remove(&node.span);
            let mut source = source?;
            let target = self.native_type(&self.mono.apply(&ty), node)?;
            if source.native == target {
                source.ty = self.mono.apply(&ty);
                return Ok(source);
            }
            let mut owners = BTreeSet::new();
            protect(&source, &mut owners);
            let local_before = self.locals.len();
            let global_before = self.compiler.globals.len();
            let rhs = self.transfer(source, target, node)?;
            owners.extend(
                self.scopes
                    .iter()
                    .flat_map(|scope| &scope.owned)
                    .filter(|slot| {
                        if slot.global {
                            slot.id >= global_before
                        } else {
                            slot.id >= local_before
                        }
                    })
                    .copied(),
            );
            let mut value = self.temporary(target, rhs, node);
            value.ty = self.mono.apply(&ty);
            value.keepalive = owners;
            return Ok(value);
        }
        // Where the checker expected a BigInt, an Int or Nat converts to one exactly. The conversion is spelled nowhere in
        // the source, so the Int value is converted here, where it is used (Q125).
        if let Some(ty) = self.checked().widened.get(&node.span).cloned()
            && self.boxing.insert(node.span)
        {
            let value = self.expression(node);
            self.boxing.remove(&node.span);
            let value = value?;
            let target = self.native_type(&self.mono.apply(&ty), node)?;
            let operation = match value.native {
                Ty::Int => "bigint.fromInt",
                Ty::Nat => "bigint.fromNat",
                _ => return Ok(value),
            };
            return Ok(self.temporary(
                target,
                Rhs::Builtin(operation.into(), target, vec![value.atom]),
                node,
            ));
        }
        // Where the checker expected a Box, the value moves into a new box right
        // here, so every branch or item reaches its destination already boxed (Q86).
        if let Some(ty) = self.checked().autoboxed.get(&node.span).cloned()
            && self.boxing.insert(node.span)
        {
            let value = self.expression(node);
            self.boxing.remove(&node.span);
            let value = value?;
            let target = self.native_type(&self.mono.apply(&ty), node)?;
            if value.native == target {
                return Ok(value);
            }
            let rhs = self.transfer(value, target, node)?;
            return Ok(self.temporary(target, rhs, node));
        }
        match node.kind {
            K::Literal => self.literal(node),
            K::One => self.one(node),
            K::Name => {
                let name = self.text(node);
                if name == "super" {
                    let this = self.lookup("this").ok_or_else(|| LocatedDiagnostic {
                        module: self.module,
                        diagnostic: Diagnostic::new(
                            "E_SUPER_CONTEXT",
                            "No receiver for super.",
                            node.span,
                        ),
                    })?;
                    let parent = self.ty(node);
                    let native = self.compiler.layout_type(&parent).map_err(|message| {
                        LocatedDiagnostic {
                            module: self.module,
                            diagnostic: Diagnostic::new("E_NATIVE_TYPE", message, node.span),
                        }
                    })?;
                    let Ty::Record(id) = native else {
                        return self.unsupported(node, "parent receiver layout");
                    };
                    let this = self.borrow_value(this);
                    let mut value = self.temporary(
                        Ty::Ref(id),
                        Rhs::Builtin("superView".into(), native, vec![this.atom]),
                        node,
                    );
                    value.ty = parent;
                    return Ok(value);
                }
                if let Some(value) = self.lookup(name) {
                    return Ok(if self.checked().owned_returns.contains(&node.span) {
                        self.move_boxed_payload(value, node)
                    } else {
                        value
                    });
                }
                if let Some(reference) = self.checked().references.get(&node.span).cloned() {
                    return match reference {
                        Reference::ScriptBinding(id) if self.compiler.global_bindings.contains_key(&id) => Ok(self.borrow_value(self.compiler.global_bindings[&id].clone())),
                        Reference::ScriptBinding(id) => self.compiler.global_slots.get(&id).copied().map(|slot| self.borrow_value(Value { keepalive: BTreeSet::new(), native: self.slot_type(slot), atom: Atom::Slot(slot), ty: self.ty(node), temporary: false, callable: None })).ok_or_else(|| LocatedDiagnostic { module: self.module, diagnostic: Diagnostic::new("E_NATIVE_CAPTURE", "This script capture needs a closure environment that is not implemented yet.", node.span) }),
                        Reference::Constant(id) => self.constant(&id, &mut BTreeSet::new()),
                        Reference::Builtin(name) if name == "std.math.PI" || name == "std.math.E" => Ok(Value { keepalive: BTreeSet::new(), native: Ty::Double, atom: Atom::Double(if name.ends_with("PI") { std::f64::consts::PI } else { std::f64::consts::E }), ty: Type::named("double"), temporary: false, callable: None }),
                        Reference::Builtin(name) if name == "std.process.args" => {
                            let native = self.native_type(&self.ty(node), node)?;
                            let slot = *self.compiler.process_args.get_or_insert_with(|| {
                                let slot = Slot { id: self.compiler.globals.len(), global: true };
                                self.compiler.globals.push(native);
                                slot
                            });
                            Ok(Value { native, atom: Atom::Slot(slot), ty: self.ty(node), ..Value::unit() })
                        }
                        Reference::Builtin(name) if name == "std.core.None" => {
                            let native = self.native_type(&self.ty(node), node)?;
                            let Ty::Record(id) = native else { unreachable!() };
                            Ok(self.temporary(native, Rhs::Aggregate(id, Some(1), vec![]), node))
                        }
                        _ => self.function_value(node),
                    };
                }
                if name == "Zero" && self.native_type(&self.ty(node), node).ok() == Some(Ty::Nat) {
                    return Ok(Value {
                        native: Ty::Nat,
                        atom: Atom::Nat(0),
                        ty: Type::named("Nat"),
                        ..Value::unit()
                    });
                }
                if let Ok(Ty::Record(id)) = self.native_type(&self.ty(node), node)
                    && let LayoutKind::Enum(cases) | LayoutKind::Step(cases) =
                        &self.compiler.layouts[id].kind
                    && let Some(case) = cases
                        .iter()
                        .position(|case| case.name == name && case.fields.is_empty())
                {
                    return Ok(self.temporary(
                        Ty::Record(id),
                        Rhs::Aggregate(id, Some(case), vec![]),
                        node,
                    ));
                }
                self.function_value(node)
            }
            K::Group | K::Argument => self.expression(&node.children[0]),
            K::Index => {
                if crate::types::is_variable(&self.ty(&node.children[0])) {
                    return self.variable_index_value(node);
                }
                if crate::types::is_tensor(&self.ty(&node.children[0])) {
                    return self.tensor_index_value(node);
                }
                // Brackets on a String read one character as a new String (chosen 2026-11-08).
                let base = match self.ty(&node.children[0]) {
                    Type::Borrow(inner) | Type::StoredView(inner) => *inner,
                    other => other,
                };
                // `s[1:3]` and `xs[2:]`: a missing bound is the start or the end.
                if node.children[1].kind == K::Slice {
                    let owner = self.expression(&node.children[0])?;
                    let owner = self.borrow_value(owner);
                    let mut bounds = vec![];
                    for (position, bound) in node.children[1].children.iter().enumerate() {
                        bounds.push(if bound.kind == K::Hole {
                            Atom::Int(if position == 0 { 0 } else { i64::MAX })
                        } else {
                            self.expression(bound)?.atom
                        });
                    }
                    let mut atoms = vec![owner.atom];
                    atoms.extend(bounds);
                    if base == Type::named("String") {
                        return Ok(self.temporary(
                            Ty::String,
                            Rhs::Builtin("substring".into(), Ty::String, atoms),
                            node,
                        ));
                    }
                    let (Ty::Record(list) | Ty::Ref(list)) = owner.native else {
                        return self.unsupported(node, "this slice");
                    };
                    return Ok(self.temporary(
                        Ty::Record(list),
                        Rhs::Builtin(format!("listShape.slice.{list}"), Ty::Unit, atoms),
                        node,
                    ));
                }
                if base == Type::named("String") {
                    let text = self.expression(&node.children[0])?;
                    let index = self.expression(&node.children[1])?;
                    return Ok(self.temporary(
                        Ty::String,
                        Rhs::Builtin("stringAt".into(), Ty::String, vec![text.atom, index.atom]),
                        node,
                    ));
                }
                // A tuple position is a field of the tuple, read where the literal names it (Q128).
                if let Type::Tuple(_) | Type::Borrow(_) = self.ty(&node.children[0])
                    && node.children[1].kind == K::Literal
                    && let Ok(position) = self.text(&node.children[1]).parse::<usize>()
                {
                    let owner = self.expression(&node.children[0])?;
                    let owner = self.unbox_cell(owner);
                    if let Ty::Record(id) | Ty::Ref(id) = owner.native
                        && let LayoutKind::Tuple = self.compiler.layouts[id].kind
                    {
                        let field = self.compiler.layouts[id].fields[position].ty;
                        let item = Value {
                            atom: Atom::Field(Box::new(owner.atom.clone()), position, field),
                            native: field,
                            ty: self.ty(node),
                            ..Value::unit()
                        };
                        return Ok(if crate::types::copies(&item.ty) {
                            self.temporary(field, Rhs::Copy(item.atom), node)
                        } else {
                            self.borrow_value(item)
                        });
                    }
                }
                // A class that has Indexable[Key, Item] reads brackets through at(key).
                if self.has_trait(&node.children[0], "Indexable") {
                    return self.indexable_read(node);
                }
                let (item, element) = self.array_item(node)?;
                let value = Value {
                    atom: item,
                    native: element,
                    ty: self.ty(node),
                    ..Value::unit()
                };
                if crate::types::copies(&value.ty) {
                    Ok(self.temporary(element, Rhs::Copy(value.atom), node))
                } else {
                    Ok(self.borrow_value(value))
                }
            }
            K::Member if self.checked().python.contains_key(&node.span) => {
                let operation = self.checked().python[&node.span].clone();
                self.python_operation(node, operation)
            }
            K::Member if self.checked().static_references.contains_key(&node.span) => {
                self.static_read(node)
            }
            K::Member => {
                if self.text(&node.children[1]) == "Zero"
                    && self.native_type(&self.ty(node), node).ok() == Some(Ty::Nat)
                {
                    return Ok(Value {
                        native: Ty::Nat,
                        atom: Atom::Nat(0),
                        ty: Type::named("Nat"),
                        ..Value::unit()
                    });
                }
                if let Ok(Ty::Record(id)) = self.native_type(&self.ty(node), node)
                    && let LayoutKind::Enum(cases) | LayoutKind::Step(cases) =
                        &self.compiler.layouts[id].kind
                    && let Some(case) = cases.iter().position(|case| {
                        case.name == self.text(&node.children[1]) && case.fields.is_empty()
                    })
                {
                    return Ok(self.temporary(
                        Ty::Record(id),
                        Rhs::Aggregate(id, Some(case), vec![]),
                        node,
                    ));
                }
                let value = self.member(node, false)?;
                Ok(if self.checked().owned_returns.contains(&node.span) {
                    self.move_boxed_payload(value, node)
                } else {
                    value
                })
            }
            K::Comprehension => self.comprehension(node),
            K::List => {
                let Ty::Record(id) = self.native_type(&self.ty(node), node)? else {
                    unreachable!()
                };
                let LayoutKind::List(element) = self.compiler.layouts[id].kind else {
                    unreachable!()
                };
                let mut values = vec![];
                let mut owners = BTreeSet::new();
                for child in &node.children {
                    let value = self.expression(child)?;
                    owners.extend(value.keepalive.iter().copied());
                    // A list of views borrows its elements; other lists take them.
                    let value = if matches!(element, Ty::Dyn(_) | Ty::Ref(_)) {
                        let value = self.borrow_value(value);
                        if let Atom::Slot(slot) = value.atom
                            && !slot.global
                            && value.native.owns()
                        {
                            owners.insert(slot);
                        }
                        value
                    } else {
                        self.freeze_owned(value, child)?
                    };
                    let value = if value.native == element {
                        value
                    } else {
                        let rhs = self.transfer(value, element, child)?;
                        self.temporary(element, rhs, child)
                    };
                    values.push((value.atom, value.native));
                }
                let mut list = self.temporary(Ty::Record(id), Rhs::ListNew(id, values), node);
                if self.compiler.holds_borrow(element, &mut BTreeSet::new()) {
                    list.keepalive = owners;
                }
                Ok(list)
            }
            K::Tuple => {
                let native = self.native_type(&self.ty(node), node)?;
                let Ty::Record(id) = native else {
                    unreachable!()
                };
                let mut values = vec![];
                for child in &node.children {
                    let value = self.expression(child)?;
                    let value = self.freeze_owned(value, child)?;
                    values.push((value.atom, value.native));
                }
                Ok(self.temporary(native, Rhs::Aggregate(id, None, values), node))
            }
            K::Match => self.match_expression(node),
            K::Propagate => self.propagate(node),
            K::Using => self.using_expression(node),
            K::Unsafe => self.expression(&node.children[0]),
            // A proof is erased: the condition it proves is not evaluated again (Q115).
            K::ProofWitness => Ok(Value::unit()),
            K::Scope => self.task_scope(node),
            K::Spawn => self.spawn(node),
            // A constructor field declaration initializes one field, so the
            // existing field-assignment lowering does the same work.
            K::FieldDeclaration => {
                let member = node
                    .children
                    .iter()
                    .find(|child| child.kind == K::Member)
                    .ok_or_else(|| LocatedDiagnostic {
                        module: self.module,
                        diagnostic: Diagnostic::new(
                            "E_NATIVE_FIELD_DECLARATION",
                            "This field declaration has no field.",
                            node.span,
                        ),
                    })?;
                let value = node.children.last().ok_or_else(|| LocatedDiagnostic {
                    module: self.module,
                    diagnostic: Diagnostic::new(
                        "E_NATIVE_FIELD_DECLARATION",
                        "This field declaration has no value.",
                        node.span,
                    ),
                })?;
                let assignment = Node::new(
                    K::Assignment,
                    node.span,
                    vec![member.clone(), value.clone()],
                );
                self.expression(&assignment)
            }

            K::Binding => {
                let pattern = node
                    .children
                    .iter()
                    .find(|node| {
                        matches!(
                            node.kind,
                            K::PatternName
                                | K::PatternWildcard
                                | K::PatternConstructor
                                | K::PatternTuple
                                | K::PatternList
                        )
                    })
                    .ok_or_else(|| LocatedDiagnostic {
                        module: self.module,
                        diagnostic: Diagnostic::new(
                            "E_NATIVE_PATTERN",
                            "Native destructuring is not implemented yet.",
                            node.span,
                        ),
                    })?;
                let value_node = node
                    .children
                    .iter()
                    .rev()
                    .find(|n| n.kind != K::Otherwise)
                    .unwrap();
                let value = self.expression(value_node)?;
                if !matches!(pattern.kind, K::PatternName | K::PatternWildcard) {
                    let (test, bindings) = self.pattern_bind(pattern, value)?;
                    if let Some(otherwise) = node.child(K::Otherwise) {
                        let condition = self.temporary(Ty::Bool, Rhs::Test(test), pattern);
                        let (_, failure) =
                            self.isolated(|builder| builder.expression(&otherwise.children[0]))?;
                        self.code
                            .push(Statement::If(condition.atom, vec![], failure));
                    }
                    self.code.extend(bindings);
                    return Ok(Value::unit());
                }
                if pattern.kind == K::PatternWildcard {
                    return Ok(Value::unit());
                }
                let name = self
                    .text(pattern.children.first().unwrap_or(pattern))
                    .to_owned();
                let mutable = node
                    .children
                    .iter()
                    .any(|child| child.kind == K::Modifier && self.text(child) == "mut");
                // A changing function variable needs run-time storage.
                let value = if mutable && value.callable.is_some() {
                    let ty = value.ty.clone();
                    self.materialize(value, &ty, value_node)?
                } else {
                    value
                };
                // The checker treats a name bound to a field or item of another
                // value as a borrow, so the name holds that part's address and
                // the owner keeps the value. Moving it out would empty the field.
                let value = if matches!(value.native, Ty::Record(_))
                    && !value.temporary
                    && value.callable.is_none()
                    && matches!(ungroup_node(value_node).kind, K::Member | K::Index)
                    && matches!(value.atom, Atom::Field(..) | Atom::ListItem(..))
                {
                    self.borrow_value(value)
                } else {
                    value
                };
                let value = if value.callable.is_some() || primitive(&value.ty) == Some(Ty::Unit)
                    // Preserve an immutable alias's place so an approved move
                    // can clear the original box, rather than a copied pointer.
                    || !mutable && !self.entry && self.boxed_view_layout(&value).is_some()
                {
                    value
                } else {
                    let expected = self
                        .checked()
                        .binding_types
                        .get(&pattern.span)
                        .map(|ty| self.mono.apply(ty));
                    let ty =
                        if let Some(expected) = expected.filter(|expected| *expected != value.ty) {
                            self.native_type(&expected, pattern)?
                        } else {
                            value.native
                        };
                    let slot = if self.entry && self.scopes.len() == 1 {
                        let slot = Slot {
                            id: self.compiler.globals.len(),
                            global: true,
                        };
                        self.compiler.globals.push(ty);
                        let file = self.compiler.project.modules[self.module]
                            .parsed
                            .source
                            .name
                            .clone();
                        self.compiler.global_slots.insert(
                            DeclarationId {
                                file,
                                span: pattern.span,
                            },
                            slot,
                        );
                        if ty.owns() {
                            self.scopes.last_mut().unwrap().owned.push(slot);
                        }
                        slot
                    } else {
                        self.local(ty, false, ty.owns())
                    };
                    let owners = value.keepalive.clone();
                    self.assign(slot, value, value_node)?;
                    Value {
                        keepalive: owners,
                        native: ty,
                        atom: Atom::Slot(slot),
                        ty: self.compiler.source_type(ty),
                        temporary: false,
                        callable: None,
                    }
                };
                self.scopes.last_mut().unwrap().names.insert(name, value);
                Ok(Value::unit())
            }
            K::Assignment => {
                // A tuple target takes the value once, then stores each item in its place (Q130).
                if node.children[0].kind == K::Tuple {
                    let value = self.expression(&node.children[1])?;
                    let value = self.unbox_cell(value);
                    let (Ty::Record(id) | Ty::Ref(id)) = value.native else {
                        return self.unsupported(node, "this tuple assignment");
                    };
                    let fields = self.compiler.layouts[id].fields.clone();
                    for (position, target) in node.children[0].children.iter().enumerate() {
                        let field = fields[position].ty;
                        let item = Value {
                            atom: Atom::Field(Box::new(value.atom.clone()), position, field),
                            native: field,
                            ty: self.compiler.source_type(field),
                            ..Value::unit()
                        };
                        self.store_into(target, item, node)?;
                    }
                    return Ok(Value::unit());
                }
                if node.children[0].kind == K::Index
                    && crate::types::is_tensor(&self.ty(&node.children[0].children[0]))
                {
                    return self.tensor_assign(node);
                }
                if node.children[0].kind == K::Index {
                    // A class that has MutableIndexable[Key, Item] writes through setAt(key, value).
                    if self.has_trait(&node.children[0].children[0], "MutableIndexable") {
                        return self.indexable_assign(node);
                    }
                    let (item, element) = self.array_item(&node.children[0])?;
                    let value = self.expression(&node.children[1])?;
                    let rhs = self.transfer(value, element, node)?;
                    self.code
                        .push(Statement::Store(item, element, rhs, self.location(node)));
                    return Ok(Value::unit());
                }
                if node.children[0].kind == K::Member
                    && self
                        .checked()
                        .static_references
                        .contains_key(&node.children[0].span)
                {
                    return self.static_write(node);
                }
                if node.children[0].kind == K::Member {
                    let place = self.member(&node.children[0], true)?;
                    let value = self.expression(&node.children[1])?;
                    let rhs = self.transfer(value, place.native, node)?;
                    self.code.push(Statement::Store(
                        place.atom,
                        place.native,
                        rhs,
                        self.location(node),
                    ));
                    return Ok(Value::unit());
                }
                if node.children[0].kind != K::Name {
                    return self.unsupported(node, "this assignment target");
                }
                if is_proof_type(&self.ty(&node.children[0])) {
                    // Evidence has no storage, but its producer may change state.
                    self.expression(&node.children[1])?;
                    return Ok(Value::unit());
                }
                let name = self.text(&node.children[0]);
                let target = self.lookup(name);
                if let Some(Value {
                    atom: Atom::Slot(slot),
                    native: Ty::String,
                    ..
                }) = &target
                    && let Some(pieces) = self.appended_pieces(name, &node.children[1])
                {
                    for piece in pieces {
                        if let Some((native, atom)) = self.scalar_text_piece(piece)? {
                            self.code.push(Statement::Evaluate(
                                Rhs::StringAppendText(*slot, native, atom),
                                self.location(piece),
                            ));
                            continue;
                        }
                        let value = self.expression(piece)?;
                        let value = self.freeze(value, piece)?;
                        self.code.push(Statement::Evaluate(
                            Rhs::StringAppend(*slot, value.atom),
                            self.location(node),
                        ));
                    }
                    return Ok(Value::unit());
                }
                // A closure assigns a captured variable through its environment pointer.
                if let Some(
                    target @ Value {
                        atom: Atom::EnvField(..),
                        ..
                    },
                ) = &target
                {
                    let value = self.expression(&node.children[1])?;
                    let rhs = self.transfer(value, target.native, node)?;
                    self.code.push(Statement::Store(
                        target.atom.clone(),
                        target.native,
                        rhs,
                        self.location(node),
                    ));
                    return Ok(Value::unit());
                }
                let Some(Value {
                    atom: Atom::Slot(slot),
                    ..
                }) = target
                else {
                    return self.unsupported(node, "assignment to this value");
                };
                fn simple(node: &Node) -> bool {
                    matches!(
                        node.kind,
                        K::Name
                            | K::Literal
                            | K::One
                            | K::Unary
                            | K::Binary
                            | K::Operator
                            | K::Group
                            | K::Member
                            | K::Tuple
                            | K::List
                            | K::Argument
                            | K::NamedArgument
                    ) && node.children.iter().all(simple)
                }
                let rhs = &node.children[1];
                let consume = if rhs.kind == K::Call
                    && rhs.children[0].kind == K::Member
                    && rhs.children[0].children[0].kind == K::Name
                    && self.text(&rhs.children[0].children[0]) == name
                    && rhs.children.iter().skip(1).all(simple)
                {
                    Some(rhs.children[0].children[0].span)
                } else {
                    None
                };
                let previous = std::mem::replace(&mut self.consume_receiver, consume);
                let value = self.expression(rhs)?;
                self.consume_receiver = previous;
                self.assign(slot, value, node)?;
                Ok(Value::unit())
            }
            K::Block => {
                self.scopes.push(Scope {
                    finalizers: vec![],
                    names: BTreeMap::new(),
                    owned: vec![],
                });
                let mut value = Value::unit();
                for (index, child) in node.children.iter().enumerate() {
                    let before = self.scopes.last().unwrap().owned.len();
                    value = self.expression(child)?;
                    if ends_with_return(&self.code) {
                        break;
                    }
                    if index + 1 < node.children.len() {
                        self.drop_statement_temporaries(before);
                    }
                }
                if ends_with_return(&self.code) {
                    self.finish_scope();
                    return Ok(Value::unit());
                }
                if !value.native.owns()
                    && value.native != Ty::Unit
                    && value.callable.is_none()
                    && !ends_with_return(&self.code)
                {
                    value = self.freeze(value, node)?;
                }
                if value.callable.is_some() {
                    let mut owners = value.keepalive.clone();
                    protect(&value, &mut owners);
                    value.keepalive = owners;
                } else if !self
                    .compiler
                    .holds_borrow(value.native, &mut BTreeSet::new())
                {
                    value.keepalive.clear();
                }
                self.promote_owners(&value.keepalive, self.scopes.len() - 2);
                let owners = value.keepalive.clone();
                // An owned result must survive the block that produced it.
                let value = if value.native.owns() {
                    let outer = self.scopes.len() - 2;
                    let slot = Slot {
                        id: self.locals.len(),
                        global: false,
                    };
                    self.locals.push(Local {
                        scope: outer,
                        ty: value.native,
                        parameter: false,
                        owned: true,
                    });
                    self.scopes[outer].owned.push(slot);
                    self.assign(slot, value, node)?;
                    Value {
                        keepalive: owners,
                        native: self.slot_type(slot),
                        atom: Atom::Slot(slot),
                        ty: self.compiler.source_type(self.slot_type(slot)),
                        temporary: true,
                        callable: None,
                    }
                } else {
                    value
                };
                self.finish_scope();
                Ok(value)
            }
            K::Unary => {
                let operator = self.text(&node.children[0]).to_owned();
                if let Some(call) = self.checked().resolved_calls.get(&node.span).cloned() {
                    let value = self.expression(&node.children[1])?;
                    let value = self.freeze(value, &node.children[1])?;
                    return self.invoke(&call, vec![], Some(value), node, None);
                }
                let result = self.native_type(&self.ty(node), node)?;
                if operator == "-" && result == Ty::Int && node.children[1].kind == K::Literal {
                    let text = self.text(&node.children[1]).replace('_', "");
                    let magnitude = if let Some(hex) =
                        text.strip_prefix("0x").or_else(|| text.strip_prefix("0X"))
                    {
                        u64::from_str_radix(hex, 16)
                    } else {
                        text.parse::<u64>()
                    };
                    if magnitude == Ok(1u64 << 63) {
                        return Ok(Value {
                            keepalive: BTreeSet::new(),
                            native: Ty::Int,
                            atom: Atom::Int(i64::MIN),
                            ty: Type::named("Int"),
                            temporary: false,
                            callable: None,
                        });
                    }
                }
                let value = self.expression(&node.children[1])?;
                if let Some(id) = self.compiler.bigint_layout(value.native) {
                    return self.bigint_operation(
                        if operator == "-" { "neg" } else { "clone" },
                        id,
                        vec![value],
                        Ty::Record(id),
                        node,
                    );
                }
                let ty = self.native_type(&value.ty, node)?;
                Ok(self.temporary(ty, Rhs::Unary(operator, value.atom), node))
            }
            K::Binary => {
                let operator = self.text(&node.children[1]).to_owned();
                let left = self.expression(&node.children[0])?;
                let left = self.freeze(left, &node.children[0])?;
                if matches!(operator.as_str(), "&&" | "||") {
                    let slot = self.local(Ty::Bool, false, false);
                    let (right, mut right_code) =
                        self.isolated(|builder| builder.expression(&node.children[2]))?;
                    right_code.push(Statement::Set(
                        slot,
                        Rhs::Copy(right.atom),
                        self.location(node),
                    ));
                    let constant = vec![Statement::Set(
                        slot,
                        Rhs::Copy(Atom::Bool(operator == "||")),
                        self.location(node),
                    )];
                    self.code.push(if operator == "&&" {
                        Statement::If(left.atom, right_code, constant)
                    } else {
                        Statement::If(left.atom, constant, right_code)
                    });
                    return Ok(Value {
                        keepalive: BTreeSet::new(),
                        native: Ty::Bool,
                        atom: Atom::Slot(slot),
                        ty: Type::named("Bool"),
                        temporary: true,
                        callable: None,
                    });
                }
                let right = self.expression(&node.children[2])?;
                if let Some(id) = self.compiler.bigint_layout(left.native) {
                    let (operation, result) = match operator.as_str() {
                        "+" => ("add", Ty::Record(id)),
                        "-" => ("sub", Ty::Record(id)),
                        "*" => ("mul", Ty::Record(id)),
                        "/" => ("div", Ty::Record(id)),
                        "%" => ("rem", Ty::Record(id)),
                        "**" => ("pow", Ty::Record(id)),
                        "==" => ("eq", Ty::Bool),
                        "!=" => ("ne", Ty::Bool),
                        "<" => ("lt", Ty::Bool),
                        "<=" => ("le", Ty::Bool),
                        ">" => ("gt", Ty::Bool),
                        ">=" => ("ge", Ty::Bool),
                        _ => return self.unsupported(node, "this BigInt operator"),
                    };
                    return self.bigint_operation(operation, id, vec![left, right], result, node);
                }
                if matches!(operator.as_str(), "==" | "!=")
                    && matches!(left.native, Ty::Record(_) | Ty::Ref(_))
                {
                    let result = self.equal_values(left, right, node)?;
                    return if operator == "!=" {
                        Ok(self.temporary(Ty::Bool, Rhs::Unary("!".into(), result.atom), node))
                    } else {
                        Ok(result)
                    };
                }
                // `xs + ys` copies both Lists' items into a new List.
                if operator == "+"
                    && let Ty::Record(id) | Ty::Ref(id) = left.native
                    && matches!(self.compiler.layouts[id].kind, LayoutKind::List(_))
                {
                    let left = self.borrow_value(left);
                    let right = self.borrow_value(right);
                    return Ok(self.temporary(
                        Ty::Record(id),
                        Rhs::ListConcat(id, left.atom, right.atom),
                        node,
                    ));
                }
                if matches!(operator.as_str(), "<" | "<=" | ">" | ">=")
                    && !matches!(left.native, Ty::Int | Ty::Nat | Ty::Double)
                {
                    let order = self.compare_values(left, right, node)?;
                    return Ok(self.temporary(
                        Ty::Bool,
                        Rhs::Binary(operator, Ty::Int, order.atom, Atom::Int(0)),
                        node,
                    ));
                }
                if matches!(left.native, Ty::Record(_) | Ty::Ref(_) | Ty::Variable)
                    || matches!(right.native, Ty::Record(_) | Ty::Ref(_) | Ty::Variable)
                {
                    let call = self
                        .checked()
                        .resolved_calls
                        .get(&node.span)
                        .cloned()
                        .ok_or_else(|| LocatedDiagnostic {
                            module: self.module,
                            diagnostic: Diagnostic::new(
                                "E_NATIVE_CALL",
                                "This operator has no checked implementation.",
                                node.span,
                            ),
                        })?;
                    let right_receiver = call.receiver == Some(node.children[2].span);
                    let (receiver, other) = if right_receiver {
                        (right, left)
                    } else {
                        (left, right)
                    };
                    let result = self.invoke(&call, vec![other], Some(receiver), node, None)?;
                    return if operator == "!=" {
                        Ok(self.temporary(Ty::Bool, Rhs::Unary("!".into(), result.atom), node))
                    } else {
                        Ok(result)
                    };
                }
                // `"ab" * 3` and `3 * "ab"` repeat the text (Q136).
                if operator == "*" && (left.native == Ty::String) != (right.native == Ty::String) {
                    let (text, count) = if left.native == Ty::String {
                        (left.atom, right.atom)
                    } else {
                        (right.atom, left.atom)
                    };
                    return Ok(self.temporary(
                        Ty::String,
                        Rhs::Binary("*".into(), Ty::String, text, count),
                        node,
                    ));
                }
                let operand = left.native;
                let result = if matches!(operator.as_str(), "==" | "!=" | "<" | ">" | "<=" | ">=") {
                    Ty::Bool
                } else {
                    operand
                };
                Ok(self.temporary(
                    result,
                    Rhs::Binary(operator, operand, left.atom, right.atom),
                    node,
                ))
            }
            K::If => {
                let condition = self.expression(&node.children[0])?;
                let ty = self.node_native(node)?;
                let slot = (ty != Ty::Unit).then(|| self.local(ty, false, ty.owns()));
                let mixed = match ty {
                    Ty::Ref(id)
                        if node.children[1..]
                            .iter()
                            .any(|branch| matches!(self.node_native(branch), Ok(Ty::Record(found)) if found == id)) =>
                    {
                        let slot = self.local(Ty::Boxed(id), false, true);
                        self.result_boxes.insert(slot);
                        Some(slot)
                    }
                    _ => None,
                };
                let (left_value, left) = self.isolated(|builder| {
                    let value = builder.expression(&node.children[1])?;
                    let value = builder.box_branch(value, mixed, node)?;
                    let owners = value.keepalive.clone();
                    if !ends_with_return(&builder.code)
                        && let Some(slot) = slot
                    {
                        builder.assign(slot, value, node)?;
                    }
                    let mut value = Value::unit();
                    value.keepalive = owners;
                    Ok(value)
                })?;
                let (right_value, right) = self.isolated(|builder| {
                    let mut owners = BTreeSet::new();
                    if let Some(other) = node.children.get(2) {
                        let value = builder.expression(other)?;
                        let value = builder.box_branch(value, mixed, node)?;
                        owners = value.keepalive.clone();
                        if !ends_with_return(&builder.code)
                            && let Some(slot) = slot
                        {
                            builder.assign(slot, value, node)?;
                        }
                    }
                    let mut value = Value::unit();
                    value.keepalive = owners;
                    Ok(value)
                })?;
                let owners = left_value
                    .keepalive
                    .union(&right_value.keepalive)
                    .copied()
                    .chain(mixed)
                    .collect();
                self.code.push(Statement::If(condition.atom, left, right));
                Ok(slot
                    .map(|slot| Value {
                        keepalive: owners,
                        native: ty,
                        atom: Atom::Slot(slot),
                        ty: self.compiler.source_type(ty),
                        temporary: true,
                        callable: None,
                    })
                    .unwrap_or_else(Value::unit))
            }
            K::Call | K::Pipe => self.call(node),
            K::Lambda => {
                // Only names the body reads are captured; parameters shadow nothing.
                let parameters = node
                    .child(K::LambdaParameters)
                    .map(|list| {
                        list.children
                            .iter()
                            .filter_map(|p| p.child(K::Name).map(|n| self.text(n).to_owned()))
                            .collect::<BTreeSet<_>>()
                    })
                    .unwrap_or_default();
                let used = node
                    .children
                    .last()
                    .unwrap()
                    .descendants()
                    .filter(|n| n.kind == K::Name)
                    .map(|n| self.text(n).to_owned())
                    .filter(|name| !parameters.contains(name))
                    .collect::<BTreeSet<_>>();
                let mut captures = BTreeMap::new();
                for scope in &self.scopes {
                    for (name, value) in &scope.names {
                        if used.contains(name) {
                            captures.insert(name.clone(), value.clone());
                        }
                    }
                }
                let moves = node
                    .children
                    .iter()
                    .any(|child| child.kind == K::Modifier && self.text(child) == "move");
                if !moves {
                    for value in captures.values_mut() {
                        if value.callable.is_none() {
                            *value = self.borrow_value(value.clone());
                        }
                    }
                }
                let value = Value {
                    keepalive: BTreeSet::new(),
                    native: Ty::Unit,
                    atom: Atom::Unit,
                    ty: self.ty(node),
                    temporary: true,
                    callable: Some(Box::new(FunctionValue::Lambda {
                        module: self.module,
                        node: node.clone(),
                        captures,
                    })),
                };
                if moves {
                    // A move lambda owns its captures now, so it becomes a closure once.
                    let ty = value.ty.clone();
                    return self.materialize(value, &ty, node);
                }
                Ok(value)
            }
            K::Return => {
                if self.inline_lambda {
                    return self.unsupported(node, "early return from an inlined lambda");
                }
                let value = node
                    .children
                    .first()
                    .map(|node| self.expression(node))
                    .transpose()?
                    .unwrap_or_else(Value::unit);
                let value = if self.constructor && node.children.is_empty() {
                    self.lookup("this").unwrap()
                } else {
                    value
                };
                self.return_value(value, node)?;
                Ok(Value::unit())
            }
            K::While | K::For => self.loop_statement(node),
            K::Break | K::Continue => {
                let (id, depth) = *self.loops.last().unwrap();
                let drops = self.cleanups(depth);
                self.code.push(if node.kind == K::Break {
                    Statement::Break(id, drops)
                } else {
                    Statement::Continue(id, drops)
                });
                Ok(Value::unit())
            }
            K::FormatString => {
                let mut parts = vec![];
                for child in &node.children {
                    if child.kind == K::FormatText {
                        parts.push(Atom::Text(
                            crate::strings::decode(self.text(child), child.span.start, true)
                                .map_err(|diagnostic| LocatedDiagnostic {
                                    module: self.module,
                                    diagnostic,
                                })?,
                        ));
                    } else if child.kind == K::Interpolation {
                        let value = self.expression(&child.children[0])?;
                        let spec = child
                            .child(K::FormatSpec)
                            .map(|spec| self.text(spec).to_owned());
                        let piece = self.format_piece(value, spec.as_deref(), child)?;
                        parts.push(piece);
                    }
                }
                Ok(self.temporary(Ty::String, Rhs::Join(parts), node))
            }
            K::Function => self.local_function(node),
            _ => self.unsupported(node, &format!("{:?}", node.kind)),
        }
    }
    /// One `{value:spec}` piece of an f-string or a `"...".format(...)` template.
    fn format_piece(&mut self, value: Value, spec: Option<&str>, child: &Node) -> Result<Atom> {
        let debug = spec == Some("?");
        let value = if matches!(
            value.native,
            Ty::Record(_) | Ty::Ref(_) | Ty::Boxed(_) | Ty::Dyn(_) | Ty::DynBox(_)
        ) || debug && value.native == Ty::String
        {
            self.render(value, debug, child)?
        } else {
            value
        };
        // A general spec such as `>8,.2f` formats in the runtime.
        if let Some(spec) = spec
            && !matches!(
                crate::strings::format_kind(spec),
                Some(crate::strings::FormatKind::Debug | crate::strings::FormatKind::Fixed(_))
            )
        {
            let (operation, value) = match value.native {
                Ty::Int | Ty::Nat => ("formatInt", value),
                Ty::Double => ("formatDouble", value),
                Ty::String => ("formatText", value),
                native => (
                    "formatText",
                    self.temporary(Ty::String, Rhs::Text(native, value.atom, None), child),
                ),
            };
            let input = value.native;
            return Ok(self
                .temporary(
                    Ty::String,
                    Rhs::Builtin(
                        operation.into(),
                        input,
                        vec![value.atom, Atom::Text(spec.to_owned())],
                    ),
                    child,
                )
                .atom);
        }
        let precision_spec = spec.is_some_and(|spec| spec != "?");
        let value = if precision_spec && matches!(value.native, Ty::Int | Ty::Nat) {
            self.temporary(
                Ty::Double,
                Rhs::Builtin("toDouble".into(), value.native, vec![value.atom]),
                child,
            )
        } else {
            value
        };
        let ty = value.native;
        let precision = spec
            .filter(|spec| *spec != "?")
            .map(|spec| spec[1..spec.len() - 1].parse::<u32>())
            .transpose()
            .map_err(|_| LocatedDiagnostic {
                module: self.module,
                diagnostic: Diagnostic::new(
                    "E_NATIVE_FORMAT",
                    "This precision is too large for the native formatter.",
                    child.span,
                ),
            })?;
        Ok(self
            .temporary(Ty::String, Rhs::Text(ty, value.atom, precision), child)
            .atom)
    }
    /// A function declared inside another function compiles as its own function. The
    /// enclosing locals it reads become hidden borrowed arguments (REVIEW 1.4).
    fn local_function(&mut self, node: &Node) -> Result<Value> {
        let source = &self.compiler.project.modules[self.module].parsed.source;
        let id = DeclarationId {
            file: source.name.clone(),
            span: node.span,
        };
        let own = node
            .child(K::Name)
            .map(|name| source.slice(name.span).to_owned())
            .unwrap_or_default();
        let mut names = BTreeSet::new();
        let mut assigned = BTreeSet::new();
        fn visit(
            node: &Node,
            source: &crate::source::SourceFile,
            names: &mut BTreeSet<String>,
            assigned: &mut BTreeSet<String>,
        ) {
            match node.kind {
                K::Name => {
                    names.insert(source.slice(node.span).to_owned());
                }
                // A member name and a named argument's name are not variables.
                K::Member => {
                    if let Some(owner) = node.children.first() {
                        visit(owner, source, names, assigned);
                    }
                    return;
                }
                K::NamedArgument => {
                    for child in node.children.iter().skip(1) {
                        visit(child, source, names, assigned);
                    }
                    return;
                }
                K::Assignment => {
                    if let Some(root) = node
                        .children
                        .first()
                        .and_then(|place| place_root_name(place, source))
                    {
                        assigned.insert(root.to_owned());
                    }
                }
                _ => {}
            }
            for child in &node.children {
                visit(child, source, names, assigned);
            }
        }
        for child in &node.children {
            if matches!(
                child.kind,
                K::Name | K::ParameterList | K::ReturnType | K::TypeParameters
            ) {
                continue;
            }
            visit(child, source, &mut names, &mut assigned);
        }
        let mut captures: Vec<(String, bool)> = names
            .iter()
            .filter(|name| **name != own && self.lookup(name).is_some())
            .map(|name| (name.clone(), assigned.contains(name)))
            .collect();
        // Calling another local function means passing its enclosing locals on too.
        for name in &names {
            let callee = self
                .compiler
                .local_functions
                .values()
                .find(|(module, declaration, _)| {
                    *module == self.module
                        && declaration
                            .child(K::Name)
                            .is_some_and(|n| source.slice(n.span) == name)
                })
                .map(|(_, _, captured)| captured.clone());
            for (captured, changes) in callee.into_iter().flatten() {
                match captures.iter_mut().find(|(name, _)| *name == captured) {
                    Some(existing) => existing.1 |= changes,
                    None => captures.push((captured, changes)),
                }
            }
        }
        self.compiler
            .local_functions
            .insert(id, (self.module, node.clone(), captures));
        Ok(Value::unit())
    }

    fn task_scope(&mut self, node: &Node) -> Result<Value> {
        let native = self.native_type(&self.ty(node), node)?;
        let fails = native != Ty::Unit;
        let output = (native != Ty::Unit).then(|| self.local(native, false, native.owns()));
        self.scopes.push(Scope {
            finalizers: vec![],
            names: BTreeMap::new(),
            owned: vec![],
        });
        let tasks = self.local(Ty::Tasks, false, true);
        self.scopes
            .last_mut()
            .unwrap()
            .finalizers
            .push(Cleanup::Join(tasks, fails));
        self.task_scopes.push((tasks, fails));
        // A task scope keeps every local alive until all its tasks have joined.
        let body = &node.children[0];
        for child in &body.children {
            self.expression(child)?;
            if ends_with_return(&self.code) {
                break;
            }
        }
        self.task_scopes.pop();
        if !ends_with_return(&self.code) {
            let error = self.temporary(Ty::Error, Rhs::JoinTasks(tasks), node);
            if let Some(output) = output {
                let Ty::Record(id) = native else {
                    unreachable!()
                };
                let LayoutKind::Union(parts, _) = &self.compiler.layouts[id].kind else {
                    unreachable!()
                };
                let failure = parts
                    .iter()
                    .position(|t| *t == Type::named("Error"))
                    .unwrap();
                let success = parts
                    .iter()
                    .position(|t| *t == Type::named("void"))
                    .unwrap();
                let condition = self.temporary(
                    Ty::Bool,
                    Rhs::Test(Test::Equal(
                        Ty::Int,
                        Atom::ErrorType(Box::new(error.atom.clone())),
                        Atom::Int(0),
                    )),
                    node,
                );
                let loc = self.location(node);
                self.code.push(Statement::If(
                    condition.atom,
                    vec![Statement::Set(
                        output,
                        Rhs::Aggregate(id, Some(success), vec![]),
                        loc.clone(),
                    )],
                    vec![Statement::Set(
                        output,
                        Rhs::Aggregate(id, Some(failure), vec![(error.atom, Ty::Error)]),
                        loc,
                    )],
                ));
            }
        }
        self.finish_scope();
        Ok(output
            .map(|slot| Value {
                keepalive: BTreeSet::new(),
                native,
                atom: Atom::Slot(slot),
                ty: self.ty(node),
                temporary: true,
                callable: None,
            })
            .unwrap_or_else(Value::unit))
    }
    fn spawn(&mut self, node: &Node) -> Result<Value> {
        let Some((scope, _)) = self.task_scopes.last().copied() else {
            return self.unsupported(node, "a task outside scope");
        };
        let call = &node.children[0];
        if self
            .checked()
            .resolved_calls
            .get(&call.span)
            .is_some_and(|c| matches!(c.target, Callable::Closure(_) | Callable::Indirect))
        {
            return self.unsupported(
                node,
                "a task with a local closure or indirect function target",
            );
        }
        if call.kind == K::Call
            && call.children[0].kind == K::Name
            && self.lookup(self.text(&call.children[0])).is_some_and(|v| {
                matches!(
                    v.callable.as_deref(),
                    Some(FunctionValue::Partial { .. } | FunctionValue::Lambda { .. })
                )
            })
        {
            return self.unsupported(node, "a task with captured arguments");
        }
        let value = self.expression(call)?;
        let result = value.native;
        let invocation = match self.code.pop() {
            Some(Statement::Set(_, rhs @ (Rhs::Call(..) | Rhs::Builtin(..)), _))
            | Some(Statement::Evaluate(rhs @ (Rhs::Call(..) | Rhs::Builtin(..)), _)) => rhs,
            Some(Statement::Print(ty, atom)) => Rhs::Builtin("print".into(), ty, vec![atom]),
            Some(other) => {
                self.code.push(other);
                return self.unsupported(node, "this task callable");
            }
            None => return self.unsupported(node, "this task callable"),
        };
        let id = self.compiler.next_task;
        self.compiler.next_task += 1;
        self.code.push(Statement::Spawn {
            id,
            scope,
            call: invocation,
            result,
            location: self.location(node),
        });
        Ok(Value::unit())
    }

    fn using_expression(&mut self, node: &Node) -> Result<Value> {
        let init = self.expression(&node.children[1])?;
        let native = self.native_type(&self.ty(node), node)?;
        let slot = (native != Ty::Unit).then(|| self.local(native, false, native.owns()));
        self.scopes.push(Scope {
            finalizers: vec![],
            names: BTreeMap::new(),
            owned: vec![],
        });
        let resource = if matches!(init.native, Ty::Record(_)) {
            let resource = self.temporary(init.native, Rhs::Take(init.atom), node);
            self.borrow_value(resource)
        } else {
            init
        };
        let Ty::Ref(id) = resource.native else {
            return self.unsupported(node, "this resource layout");
        };
        let file = matches!(
            self.compiler.layouts[id].kind,
            LayoutKind::Resource(Resource::Reader | Resource::Writer)
        );
        if file {
            self.scopes
                .last_mut()
                .unwrap()
                .finalizers
                .push(Cleanup::File(resource.atom.clone()));
        }
        let name = self.text(&node.children[0]).to_owned();
        self.scopes
            .last_mut()
            .unwrap()
            .names
            .insert(name, resource.clone());
        let value = self.expression(node.children.last().unwrap())?;
        if !ends_with_return(&self.code) {
            if let Some(slot) = slot {
                self.assign(slot, value, node)?;
            }
            if file {
                let closed = self.temporary(Ty::Error, Rhs::FileClose(resource.atom), node);
                let Ty::Record(result) = native else {
                    unreachable!()
                };
                let LayoutKind::Union(parts, _) = &self.compiler.layouts[result].kind else {
                    unreachable!()
                };
                let case = parts
                    .iter()
                    .position(|p| *p == Type::named("Error"))
                    .unwrap();
                let mut body_error = Test::Equal(
                    Ty::Int,
                    Atom::Tag(Box::new(Atom::Slot(slot.unwrap()))),
                    Atom::Int(case as i64 + 1),
                );
                for (index, part) in parts.iter().enumerate() {
                    if index != case && self.compiler.is_error_type(part) {
                        body_error = Test::Or(
                            Box::new(body_error),
                            Box::new(Test::Equal(
                                Ty::Int,
                                Atom::Tag(Box::new(Atom::Slot(slot.unwrap()))),
                                Atom::Int(index as i64 + 1),
                            )),
                        );
                    }
                }
                let test = Test::And(
                    Box::new(Test::Not(Box::new(Test::Equal(
                        Ty::Int,
                        Atom::ErrorType(Box::new(closed.atom.clone())),
                        Atom::Int(0),
                    )))),
                    Box::new(Test::Not(Box::new(body_error))),
                );
                let condition = self.temporary(Ty::Bool, Rhs::Test(test), node);
                let location = self.location(node);
                self.code.push(Statement::If(
                    condition.atom,
                    vec![Statement::Set(
                        slot.unwrap(),
                        Rhs::Aggregate(result, Some(case), vec![(closed.atom, Ty::Error)]),
                        location,
                    )],
                    vec![],
                ));
            }
        }
        self.finish_scope();
        Ok(slot
            .map(|slot| Value {
                keepalive: BTreeSet::new(),
                atom: Atom::Slot(slot),
                native,
                ty: self.ty(node),
                temporary: true,
                callable: None,
            })
            .unwrap_or_else(Value::unit))
    }

    /// `?` on an Option: a Some gives its payload, and a None returns None from this function (Q115).
    fn propagate_option(&mut self, node: &Node, value: Value, option: usize) -> Result<Value> {
        let cases = self.compiler.layouts[option]
            .kind
            .cases()
            .map(|cases| cases.to_vec())
            .unwrap_or_default();
        let Some(some) = cases.iter().position(|case| case.name == "Some") else {
            return self.unsupported(node, "an Option without Some");
        };
        // The function returns an Option too, and the caller receives its None.
        let result = match self.result {
            Ty::Record(id) | Ty::Ref(id) => id,
            _ => {
                return self.unsupported(node, "propagation of an Option outside an Option result");
            }
        };
        let Some(result_none) = self.compiler.layouts[result]
            .kind
            .cases()
            .and_then(|cases| cases.iter().position(|case| case.name == "None"))
        else {
            return self.unsupported(node, "propagation of an Option outside an Option result");
        };
        let condition = self.temporary(
            Ty::Bool,
            Rhs::Test(Test::Equal(
                Ty::Int,
                Atom::Tag(Box::new(value.atom.clone())),
                Atom::Int((some + 1) as i64),
            )),
            node,
        );
        let (_, absent) = self.isolated(|builder| {
            let none = builder.temporary(
                Ty::Record(result),
                Rhs::Aggregate(result, Some(result_none), vec![]),
                node,
            );
            builder.return_value(none, node)?;
            Ok(Value::unit())
        })?;
        self.code
            .push(Statement::If(condition.atom, vec![], absent));
        let native = cases[some].fields.first().copied().unwrap_or(Ty::Unit);
        let payload = Value {
            keepalive: value.keepalive.clone(),
            atom: if native == Ty::Unit {
                Atom::Unit
            } else {
                Atom::Payload(Box::new(value.atom.clone()), some, 0, native)
            },
            native,
            ty: self.compiler.source_type(native),
            temporary: false,
            callable: None,
        };
        Ok(if matches!(value.native, Ty::Ref(_)) {
            self.borrow_value(payload)
        } else {
            payload
        })
    }

    fn propagate(&mut self, node: &Node) -> Result<Value> {
        let value = self.expression(&node.children[0])?;
        let layout = match value.native {
            Ty::Record(id) | Ty::Ref(id) => id,
            _ => return self.unsupported(node, "propagation of this value"),
        };
        if self.compiler.layouts[layout]
            .kind
            .cases()
            .is_some_and(|cases| cases.iter().any(|case| case.name == "None"))
        {
            return self.propagate_option(node, value, layout);
        }
        let LayoutKind::Union(parts, cases) = self.compiler.layouts[layout].kind.clone() else {
            return self.unsupported(node, "propagation outside an error union");
        };
        let Some(success) = parts
            .iter()
            .position(|part| !self.compiler.is_error_type(part))
        else {
            return self.unsupported(node, "an error union with no success value");
        };
        for (case, part) in parts.iter().enumerate() {
            if !self.compiler.is_error_type(part) {
                continue;
            }
            let native = cases[case].fields[0];
            let error = Value {
                keepalive: BTreeSet::new(),
                atom: Atom::Payload(Box::new(value.atom.clone()), case, 0, native),
                native,
                ty: part.clone(),
                temporary: false,
                callable: None,
            };
            let error = if matches!(value.native, Ty::Ref(_)) {
                self.borrow_value(error)
            } else {
                error
            };
            let condition = self.temporary(
                Ty::Bool,
                Rhs::Test(Test::Equal(
                    Ty::Int,
                    Atom::Tag(Box::new(value.atom.clone())),
                    Atom::Int((case + 1) as i64),
                )),
                node,
            );
            let (_, body) = self.isolated(|builder| {
                if builder.entry && builder.compiler.session && builder.result == Ty::Unit {
                    builder.abort_entry(error, node)?;
                } else if builder.entry && builder.result == Ty::Unit {
                    // A script's top-level `?` reports the Error and exits with status 1.
                    let text = builder.render(error, false, node)?;
                    builder.code.push(Statement::Evaluate(
                        Rhs::Builtin("scriptFail".into(), Ty::Unit, vec![text.atom]),
                        builder.location(node),
                    ));
                } else {
                    builder.return_value(error, node)?;
                }
                Ok(Value::unit())
            })?;
            self.code.push(Statement::If(condition.atom, body, vec![]));
        }
        let native = cases[success].fields.first().copied().unwrap_or(Ty::Unit);
        let result = Value {
            keepalive: BTreeSet::new(),
            atom: if native == Ty::Unit {
                Atom::Unit
            } else {
                Atom::Payload(Box::new(value.atom), success, 0, native)
            },
            native,
            ty: parts[success].clone(),
            temporary: false,
            callable: None,
        };
        Ok(if matches!(value.native, Ty::Ref(_)) {
            self.borrow_value(result)
        } else {
            result
        })
    }

    /// The value of a range bound, which the checker already proved is an integer.
    fn pattern_integer_bound(&self, pattern: &Node, node: &Node) -> Result<i64> {
        let text = self.text(node).replace('_', "");
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
        };
        let number = magnitude.ok().and_then(|n| {
            if negative && n == 1u64 << 63 {
                Some(i64::MIN)
            } else {
                i64::try_from(n).ok().map(|n| if negative { -n } else { n })
            }
        });
        number.ok_or_else(|| LocatedDiagnostic {
            module: self.module,
            diagnostic: Diagnostic::new(
                "E_INTEGER_LITERAL_RANGE",
                "This range bound does not fit Int.",
                pattern.span,
            ),
        })
    }
    fn pattern_bind(&mut self, pattern: &Node, value: Value) -> Result<(Test, Vec<Statement>)> {
        let mut bindings = vec![];
        let test = self.pattern_bind_into(pattern, value, &mut bindings)?;
        Ok((test, bindings))
    }
    fn pattern_bind_into(
        &mut self,
        pattern: &Node,
        value: Value,
        bindings: &mut Vec<Statement>,
    ) -> Result<Test> {
        let value = if matches!(value.native, Ty::Boxed(_)) {
            self.borrow_value(value)
        } else {
            value
        };
        if value.native == Ty::Error && pattern.kind == K::PatternConstructor {
            let name = self.text(&pattern.children[0]).to_owned();
            if name == "Error" {
                let Some(argument) = pattern.children.iter().find(|n| n.kind != K::Name) else {
                    return self.unsupported(pattern, "Error pattern without a binding");
                };
                return self.pattern_bind_into(argument, value, bindings);
            }
            let Some((layout, _)) = self.compiler.error_case(&name) else {
                return self.unsupported(pattern, "this Error case");
            };
            let test = Test::Equal(
                Ty::Int,
                Atom::ErrorType(Box::new(value.atom.clone())),
                Atom::Int((layout + 1) as i64),
            );
            let payload = Value {
                keepalive: BTreeSet::new(),
                atom: Atom::ErrorPayload(Box::new(value.atom), layout),
                native: Ty::Ref(layout),
                ty: self.compiler.layouts[layout].source.clone(),
                temporary: false,
                callable: None,
            };
            let inner = self.pattern_bind_into(pattern, payload, bindings)?;
            return Ok(Test::And(Box::new(test), Box::new(inner)));
        }
        if let Ty::Record(layout) | Ty::Ref(layout) = value.native
            && let LayoutKind::Union(parts, cases) = self.compiler.layouts[layout].kind.clone()
            && pattern.kind != K::PatternWildcard
        {
            let mut selected = None;
            for (case, part) in parts.iter().enumerate() {
                let matches = match pattern.kind {
                    K::PatternName => !self.compiler.is_error_type(part),
                    K::PatternConstructor => {
                        let name = self.text(&pattern.children[0]).to_owned();
                        part == &Type::named("Error") && (name == "Error" || self.compiler.error_case(&name).is_some())
                            || cases[case].fields.first().is_some_and(|native| matches!(native, Ty::Record(id) | Ty::Ref(id) if self.compiler.layouts[*id].kind.cases().is_some_and(|variants| variants.iter().any(|v| v.name == name.rsplit('.').next().unwrap()))))
                    }
                    K::PatternTuple => matches!(part, Type::Tuple(_)),
                    K::PatternList => matches!(part, Type::Named(name, _) if name == "List"),
                    K::PatternRange | K::PatternRangeInclusive => {
                        matches!(part, Type::Named(name, _) if name == "Int" || name == "Nat")
                    }
                    K::PatternLiteral => {
                        let text = self.text(pattern);
                        let wanted = if text.starts_with('"') {
                            "String"
                        } else if matches!(text, "true" | "false") {
                            "Bool"
                        } else if crate::lexer::is_double_literal(text) {
                            "double"
                        } else {
                            "Int"
                        };
                        part == &Type::named(wanted)
                            || wanted == "Int"
                                && !parts.contains(&Type::named("Int"))
                                && part == &Type::named("BigInt")
                    }
                    _ => false,
                };
                if matches {
                    selected = Some(case);
                    break;
                }
            }
            let Some(case) = selected else {
                return self.unsupported(pattern, "this union pattern");
            };
            let test = Test::Equal(
                Ty::Int,
                Atom::Tag(Box::new(value.atom.clone())),
                Atom::Int((case + 1) as i64),
            );
            let native = cases[case].fields.first().copied().unwrap_or(Ty::Unit);
            let child = Value {
                keepalive: BTreeSet::new(),
                atom: if native == Ty::Unit {
                    Atom::Unit
                } else {
                    Atom::Payload(Box::new(value.atom), case, 0, native)
                },
                native,
                ty: parts[case].clone(),
                temporary: false,
                callable: None,
            };
            let child = if matches!(value.native, Ty::Ref(_)) {
                self.borrow_value(child)
            } else {
                child
            };
            let inner = self.pattern_bind_into(pattern, child, bindings)?;
            return Ok(Test::And(Box::new(test), Box::new(inner)));
        }
        match pattern.kind {
            K::PatternWildcard => Ok(Test::Yes),
            K::PatternName | K::Name => {
                let value = if matches!(value.native, Ty::Boxed(_)) {
                    self.borrow_value(value)
                } else {
                    value
                };
                let name = self
                    .text(pattern.child(K::Name).unwrap_or(pattern))
                    .to_owned();
                if self.entry && self.compiler.session && self.scopes.len() == 1 {
                    self.compiler.global_bindings.insert(
                        DeclarationId {
                            file: self.compiler.project.modules[self.module]
                                .parsed
                                .source
                                .name
                                .clone(),
                            span: pattern.span,
                        },
                        value.clone(),
                    );
                }
                self.scopes.last_mut().unwrap().names.insert(name, value);
                Ok(Test::Yes)
            }
            K::PatternLiteral => {
                let text = self.text(pattern);
                if self.compiler.bigint_layout(value.native).is_some() {
                    return Ok(Test::BigIntLiteral(value.atom, text.to_owned()));
                }
                let atom = match value.native {
                    Ty::Bool => Atom::Bool(text == "true"),
                    Ty::String => Atom::Text(
                        crate::strings::decode(
                            &text[1..text.len() - 1],
                            pattern.span.start + 1,
                            false,
                        )
                        .map_err(|diagnostic| LocatedDiagnostic {
                            module: self.module,
                            diagnostic,
                        })?,
                    ),
                    Ty::Double => {
                        let text = text.replace('_', "");
                        let parsed: std::result::Result<f64, ()> = if let Some(hex) =
                            text.strip_prefix("0x").or_else(|| text.strip_prefix("0X"))
                        {
                            u128::from_str_radix(hex, 16)
                                .map(|value| value as f64)
                                .map_err(|_| ())
                        } else {
                            text.parse::<f64>().map_err(|_| ())
                        };
                        Atom::Double(parsed.map_err(|_| LocatedDiagnostic {
                            module: self.module,
                            diagnostic: Diagnostic::new(
                                "E_NATIVE_LITERAL",
                                "This pattern literal does not fit double.",
                                pattern.span,
                            ),
                        })?)
                    }
                    Ty::Int => {
                        let text = text.replace('_', "");
                        let negative = text.starts_with('-');
                        let magnitude = text.trim_start_matches('-');
                        let magnitude = if let Some(hex) = magnitude
                            .strip_prefix("0x")
                            .or_else(|| magnitude.strip_prefix("0X"))
                        {
                            u64::from_str_radix(hex, 16)
                        } else {
                            magnitude.parse::<u64>()
                        };
                        let number = magnitude.ok().and_then(|n| {
                            if negative && n == 1u64 << 63 {
                                Some(i64::MIN)
                            } else {
                                i64::try_from(n).ok().map(|n| if negative { -n } else { n })
                            }
                        });
                        Atom::Int(number.ok_or_else(|| LocatedDiagnostic {
                            module: self.module,
                            diagnostic: Diagnostic::new(
                                "E_INTEGER_LITERAL_RANGE",
                                "This pattern literal does not fit Int.",
                                pattern.span,
                            ),
                        })?)
                    }
                    _ => return self.unsupported(pattern, "this literal pattern"),
                };
                Ok(Test::Equal(value.native, value.atom, atom))
            }
            K::PatternRange | K::PatternRangeInclusive => {
                if !matches!(value.native, Ty::Int | Ty::Nat) {
                    return self.unsupported(pattern, "a range outside an Int or Nat match");
                }
                let lower = self.pattern_integer_bound(pattern, &pattern.children[0])?;
                let upper = self.pattern_integer_bound(pattern, &pattern.children[1])?;
                let atom = value.atom;
                let test = Test::Not(Box::new(Test::Less(
                    value.native,
                    atom.clone(),
                    Atom::Int(lower),
                )));
                let upper = if pattern.kind == K::PatternRangeInclusive {
                    Test::Not(Box::new(Test::Less(value.native, Atom::Int(upper), atom)))
                } else {
                    Test::Less(value.native, atom, Atom::Int(upper))
                };
                Ok(Test::And(Box::new(test), Box::new(upper)))
            }
            K::PatternList => {
                let id = match value.native {
                    Ty::Record(id) => id,
                    _ => return self.unsupported(pattern, "this list pattern"),
                };
                let LayoutKind::List(element) = self.compiler.layouts[id].kind else {
                    return self.unsupported(pattern, "this list pattern");
                };
                let rest_at = pattern
                    .children
                    .iter()
                    .position(|p| p.kind == K::PatternRest);
                let rest = rest_at.is_some();
                let count = pattern.children.len() - usize::from(rest);
                let prefix = rest_at.unwrap_or(count);
                let suffix = count - prefix;
                let mut test = Test::ListLength(value.atom.clone(), count, !rest);
                let parts = pattern
                    .children
                    .iter()
                    .filter(|p| p.kind != K::PatternRest)
                    .collect::<Vec<_>>();
                for (index, child) in parts.into_iter().enumerate() {
                    // Parts after the rest count back from the end of the list.
                    let position = if index < prefix {
                        Atom::Int(index as i64)
                    } else {
                        Atom::LengthLess(Box::new(value.atom.clone()), count - index)
                    };
                    let item = self.borrow_value(Value {
                        native: element,
                        atom: Atom::ListItem(
                            Box::new(value.atom.clone()),
                            Box::new(position),
                            element,
                        ),
                        ty: self.compiler.source_type(element),
                        keepalive: value.keepalive.clone(),
                        ..Value::unit()
                    });
                    let inner = self.pattern_bind_into(child, item, bindings)?;
                    test = Test::And(Box::new(test), Box::new(inner));
                }
                if let Some(at) = rest_at
                    && let Some(name) = pattern.children[at].children.first()
                {
                    let slot = self.local(value.native, false, true);
                    bindings.push(Statement::Set(
                        slot,
                        Rhs::ListSlice(id, value.atom, prefix, suffix),
                        self.location(pattern),
                    ));
                    let identifier = name;
                    let name = self.text(name).to_owned();
                    let rest = Value {
                        native: value.native,
                        atom: Atom::Slot(slot),
                        ty: value.ty,
                        keepalive: value.keepalive,
                        ..Value::unit()
                    };
                    if self.entry && self.compiler.session && self.scopes.len() == 1 {
                        self.compiler.global_bindings.insert(
                            DeclarationId {
                                file: self.compiler.project.modules[self.module]
                                    .parsed
                                    .source
                                    .name
                                    .clone(),
                                span: identifier.span,
                            },
                            rest.clone(),
                        );
                    }
                    self.scopes.last_mut().unwrap().names.insert(name, rest);
                }
                Ok(test)
            }
            K::PatternTuple => {
                let id = match value.native {
                    Ty::Record(id) | Ty::Ref(id) => id,
                    _ => return self.unsupported(pattern, "this tuple pattern"),
                };
                let fields = self.compiler.layouts[id].fields.clone();
                let mut test = Test::Yes;
                for (i, (pattern, field)) in pattern.children.iter().zip(fields).enumerate() {
                    let child = Value {
                        keepalive: BTreeSet::new(),
                        atom: Atom::Field(Box::new(value.atom.clone()), i, field.ty),
                        native: field.ty,
                        ty: self.compiler.source_type(field.ty),
                        temporary: false,
                        callable: None,
                    };
                    let child = if matches!(value.native, Ty::Ref(_)) {
                        self.borrow_value(child)
                    } else {
                        child
                    };
                    test = Test::And(
                        Box::new(test),
                        Box::new(self.pattern_bind_into(pattern, child, bindings)?),
                    );
                }
                Ok(test)
            }
            K::PatternConstructor if value.native == Ty::Nat => {
                let name = self.text(&pattern.children[0]).rsplit('.').next().unwrap();
                let zero = Test::Equal(Ty::Nat, value.atom.clone(), Atom::Nat(0));
                match (name, pattern.children.get(1)) {
                    ("Zero", None) => Ok(zero),
                    ("Succ", Some(inner)) => {
                        let child = Value {
                            native: Ty::Nat,
                            atom: Atom::NatPred(Box::new(value.atom.clone())),
                            ty: Type::named("Nat"),
                            ..Value::unit()
                        };
                        let inner = self.pattern_bind_into(inner, child, bindings)?;
                        Ok(Test::And(
                            Box::new(Test::Not(Box::new(zero))),
                            Box::new(inner),
                        ))
                    }
                    _ => self.unsupported(pattern, "this Nat pattern"),
                }
            }
            K::PatternConstructor => {
                let id = match value.native {
                    Ty::Record(id) | Ty::Ref(id) => id,
                    _ => return self.unsupported(pattern, "this case pattern"),
                };
                let (LayoutKind::Enum(cases) | LayoutKind::Step(cases)) =
                    &self.compiler.layouts[id].kind
                else {
                    return self.unsupported(pattern, "a case of this layout");
                };
                let name = self.text(&pattern.children[0]).rsplit('.').next().unwrap();
                let Some((case, variant)) = cases.iter().enumerate().find(|(_, c)| c.name == name)
                else {
                    return self.unsupported(pattern, "this enum case");
                };
                let fields = variant.fields.clone();
                let names = variant.names.clone();
                let mut test = Test::Equal(
                    Ty::Int,
                    Atom::Tag(Box::new(value.atom.clone())),
                    Atom::Int((case + 1) as i64),
                );
                // A named payload reaches its field by label, so the lowering
                // puts the patterns in field order first.
                let mut arguments: Vec<Option<&Node>> = vec![None; fields.len()];
                let mut positional = 0;
                for child in pattern.children.iter().skip(1) {
                    if child.kind == K::NamedPattern {
                        let label = child.children.first().map(|n| self.text(n));
                        let slot = names.iter().position(|name| name.as_deref() == label);
                        if let (Some(slot), Some(value)) = (slot, child.children.last()) {
                            arguments[slot] = Some(value);
                        }
                    } else if positional < arguments.len() {
                        arguments[positional] = Some(child);
                        positional += 1;
                    }
                }
                for (index, (pattern, native)) in arguments.into_iter().zip(fields).enumerate() {
                    let Some(pattern) = pattern else {
                        continue;
                    };
                    let child = Value {
                        keepalive: BTreeSet::new(),
                        atom: Atom::Payload(Box::new(value.atom.clone()), case, index, native),
                        native,
                        ty: self.compiler.source_type(native),
                        temporary: false,
                        callable: None,
                    };
                    let child = if matches!(value.native, Ty::Ref(_)) {
                        self.borrow_value(child)
                    } else {
                        child
                    };
                    test = Test::And(
                        Box::new(test),
                        Box::new(self.pattern_bind_into(pattern, child, bindings)?),
                    );
                }
                Ok(test)
            }
            _ => self.unsupported(pattern, "this pattern"),
        }
    }
    /// The arms' common layout, and whether owned arms must box beside borrowed ones.
    fn match_native(&mut self, node: &Node, value: &Value) -> Result<(Ty, bool)> {
        if matches!(self.ty(node), Type::Union(_)) {
            return Ok((self.native_type(&self.ty(node), node)?, false));
        }

        let mut result = Ty::Unit;
        let mut mixed = false;
        for arm in node.children.iter().skip(1) {
            self.scopes.push(Scope {
                finalizers: vec![],
                names: BTreeMap::new(),
                owned: vec![],
            });
            // Every alternative binds the same names, so the first one gives the
            // arm body the bindings it reads.
            let pattern = &arm.children[0];
            let pattern = if pattern.kind == K::PatternAlt {
                pattern.children.first().unwrap_or(pattern)
            } else {
                pattern
            };
            self.pattern_bind(pattern, value.clone())?;
            let native = self.node_native(arm.children.last().unwrap())?;
            self.scopes.pop();
            if native != Ty::Unit {
                if let Some(id) = mixed_view(result, native) {
                    result = Ty::Ref(id);
                    mixed = true;
                    continue;
                }
                if result != Ty::Unit && result != native {
                    if let Some(view) =
                        self.merge_class_views(result, native, &self.ty(node), node)?
                    {
                        result = view;
                        continue;
                    }
                    return self.error(
                        node,
                        "E_NATIVE_OWNERSHIP",
                        "These match arms mix owned values and borrows.",
                    );
                }
                result = native;
            }
        }
        Ok((result, mixed))
    }
    /// `match (a, b) { ... }` reads its parts, so a part that does not copy enters the
    /// tuple as a borrowed view instead of being taken (REVIEW 1.6).
    fn view_tuple(&mut self, tuple: &Node) -> Result<Option<Value>> {
        let mut parts = vec![];
        for child in &tuple.children {
            parts.push(self.expression(child)?);
        }
        if parts.iter().all(|part| crate::types::copies(&part.ty)) {
            let native = self.native_type(&self.ty(tuple), tuple)?;
            let Ty::Record(id) = native else {
                return Ok(None);
            };
            let mut values = vec![];
            for (part, child) in parts.into_iter().zip(&tuple.children) {
                let part = self.freeze_owned(part, child)?;
                values.push((part.atom, part.native));
            }
            return Ok(Some(self.temporary(
                native,
                Rhs::Aggregate(id, None, values),
                tuple,
            )));
        }
        let mut types = vec![];
        let mut values = vec![];
        let mut keepalive = BTreeSet::new();
        for (part, child) in parts.into_iter().zip(&tuple.children) {
            let part = if crate::types::copies(&part.ty) {
                self.freeze_owned(part, child)?
            } else {
                self.borrow_value(part)
            };
            keepalive.extend(part.keepalive.iter().copied());
            types.push(if matches!(part.native, Ty::Ref(_)) {
                Type::StoredView(Box::new(part.ty.clone()))
            } else {
                part.ty.clone()
            });
            values.push((part.atom, part.native));
        }
        let ty = Type::Tuple(types);
        let native = self.native_type(&ty, tuple)?;
        let Ty::Record(id) = native else {
            return Ok(None);
        };
        let mut value = self.temporary(native, Rhs::Aggregate(id, None, values), tuple);
        value.ty = ty;
        value.keepalive = keepalive;
        Ok(Some(value))
    }
    fn match_expression(&mut self, node: &Node) -> Result<Value> {
        let scrutinee = ungroup_node(&node.children[0]);
        let value = match scrutinee.kind {
            K::Tuple => match self.view_tuple(scrutinee)? {
                Some(value) => value,
                None => return self.unsupported(scrutinee, "this tuple in a match"),
            },
            _ => self.expression(&node.children[0])?,
        };
        let value = if matches!(value.native, Ty::Record(_)) {
            value
        } else {
            self.freeze(value, &node.children[0])?
        };
        let (native, mixed) = self.match_native(node, &value)?;
        let slot = (native != Ty::Unit).then(|| self.local(native, false, native.owns()));
        let mixed = match native {
            Ty::Ref(id) if mixed => {
                let slot = self.local(Ty::Boxed(id), false, true);
                self.result_boxes.insert(slot);
                Some(slot)
            }
            _ => None,
        };
        let id = self.next_loop;
        self.next_loop += 1;
        let mut arms = vec![];
        let mut owners = BTreeSet::new();
        for arm in node.children.iter().skip(1) {
            // An alternative arm becomes one arm per alternative, all with the same
            // guard and body, so the body runs once for whichever one matched.
            let alternates: Vec<&Node> = if arm.children[0].kind == K::PatternAlt {
                arm.children[0].children.iter().collect()
            } else {
                vec![&arm.children[0]]
            };
            for pattern in alternates {
                let mut lowered = None;
                let (_, cleanup) = self.isolated(|builder| {
                    let (test, bindings) = builder.pattern_bind(pattern, value.clone())?;
                    let condition = builder.temporary(Ty::Bool, Rhs::Test(test), pattern);
                    let test = std::mem::take(&mut builder.code);
                    let guard = if let Some(guard) = arm.child(K::Guard) {
                        let (value, code) =
                            builder.isolated(|builder| builder.expression(&guard.children[0]))?;
                        Some((code, value.atom))
                    } else {
                        None
                    };
                    let (_, body) = builder.isolated(|builder| {
                        let expression = arm.children.last().unwrap();
                        let result = builder.expression(expression)?;
                        let result = builder.box_branch(result, mixed, expression)?;
                        owners.extend(result.keepalive.iter().copied());
                        if !ends_with_return(&builder.code)
                            && let Some(slot) = slot
                        {
                            builder.assign(slot, result, expression)?;
                        }
                        Ok(Value::unit())
                    })?;
                    lowered = Some(MatchArm {
                        test,
                        condition: condition.atom,
                        bindings,
                        guard,
                        body,
                        cleanup: vec![],
                    });
                    Ok(Value::unit())
                })?;
                let mut arm = lowered.unwrap();
                arm.cleanup = cleanup;
                arms.push(arm);
            }
        }
        self.code.push(Statement::Match(id, arms));
        owners.extend(mixed);
        Ok(slot
            .map(|slot| Value {
                keepalive: owners,
                native,
                atom: Atom::Slot(slot),
                ty: self.compiler.source_type(native),
                temporary: true,
                callable: None,
            })
            .unwrap_or_else(Value::unit))
    }

    /// Read a `Type.field` static final or var. Build the initializer
    /// function on first use; the native getter handles initialization.
    fn static_read(&mut self, node: &Node) -> Result<Value> {
        let Some(key) = self.checked().static_references.get(&node.span).cloned() else {
            return self.unsupported(node, "this static field");
        };
        let index = self.static_index(&key, node)?;
        let ty = self.compiler.statics[index].ty;
        Ok(self.temporary(ty, Rhs::StaticRead(index), node))
    }

    /// Intern a static field and build its initializer function once.
    fn static_index(&mut self, key: &(String, String), node: &Node) -> Result<usize> {
        let index = if let Some(index) = self.compiler.static_ids.get(key).copied() {
            index
        } else {
            let Some(info) = self
                .compiler
                .project
                .modules
                .iter()
                .find_map(|module| module.checked.static_fields.get(key).cloned())
            else {
                return self.error(
                    node,
                    "E_NATIVE_STATIC",
                    "This static field has no recorded initializer.",
                );
            };
            let ty = self.native_type(&info.ty, node)?;
            let index = self.compiler.statics.len();
            self.compiler.statics.push(StaticField {
                key: format!("{}.{}", key.0, key.1),
                ty,
                function: usize::MAX,
                mutable: info.mutable,
            });
            self.compiler.static_ids.insert(key.clone(), index);
            self.compiler.static_infos.insert(index, info);
            index
        };
        if self.compiler.statics[index].function == usize::MAX {
            if !self.compiler.static_visiting.insert(index) {
                return self.error(
                    node,
                    "E_NATIVE_STATIC_CYCLE",
                    "This static initializer depends on itself.",
                );
            }
            let info = self.compiler.static_infos[&index].clone();
            let Some(module) = self
                .compiler
                .project
                .modules
                .iter()
                .position(|module| module.parsed.source.name == info.file)
            else {
                return self.error(
                    node,
                    "E_NATIVE_STATIC",
                    "The static initializer's module is missing.",
                );
            };
            let Some(initializer) = self.compiler.nodes[module].get(&info.initializer).copied()
            else {
                return self.error(
                    node,
                    "E_NATIVE_STATIC",
                    "The static initializer is missing.",
                );
            };
            let native = self.compiler.statics[index].ty;
            let mut builder = Builder::new(
                &mut *self.compiler,
                module,
                Mono::default(),
                format!("static {}.{}", key.0, key.1),
                native,
                false,
            );
            let value = builder.expression(initializer)?;
            builder.return_value(value, initializer)?;
            let (compiler, function) = builder.finish_function();
            let id = compiler.functions.len();
            compiler.functions.push(Some(function));
            compiler.statics[index].function = id;
            compiler.static_visiting.remove(&index);
        }
        Ok(index)
    }

    /// Assign to a synchronized static var. The value is copied into the
    /// stored slot, so the caller's copy stays valid.
    fn static_write(&mut self, node: &Node) -> Result<Value> {
        let target = &node.children[0];
        let Some(key) = self.checked().static_references.get(&target.span).cloned() else {
            return self.unsupported(node, "this static assignment");
        };
        let index = self.static_index(&key, node)?;
        if !self.compiler.statics[index].mutable {
            return self.error(
                node,
                "E_STATIC_ASSIGNMENT",
                "A static final field cannot be assigned.",
            );
        }
        let ty = self.compiler.statics[index].ty;
        let value = self.expression(&node.children[1])?;
        let value = if value.native == ty {
            value
        } else {
            let rhs = self.transfer(value, ty, node)?;
            self.temporary(ty, rhs, node)
        };
        self.code.push(Statement::Evaluate(
            Rhs::StaticWrite(index, value.atom),
            self.location(node),
        ));
        Ok(Value::unit())
    }

    fn member(&mut self, node: &Node, storing: bool) -> Result<Value> {
        if matches!(self.ty(&node.children[0]), Type::Namespace(_)) {
            return self.expression(&Node::leaf(K::Name, node.span));
        }
        let owner = self.expression(&node.children[0])?;
        // Fields are reached through a Box with no extra marker.
        let owner = if matches!(owner.native, Ty::Boxed(_)) {
            self.borrow_value(owner)
        } else {
            owner
        };
        let id = match owner.native {
            Ty::Record(id) | Ty::Ref(id) => id,
            _ => return self.method_value(node, owner),
        };
        let name = self.text(&node.children[1]);
        if name == "T" {
            if storing {
                return Err(LocatedDiagnostic {
                    module: self.module,
                    diagnostic: Diagnostic::new(
                        "E_TENSOR_VIEW_WRITE",
                        "A transpose view is read-only.",
                        node.span,
                    ),
                });
            }
            let native = self.native_type(&self.ty(node), node)?;
            let mut owners = owner.keepalive.clone();
            if owner.temporary
                && let Atom::Slot(slot) = owner.atom
                && owner.native.owns()
            {
                owners.insert(slot);
            }
            let mut value = self.temporary(
                native,
                Rhs::Builtin("tensor.transposeView".into(), native, vec![owner.atom]),
                node,
            );
            value.keepalive = owners;
            return Ok(value);
        }
        if name == "value"
            && let LayoutKind::Resource(Resource::Guard(payload)) = self.compiler.layouts[id].kind
        {
            let owner = self.borrow_value(owner);
            let value = Value {
                keepalive: owner.keepalive,
                atom: Atom::ResourceValue(Box::new(owner.atom), payload),
                native: payload,
                ty: self.compiler.source_type(payload),
                temporary: false,
                callable: None,
            };
            return Ok(if storing {
                value
            } else {
                self.borrow_value(value)
            });
        }
        let Some((index, field)) = self.compiler.layouts[id]
            .fields
            .iter()
            .enumerate()
            .find(|(_, field)| field.name == name)
        else {
            return self.method_value(node, owner);
        };
        let native = field.ty;
        let borrowed_owner = matches!(owner.native, Ty::Ref(_))
            || matches!(self.compiler.layouts[id].kind, LayoutKind::ClassView(_));
        let mut owners = owner.keepalive.clone();
        if owner.temporary
            && let Atom::Slot(slot) = owner.atom
            && owner.native.owns()
        {
            owners.insert(slot);
        }
        let value = Value {
            keepalive: owners,
            atom: Atom::Field(Box::new(owner.atom), index, native),
            ty: self.compiler.source_type(native),
            native,
            temporary: false,
            callable: None,
        };
        Ok(if borrowed_owner && !storing {
            self.borrow_value(value)
        } else {
            value
        })
    }

    /// `c.increment` without a call: a partial with every argument open that keeps `c`.
    fn method_value(&mut self, node: &Node, owner: Value) -> Result<Value> {
        let owner = if crate::types::copies(&owner.ty) {
            let mut owner = self.freeze(owner, &node.children[0])?;
            if !self
                .compiler
                .holds_borrow(owner.native, &mut BTreeSet::new())
            {
                owner.keepalive.clear();
            }
            owner
        } else {
            owner
        };
        let Type::Function(signature) = self.ty(node) else {
            return self.unsupported(node, "an overloaded method value");
        };
        let Some(target) = signature.callable.clone() else {
            return self.unsupported(node, "this method value");
        };
        let call = ResolvedCall {
            target,
            callee: Some(node.span),
            overload: 0,
            arguments: signature
                .parameters
                .iter()
                .map(|_| Argument::Supplied {
                    expression: node.span,
                    hole: true,
                })
                .collect(),
            signature,
            type_arguments: BTreeMap::new(),
            evaluation_order: vec![],
            receiver: Some(node.children[0].span),
            partial: true,
        };
        Ok(Value {
            keepalive: BTreeSet::new(),
            native: Ty::Unit,
            atom: Atom::Unit,
            ty: self.ty(node),
            temporary: true,
            callable: Some(Box::new(FunctionValue::Partial {
                module: self.module,
                call: Box::new(call),
                captures: vec![],
                receiver: Some(Box::new(owner)),
            })),
        })
    }

    fn function_value(&self, node: &Node) -> Result<Value> {
        if matches!(self.ty(node), Type::Function(_) | Type::Overloaded(_)) {
            Ok(Value {
                keepalive: BTreeSet::new(),
                native: Ty::Unit,
                atom: Atom::Unit,
                ty: self.ty(node),
                temporary: false,
                callable: Some(Box::new(FunctionValue::Named)),
            })
        } else {
            self.unsupported(node, &format!("value {}", self.text(node)))
        }
    }

    fn append_list(
        &mut self,
        id: usize,
        output: &mut Value,
        value: Value,
        cell: Option<IterationOwner>,
        node: &Node,
    ) -> Result<()> {
        if self
            .compiler
            .borrow_layout(value.native, &mut BTreeSet::new(), false)
            && cell.is_none()
        {
            return self.error(node,"E_NATIVE_OWNERSHIP","A List entry cannot take a returned borrow or an object that still borrows another owner.");
        }
        output.keepalive.extend(value.keepalive.iter().copied());
        if let Atom::Slot(slot) = output.atom {
            let depth = if slot.global {
                0
            } else {
                self.locals[slot.id].scope
            };
            self.promote_owners(&value.keepalive, depth);
        }
        let rhs = if let Some(IterationOwner::Step(step)) = &cell
            && let LayoutKind::List(element) = self.compiler.layouts[id].kind
            && matches!((element,value.native),(Ty::Record(a) | Ty::Boxed(a),Ty::Ref(b)) | (Ty::DynBox(a),Ty::Dyn(b)) if a == b)
        {
            Rhs::ListAppendOwner(
                id,
                output.atom.clone(),
                value.atom,
                value.native,
                Atom::StepOwner(Box::new(step.clone())),
            )
        } else if let Some(IterationOwner::Collection(source, list, index)) = &cell
            && *source == id
        {
            Rhs::ListAppendFrom(id, output.atom.clone(), list.clone(), index.clone())
        } else if let Some(IterationOwner::Collection(source, set, index)) = &cell
            && matches!(self.compiler.layouts[*source].kind,LayoutKind::Map(info) if info.style!=MapStyle::Map)
            && matches!(value.native, Ty::Ref(_))
        {
            Rhs::ListAppendSet(id, *source, output.atom.clone(), set.clone(), index.clone())
        } else {
            let value = self.freeze_owned(value, node)?;
            if matches!(value.native, Ty::Ref(_)) {
                return self.error(
                    node,
                    "E_NATIVE_OWNERSHIP",
                    "A List cannot take ownership of this returned borrow.",
                );
            }
            Rhs::ListAppend(id, output.atom.clone(), value.atom, value.native)
        };
        self.code
            .push(Statement::Evaluate(rhs, self.location(node)));
        Ok(())
    }
    fn callback(&mut self, function: &Value, arguments: Vec<Value>, node: &Node) -> Result<Value> {
        match function.callable.as_deref() {
            Some(FunctionValue::Lambda {
                module,
                node: lambda,
                captures,
            }) => self.lambda_call(*module, lambda, captures, arguments, node),
            Some(FunctionValue::Partial {
                module,
                call,
                captures,
                receiver,
            }) => {
                let mut provided = arguments.into_iter();
                let mut complete = vec![];
                for (index, argument) in call.arguments.iter().enumerate() {
                    let value = match argument {
                        Argument::Supplied { hole: true, .. } => provided.next().unwrap(),
                        Argument::Supplied { hole: false, .. } => captures[index].clone().unwrap(),
                        Argument::Default => self.default_argument(
                            call,
                            index,
                            node,
                            &mut complete,
                            receiver.as_deref(),
                        )?,
                    };
                    complete.push(value);
                }
                let Type::Function(signature) = &function.ty else {
                    unreachable!()
                };
                let result = self.mono.apply(&signature.result);
                let previous = self.module;
                self.module = *module;
                let value = self.invoke(
                    call,
                    complete,
                    receiver.as_deref().cloned(),
                    node,
                    Some(result),
                );
                self.module = previous;
                value
            }
            Some(FunctionValue::Named) => {
                let Type::Function(signature) = &function.ty else {
                    return self.unsupported(node, "an overloaded callback");
                };
                let Some(target) = &signature.callable else {
                    return self.unsupported(node, "an indirect callback");
                };
                let mut mono = self.mono.clone();
                for (parameter, argument) in signature.parameters.iter().zip(&arguments) {
                    mono.bind(&parameter.ty, &argument.ty);
                }
                let result = mono.apply(&signature.result);
                let selected = ResolvedCall {
                    target: target.clone(),
                    callee: None,
                    overload: 0,
                    signature: signature.clone(),
                    type_arguments: BTreeMap::new(),
                    arguments: vec![],
                    evaluation_order: vec![],
                    receiver: None,
                    partial: false,
                };
                self.invoke(&selected, arguments, None, node, Some(result))
            }
            None if self.is_closure(function.native).is_some() => {
                self.closure_call(function.clone(), arguments, node)
            }
            _ => self.unsupported(node, "this runtime callback"),
        }
    }
    fn for_each<F>(&mut self, sequence: Value, node: &Node, mut body: F) -> Result<()>
    where
        F: FnMut(&mut Self, Value, Option<IterationOwner>) -> Result<()>,
    {
        let (collection, steps) =
            if let Some(FunctionValue::Sequence { source, steps }) = sequence.callable.as_deref() {
                (source.as_ref().clone(), steps.clone())
            } else {
                (sequence, vec![])
            };
        if let Some(FunctionValue::Range { start, stop, step }) = collection.callable.as_deref() {
            let start = start.clone();
            let stop = stop.clone();
            let step = step.clone();
            return self.range_loop(start, stop, step, steps, node, &mut body);
        }
        let collection = self.freeze(collection, node)?;
        // A String loops over its characters, as the List that split("") makes (chosen 2026-11-08).
        let collection = if collection.native == Ty::String {
            self.string_characters(collection.atom.clone(), node)?
        } else {
            collection
        };
        if matches!(collection.native, Ty::Dyn(_) | Ty::DynBox(_)) {
            return self.cursor_loop(collection, steps, node, &mut body);
        }
        if let Ty::Record(layout) | Ty::Ref(layout) = collection.native
            && matches!(
                self.compiler.layouts[layout].kind,
                LayoutKind::Class | LayoutKind::Enum(_)
            )
        {
            return self.cursor_loop(collection, steps, node, &mut body);
        }
        // An Array is often lent to a loop by reference, so both forms name its layout.
        let (Ty::Record(layout) | Ty::Ref(layout)) = collection.native else {
            return self.unsupported(node, "this Iterable layout");
        };
        let array = matches!(self.compiler.layouts[layout].kind, LayoutKind::Array(_));
        let (element, map_item, set) = match self.compiler.layouts[layout].kind {
            LayoutKind::List(element) | LayoutKind::Array(element) => (element, None, None),
            LayoutKind::Map(info) if info.style == MapStyle::Map => {
                (Ty::Ref(info.item), Some(info.item), None)
            }
            LayoutKind::Map(info) => (info.key, None, Some(info)),
            _ => return self.unsupported(node, "this Iterable layout"),
        };
        self.scopes.push(Scope {
            finalizers: vec![],
            names: BTreeMap::new(),
            owned: vec![],
        });
        let index = self.local(Ty::Int, false, false);
        self.code.push(Statement::Set(
            index,
            Rhs::Copy(Atom::Int(0)),
            self.location(node),
        ));
        let id = self.next_loop;
        self.next_loop += 1;
        let (condition, condition_code) = self.isolated(|builder| {
            Ok(builder.temporary(
                Ty::Bool,
                Rhs::Binary(
                    "<".into(),
                    Ty::Int,
                    Atom::Slot(index),
                    if map_item.is_some() || set.is_some_and(|s| s.style == MapStyle::Set) {
                        Atom::MapCapacity(Box::new(collection.atom.clone()))
                    } else if array {
                        Atom::ArrayLength(Box::new(collection.atom.clone()))
                    } else {
                        Atom::CollectionLength(Box::new(collection.atom.clone()))
                    },
                ),
                node,
            ))
        })?;
        let depth = self.scopes.len();
        self.loops.push((id, depth));
        let (_, code) = self.isolated(|builder| {
            if map_item.is_some() || set.is_some_and(|s| s.style == MapStyle::Set) {
                builder.code.push(Statement::If(
                    Atom::MapOccupied(
                        Box::new(collection.atom.clone()),
                        Box::new(Atom::Slot(index)),
                    ),
                    vec![],
                    vec![Statement::Continue(id, builder.cleanups(depth))],
                ));
            }
            let value = Value {
                keepalive: collection.keepalive.clone(),
                native: element,
                atom: if let Some(info) = set {
                    Atom::SetItem(
                        Box::new(collection.atom.clone()),
                        Box::new(Atom::Slot(index)),
                        info.key,
                        info.style == MapStyle::SortedSet,
                    )
                } else if let Some(item) = map_item {
                    Atom::MapItem(
                        Box::new(collection.atom.clone()),
                        Box::new(Atom::Slot(index)),
                        item,
                    )
                } else if array {
                    Atom::ArrayItem(
                        Box::new(collection.atom.clone()),
                        Box::new(Atom::Slot(index)),
                        element,
                    )
                } else {
                    Atom::ListItem(
                        Box::new(collection.atom.clone()),
                        Box::new(Atom::Slot(index)),
                        element,
                    )
                },
                ty: builder.compiler.source_type(element),
                temporary: false,
                callable: None,
            };
            let mut value = builder.borrow_value(value);
            let mut cell = if map_item.is_none() && !array {
                Some(IterationOwner::Collection(
                    layout,
                    collection.atom.clone(),
                    Atom::Slot(index),
                ))
            } else {
                None
            };
            for step in &steps {
                match step {
                    SequenceStep::Map(function) => {
                        value = builder.callback(function, vec![value], node)?;
                        cell = None;
                    }
                    SequenceStep::Filter(function) => {
                        let predicate = builder.callback(function, vec![value.clone()], node)?;
                        builder.code.push(Statement::If(
                            predicate.atom,
                            vec![],
                            vec![Statement::Continue(id, builder.cleanups(depth))],
                        ));
                    }
                }
            }
            body(builder, value, cell)?;
            Ok(Value::unit())
        })?;
        self.loops.pop();
        let step = vec![Statement::Set(
            index,
            Rhs::Binary("+".into(), Ty::Int, Atom::Slot(index), Atom::Int(1)),
            self.location(node),
        )];
        self.code.push(Statement::Loop {
            id,
            condition: condition_code,
            value: condition.atom,
            body: code,
            step,
        });
        self.finish_scope();
        Ok(())
    }
    fn range_loop<F>(
        &mut self,
        start: Atom,
        stop: Atom,
        step: Atom,
        steps: Vec<SequenceStep>,
        node: &Node,
        body: &mut F,
    ) -> Result<()>
    where
        F: FnMut(&mut Self, Value, Option<IterationOwner>) -> Result<()>,
    {
        self.scopes.push(Scope {
            finalizers: vec![],
            names: BTreeMap::new(),
            owned: vec![],
        });
        let current = self.local(Ty::Int, false, false);
        self.code.push(Statement::Set(
            current,
            Rhs::Copy(start),
            self.location(node),
        ));
        let (condition, condition_code) = self.isolated(|builder| {
            let positive = builder.temporary(
                Ty::Bool,
                Rhs::Binary(">".into(), Ty::Int, step.clone(), Atom::Int(0)),
                node,
            );
            let negative = builder.temporary(
                Ty::Bool,
                Rhs::Binary("<".into(), Ty::Int, step.clone(), Atom::Int(0)),
                node,
            );
            let ascending = builder.temporary(
                Ty::Bool,
                Rhs::Binary("<".into(), Ty::Int, Atom::Slot(current), stop.clone()),
                node,
            );
            let descending = builder.temporary(
                Ty::Bool,
                Rhs::Binary(">".into(), Ty::Int, Atom::Slot(current), stop.clone()),
                node,
            );
            let condition = builder.local(Ty::Bool, false, false);
            builder.code.push(Statement::Set(
                condition,
                Rhs::Copy(Atom::Bool(false)),
                builder.location(node),
            ));
            builder.code.push(Statement::If(
                positive.atom,
                vec![Statement::Set(
                    condition,
                    Rhs::Copy(ascending.atom),
                    builder.location(node),
                )],
                vec![],
            ));
            builder.code.push(Statement::If(
                negative.atom,
                vec![Statement::Set(
                    condition,
                    Rhs::Copy(descending.atom),
                    builder.location(node),
                )],
                vec![],
            ));
            Ok(Value {
                keepalive: BTreeSet::new(),
                native: Ty::Bool,
                atom: Atom::Slot(condition),
                ty: Type::named("Bool"),
                temporary: true,
                callable: None,
            })
        })?;
        let loop_id = self.next_loop;
        self.next_loop += 1;
        let depth = self.scopes.len();
        self.loops.push((loop_id, depth));
        let (_, code) = self.isolated(|builder| {
            let mut value = Value {
                keepalive: BTreeSet::new(),
                native: Ty::Int,
                atom: Atom::Slot(current),
                ty: Type::named("Int"),
                temporary: false,
                callable: None,
            };
            for sequence_step in &steps {
                match sequence_step {
                    SequenceStep::Map(function) => {
                        value = builder.callback(function, vec![value], node)?;
                    }
                    SequenceStep::Filter(function) => {
                        let predicate = builder.callback(function, vec![value.clone()], node)?;
                        builder.code.push(Statement::If(
                            predicate.atom,
                            vec![],
                            vec![Statement::Continue(loop_id, builder.cleanups(depth))],
                        ));
                    }
                }
            }
            body(builder, value, None)?;
            Ok(Value::unit())
        })?;
        self.loops.pop();
        let step_code = vec![Statement::Set(
            current,
            Rhs::Binary("+".into(), Ty::Int, Atom::Slot(current), step),
            self.location(node),
        )];
        self.code.push(Statement::Loop {
            id: loop_id,
            condition: condition_code,
            value: condition.atom,
            body: code,
            step: step_code,
        });
        self.finish_scope();
        Ok(())
    }

    /// A user Iterable: `items()` gives a cursor, and `next()` runs until it returns None.
    fn cursor_loop<F>(
        &mut self,
        collection: Value,
        steps: Vec<SequenceStep>,
        node: &Node,
        body: &mut F,
    ) -> Result<()>
    where
        F: FnMut(&mut Self, Value, Option<IterationOwner>) -> Result<()>,
    {
        let dynamic = match collection.native {
            Ty::Dyn(id) | Ty::DynBox(id) => Some(id),
            _ => None,
        };
        let actual = self.compiler.source_type(collection.native);
        let cursor_type = if let Some(id) = dynamic {
            let LayoutKind::Trait(info) = &self.compiler.layouts[id].kind else {
                unreachable!()
            };
            let Some(items) = info.methods.iter().find(|method| method.name == "items") else {
                return self.unsupported(node, "this Iterable view");
            };
            self.compiler.source_type(items.result)
        } else {
            let Some(items) = self
                .compiler
                .methods(&actual, "items")
                .into_iter()
                .find(|m| m.parameters.is_empty())
            else {
                return self.unsupported(node, "this Iterable layout");
            };
            self.mono.apply(&items.result)
        };
        self.scopes.push(Scope {
            finalizers: vec![],
            names: BTreeMap::new(),
            owned: vec![],
        });
        let cursor = if let Some(id) = dynamic {
            let collection = self.borrow_value(collection);
            self.dyn_call(id, "items", collection, vec![], node)?
        } else {
            self.method_call(collection, "items", vec![], cursor_type.clone(), node)?
        };
        let native = cursor.native;
        let slot = self.local(native, false, native.owns());
        self.assign(slot, cursor, node)?;
        let cursor = Value {
            native,
            atom: Atom::Slot(slot),
            ty: cursor_type.clone(),
            ..Value::unit()
        };
        let dynamic_cursor = match native {
            Ty::DynBox(id) | Ty::Dyn(id) => Some(id),
            _ => None,
        };
        let option_type = if let Some(id) = dynamic_cursor {
            let LayoutKind::Trait(info) = &self.compiler.layouts[id].kind else {
                unreachable!()
            };
            self.compiler.source_type(info.methods[0].result)
        } else {
            let Some(next) = self
                .compiler
                .methods(&cursor_type, "next")
                .into_iter()
                .find(|m| m.parameters.is_empty())
            else {
                return self.unsupported(node, "this cursor");
            };
            self.mono.apply(&next.result)
        };
        let id = self.next_loop;
        self.next_loop += 1;
        let depth = self.scopes.len();
        self.loops.push((id, depth));
        let (_, code) = self.isolated(|builder| {
            let receiver = builder.borrow_value(cursor.clone());
            let item = if let Some(id) = dynamic_cursor {
                builder.dyn_call(id, "next", receiver, vec![], node)?
            } else {
                builder.method_call(receiver, "next", vec![], option_type.clone(), node)?
            };
            let Ty::Record(option) = item.native else {
                return builder.unsupported(node, "this cursor result");
            };
            // Option's cases are Some (tag 1) and None (tag 2).
            let stop = builder.temporary(
                Ty::Bool,
                Rhs::Test(Test::Equal(
                    Ty::Int,
                    Atom::Tag(Box::new(item.atom.clone())),
                    Atom::Int(2),
                )),
                node,
            );
            let exit = builder.cleanups(depth);
            builder.code.push(Statement::If(
                stop.atom,
                vec![Statement::Break(id, exit)],
                vec![],
            ));
            let payload = builder.compiler.layouts[option].kind.cases().unwrap()[0].fields[0];
            let mut value = Value {
                native: payload,
                atom: Atom::Payload(Box::new(item.atom.clone()), 0, 0, payload),
                ty: builder.compiler.source_type(payload),
                ..Value::unit()
            };
            if let Atom::Slot(slot) = item.atom {
                value.keepalive.insert(slot);
            }
            let mut cell = matches!(builder.compiler.layouts[option].kind, LayoutKind::Step(_))
                .then(|| IterationOwner::Step(item.atom.clone()));
            for step in &steps {
                match step {
                    SequenceStep::Map(function) => {
                        value = builder.callback(function, vec![value], node)?;
                        cell = None;
                    }
                    SequenceStep::Filter(function) => {
                        let predicate = builder.callback(function, vec![value.clone()], node)?;
                        builder.code.push(Statement::If(
                            predicate.atom,
                            vec![],
                            vec![Statement::Continue(id, builder.cleanups(depth))],
                        ));
                    }
                }
            }
            body(builder, value, cell)?;
            Ok(Value::unit())
        })?;
        self.loops.pop();
        self.code.push(Statement::Loop {
            id,
            condition: vec![],
            value: Atom::Bool(true),
            body: code,
            step: vec![],
        });
        self.finish_scope();
        Ok(())
    }
    fn list_loop(&mut self, node: &Node) -> Result<Value> {
        let binding = &node.children[0];
        let collection = self.expression(binding.children.last().unwrap())?;
        self.for_each(collection, node, |builder, value, _| {
            let pattern = binding
                .children
                .iter()
                .find(|n| {
                    matches!(
                        n.kind,
                        K::PatternName | K::PatternTuple | K::PatternWildcard
                    )
                })
                .unwrap();
            let (_, bindings) = builder.pattern_bind(pattern, value)?;
            builder.code.extend(bindings);
            builder.expression(node.children.last().unwrap())?;
            Ok(())
        })?;
        Ok(Value::unit())
    }
    fn comprehension(&mut self, node: &Node) -> Result<Value> {
        let native = self.native_type(&self.ty(node), node)?;
        let Ty::Record(id) = native else {
            unreachable!()
        };
        let mut output = self.temporary(native, Rhs::ListNew(id, vec![]), node);
        // Each clause is a pattern, a collection and an optional guard; later clauses loop
        // inside the earlier ones.
        let mut clauses = vec![(&node.children[1], &node.children[2], node.child(K::Guard))];
        clauses.extend(
            node.children
                .iter()
                .filter(|n| n.kind == K::ComprehensionFor)
                .map(|n| (&n.children[0], &n.children[1], n.child(K::Guard))),
        );
        self.comprehension_clauses(node, &clauses, id, &mut output)?;
        Ok(output)
    }

    fn comprehension_clauses(
        &mut self,
        node: &Node,
        clauses: &[(&Node, &Node, Option<&Node>)],
        list: usize,
        output: &mut Value,
    ) -> Result<()> {
        let Some(((pattern, collection, guard), rest)) = clauses.split_first() else {
            let value = self.expression(&node.children[0])?;
            return self.append_list(list, output, value, None, node);
        };
        let collection = self.expression(collection)?;
        self.for_each(collection, node, |builder, value, _| {
            let (_, bindings) = builder.pattern_bind(pattern, value)?;
            builder.code.extend(bindings);
            if let Some(guard) = guard {
                let condition = builder.expression(&guard.children[0])?;
                let (id, depth) = *builder.loops.last().unwrap();
                builder.code.push(Statement::If(
                    condition.atom,
                    vec![],
                    vec![Statement::Continue(id, builder.cleanups(depth))],
                ));
            }
            builder.comprehension_clauses(node, rest, list, output)
        })
    }

    fn loop_statement(&mut self, node: &Node) -> Result<Value> {
        if node.kind == K::For && node.children[0].kind == K::ForBinding {
            return self.list_loop(node);
        }
        self.scopes.push(Scope {
            finalizers: vec![],
            names: BTreeMap::new(),
            owned: vec![],
        });
        let (condition_node, step_node) = if node.kind == K::While {
            (&node.children[0], None)
        } else {
            self.expression(&node.children[0])?;
            (&node.children[1], Some(&node.children[2]))
        };
        let id = self.next_loop;
        self.next_loop += 1;
        let (condition, condition_code) =
            self.isolated(|builder| builder.expression(condition_node))?;
        self.loops.push((id, self.scopes.len()));
        let (_, body) =
            self.isolated(|builder| builder.expression(node.children.last().unwrap()))?;
        self.loops.pop();
        let step = if let Some(step) = step_node {
            self.isolated(|builder| builder.expression(step))?.1
        } else {
            vec![]
        };
        self.code.push(Statement::Loop {
            id,
            condition: condition_code,
            value: condition.atom,
            body,
            step,
        });
        self.finish_scope();
        Ok(Value::unit())
    }

    fn call(&mut self, node: &Node) -> Result<Value> {
        if let Some(operation) = self.checked().python.get(&node.span).cloned() {
            return self.python_operation(node, operation);
        }
        let selected = self
            .checked()
            .resolved_calls
            .get(&node.span)
            .cloned()
            .ok_or_else(|| LocatedDiagnostic {
                module: self.module,
                diagnostic: Diagnostic::new(
                    "E_NATIVE_CALL",
                    "This call has no checked target.",
                    node.span,
                ),
            })?;
        let callee = if node.kind == K::Pipe {
            &node.children[2]
        } else {
            &node.children[0]
        };
        let mutable = selected
            .arguments
            .iter()
            .zip(&selected.signature.parameters)
            .filter_map(|(arg, param)| {
                if param.passing == Passing::Mut
                    && let Argument::Supplied { expression, .. } = arg
                {
                    Some(*expression)
                } else {
                    None
                }
            })
            .collect::<BTreeSet<_>>();
        let mut supplied = BTreeMap::new();
        if node.kind == K::Pipe {
            let value = self.expression(&node.children[0])?;
            let value = if mutable.contains(&node.children[0].span) {
                value
            } else {
                self.freeze(value, &node.children[0])?
            };
            supplied.insert(node.children[0].span, value);
        }
        let callable = match &selected.target {
            Callable::Declaration {
                kind:
                    DeclarationKind::Function
                    | DeclarationKind::StaticMethod
                    | DeclarationKind::Constructor
                    | DeclarationKind::Variant,
                ..
            }
            | Callable::Builtin(_)
                if selected.receiver.is_none()
                    && matches!(callee.kind, K::Name | K::Member | K::BracketApplication) =>
            {
                None
            }
            _ if selected.receiver.is_some() => None,
            _ => Some(self.expression(callee)?),
        };
        let receiver = if let Some(span) = selected.receiver {
            let receiver = self.compiler.nodes[self.module]
                .get(&span)
                .copied()
                .unwrap();
            let value = self.expression(receiver)?;
            let persistent_update = matches!(&selected.target,Callable::Builtin(name) if (name.ends_with(".put") || name.ends_with(".add") || name.ends_with(".remove")))
                && matches!(value.native,Ty::Record(id) if matches!(self.compiler.layouts[id].kind,LayoutKind::Map(info) if !info.mutable));
            Some(
                if persistent_update
                    && (value.temporary || self.consume_receiver == Some(receiver.span))
                {
                    value
                } else {
                    self.freeze(value, receiver)?
                },
            )
        } else {
            None
        };
        for span in &selected.evaluation_order {
            if supplied.contains_key(span) {
                continue;
            }
            let expression = self.compiler.nodes[self.module]
                .get(span)
                .copied()
                .ok_or_else(|| LocatedDiagnostic {
                    module: self.module,
                    diagnostic: Diagnostic::new(
                        "E_NATIVE_CALL",
                        "A call argument has no source expression.",
                        *span,
                    ),
                })?;
            let value = self.expression(expression)?;
            let mode = selected
                .arguments
                .iter()
                .zip(&selected.signature.parameters)
                .find_map(|(arg, param)| {
                    matches!(arg, Argument::Supplied { expression, .. } if expression == span)
                        .then_some(param.passing)
                })
                .unwrap_or(Passing::Borrow);
            let consumes = matches!(
                selected.target,
                Callable::Declaration {
                    kind: DeclarationKind::Variant,
                    ..
                }
            ) || matches!(&selected.target, Callable::Builtin(name) if matches!(name.as_str(), "std.core.Some" | "std.core.Mutex" | "std.core.Atomic" | "std.core.Box" | "std.core.box" | "std.collections.Map" | "std.core.Map" | "std.core.Set" | "std.core.SortedSet") || name.ends_with(".put") || name.ends_with(".add") || name.ends_with(".insert") || name.ends_with(".replace"));
            let value = if (consumes || mode == Passing::Move) && !crate::types::copies(&value.ty) {
                self.move_boxed_payload(value, expression)
            } else {
                value
            };
            let value = if consumes && matches!(value.native, Ty::Record(_) | Ty::Boxed(_)) {
                self.freeze_owned(value, expression)?
            } else if value.native.owns()
                && mode == Passing::Move
                && !crate::types::copies(&value.ty)
            {
                let native = value.native;
                self.temporary(native, Rhs::Take(value.atom), expression)
            } else if mutable.contains(span) {
                self.borrow_value(value)
            } else {
                self.freeze(value, expression)?
            };
            supplied.insert(*span, value);
        }
        if selected.partial {
            let captures = selected
                .arguments
                .iter()
                .map(|argument| match argument {
                    Argument::Supplied {
                        expression,
                        hole: false,
                    } => supplied.get(expression).cloned(),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if captures
                .iter()
                .flatten()
                .any(|value| value.callable.is_some())
            {
                return self.unsupported(node, "a partial call that captures a function value");
            }
            return Ok(Value {
                keepalive: BTreeSet::new(),
                native: Ty::Unit,
                atom: Atom::Unit,
                ty: self.ty(node),
                temporary: true,
                callable: Some(Box::new(FunctionValue::Partial {
                    module: self.module,
                    call: Box::new(selected),
                    captures,
                    receiver: receiver.map(Box::new),
                })),
            });
        }
        if let Some(FunctionValue::Partial {
            module,
            call,
            captures,
            receiver,
        }) = callable
            .as_ref()
            .and_then(|value| value.callable.as_deref())
        {
            let mut provided = selected.arguments.iter();
            let mut complete = vec![];
            for (index, argument) in call.arguments.iter().enumerate() {
                let argument = match argument {
                    Argument::Supplied { hole: true, .. } => provided.next().unwrap(),
                    Argument::Supplied { hole: false, .. } => {
                        complete.push(captures[index].clone().unwrap());
                        continue;
                    }
                    Argument::Default => argument,
                };
                let value = match argument {
                    Argument::Supplied {
                        expression,
                        hole: false,
                    } => supplied.get(expression).cloned().unwrap(),
                    Argument::Default => self.default_argument(
                        call,
                        index,
                        node,
                        &mut complete,
                        receiver.as_deref(),
                    )?,
                    _ => return self.unsupported(node, "an unfilled argument"),
                };
                complete.push(value);
            }
            self.check_mut_arguments(&complete, &call.signature, node)?;
            let result_type = self.mono.apply(&selected.signature.result);
            let previous = self.module;
            self.module = *module;
            let result = self.invoke(
                call,
                complete,
                receiver.as_deref().cloned(),
                node,
                Some(result_type),
            );
            self.module = previous;
            return result;
        }
        let mut arguments = vec![];
        for (index, argument) in selected.arguments.iter().enumerate() {
            let value = match argument {
                Argument::Supplied {
                    expression,
                    hole: false,
                } => supplied.get(expression).cloned().unwrap(),
                Argument::Default => self.default_argument(
                    &selected,
                    index,
                    node,
                    &mut arguments,
                    receiver.as_ref(),
                )?,
                _ => return self.unsupported(node, "an unfilled argument"),
            };
            arguments.push(value);
        }
        self.check_mut_arguments(&arguments, &selected.signature, node)?;
        if let Some(value) = callable {
            match value.callable.as_deref() {
                Some(FunctionValue::Lambda {
                    module,
                    node: lambda,
                    captures,
                }) => return self.lambda_call(*module, lambda, captures, arguments, node),
                Some(FunctionValue::Named) => {
                    let function_type = Type::Function(selected.signature.clone());
                    let function = self.materialize(value, &function_type, node)?;
                    return self.closure_call(function, arguments, node);
                }
                None if self.is_closure(value.native).is_some() => {
                    return self.closure_call(value, arguments, node);
                }
                _ => {}
            }
        }
        if callee.kind == K::Name && self.text(callee) == "super" {
            let parent_type = self.mono.apply(&selected.signature.result);
            // The subclass constructor checks the whole chain when it finishes, so
            // the parent's part does not check the same clauses again.
            let checking = std::mem::replace(&mut self.checking_invariants, true);
            let parent = self.invoke(&selected, arguments, None, node, Some(parent_type));
            self.checking_invariants = checking;
            let parent = parent?;
            let this = self.lookup("this").unwrap();
            let Ty::Record(parent_id) = parent.native else {
                return self.unsupported(node, "parent construction layout");
            };
            let (Ty::Record(child_id) | Ty::Ref(child_id)) = this.native else {
                return self.unsupported(node, "subclass storage");
            };
            let fields = self.compiler.layouts[parent_id].fields.clone();
            for (index, field) in fields.iter().enumerate() {
                let destination = self.compiler.layouts[child_id]
                    .fields
                    .iter()
                    .position(|f| f.name == field.name)
                    .unwrap_or(index);
                let source = Atom::Field(Box::new(parent.atom.clone()), index, field.ty);
                let rhs = if field.ty.owns() {
                    Rhs::Take(source)
                } else {
                    Rhs::Copy(source)
                };
                self.code.push(Statement::Store(
                    Atom::Field(Box::new(this.atom.clone()), destination, field.ty),
                    field.ty,
                    rhs,
                    self.location(node),
                ));
            }
            return Ok(Value::unit());
        }
        self.invoke(&selected, arguments, receiver, node, None)
    }

    fn check_mut_arguments(
        &self,
        arguments: &[Value],
        signature: &FunctionType,
        node: &Node,
    ) -> Result<()> {
        for (argument, parameter) in arguments.iter().zip(&signature.parameters) {
            if parameter.passing == Passing::Mut
                && argument.callable.is_none()
                && !matches!(argument.native, Ty::Ref(_))
                && !matches!(argument.atom, Atom::Slot(_) | Atom::Field(..))
            {
                return self.unsupported(node, "mutable borrowing of a temporary");
            }
        }
        Ok(())
    }

    fn default_argument(
        &mut self,
        selected: &ResolvedCall,
        index: usize,
        node: &Node,
        earlier: &mut [Value],
        receiver: Option<&Value>,
    ) -> Result<Value> {
        let Callable::Declaration { id, .. } = &selected.target else {
            return self.unsupported(node, "this default argument");
        };
        let Some(definition) = self.compiler.definitions.get(id).cloned() else {
            return self.unsupported(node, "this default declaration");
        };
        let list = definition.node.child(K::ParameterList).unwrap();
        let parameter = &list.children[index];
        let value = parameter.children.last().unwrap();
        if !self.compiler.types[definition.module].contains_key(&value.span) {
            return self.error(node,"E_DEFAULT_DEPENDENCY","This default has no checked expression in its declaration module. Pass the argument explicitly.");
        }
        let plain_result = plain_default_type(
            &self.mono.apply(
                &selected.signature.parameters[index]
                    .ty
                    .replace_parameters(&selected.type_arguments),
            ),
        );
        let source = &self.compiler.project.modules[definition.module]
            .parsed
            .source;
        let mut simple = value;
        while simple.kind == K::Group {
            simple = &simple.children[0];
        }
        let root = place_root_name(value, source).map(str::to_owned);
        let alias_this = root.as_deref() == Some("this");
        // A call whose arguments are earlier parameters, `this` or constants can
        // still receive a borrowing default: the result is either a borrow of a
        // live caller value or a temporary whose cleanup follows this call.
        let call_shape = simple.kind == K::Call
            && !value.descendants().any(|node| {
                matches!(
                    node.kind,
                    K::Block
                        | K::Lambda
                        | K::Return
                        | K::Break
                        | K::Continue
                        | K::Match
                        | K::If
                        | K::While
                        | K::For
                        | K::Binding
                        | K::Assignment
                )
            });
        let borrow_alias = selected.signature.parameters[index].passing == Passing::Borrow
            && (root.is_some() || alias_this || call_shape);
        let names = list
            .children
            .iter()
            .enumerate()
            .filter_map(|(i, p)| {
                p.child(K::Name)
                    .map(|n| (source.slice(n.span).to_owned(), i))
            })
            .collect::<BTreeMap<_, _>>();
        for name in value.descendants().filter(|n| {
            n.kind == K::Name && self.compiler.types[definition.module].contains_key(&n.span)
        }) {
            let text = source.slice(name.span);
            if text == "this" {
                if receiver.is_none()
                    || (!plain_result && !borrow_alias)
                    || selected.signature.parameters[index].passing == Passing::Mut
                {
                    return self.error(node, "E_DEFAULT_DEPENDENCY", "A default that reads this needs a live method receiver and a plain result; pass the argument explicitly.");
                }
                continue;
            }
            if self.compiler.project.modules[definition.module]
                .checked
                .references
                .contains_key(&name.span)
            {
                continue;
            }
            if let Some(i) = names.get(text) {
                let result = self.mono.apply(
                    &selected.signature.parameters[index]
                        .ty
                        .replace_parameters(&selected.type_arguments),
                );
                let readable = selected.signature.parameters[index].passing != Passing::Mut
                    && *i < index
                    && earlier.get(*i).is_some_and(|v| {
                        matches!(
                            v.native,
                            Ty::Int
                                | Ty::Nat
                                | Ty::Double
                                | Ty::Bool
                                | Ty::String
                                | Ty::C(_)
                                | Ty::Pointer(_)
                                | Ty::ArenaRef(_)
                        ) || plain_default_type(&result)
                    });
                if !readable {
                    let simple_alias = borrow_alias
                        && (root.as_deref() == Some(text) || call_shape)
                        && *i < index
                        && selected.signature.parameters[*i].passing != Passing::Move;
                    if !simple_alias {
                        return self.error(node, "E_DEFAULT_DEPENDENCY", "Defaults can read earlier parameters when producing a plain value. Borrowed or aggregate results from these dependencies still need a native helper; pass the argument explicitly.");
                    }
                }
            }
        }
        let key = (id.clone(), index);
        if !self.active_defaults.insert(key.clone()) {
            return self.error(node,"E_DEFAULT_RECURSION","Evaluating these omitted defaults recursively needs the same default again. Pass an argument that breaks this cycle.");
        }
        for (parameter, value) in selected.signature.parameters.iter().zip(earlier.iter_mut()) {
            if value.callable.is_some()
                && matches!(value.ty, Type::Function(_) | Type::Overloaded(_))
            {
                let ty = self
                    .mono
                    .apply(&parameter.ty.replace_parameters(&selected.type_arguments));
                match self.materialize(value.clone(), &ty, node) {
                    Ok(materialized) => *value = materialized,
                    Err(error) => {
                        self.active_defaults.remove(&key);
                        return Err(error);
                    }
                }
            }
        }
        let previous = self.module;
        let previous_mono = self.mono.clone();
        for (name, ty) in &selected.type_arguments {
            self.mono
                .names
                .insert(name.clone(), previous_mono.apply(ty));
        }
        for (parameter, argument) in definition.signature.parameters.iter().zip(earlier.iter()) {
            self.mono.bind(&parameter.ty, &argument.ty);
        }
        let receiver_schema = definition.signature.receiver.as_deref().or_else(|| {
            value.descendants().find_map(|part| {
                (part.kind == K::Name && source.slice(part.span) == "this")
                    .then(|| self.compiler.types[definition.module].get(&part.span))
                    .flatten()
            })
        });
        if let Some(receiver) = receiver
            && let Some(schema) = receiver_schema
        {
            let actual = self.compiler.source_type(receiver.native);
            let mut actual = &actual;
            loop {
                actual = match actual {
                    Type::Borrow(inner) | Type::StoredView(inner) | Type::Dynamic(inner) => inner,
                    Type::Named(name, arguments) if name == "Box" && arguments.len() == 1 => {
                        &arguments[0]
                    }
                    Type::Associated(base, bindings) => {
                        self.mono.names.extend(bindings.clone());
                        base
                    }
                    _ => break,
                };
            }
            let owner = if let Type::Named(owner, _) = schema {
                self.compiler
                    .ancestor(actual, owner)
                    .unwrap_or_else(|| actual.clone())
            } else {
                actual.clone()
            };
            self.mono.bind(schema, &owner);
        }
        let names = self
            .scopes
            .iter_mut()
            .map(|scope| std::mem::take(&mut scope.names))
            .collect::<Vec<_>>();
        for (parameter, value) in list.children.iter().take(index).zip(earlier.iter()) {
            if let Some(name) = parameter.child(K::Name) {
                let mut value = value.clone();
                value.temporary = false;
                self.scopes
                    .last_mut()
                    .unwrap()
                    .names
                    .insert(source.slice(name.span).to_owned(), value);
            }
        }
        if let Some(receiver) = receiver {
            let mut receiver = receiver.clone();
            receiver.temporary = false;
            self.scopes
                .last_mut()
                .unwrap()
                .names
                .insert("this".into(), receiver);
        }
        self.module = definition.module;
        let result = self.expression(value).and_then(|result| {
            if selected.signature.parameters[index].passing != Passing::Mut
                && (result.native == Ty::String || crate::types::copies(&result.ty))
            {
                self.freeze(result, value)
            } else {
                Ok(result)
            }
        });
        self.module = previous;
        self.mono = previous_mono;
        for (scope, names) in self.scopes.iter_mut().zip(names) {
            scope.names = names;
        }
        self.active_defaults.remove(&key);
        result
    }

    fn runtime_error(&mut self, case_name: &str, message: Atom, node: &Node) -> Result<Value> {
        let native = self.native_type(&Type::named("std.core.RuntimeError"), node)?;
        let Ty::Record(id) = native else {
            unreachable!()
        };
        let cases = self.compiler.layouts[id].kind.cases().unwrap();
        let case = cases
            .iter()
            .position(|case| case.name == case_name)
            .unwrap();
        let payload = self.temporary(
            native,
            Rhs::Aggregate(id, Some(case), vec![(message, Ty::String)]),
            node,
        );
        Ok(self.temporary(Ty::Error, Rhs::ErrorBox(id, payload.atom, false), node))
    }

    /// A record element is ordered by its `compare` method, so the native sort calls this helper for each
    /// pair of items. Scalars and Strings compare natively and need no helper (Q123).
    fn ensure_sort_comparator(&mut self, element: Ty, node: &Node) -> Result<()> {
        let Ty::Record(id) = element else {
            return Ok(());
        };
        if self.compiler.sort_comparators.contains_key(&id) {
            return Ok(());
        }
        let item = self.compiler.source_type(element);
        let parameter = if crate::types::copies(&item) {
            element
        } else {
            Ty::Ref(id)
        };
        let index = self.compiler.functions.len();
        self.compiler.functions.push(None);
        let mut builder = Builder::new(
            self.compiler,
            self.module,
            Mono::default(),
            format!("sortCompare_{id}"),
            Ty::Int,
            false,
        );
        let mut values = vec![];
        for _ in 0..2 {
            let slot = builder.local(parameter, true, false);
            builder.parameters.push((slot, parameter, Passing::Borrow));
            values.push(Value {
                keepalive: BTreeSet::new(),
                native: parameter,
                atom: Atom::Slot(slot),
                ty: item.clone(),
                temporary: false,
                callable: None,
            });
        }
        let right = values.pop().unwrap();
        let left = values.pop().unwrap();
        let result = builder.method_call(left, "compare", vec![right], Type::named("Int"), node)?;
        builder.return_value(result, node)?;
        let function = builder.finish();
        self.compiler.functions[index] = Some(function);
        self.compiler.sort_comparators.insert(id, index);
        Ok(())
    }
    /// An enum or record without its own `equals` or `text` compares and prints by
    /// its cases and fields (REVIEW 2.2). Each layout gets one helper per mode, so a
    /// recursive type such as a tree calls its own helper for the children.
    fn structural_layout(&self, id: usize, method: &str) -> bool {
        let layout = &self.compiler.layouts[id];
        // An error enum value prints as its boxed Error would; it has no other structure.
        if layout.is_error {
            return method == "text" && matches!(layout.kind, LayoutKind::Enum(_));
        }
        let structural = match &layout.kind {
            LayoutKind::Enum(_) => {
                !matches!(&layout.source, Type::Named(name, _) if matches!(name.as_str(), "Option" | "Nat"))
            }
            LayoutKind::Class => true,
            _ => false,
        };
        structural && self.compiler.methods(&layout.source, method).is_empty()
    }
    fn structural_helper(&mut self, id: usize, mode: u8, node: &Node) -> Result<usize> {
        if let Some(index) = self.compiler.structural.get(&(id, mode)) {
            return Ok(*index);
        }
        let source = self.compiler.layouts[id].source.clone();
        let index = self.compiler.functions.len();
        self.compiler.functions.push(None);
        self.compiler.structural.insert((id, mode), index);
        let mut builder = Builder::new(
            self.compiler,
            self.module,
            Mono::default(),
            format!("structural{mode}_{id}"),
            match mode {
                0 => Ty::Bool,
                3 | 4 => Ty::Int,
                _ => Ty::String,
            },
            false,
        );
        let mut values = vec![];
        for _ in 0..if matches!(mode, 0 | 4) { 2 } else { 1 } {
            let slot = builder.local(Ty::Ref(id), true, false);
            builder
                .parameters
                .push((slot, Ty::Ref(id), Passing::Borrow));
            values.push(Value {
                keepalive: BTreeSet::new(),
                native: Ty::Ref(id),
                atom: Atom::Slot(slot),
                ty: source.clone(),
                temporary: false,
                callable: None,
            });
        }
        let part = |builder: &mut Builder,
                    owner: &Value,
                    case: Option<usize>,
                    index: usize,
                    native: Ty| {
            let atom = match case {
                Some(case) => Atom::Payload(Box::new(owner.atom.clone()), case, index, native),
                None => Atom::Field(Box::new(owner.atom.clone()), index, native),
            };
            let value = Value {
                keepalive: BTreeSet::new(),
                native,
                atom,
                ty: builder.compiler.source_type(native),
                temporary: false,
                callable: None,
            };
            builder.borrow_value(value)
        };
        let kind = builder.compiler.layouts[id].kind.clone();
        let fields = builder.compiler.layouts[id].fields.clone();
        let result = if mode == 4 {
            let right = values.pop().unwrap();
            let left = values.pop().unwrap();
            // Earlier parts decide; a later part counts only while all earlier ones tie.
            let output = match &kind {
                LayoutKind::Enum(_) => builder.temporary(
                    Ty::Int,
                    Rhs::Builtin(
                        "compareOf".into(),
                        Ty::Int,
                        vec![
                            Atom::Tag(Box::new(left.atom.clone())),
                            Atom::Tag(Box::new(right.atom.clone())),
                        ],
                    ),
                    node,
                ),
                _ => builder.temporary(Ty::Int, Rhs::Copy(Atom::Int(0)), node),
            };
            let Atom::Slot(slot) = output.atom else {
                unreachable!()
            };
            let groups: Vec<(Option<usize>, Vec<Ty>)> = match &kind {
                LayoutKind::Enum(cases) => cases
                    .iter()
                    .enumerate()
                    .filter(|(_, case)| !case.fields.is_empty())
                    .map(|(index, case)| (Some(index), case.fields.clone()))
                    .collect(),
                _ => vec![(None, fields.iter().map(|field| field.ty).collect())],
            };
            for (case, parts) in groups {
                let (_, compare) = builder.isolated(|builder| {
                    for (index, native) in parts.iter().enumerate() {
                        let tied = builder.temporary(
                            Ty::Bool,
                            Rhs::Test(Test::Equal(Ty::Int, Atom::Slot(slot), Atom::Int(0))),
                            node,
                        );
                        let (_, code) = builder.isolated(|builder| {
                            let a = part(builder, &left, case, index, *native);
                            let b = part(builder, &right, case, index, *native);
                            let order = builder.compare_values(a, b, node)?;
                            builder.assign(slot, order, node)?;
                            Ok(Value::unit())
                        })?;
                        builder.code.push(Statement::If(tied.atom, code, vec![]));
                    }
                    Ok(Value::unit())
                })?;
                match case {
                    Some(case) => {
                        let selected = builder.temporary(
                            Ty::Bool,
                            Rhs::Test(Test::And(
                                Box::new(Test::Equal(Ty::Int, Atom::Slot(slot), Atom::Int(0))),
                                Box::new(Test::Equal(
                                    Ty::Int,
                                    Atom::Tag(Box::new(left.atom.clone())),
                                    Atom::Int((case + 1) as i64),
                                )),
                            )),
                            node,
                        );
                        builder
                            .code
                            .push(Statement::If(selected.atom, compare, vec![]));
                    }
                    None => builder.code.extend(compare),
                }
            }
            output
        } else if mode == 3 {
            let value = values.pop().unwrap();
            let seed = match &kind {
                LayoutKind::Enum(_) => Rhs::Builtin(
                    "hashOf".into(),
                    Ty::Int,
                    vec![Atom::Tag(Box::new(value.atom.clone()))],
                ),
                _ => Rhs::Copy(Atom::Int(fields.len() as i64)),
            };
            let output = builder.temporary(Ty::Int, seed, node);
            let Atom::Slot(slot) = output.atom else {
                unreachable!()
            };
            let groups: Vec<(Option<usize>, Vec<Ty>)> = match &kind {
                LayoutKind::Enum(cases) => cases
                    .iter()
                    .enumerate()
                    .filter(|(_, case)| !case.fields.is_empty())
                    .map(|(index, case)| (Some(index), case.fields.clone()))
                    .collect(),
                _ => vec![(None, fields.iter().map(|field| field.ty).collect())],
            };
            for (case, parts) in groups {
                let (_, code) = builder.isolated(|builder| {
                    for (index, native) in parts.iter().enumerate() {
                        let item = part(builder, &value, case, index, *native);
                        let hashed = builder.hash_value(item, node)?;
                        let combined = builder.temporary(
                            Ty::Int,
                            Rhs::Builtin(
                                "hashCombine".into(),
                                Ty::Int,
                                vec![Atom::Slot(slot), hashed.atom],
                            ),
                            node,
                        );
                        builder.assign(slot, combined, node)?;
                    }
                    Ok(Value::unit())
                })?;
                match case {
                    Some(case) => {
                        let selected = builder.temporary(
                            Ty::Bool,
                            Rhs::Test(Test::Equal(
                                Ty::Int,
                                Atom::Tag(Box::new(value.atom.clone())),
                                Atom::Int((case + 1) as i64),
                            )),
                            node,
                        );
                        builder
                            .code
                            .push(Statement::If(selected.atom, code, vec![]));
                    }
                    None => builder.code.extend(code),
                }
            }
            output
        } else if mode == 0 {
            let right = values.pop().unwrap();
            let left = values.pop().unwrap();
            let result = match &kind {
                LayoutKind::Enum(_) => builder.temporary(
                    Ty::Bool,
                    Rhs::Test(Test::Equal(
                        Ty::Int,
                        Atom::Tag(Box::new(left.atom.clone())),
                        Atom::Tag(Box::new(right.atom.clone())),
                    )),
                    node,
                ),
                _ => builder.temporary(Ty::Bool, Rhs::Copy(Atom::Bool(true)), node),
            };
            let Atom::Slot(slot) = result.atom else {
                unreachable!()
            };
            // Each case (or the record itself) compares its parts while all earlier ones matched.
            let groups: Vec<(Option<usize>, Vec<Ty>)> = match &kind {
                LayoutKind::Enum(cases) => cases
                    .iter()
                    .enumerate()
                    .filter(|(_, case)| !case.fields.is_empty())
                    .map(|(index, case)| (Some(index), case.fields.clone()))
                    .collect(),
                _ => vec![(None, fields.iter().map(|field| field.ty).collect())],
            };
            for (case, parts) in groups {
                let (_, compare) = builder.isolated(|builder| {
                    for (index, native) in parts.iter().enumerate() {
                        let (_, code) = builder.isolated(|builder| {
                            let a = part(builder, &left, case, index, *native);
                            let b = part(builder, &right, case, index, *native);
                            let equal = builder.equal_values(a, b, node)?;
                            builder.assign(slot, equal, node)?;
                            Ok(Value::unit())
                        })?;
                        builder
                            .code
                            .push(Statement::If(result.atom.clone(), code, vec![]));
                    }
                    Ok(Value::unit())
                })?;
                match case {
                    Some(case) => {
                        let selected = builder.temporary(
                            Ty::Bool,
                            Rhs::Test(Test::Equal(
                                Ty::Int,
                                Atom::Tag(Box::new(left.atom.clone())),
                                Atom::Int((case + 1) as i64),
                            )),
                            node,
                        );
                        let both = builder.temporary(
                            Ty::Bool,
                            Rhs::Test(Test::And(
                                Box::new(Test::Equal(
                                    Ty::Bool,
                                    result.atom.clone(),
                                    Atom::Bool(true),
                                )),
                                Box::new(Test::Equal(Ty::Bool, selected.atom, Atom::Bool(true))),
                            )),
                            node,
                        );
                        builder.code.push(Statement::If(both.atom, compare, vec![]));
                    }
                    None => builder.code.extend(compare),
                }
            }
            result
        } else {
            let value = values.pop().unwrap();
            // Mode 5 is the session's display: a debug view whose Strings stay unquoted.
            let debug = matches!(mode, 2 | 5);
            builder.plain_strings = mode == 5;
            // A session renames each entry's declarations `Session__N_Name`; show the name
            // the user wrote (REVIEW2 1.8).
            let type_name = match &source {
                Type::Named(name, _) => {
                    let name = name.rsplit('.').next().unwrap_or(name);
                    name.strip_prefix("Session__")
                        .and_then(|rest| rest.split_once('_'))
                        .filter(|(serial, _)| serial.chars().all(|c| c.is_ascii_digit()))
                        .map_or(name, |(_, name)| name)
                        .to_owned()
                }
                other => other.to_string(),
            };
            let output =
                builder.temporary(Ty::String, Rhs::Copy(Atom::Text(type_name.clone())), node);
            let Atom::Slot(slot) = output.atom else {
                unreachable!()
            };
            match &kind {
                LayoutKind::Enum(cases) => {
                    let is_error = builder.compiler.layouts[id].is_error;
                    for (index, case) in cases.iter().enumerate() {
                        let (_, code) = builder.isolated(|builder| {
                            let mut pieces = vec![Atom::Text(if is_error {
                                format!("{type_name}.{}", case.name)
                            } else {
                                case.name.clone()
                            })];
                            if !case.fields.is_empty() {
                                pieces.push(Atom::Text("(".into()));
                                for (position, native) in case.fields.iter().enumerate() {
                                    if position > 0 {
                                        pieces.push(Atom::Text(", ".into()));
                                    }
                                    // A named payload prints its label, as a record prints its fields.
                                    if let Some(label) =
                                        case.names.get(position).and_then(|name| name.as_deref())
                                    {
                                        pieces.push(Atom::Text(format!("{label} = ")));
                                    }
                                    let item =
                                        part(builder, &value, Some(index), position, *native);
                                    pieces
                                        .push(builder.render(item, debug || is_error, node)?.atom);
                                }
                                pieces.push(Atom::Text(")".into()));
                            }
                            builder.code.push(Statement::Set(
                                slot,
                                Rhs::Join(pieces),
                                builder.location(node),
                            ));
                            Ok(Value::unit())
                        })?;
                        let selected = builder.temporary(
                            Ty::Bool,
                            Rhs::Test(Test::Equal(
                                Ty::Int,
                                Atom::Tag(Box::new(value.atom.clone())),
                                Atom::Int((index + 1) as i64),
                            )),
                            node,
                        );
                        builder
                            .code
                            .push(Statement::If(selected.atom, code, vec![]));
                    }
                }
                _ => {
                    let mut pieces = vec![Atom::Text(format!("{type_name}("))];
                    for (position, field) in fields.iter().enumerate() {
                        pieces.push(Atom::Text(format!(
                            "{}{} = ",
                            if position > 0 { ", " } else { "" },
                            field.name
                        )));
                        let item = part(&mut builder, &value, None, position, field.ty);
                        pieces.push(builder.render(item, debug, node)?.atom);
                    }
                    pieces.push(Atom::Text(")".into()));
                    builder.code.push(Statement::Set(
                        slot,
                        Rhs::Join(pieces),
                        builder.location(node),
                    ));
                }
            }
            output
        };
        builder.return_value(result, node)?;
        let function = builder.finish();
        self.compiler.functions[index] = Some(function);
        Ok(index)
    }
    fn structural_call(
        &mut self,
        id: usize,
        mode: u8,
        values: Vec<Value>,
        node: &Node,
    ) -> Result<Value> {
        let index = self.structural_helper(id, mode, node)?;
        let mut keepalive = BTreeSet::new();
        let arguments = values
            .into_iter()
            .map(|value| {
                let mut value = self.borrow_value(value);
                // A copyable tuple stays a value when borrowed; the helper reads it by address.
                if let Ty::Record(layout) = value.native {
                    if let Atom::Slot(slot) = value.atom {
                        value.keepalive.insert(slot);
                    }
                    value.atom = Atom::Address(Box::new(value.atom), layout);
                    value.native = Ty::Ref(layout);
                }
                keepalive.extend(value.keepalive.iter().copied());
                (value.atom, Passing::Borrow)
            })
            .collect();
        let mut result = self.temporary(
            match mode {
                0 => Ty::Bool,
                3 | 4 => Ty::Int,
                _ => Ty::String,
            },
            Rhs::Call(index, arguments),
            node,
        );
        result.keepalive = keepalive;
        Ok(result)
    }
    fn ensure_map_equality(&mut self, id: usize, node: &Node) -> Result<()> {
        let LayoutKind::Map(info) = self.compiler.layouts[id].kind else {
            unreachable!()
        };
        if info.equality.is_some() || !matches!(info.key, Ty::Record(_)) {
            return Ok(());
        }
        let key = self.compiler.source_type(info.key);
        let parameter = if crate::types::copies(&key) {
            info.key
        } else {
            if let Ty::Record(id) = info.key {
                Ty::Ref(id)
            } else {
                info.key
            }
        };
        let index = self.compiler.functions.len();
        self.compiler.functions.push(None);
        let mut builder = Builder::new(
            self.compiler,
            self.module,
            Mono::default(),
            format!("mapKeyEquals_{id}"),
            if info.style == MapStyle::SortedSet {
                Ty::Int
            } else {
                Ty::Bool
            },
            false,
        );
        let mut values = vec![];
        for _ in 0..2 {
            let slot = builder.local(parameter, true, false);
            builder.parameters.push((slot, parameter, Passing::Borrow));
            values.push(Value {
                keepalive: BTreeSet::new(),
                native: parameter,
                atom: Atom::Slot(slot),
                ty: key.clone(),
                temporary: false,
                callable: None,
            });
        }
        let right = values.pop().unwrap();
        let left = values.pop().unwrap();
        let result = if info.style == MapStyle::SortedSet {
            builder.method_call(left, "compare", vec![right], Type::named("Int"), node)?
        } else {
            builder.equal_values(left, right, node)?
        };
        builder.return_value(result, node)?;
        let function = builder.finish();
        self.compiler.functions[index] = Some(function);
        // Hash tuple, enum and record keys by their parts, so lookups stay constant
        // time. Other keys keep one bucket hash, which equality still keeps correct.
        let hash = if info.style != MapStyle::SortedSet {
            let hash_index = self.compiler.functions.len();
            self.compiler.functions.push(None);
            let mut builder = Builder::new(
                self.compiler,
                self.module,
                Mono::default(),
                format!("mapKeyHash_{id}"),
                Ty::Int,
                false,
            );
            let slot = builder.local(parameter, true, false);
            builder.parameters.push((slot, parameter, Passing::Borrow));
            let key_value = Value {
                keepalive: BTreeSet::new(),
                native: parameter,
                atom: Atom::Slot(slot),
                ty: key.clone(),
                temporary: false,
                callable: None,
            };
            let hashed = builder.hash_value(key_value, node)?;
            builder.return_value(hashed, node)?;
            let function = builder.finish();
            self.compiler.functions[hash_index] = Some(function);
            Some(hash_index)
        } else {
            None
        };
        let LayoutKind::Map(info) = &mut self.compiler.layouts[id].kind else {
            unreachable!()
        };
        info.equality = Some(index);
        info.hash = hash;
        Ok(())
    }
    /// A negative, zero or positive Int ordering two values of one type.
    fn compare_values(&mut self, left: Value, right: Value, node: &Node) -> Result<Value> {
        let left = self.unbox_cell(left);
        let right = self.unbox_cell(right);
        match left.native {
            Ty::Int | Ty::Nat | Ty::Bool | Ty::Double | Ty::String => Ok(self.temporary(
                Ty::Int,
                Rhs::Builtin("compareOf".into(), left.native, vec![left.atom, right.atom]),
                node,
            )),
            _ => self.method_call(left, "compare", vec![right], Type::named("Int"), node),
        }
    }
    /// A hash that agrees with structural equality: equal values hash alike.
    fn hash_value(&mut self, value: Value, node: &Node) -> Result<Value> {
        let value = self.unbox_cell(value);
        match value.native {
            Ty::Int | Ty::Nat | Ty::Bool | Ty::Double | Ty::String => {
                return Ok(self.temporary(
                    Ty::Int,
                    Rhs::Builtin("hashOf".into(), value.native, vec![value.atom]),
                    node,
                ));
            }
            Ty::Record(id) | Ty::Ref(id) | Ty::Boxed(id) => {
                let hashable = match &self.compiler.layouts[id].kind {
                    LayoutKind::Tuple => true,
                    LayoutKind::Enum(_) | LayoutKind::Class => self.structural_layout(id, "equals"),
                    _ => false,
                };
                if hashable {
                    return self.structural_call(id, 3, vec![value], node);
                }
            }
            _ => {}
        }
        Ok(self.temporary(Ty::Int, Rhs::Copy(Atom::Int(0)), node))
    }
    fn bigint_operation(
        &mut self,
        operation: &str,
        id: usize,
        arguments: Vec<Value>,
        result: Ty,
        node: &Node,
    ) -> Result<Value> {
        let arguments = arguments
            .into_iter()
            .map(|value| self.borrow_value(value).atom)
            .collect();
        Ok(self.temporary(
            result,
            Rhs::Builtin(format!("bigint.{operation}"), Ty::Record(id), arguments),
            node,
        ))
    }
    fn bigint_method(
        &mut self,
        receiver: Value,
        id: usize,
        name: &str,
        mut arguments: Vec<Value>,
        node: &Node,
    ) -> Result<Value> {
        let (operation, result) = match name {
            "text" | "debug" => ("text", Ty::String),
            "equals" => ("eq", Ty::Bool),
            "compare" => ("compare", Ty::Int),
            "checkedDivide" => (
                "checkedDivide",
                self.native_type(
                    &Type::union(Type::named("BigInt"), Type::named("Error")),
                    node,
                )?,
            ),
            "checkedToInt" => (
                "checkedToInt",
                self.native_type(&Type::union(Type::named("Int"), Type::named("Error")), node)?,
            ),
            "checkedToDouble" => (
                "checkedToDouble",
                self.native_type(
                    &Type::union(Type::named("double"), Type::named("Error")),
                    node,
                )?,
            ),
            "add" | "rightAdd" => ("add", Ty::Record(id)),
            "subtract" | "rightSubtract" => ("sub", Ty::Record(id)),
            "multiply" | "rightMultiply" => ("mul", Ty::Record(id)),
            "divide" | "rightDivide" => ("div", Ty::Record(id)),
            _ => return self.unsupported(node, "this BigInt method"),
        };
        arguments.insert(0, receiver);
        if name.starts_with("right") {
            arguments.reverse();
        }
        self.bigint_operation(operation, id, arguments, result, node)
    }
    /// Whether a receiver node has a trait, whose name this module may qualify.
    fn has_trait(&self, node: &Node, name: &str) -> bool {
        let id = self.compiler.trait_id(self.module, name);
        self.compiler
            .trait_instance_for(&self.ty(node), &id)
            .is_some()
    }
    /// `a[k]` on a class that has Indexable[Key, Item] reads through at(k).
    fn indexable_read(&mut self, node: &Node) -> Result<Value> {
        let receiver = self.expression(&node.children[0])?;
        let receiver = self.freeze(receiver, &node.children[0])?;
        let key = self.expression(&node.children[1])?;
        let result = self.ty(node);
        self.method_call(receiver, "at", vec![key], result, node)
    }
    /// `a[k] = value` on a class that has MutableIndexable[Key, Item] calls setAt(k, value).
    fn indexable_assign(&mut self, node: &Node) -> Result<Value> {
        let place = &node.children[0];
        let receiver = self.expression(&place.children[0])?;
        let receiver = self.freeze(receiver, &place.children[0])?;
        let key = self.expression(&place.children[1])?;
        let value = self.expression(&node.children[1])?;
        self.method_call(
            receiver,
            "setAt",
            vec![key, value],
            Type::named("void"),
            node,
        )
    }
    /// The same write where the value was already evaluated, as tuple assignment does.
    fn indexable_store(&mut self, node: &Node, place: &Node, value: Value) -> Result<()> {
        let receiver = self.expression(&place.children[0])?;
        let receiver = self.freeze(receiver, &place.children[0])?;
        let key = self.expression(&place.children[1])?;
        self.method_call(
            receiver,
            "setAt",
            vec![key, value],
            Type::named("void"),
            node,
        )?;
        Ok(())
    }
    fn method_call(
        &mut self,
        receiver: Value,
        name: &str,
        arguments: Vec<Value>,
        result: Type,
        node: &Node,
    ) -> Result<Value> {
        if let Some(id) = self.compiler.bigint_layout(receiver.native) {
            return self.bigint_method(receiver, id, name, arguments, node);
        }
        // A tuple, record or enum without its own compare orders by its parts.
        if name == "compare"
            && arguments.len() == 1
            && let Ty::Record(id) | Ty::Ref(id) = receiver.native
            && (matches!(self.compiler.layouts[id].kind, LayoutKind::Tuple)
                || self.structural_layout(id, "compare"))
        {
            let mut values = vec![receiver];
            values.extend(arguments);
            return self.structural_call(id, 4, values, node);
        }
        let actual = self.compiler.source_type(receiver.native);
        let method = self
            .compiler
            .methods(&actual, name)
            .into_iter()
            .find(|f| f.parameters.len() == arguments.len())
            .ok_or_else(|| LocatedDiagnostic {
                module: self.module,
                diagnostic: Diagnostic::new(
                    "E_NATIVE_CALL",
                    format!("No concrete {name} method is available for {actual}."),
                    node.span,
                ),
            })?;
        let selected = ResolvedCall {
            target: method.callable.clone().unwrap(),
            callee: None,
            overload: 0,
            signature: method,
            type_arguments: BTreeMap::new(),
            arguments: vec![],
            evaluation_order: vec![],
            receiver: None,
            partial: false,
        };
        self.invoke(&selected, arguments, Some(receiver), node, Some(result))
    }
    fn equal_values(&mut self, left: Value, right: Value, node: &Node) -> Result<Value> {
        if let Some(id) = self.compiler.bigint_layout(left.native) {
            return self.bigint_operation("eq", id, vec![left, right], Ty::Bool, node);
        }
        let layout = if let Ty::Record(id) | Ty::Ref(id) = left.native {
            Some(id)
        } else {
            None
        };
        if let Some(id) = layout {
            match self.compiler.layouts[id].kind.clone() {
                LayoutKind::List(element) => {
                    let result = self.temporary(
                        Ty::Bool,
                        Rhs::Test(Test::Equal(
                            Ty::Int,
                            Atom::CollectionLength(Box::new(left.atom.clone())),
                            Atom::CollectionLength(Box::new(right.atom.clone())),
                        )),
                        node,
                    );
                    let Atom::Slot(slot) = result.atom else {
                        unreachable!()
                    };
                    self.for_each(left, node, |builder, item, cell| {
                        let Some(IterationOwner::Collection(_, _, index)) = cell else {
                            unreachable!()
                        };
                        let (loop_id, depth) = *builder.loops.last().unwrap();
                        builder.code.push(Statement::If(
                            result.atom.clone(),
                            vec![],
                            vec![Statement::Break(loop_id, builder.cleanups(depth))],
                        ));
                        let other = Value {
                            keepalive: right.keepalive.clone(),
                            native: element,
                            atom: Atom::ListItem(
                                Box::new(right.atom.clone()),
                                Box::new(index),
                                element,
                            ),
                            ty: builder.compiler.source_type(element),
                            temporary: false,
                            callable: None,
                        };
                        let other = builder.borrow_value(other);
                        let equal = builder.equal_values(item, other, node)?;
                        builder.assign(slot, equal, node)
                    })?;
                    return Ok(result);
                }
                // Equal Maps hold the same keys with equal values; equal Sets the same keys.
                LayoutKind::Map(info) => {
                    self.ensure_map_equality(id, node)?;
                    let result = self.temporary(
                        Ty::Bool,
                        Rhs::Test(Test::Equal(
                            Ty::Int,
                            Atom::CollectionLength(Box::new(left.atom.clone())),
                            Atom::CollectionLength(Box::new(right.atom.clone())),
                        )),
                        node,
                    );
                    let Atom::Slot(slot) = result.atom else {
                        unreachable!()
                    };
                    let option = if info.style == MapStyle::Map {
                        let value = self.compiler.source_type(info.value);
                        let option =
                            Type::applied("Option", vec![Type::StoredView(Box::new(value))]);
                        Some(self.native_type(&option, node)?)
                    } else {
                        None
                    };
                    self.for_each(left, node, |builder, item, _| {
                        let (loop_id, depth) = *builder.loops.last().unwrap();
                        builder.code.push(Statement::If(
                            result.atom.clone(),
                            vec![],
                            vec![Statement::Break(loop_id, builder.cleanups(depth))],
                        ));
                        let field = |builder: &mut Self, index: usize, native: Ty| {
                            let value = Value {
                                keepalive: item.keepalive.clone(),
                                native,
                                atom: Atom::Field(Box::new(item.atom.clone()), index, native),
                                ty: builder.compiler.source_type(native),
                                temporary: false,
                                callable: None,
                            };
                            builder.borrow_value(value)
                        };
                        let Some(Ty::Record(option)) = option else {
                            let found = builder.temporary(
                                Ty::Bool,
                                Rhs::MapGet(id, right.atom.clone(), item.atom.clone(), true),
                                node,
                            );
                            return builder.assign(slot, found, node);
                        };
                        let key = field(builder, 0, info.key);
                        let found = builder.temporary(
                            Ty::Record(option),
                            Rhs::MapGet(id, right.atom.clone(), key.atom, false),
                            node,
                        );
                        let some = builder.temporary(
                            Ty::Bool,
                            Rhs::Test(Test::Equal(
                                Ty::Int,
                                Atom::Tag(Box::new(found.atom.clone())),
                                Atom::Int(1),
                            )),
                            node,
                        );
                        builder.assign(slot, some.clone(), node)?;
                        let (_, compare) = builder.isolated(|builder| {
                            let (LayoutKind::Enum(cases) | LayoutKind::Step(cases)) =
                                builder.compiler.layouts[option].kind.clone()
                            else {
                                unreachable!()
                            };
                            let payload = cases[0].fields[0];
                            let other = Value {
                                keepalive: found.keepalive.clone(),
                                native: payload,
                                atom: Atom::Payload(Box::new(found.atom.clone()), 0, 0, payload),
                                ty: builder.compiler.source_type(payload),
                                temporary: false,
                                callable: None,
                            };
                            let other = builder.borrow_value(other);
                            let mine = field(builder, 1, info.value);
                            let equal = builder.equal_values(mine, other, node)?;
                            builder.assign(slot, equal, node)?;
                            Ok(Value::unit())
                        })?;
                        builder.code.push(Statement::If(some.atom, compare, vec![]));
                        Ok(())
                    })?;
                    return Ok(result);
                }
                LayoutKind::Tuple => {
                    let result = self.temporary(Ty::Bool, Rhs::Copy(Atom::Bool(true)), node);
                    let Atom::Slot(slot) = result.atom else {
                        unreachable!()
                    };
                    let fields = self.compiler.layouts[id].fields.clone();
                    for (index, field) in fields.iter().enumerate() {
                        let (_, code) = self.isolated(|builder| {
                            let a = Value {
                                keepalive: left.keepalive.clone(),
                                native: field.ty,
                                atom: Atom::Field(Box::new(left.atom.clone()), index, field.ty),
                                ty: builder.compiler.source_type(field.ty),
                                temporary: false,
                                callable: None,
                            };
                            let b = Value {
                                keepalive: right.keepalive.clone(),
                                native: field.ty,
                                atom: Atom::Field(Box::new(right.atom.clone()), index, field.ty),
                                ty: builder.compiler.source_type(field.ty),
                                temporary: false,
                                callable: None,
                            };
                            let a = builder.borrow_value(a);
                            let b = builder.borrow_value(b);
                            let equal = builder.equal_values(a, b, node)?;
                            builder.assign(slot, equal, node)?;
                            Ok(Value::unit())
                        })?;
                        self.code
                            .push(Statement::If(result.atom.clone(), code, vec![]));
                    }
                    return Ok(result);
                }
                LayoutKind::Enum(cases) | LayoutKind::Step(cases) if matches!(&self.compiler.layouts[id].source,Type::Named(name,_) if name=="Option") =>
                {
                    let result = self.temporary(
                        Ty::Bool,
                        Rhs::Test(Test::Equal(
                            Ty::Int,
                            Atom::Tag(Box::new(left.atom.clone())),
                            Atom::Tag(Box::new(right.atom.clone())),
                        )),
                        node,
                    );
                    let Atom::Slot(slot) = result.atom else {
                        unreachable!()
                    };
                    let some = self.temporary(
                        Ty::Bool,
                        Rhs::Test(Test::And(
                            Box::new(Test::Equal(Ty::Bool, result.atom.clone(), Atom::Bool(true))),
                            Box::new(Test::Equal(
                                Ty::Int,
                                Atom::Tag(Box::new(left.atom.clone())),
                                Atom::Int(1),
                            )),
                        )),
                        node,
                    );
                    let (_, code) = self.isolated(|builder| {
                        let native = cases[0].fields[0];
                        // The right Option may be a view of the left's item type, or the reverse.
                        let right_native = match right.native {
                            Ty::Record(other) | Ty::Ref(other) => builder.compiler.layouts[other]
                                .kind
                                .cases()
                                .and_then(|cases| cases.first())
                                .and_then(|case| case.fields.first().copied())
                                .unwrap_or(native),
                            _ => native,
                        };
                        let a = Value {
                            keepalive: left.keepalive.clone(),
                            native,
                            atom: Atom::Payload(Box::new(left.atom.clone()), 0, 0, native),
                            ty: builder.compiler.source_type(native),
                            temporary: false,
                            callable: None,
                        };
                        let b = Value {
                            keepalive: right.keepalive.clone(),
                            native: right_native,
                            atom: Atom::Payload(Box::new(right.atom.clone()), 0, 0, right_native),
                            ty: builder.compiler.source_type(right_native),
                            temporary: false,
                            callable: None,
                        };
                        let a = if matches!(left.native, Ty::Ref(_)) {
                            builder.borrow_value(a)
                        } else {
                            a
                        };
                        let b = if matches!(right.native, Ty::Ref(_)) {
                            builder.borrow_value(b)
                        } else {
                            b
                        };
                        let equal = builder.equal_values(a, b, node)?;
                        builder.assign(slot, equal, node)?;
                        Ok(Value::unit())
                    })?;
                    self.code.push(Statement::If(some.atom, code, vec![]));
                    return Ok(result);
                }
                _ if self.structural_layout(id, "equals") => {
                    return self.structural_call(id, 0, vec![left, right], node);
                }
                _ => {
                    return self.method_call(
                        left,
                        "equals",
                        vec![right],
                        Type::named("Bool"),
                        node,
                    );
                }
            }
        }
        Ok(self.temporary(
            Ty::Bool,
            Rhs::Binary("==".into(), left.native, left.atom, right.atom),
            node,
        ))
    }
    fn render_tuple(&mut self, value: Value, id: usize, node: &Node) -> Result<Value> {
        let mut parts = vec![Atom::Text("(".into())];
        let fields = self.compiler.layouts[id].fields.clone();
        for (index, field) in fields.iter().enumerate() {
            if index > 0 {
                parts.push(Atom::Text(", ".into()));
            }
            let item = Value {
                keepalive: value.keepalive.clone(),
                native: field.ty,
                atom: Atom::Field(Box::new(value.atom.clone()), index, field.ty),
                ty: self.compiler.source_type(field.ty),
                temporary: false,
                callable: None,
            };
            let item = self.borrow_value(item);
            let text = self.render(item, false, node)?;
            parts.push(text.atom);
        }
        parts.push(Atom::Text(")".into()));
        Ok(self.temporary(Ty::String, Rhs::Join(parts), node))
    }
    fn render_option(&mut self, value: Value, id: usize, node: &Node) -> Result<Value> {
        let output = self.temporary(Ty::String, Rhs::Copy(Atom::Text("None".into())), node);
        let Atom::Slot(slot) = output.atom else {
            unreachable!()
        };
        let cases = self.compiler.layouts[id].kind.cases().unwrap();
        let native = cases[0].fields[0];
        let condition = self.temporary(
            Ty::Bool,
            Rhs::Test(Test::Equal(
                Ty::Int,
                Atom::Tag(Box::new(value.atom.clone())),
                Atom::Int(1),
            )),
            node,
        );
        let (_, body) = self.isolated(|builder| {
            let item = Value {
                keepalive: value.keepalive.clone(),
                native,
                atom: Atom::Payload(Box::new(value.atom.clone()), 0, 0, native),
                ty: builder.compiler.source_type(native),
                temporary: false,
                callable: None,
            };
            let item = builder.borrow_value(item);
            let text = builder.render(item, false, node)?;
            builder.code.push(Statement::Set(
                slot,
                Rhs::Join(vec![
                    Atom::Text("Some(".into()),
                    text.atom,
                    Atom::Text(")".into()),
                ]),
                builder.location(node),
            ));
            Ok(Value::unit())
        })?;
        self.code.push(Statement::If(condition.atom, body, vec![]));
        Ok(output)
    }
    fn render_map(&mut self, value: Value, info: MapLayout, node: &Node) -> Result<Value> {
        let spelling = if info.style == MapStyle::Map {
            "Map("
        } else if info.style == MapStyle::SortedSet {
            "SortedSet("
        } else {
            "Set("
        };
        let output = self.temporary(Ty::String, Rhs::Copy(Atom::Text(spelling.into())), node);
        let Atom::Slot(slot) = output.atom else {
            unreachable!()
        };
        let first = self.local(Ty::Bool, false, false);
        self.code.push(Statement::Set(
            first,
            Rhs::Copy(Atom::Bool(true)),
            self.location(node),
        ));
        self.for_each(value, node, |builder, item, _| {
            let mut pieces = vec![];
            if info.style != MapStyle::Map {
                let text = builder.render(item.clone(), false, node)?;
                pieces.push(text.atom);
            }
            for (index, native) in if info.style == MapStyle::Map {
                vec![info.key, info.value]
            } else {
                vec![]
            }
            .into_iter()
            .enumerate()
            {
                if index == 1 {
                    pieces.push(Atom::Text(" => ".into()));
                }
                let field = Value {
                    keepalive: item.keepalive.clone(),
                    native,
                    atom: Atom::Field(Box::new(item.atom.clone()), index, native),
                    ty: builder.compiler.source_type(native),
                    temporary: false,
                    callable: None,
                };
                let field = builder.borrow_value(field);
                let text = builder.render(field, false, node)?;
                pieces.push(text.atom);
            }
            let loc = builder.location(node);
            builder.code.push(Statement::If(
                Atom::Slot(first),
                vec![],
                vec![Statement::Set(
                    slot,
                    Rhs::Join(vec![output.atom.clone(), Atom::Text(", ".into())]),
                    loc.clone(),
                )],
            ));
            let mut joined = vec![output.atom.clone()];
            joined.extend(pieces);
            builder
                .code
                .push(Statement::Set(slot, Rhs::Join(joined), loc.clone()));
            builder
                .code
                .push(Statement::Set(first, Rhs::Copy(Atom::Bool(false)), loc));
            Ok(())
        })?;
        self.code.push(Statement::Set(
            slot,
            Rhs::Join(vec![output.atom.clone(), Atom::Text(")".into())]),
            self.location(node),
        ));
        Ok(output)
    }
    fn render_list(&mut self, value: Value, node: &Node) -> Result<Value> {
        let output = self.temporary(Ty::String, Rhs::Copy(Atom::Text("[".into())), node);
        let first = self.local(Ty::Bool, false, false);
        self.code.push(Statement::Set(
            first,
            Rhs::Copy(Atom::Bool(true)),
            self.location(node),
        ));
        self.for_each(value, node, |builder, item, _| {
            let text = builder.render(item, false, node)?;
            let loc = builder.location(node);
            builder.code.push(Statement::If(
                Atom::Slot(first),
                vec![],
                vec![Statement::Set(
                    match output.atom {
                        Atom::Slot(s) => s,
                        _ => unreachable!(),
                    },
                    Rhs::Join(vec![output.atom.clone(), Atom::Text(", ".into())]),
                    loc.clone(),
                )],
            ));
            builder.code.push(Statement::Set(
                match output.atom {
                    Atom::Slot(s) => s,
                    _ => unreachable!(),
                },
                Rhs::Join(vec![output.atom.clone(), text.atom]),
                loc.clone(),
            ));
            builder
                .code
                .push(Statement::Set(first, Rhs::Copy(Atom::Bool(false)), loc));
            Ok(())
        })?;
        let Atom::Slot(slot) = output.atom else {
            unreachable!()
        };
        self.code.push(Statement::Set(
            slot,
            Rhs::Join(vec![Atom::Slot(slot), Atom::Text("]".into())]),
            self.location(node),
        ));
        Ok(output)
    }
    fn render_array(&mut self, value: Value, id: usize, node: &Node) -> Result<Value> {
        let LayoutKind::Array(element) = self.compiler.layouts[id].kind else {
            unreachable!()
        };
        let output = self.temporary(Ty::String, Rhs::Copy(Atom::Text("[".into())), node);
        let Atom::Slot(slot) = output.atom else {
            unreachable!()
        };
        let first = self.local(Ty::Bool, false, false);
        self.code.push(Statement::Set(
            first,
            Rhs::Copy(Atom::Bool(true)),
            self.location(node),
        ));
        let index = self.local(Ty::Int, false, false);
        self.code.push(Statement::Set(
            index,
            Rhs::Copy(Atom::Int(0)),
            self.location(node),
        ));
        let length = Atom::ArrayLength(Box::new(value.atom.clone()));
        let (condition, condition_code) = self.isolated(|builder| {
            Ok(builder.temporary(
                Ty::Bool,
                Rhs::Binary("<".into(), Ty::Int, Atom::Slot(index), length),
                node,
            ))
        })?;
        let (_, body) = self.isolated(|builder| {
            let item = Value {
                native: element,
                atom: Atom::ArrayItem(
                    Box::new(value.atom.clone()),
                    Box::new(Atom::Slot(index)),
                    element,
                ),
                ty: builder.compiler.source_type(element),
                keepalive: value.keepalive.clone(),
                temporary: false,
                callable: None,
            };
            let item = builder.borrow_value(item);
            let text = builder.render(item, false, node)?;
            let loc = builder.location(node);
            builder.code.push(Statement::If(
                Atom::Slot(first),
                vec![],
                vec![Statement::Set(
                    slot,
                    Rhs::Join(vec![Atom::Slot(slot), Atom::Text(", ".into())]),
                    loc.clone(),
                )],
            ));
            builder.code.push(Statement::Set(
                slot,
                Rhs::Join(vec![Atom::Slot(slot), text.atom]),
                loc.clone(),
            ));
            builder
                .code
                .push(Statement::Set(first, Rhs::Copy(Atom::Bool(false)), loc));
            Ok(Value::unit())
        })?;
        let step = vec![Statement::Set(
            index,
            Rhs::Binary("+".into(), Ty::Int, Atom::Slot(index), Atom::Int(1)),
            self.location(node),
        )];
        let loop_id = self.next_loop;
        self.next_loop += 1;
        self.code.push(Statement::Loop {
            id: loop_id,
            condition: condition_code,
            value: condition.atom.clone(),
            body,
            step,
        });
        self.code.push(Statement::Set(
            slot,
            Rhs::Join(vec![Atom::Slot(slot), Atom::Text("]".into())]),
            self.location(node),
        ));
        Ok(output)
    }

    /// Orders a copyable item against a stored one: a negative, zero or positive Int. Classes use their compare method.
    fn compare_items(
        &mut self,
        left: Value,
        right: Atom,
        element: Ty,
        node: &Node,
    ) -> Result<Atom> {
        if matches!(element, Ty::Record(_) | Ty::Ref(_)) {
            let right = Value {
                keepalive: BTreeSet::new(),
                native: element,
                atom: right,
                ty: self.compiler.source_type(element),
                temporary: false,
                callable: None,
            };
            return Ok(self
                .method_call(left, "compare", vec![right], Type::named("Int"), node)?
                .atom);
        }
        let order = self.temporary(
            Ty::Int,
            Rhs::Builtin("compare".into(), element, vec![left.atom, right]),
            node,
        );
        Ok(order.atom)
    }
    fn print_marker(&mut self, marker: &str) {
        self.code
            .push(Statement::Print(Ty::String, Atom::Text(marker.into())));
    }

    /// A session entry that meets an Error with `?` at its top level prints the Error as a
    /// marked block, tells the host, and ends the entry.
    fn abort_entry(&mut self, error: Value, node: &Node) -> Result<()> {
        self.print_marker("\u{1e}[error]");
        let text = self.render(error, false, node)?;
        self.code.push(Statement::Print(text.native, text.atom));
        self.print_marker("\u{1e}[end]");
        self.code.push(Statement::Evaluate(
            Rhs::Builtin("sessionAbort".into(), Ty::Unit, vec![]),
            self.location(node),
        ));
        // Session values from earlier entries stay alive; only temporaries are released.
        let cleanups = self
            .cleanups(0)
            .into_iter()
            .filter(|cleanup| !matches!(cleanup, Cleanup::Drop(slot) if slot.global))
            .collect();
        self.code.push(Statement::Return(Atom::Unit, cleanups));
        Ok(())
    }

    /// The value's `html()` text, when its type has such a method.
    fn html_text(&mut self, value: &Value, node: &Node) -> Result<Option<Value>> {
        if !matches!(value.native, Ty::Record(_) | Ty::Ref(_)) {
            return Ok(None);
        }
        let actual = self.compiler.source_type(value.native);
        let Some(method) = self
            .compiler
            .methods(&actual, "html")
            .into_iter()
            .find(|method| method.parameters.is_empty())
        else {
            return Ok(None);
        };
        let call = ResolvedCall {
            target: method.callable.clone().unwrap(),
            signature: method,
            arguments: vec![],
            type_arguments: BTreeMap::new(),
            evaluation_order: vec![],
            receiver: None,
            callee: None,
            overload: 0,
            partial: false,
        };
        self.invoke(
            &call,
            vec![],
            Some(value.clone()),
            node,
            Some(Type::named("String")),
        )
        .map(Some)
    }

    /// Prints a notebook result: a plain-text part and, when the type has `html()`, an HTML
    /// part. An `X or Error` result shows X, or the Error as an error block.
    fn notebook_display(&mut self, value: Value, node: &Node) -> Result<()> {
        if let Ty::Record(id) | Ty::Ref(id) = value.native
            && let LayoutKind::Union(parts, cases) = self.compiler.layouts[id].kind.clone()
            && parts.iter().any(|part| self.compiler.is_error_type(part))
        {
            for (case, part) in parts.iter().enumerate() {
                let native = cases[case].fields.first().copied().unwrap_or(Ty::Unit);
                let condition = self.temporary(
                    Ty::Bool,
                    Rhs::Test(Test::Equal(
                        Ty::Int,
                        Atom::Tag(Box::new(value.atom.clone())),
                        Atom::Int((case + 1) as i64),
                    )),
                    node,
                );
                let (_, body) = self.isolated(|builder| {
                    let item = Value {
                        keepalive: value.keepalive.clone(),
                        atom: if native == Ty::Unit {
                            Atom::Unit
                        } else {
                            Atom::Payload(Box::new(value.atom.clone()), case, 0, native)
                        },
                        native,
                        ty: part.clone(),
                        temporary: false,
                        callable: None,
                    };
                    let item = if matches!(value.native, Ty::Ref(_)) || native != Ty::Unit {
                        builder.borrow_value(item)
                    } else {
                        item
                    };
                    if builder.compiler.is_error_type(part) {
                        builder.print_marker("\u{1e}[error]");
                        let text = builder.render(item, false, node)?;
                        builder.code.push(Statement::Print(text.native, text.atom));
                        builder.print_marker("\u{1e}[end]");
                    } else if native != Ty::Unit {
                        builder.notebook_display(item, node)?;
                    }
                    Ok(Value::unit())
                })?;
                self.code.push(Statement::If(condition.atom, body, vec![]));
            }
            return Ok(());
        }
        let value = self.borrow_value(value);
        let html = self.html_text(&value, node)?;
        let text = self.display_render(value, node)?;
        self.print_marker("\u{1e}[result]");
        self.print_marker("\u{1e}[text/plain]");
        self.code.push(Statement::Print(text.native, text.atom));
        if let Some(html) = html {
            self.print_marker("\u{1e}[text/html]");
            self.code.push(Statement::Print(html.native, html.atom));
        }
        self.print_marker("\u{1e}[end]");
        Ok(())
    }

    /// A session shows its last value in the debug view, with Strings unquoted as before.
    fn display_render(&mut self, value: Value, node: &Node) -> Result<Value> {
        let previous = std::mem::replace(&mut self.plain_strings, true);
        let rendered = self.render(value, true, node);
        self.plain_strings = previous;
        rendered
    }
    fn render(&mut self, value: Value, debug: bool, node: &Node) -> Result<Value> {
        // A debug view quotes Strings at every depth: `["a", "b"]` (REVIEW2 section 2).
        let debug = debug || self.debug_render;
        let previous = std::mem::replace(&mut self.debug_render, debug);
        let rendered = self.render_view(value, debug, node);
        self.debug_render = previous;
        rendered
    }
    fn render_view(&mut self, value: Value, debug: bool, node: &Node) -> Result<Value> {
        let value = self.unbox_cell(value);
        let value = if matches!(value.native, Ty::Boxed(_)) {
            self.borrow_value(value)
        } else {
            value
        };
        if let Some(id) = self.compiler.bigint_layout(value.native) {
            return self.bigint_operation("text", id, vec![value], Ty::String, node);
        }
        if let Ty::Record(id) | Ty::Ref(id) = value.native
            && crate::types::is_tensor(&self.compiler.layouts[id].source)
        {
            let value = self.borrow_value(value);
            return Ok(self.temporary(
                Ty::String,
                Rhs::Builtin(
                    if debug { "tensor.debug" } else { "tensor.text" }.into(),
                    Ty::Unit,
                    vec![value.atom],
                ),
                node,
            ));
        }
        if let Ty::Dyn(id) | Ty::DynBox(id) = value.native {
            let LayoutKind::Trait(info) = &self.compiler.layouts[id].kind else {
                unreachable!()
            };
            let name = if debug && info.methods.iter().any(|m| m.name == "debug") {
                "debug"
            } else {
                "text"
            };
            let receiver = self.borrow_value(value);
            return self.dyn_call(id, name, receiver, vec![], node);
        }
        if let Ty::Record(id) | Ty::Ref(id) = value.native {
            match self.compiler.layouts[id].kind {
                LayoutKind::Map(info) => return self.render_map(value, info, node),
                LayoutKind::Tuple => return self.render_tuple(value, id, node),
                LayoutKind::Enum(_) | LayoutKind::Step(_) if matches!(&self.compiler.layouts[id].source,Type::Named(name,_) if name=="Option") =>
                {
                    return self.render_option(value, id, node);
                }
                _ if self.structural_layout(id, "text")
                    && (!debug
                        || self
                            .compiler
                            .methods(&self.compiler.layouts[id].source, "debug")
                            .is_empty()) =>
                {
                    let mode = match (debug, self.plain_strings) {
                        (false, _) => 1,
                        (true, false) => 2,
                        (true, true) => 5,
                    };
                    return self.structural_call(id, mode, vec![value], node);
                }
                _ => {}
            }
        }
        if let Ty::Record(id) = value.native
            && matches!(self.compiler.layouts[id].kind, LayoutKind::List(_))
        {
            return self.render_list(value, node);
        }
        if let Ty::Record(id) | Ty::Ref(id) = value.native
            && matches!(self.compiler.layouts[id].kind, LayoutKind::Array(_))
        {
            return self.render_array(value, id, node);
        }
        // A C boundary number prints as the Kestrel number it holds (REVIEW2 section 2).
        if let Ty::C(index) = value.native {
            let name = crate::interop::C_TYPES[index];
            let (target, operation) = match name {
                "CFloat32" => (Ty::Double, "toDouble"),
                "CUInt64" => (Ty::Nat, "toInt"),
                _ => (Ty::Int, "toInt"),
            };
            let number = self.temporary(
                target,
                Rhs::Builtin(operation.into(), value.native, vec![value.atom]),
                node,
            );
            return Ok(self.temporary(Ty::String, Rhs::Text(target, number.atom, None), node));
        }
        if debug && !self.plain_strings && value.native == Ty::String {
            return Ok(self.temporary(
                Ty::String,
                Rhs::Builtin("stringDebug".into(), Ty::String, vec![value.atom]),
                node,
            ));
        }
        if !matches!(value.native, Ty::Record(_) | Ty::Ref(_)) {
            return Ok(self.temporary(Ty::String, Rhs::Text(value.native, value.atom, None), node));
        }
        let value = self.borrow_value(value);
        let actual = self.compiler.source_type(value.native);
        let mut methods = if debug {
            self.compiler.methods(&actual, "debug")
        } else {
            vec![]
        };
        methods.retain(|method| method.parameters.is_empty());
        if methods.is_empty() {
            methods = self.compiler.methods(&actual, "text");
        }
        let Some(method) = methods
            .into_iter()
            .find(|method| method.parameters.is_empty())
        else {
            return self.unsupported(node, "text conversion for this value");
        };
        let call = ResolvedCall {
            target: method.callable.clone().unwrap(),
            signature: method,
            arguments: vec![],
            type_arguments: BTreeMap::new(),
            evaluation_order: vec![],
            receiver: None,
            callee: None,
            overload: 0,
            partial: false,
        };
        self.invoke(
            &call,
            vec![],
            Some(value),
            node,
            Some(Type::named("String")),
        )
    }

    fn class_view_call(
        &mut self,
        selected: &ResolvedCall,
        arguments: Vec<Value>,
        receiver: Value,
        cases: Vec<Case>,
        node: &Node,
        result_type: Type,
    ) -> Result<Value> {
        let mut branches = vec![];
        let mut native = Ty::Unit;
        let mut mixed = false;
        let mut owners = receiver.keepalive.clone();
        for (case, info) in cases.iter().enumerate() {
            let actual = info.fields[0];
            let receiver = Value {
                native: actual,
                atom: Atom::Payload(Box::new(receiver.atom.clone()), case, 0, actual),
                ty: self.compiler.source_type(actual),
                keepalive: receiver.keepalive.clone(),
                ..Value::unit()
            };
            let (value, code) = self.isolated(|builder| {
                let value = builder.invoke(
                    selected,
                    arguments.clone(),
                    Some(receiver),
                    node,
                    Some(result_type.clone()),
                )?;
                let mut keep = value.keepalive.clone();
                if value.native.owns()
                    && let Atom::Slot(slot) = value.atom
                {
                    keep.insert(slot);
                }
                builder.promote_owners(&keep, builder.scopes.len() - 2);
                Ok(value)
            })?;
            owners.extend(value.keepalive.iter().copied());
            if native == Ty::Unit {
                native = value.native;
            } else if native != value.native {
                if let Some(id) = mixed_view(native, value.native) {
                    native = Ty::Ref(id);
                    mixed = true;
                } else if let Some(view) =
                    self.merge_class_views(native, value.native, &result_type, node)?
                {
                    native = view;
                } else {
                    return self
                        .unsupported(node, "these method results through a superclass view");
                }
            }
            branches.push((value, code));
        }
        let output = (native != Ty::Unit).then(|| self.local(native, false, native.owns()));
        let owned = if mixed && let Ty::Ref(id) = native {
            let slot = self.local(Ty::Boxed(id), false, true);
            self.result_boxes.insert(slot);
            owners.insert(slot);
            Some(slot)
        } else {
            None
        };
        for (case, (value, mut code)) in branches.into_iter().enumerate() {
            if let Some(slot) = output {
                let (_, store) = self.isolated(|builder| {
                    let value = builder.box_branch(value, owned, node)?;
                    builder.assign(slot, value, node)?;
                    Ok(Value::unit())
                })?;
                code.extend(store);
            }
            let condition = self.temporary(
                Ty::Bool,
                Rhs::Test(Test::Equal(
                    Ty::Int,
                    Atom::Tag(Box::new(receiver.atom.clone())),
                    Atom::Int((case + 1) as i64),
                )),
                node,
            );
            self.code.push(Statement::If(condition.atom, code, vec![]));
        }
        Ok(output
            .map(|slot| Value {
                native,
                atom: Atom::Slot(slot),
                ty: result_type,
                keepalive: owners,
                temporary: true,
                callable: None,
            })
            .unwrap_or_else(Value::unit))
    }

    fn invoke(
        &mut self,
        selected: &ResolvedCall,
        arguments: Vec<Value>,
        receiver: Option<Value>,
        node: &Node,
        result_override: Option<Type>,
    ) -> Result<Value> {
        let result_type =
            result_override.unwrap_or_else(|| self.mono.apply(&selected.signature.result));
        if let Some(value) = &receiver
            && let Ty::Record(id) = value.native
            && let LayoutKind::ClassView(cases) = self.compiler.layouts[id].kind.clone()
        {
            return self.class_view_call(
                selected,
                arguments,
                value.clone(),
                cases,
                node,
                result_type,
            );
        }
        let mut result = if matches!(result_type, Type::Lazy(..)) {
            Ty::Unit
        } else {
            self.native_type(&result_type, node)?
        };
        let mut owners = BTreeSet::new();
        for value in arguments.iter().chain(receiver.iter()) {
            protect(value, &mut owners);
        }
        // A record's constructor builds the object from its fields, each argument going to the field its
        // header name gives (Q131).
        if let Callable::Builtin(name) = &selected.target
            && let Some(rest) = name.strip_prefix("record.")
        {
            let (_, names) = rest.split_once('|').unwrap_or((rest, ""));
            // The call's own result carries a generic record's type arguments.
            let native = self.native_type(&result_type, node)?;
            let Ty::Record(id) = native else {
                return self.unsupported(node, "this record");
            };
            // Like a class constructor, the object is a local whose fields are stored one by one.
            let fields = self.compiler.layouts[id].fields.clone();
            let object = self.local(native, false, true);
            for (field_name, value) in names.split(',').zip(arguments) {
                let Some(position) = fields.iter().position(|field| field.name == field_name)
                else {
                    return self.unsupported(node, "this record field");
                };
                let ty = fields[position].ty;
                let rhs = self.transfer(value, ty, node)?;
                self.code.push(Statement::Store(
                    Atom::Field(Box::new(Atom::Slot(object)), position, ty),
                    ty,
                    rhs,
                    self.location(node),
                ));
            }
            return Ok(Value {
                keepalive: BTreeSet::new(),
                atom: Atom::Slot(object),
                native,
                ty: result_type.clone(),
                temporary: true,
                callable: None,
            });
        }
        if let Callable::Builtin(name) = &selected.target
            && let Some(value) = receiver.as_ref()
            && let Some(id) = self.compiler.bigint_layout(value.native)
        {
            return self.bigint_method(
                value.clone(),
                id,
                name.rsplit('.').next().unwrap(),
                arguments,
                node,
            );
        }

        if let Ty::Record(id) = result
            && !crate::types::copies(&result_type)
            && self.compiler.returns_borrow(&selected.target)
        {
            result = Ty::Ref(id);
        }
        let variant = match &selected.target {
            Callable::Declaration {
                name,
                kind: DeclarationKind::Variant,
                ..
            } => Some(name.as_str()),
            Callable::Builtin(name) if name == "std.core.Some" => Some("Some"),
            _ => None,
        };
        if variant == Some("Succ") && result == Ty::Nat && arguments.len() == 1 {
            let value = arguments.into_iter().next().unwrap();
            return Ok(self.temporary(
                Ty::Nat,
                Rhs::Builtin("natSucc".into(), Ty::Nat, vec![value.atom]),
                node,
            ));
        }
        if let Some(name) = variant
            && let Ty::Record(id) = result
            && let LayoutKind::Enum(cases) | LayoutKind::Step(cases) =
                &self.compiler.layouts[id].kind
            && let Some(case) = cases.iter().position(|case| case.name == name)
        {
            let mut values = vec![];
            for value in arguments {
                // A view of a copyable boxed payload, such as an unchanged subtree, shares
                // its box: the new value counts one more reference instead of stealing it.
                if let Ty::Ref(child) = value.native
                    && crate::types::copies(&value.ty)
                    && matches!(&value.atom, Atom::Payload(_, _, _, Ty::Boxed(boxed)) | Atom::Field(_, _, Ty::Boxed(boxed)) if *boxed == child)
                {
                    values.push((value.atom, Ty::Boxed(child)));
                    continue;
                }
                let value = self.freeze_owned(value, node)?;
                values.push((value.atom, value.native));
            }
            return Ok(self.temporary(result, Rhs::Aggregate(id, Some(case), values), node));
        }
        // `next` from the Iterator protocol: a trait object dispatches through its table,
        // and a concrete cursor calls its own method.
        if let Callable::Builtin(name) = &selected.target
            && let Some(value) = receiver.clone()
        {
            let value = self.borrow_value(value);
            if let Ty::Dyn(id) = value.native
                && let LayoutKind::Trait(info) = &self.compiler.layouts[id].kind
                && info.methods.iter().any(|method| {
                    method.name == name.rsplit('.').next().unwrap_or(name)
                        && method.parameters.len() == arguments.len()
                })
            {
                return self.dyn_call(
                    id,
                    name.rsplit('.').next().unwrap_or(name),
                    value,
                    arguments,
                    node,
                );
            }
        }
        if let Callable::Builtin(name) = &selected.target
            && matches!(
                name.as_str(),
                "std.core.Iterator.next" | "std.core.protocol.next"
            )
            && let Some(cursor) = receiver.clone()
        {
            let cursor = self.borrow_value(cursor);
            if let Ty::Dyn(trait_id) = cursor.native {
                return self.dyn_call(trait_id, "next", cursor, arguments, node);
            }
            let result = result_type.clone();
            return self.method_call(cursor, "next", arguments, result, node);
        }
        // `Error(message)` builds RuntimeError.Message(message).
        if let Callable::Builtin(name) = &selected.target
            && name == "std.core.Error"
            && let [message] = arguments.as_slice()
        {
            return self.runtime_error("Message", message.atom.clone(), node);
        }
        if let Callable::Builtin(name) = &selected.target
            && name.ends_with(".items")
            && let Some(collection) = receiver.clone()
        {
            let collection = self.borrow_value(collection);
            if let Ty::Dyn(trait_id) = collection.native {
                return self.dyn_call(trait_id, "items", collection, arguments, node);
            }
        }
        if let Callable::Builtin(name) = &selected.target {
            let receiver = receiver.map(|value| self.unbox_cell(value));
            let mut values = receiver.into_iter().chain(arguments).collect::<Vec<_>>();
            let short = name.rsplit('.').next().unwrap_or(name);
            if name.starts_with("std.core.Bytes.")
                || name == "std.core.String.bytes"
                || name == "std.core.String.nextCharBoundary"
                || name == "std.core.String.nextGraphemeBoundary"
            {
                self.native_type(&Type::named("std.core.RuntimeError"), node)?;
                let operation = if name == "std.core.String.bytes" {
                    "fromUtf8"
                } else {
                    short
                };
                return Ok(self.temporary(
                    result,
                    Rhs::Builtin(
                        format!("system.bytes.{operation}"),
                        Ty::String,
                        values.into_iter().map(|value| value.atom).collect(),
                    ),
                    node,
                ));
            }
            if short == "cause"
                && values
                    .first()
                    .is_some_and(|value| self.compiler.is_error_type(&value.ty))
            {
                let error = values.remove(0);
                let rhs = self.transfer(error, Ty::Error, node)?;
                let error = self.temporary(Ty::Error, rhs, node);
                let mut atoms = vec![error.atom];
                atoms.extend(values.into_iter().map(|value| value.atom));
                return Ok(self.temporary(
                    result,
                    Rhs::Builtin(format!("system.error.{short}"), Ty::Error, atoms),
                    node,
                ));
            }
            if name.starts_with("std.error.") {
                let source = values.remove(0);
                let action = values.remove(0);
                if source.native == Ty::Error || self.compiler.is_error_type(&source.ty) {
                    let rhs = self.transfer(source, Ty::Error, node)?;
                    let error = self.temporary(Ty::Error, rhs, node);
                    return if short == "mapError" {
                        self.callback(&action, vec![error], node)
                    } else {
                        Ok(self.temporary(
                            Ty::Error,
                            Rhs::Builtin(
                                "system.error.context".into(),
                                Ty::Error,
                                vec![error.atom, action.atom],
                            ),
                            node,
                        ))
                    };
                }
                let Ty::Record(id) = source.native else {
                    return self.unsupported(node, "fallible result layout");
                };
                let LayoutKind::Union(parts, cases) = self.compiler.layouts[id].kind.clone() else {
                    return self.unsupported(node, "fallible result layout");
                };
                let failure = parts
                    .iter()
                    .position(|part| self.compiler.is_error_type(part))
                    .unwrap();
                // A named error enum's union becomes `T or Error`: rebuild each case in the
                // result's own layout (REVIEW 2026-11-15, errors).
                if let Ty::Record(target) = result
                    && target != id
                    && let LayoutKind::Union(result_parts, result_cases) =
                        self.compiler.layouts[target].kind.clone()
                {
                    let result_failure = result_parts
                        .iter()
                        .position(|part| self.compiler.is_error_type(part))
                        .unwrap();
                    let success = 1 - failure;
                    let result_success = 1 - result_failure;
                    let output = self.local(result, false, true);
                    let condition = self.temporary(
                        Ty::Bool,
                        Rhs::Test(Test::Equal(
                            Ty::Int,
                            Atom::Tag(Box::new(source.atom.clone())),
                            Atom::Int((failure + 1) as i64),
                        )),
                        node,
                    );
                    let (_, error_branch) = self.isolated(|builder| {
                        let native = cases[failure].fields[0];
                        let payload = builder.borrow_value(Value {
                            native,
                            atom: Atom::Payload(Box::new(source.atom.clone()), failure, 0, native),
                            ty: parts[failure].clone(),
                            ..Value::unit()
                        });
                        let rhs = builder.transfer(payload, Ty::Error, node)?;
                        let error = builder.temporary(Ty::Error, rhs, node);
                        let changed = if short == "mapError" {
                            builder.callback(&action, vec![error], node)?
                        } else {
                            builder.temporary(
                                Ty::Error,
                                Rhs::Builtin(
                                    "system.error.context".into(),
                                    Ty::Error,
                                    vec![error.atom, action.atom.clone()],
                                ),
                                node,
                            )
                        };
                        let rhs = builder.transfer(changed, Ty::Error, node)?;
                        let error = builder.temporary(Ty::Error, rhs, node);
                        builder.code.push(Statement::Set(
                            output,
                            Rhs::Aggregate(
                                target,
                                Some(result_failure),
                                vec![(error.atom, Ty::Error)],
                            ),
                            builder.location(node),
                        ));
                        Ok(Value::unit())
                    })?;
                    let (_, success_branch) = self.isolated(|builder| {
                        let native = cases[success].fields[0];
                        let wanted = result_cases[result_success].fields[0];
                        let atom = Atom::Payload(Box::new(source.atom.clone()), success, 0, native);
                        // A copyable value is copied; anything else moves out of the consumed source.
                        let value = if crate::types::copies(&parts[success]) {
                            let payload = builder.borrow_value(Value {
                                native,
                                atom,
                                ty: parts[success].clone(),
                                ..Value::unit()
                            });
                            let rhs = builder.transfer(payload, wanted, node)?;
                            builder.temporary(wanted, rhs, node)
                        } else {
                            builder.temporary(wanted, Rhs::Take(atom), node)
                        };
                        builder.code.push(Statement::Set(
                            output,
                            Rhs::Aggregate(
                                target,
                                Some(result_success),
                                vec![(value.atom, wanted)],
                            ),
                            builder.location(node),
                        ));
                        Ok(Value::unit())
                    })?;
                    self.code
                        .push(Statement::If(condition.atom, error_branch, success_branch));
                    return Ok(Value {
                        native: result,
                        atom: Atom::Slot(output),
                        ty: result_type.clone(),
                        temporary: true,
                        ..Value::unit()
                    });
                }
                let output = self.local(result, false, true);
                let condition = self.temporary(
                    Ty::Bool,
                    Rhs::Test(Test::Equal(
                        Ty::Int,
                        Atom::Tag(Box::new(source.atom.clone())),
                        Atom::Int((failure + 1) as i64),
                    )),
                    node,
                );
                let (_, error_branch) = self.isolated(|builder| {
                    let payload = Atom::Payload(
                        Box::new(source.atom.clone()),
                        failure,
                        0,
                        cases[failure].fields[0],
                    );
                    let error = builder.temporary(Ty::Error, Rhs::Copy(payload), node);
                    let changed = if short == "mapError" {
                        builder.callback(&action, vec![error], node)?
                    } else {
                        builder.temporary(
                            Ty::Error,
                            Rhs::Builtin(
                                "system.error.context".into(),
                                Ty::Error,
                                vec![error.atom, action.atom.clone()],
                            ),
                            node,
                        )
                    };
                    let rhs = builder.transfer(changed, Ty::Error, node)?;
                    let error = builder.temporary(Ty::Error, rhs, node);
                    let rhs = Rhs::Aggregate(id, Some(failure), vec![(error.atom, Ty::Error)]);
                    builder
                        .code
                        .push(Statement::Set(output, rhs, builder.location(node)));
                    Ok(Value::unit())
                })?;
                let (_, success_branch) = self.isolated(|builder| {
                    let rhs = builder.transfer(source.clone(), result, node)?;
                    builder
                        .code
                        .push(Statement::Set(output, rhs, builder.location(node)));
                    Ok(Value::unit())
                })?;
                self.code
                    .push(Statement::If(condition.atom, error_branch, success_branch));
                return Ok(Value {
                    native: result,
                    atom: Atom::Slot(output),
                    ty: result_type.clone(),
                    temporary: true,
                    ..Value::unit()
                });
            }
            if name.starts_with("std.core.Variable.") {
                return self.variable_call_value(short, values, result, node);
            }
            if name.starts_with("std.prob.") {
                return self.probability_call_value(
                    name,
                    values,
                    &selected.signature,
                    result,
                    node,
                );
            }
            if name.starts_with("std.linalg.")
                || name.starts_with("std.stats.")
                || name.starts_with("std.mlrt.")
                || name.starts_with("std.random.")
                || name.starts_with("std.prob.")
                || name.starts_with("std.math.")
                    && crate::numerical::UNARY.contains(&short)
                    && values
                        .first()
                        .is_some_and(|v| v.native == Ty::Double || crate::types::is_tensor(&v.ty))
            {
                return self.numerical_call_value(name, values, &selected.signature, result, node);
            }
            if name.starts_with("std.tensor.")
                || matches!(
                    short,
                    "copy"
                        | "text"
                        | "debug"
                        | "transpose"
                        | "reshape"
                        | "map"
                        | "rank"
                        | "shape"
                        | "count"
                        | "sum"
                        | "mean"
                        | "min"
                        | "max"
                        | "multiply"
                        | "rightMultiply"
                        | "add"
                        | "rightAdd"
                        | "subtract"
                        | "rightSubtract"
                        | "divide"
                        | "matmul"
                        | "norm"
                        | "negate"
                        | "power"
                ) && values
                    .first()
                    .is_some_and(|v| crate::types::is_tensor(&v.ty))
            {
                return self.tensor_call_value(short, values, result, node);
            }
            if short == "negate"
                && values.len() == 1
                && let Some(id) = self.compiler.bigint_layout(values[0].native)
            {
                return self.bigint_operation("neg", id, values, Ty::Record(id), node);
            }
            if short == "negate"
                && values.len() == 1
                && matches!(values[0].native, Ty::Record(_) | Ty::Ref(_))
            {
                let receiver = values.pop().unwrap();
                return self.method_call(receiver, "negate", vec![], result_type, node);
            }
            if short == "power"
                && values.len() == 2
                && matches!(values[0].native, Ty::Record(_) | Ty::Ref(_))
            {
                let exponent = values.pop().unwrap();
                let receiver = values.pop().unwrap();
                return self.method_call(receiver, "power", vec![exponent], result_type, node);
            }
            if short == "rightPower"
                && values.len() == 2
                && matches!(values[0].native, Ty::Record(_) | Ty::Ref(_))
            {
                let base = values.pop().unwrap();
                let receiver = values.pop().unwrap();
                return self.method_call(receiver, "rightPower", vec![base], result_type, node);
            }
            if short == "negate"
                && values.len() == 1
                && matches!(values[0].native, Ty::Int | Ty::Double)
            {
                return Ok(self.temporary(
                    result,
                    Rhs::Unary("-".into(), values.pop().unwrap().atom),
                    node,
                ));
            }
            if short == "power"
                && values.len() == 2
                && matches!(values[0].native, Ty::Int | Ty::Double)
                && matches!(values[1].native, Ty::Int | Ty::Double)
            {
                let exponent = values.pop().unwrap();
                let base = values.pop().unwrap();
                return Ok(self.temporary(
                    result,
                    Rhs::Binary("**".into(), base.native, base.atom, exponent.atom),
                    node,
                ));
            }
            if matches!(short, "multiply" | "rightMultiply")
                && values.len() == 2
                && matches!(values[0].native, Ty::Int | Ty::Double)
                && values[0].native == values[1].native
            {
                let right = values.pop().unwrap();
                let left = values.pop().unwrap();
                let (a, b) = if short == "multiply" {
                    (left.atom, right.atom)
                } else {
                    (right.atom, left.atom)
                };
                return Ok(self.temporary(
                    result,
                    Rhs::Binary("*".into(), left.native, a, b),
                    node,
                ));
            }
            if short == "items"
                && let Ty::DynBox(iterator) = result
                && let Some(collection) = values.first().cloned()
                && matches!(collection.native,Ty::Record(id) if matches!(self.compiler.layouts[id].kind,LayoutKind::List(_)|LayoutKind::Map(_)))
            {
                return self.collection_cursor(collection, iterator, node);
            }
            // fold, sum and product walk the items once, updating one accumulator (Q133). sum and product use the
            // checked operators of the number type, so an overflow stops the program as it does elsewhere.
            if matches!(short, "fold" | "sum" | "product")
                && (short != "fold" && values.len() == 1 || short == "fold" && values.len() == 3)
            {
                let step = (short == "fold").then(|| values.pop().unwrap());
                let initial = (short == "fold").then(|| values.pop().unwrap());
                let sequence = values.pop().unwrap();
                let accumulator = self.local(result, false, result.owns());
                let start = match (short, initial) {
                    ("fold", Some(initial)) => self.transfer(initial, result, node)?,
                    ("sum", _) => self.zero_or_one(result, true, node)?,
                    _ => self.zero_or_one(result, false, node)?,
                };
                self.code
                    .push(Statement::Set(accumulator, start, self.location(node)));
                let operator = match short {
                    "product" => "*",
                    _ => "+",
                };
                self.for_each(sequence, node, |builder, value, _| {
                    let current = Value {
                        keepalive: BTreeSet::new(),
                        atom: Atom::Slot(accumulator),
                        native: result,
                        ty: builder.compiler.source_type(result),
                        temporary: false,
                        callable: None,
                    };
                    let next = match &step {
                        Some(step) => builder.callback(step, vec![current, value], node)?,
                        None => {
                            let value = builder.freeze(value, node)?;
                            let rhs =
                                Rhs::Binary(operator.into(), result, current.atom, value.atom);
                            builder.temporary(result, rhs, node)
                        }
                    };
                    builder.assign(accumulator, next, node)
                })?;
                return Ok(Value {
                    keepalive: BTreeSet::new(),
                    atom: Atom::Slot(accumulator),
                    native: result,
                    ty: result_type.clone(),
                    temporary: true,
                    callable: None,
                });
            }
            // enumerate and zip build a List of pairs. zip collects the second sequence first, so each item can be
            // read by its position (Q134). Both need copyable items, since a pair holds its items by value.
            if short == "enumerate" && values.len() == 1 {
                let (list, pair, tuple) = self.pair_list_layout(result, node)?;
                let fields = self.compiler.layouts[tuple].fields.clone();
                let item = fields[1].ty;
                self.require_copyable_pair_item(item, node)?;
                let sequence = values.pop().unwrap();
                let mut output = self.temporary(result, Rhs::ListNew(list, vec![]), node);
                let counter = self.local(Ty::Int, false, false);
                self.code.push(Statement::Set(
                    counter,
                    Rhs::Copy(Atom::Int(0)),
                    self.location(node),
                ));
                self.for_each(sequence, node, |builder, value, cell| {
                    let value = builder.freeze(value, node)?;
                    let pair_value = builder.temporary(
                        pair,
                        Rhs::Aggregate(
                            tuple,
                            None,
                            vec![(Atom::Slot(counter), Ty::Int), (value.atom, item)],
                        ),
                        node,
                    );
                    builder.append_list(list, &mut output, pair_value, cell, node)?;
                    let next = builder.temporary(
                        Ty::Int,
                        Rhs::Binary("+".into(), Ty::Int, Atom::Slot(counter), Atom::Int(1)),
                        node,
                    );
                    builder.code.push(Statement::Set(
                        counter,
                        Rhs::Copy(next.atom),
                        builder.location(node),
                    ));
                    Ok(())
                })?;
                return Ok(output);
            }
            if short == "zip" && values.len() == 2 {
                let (list, pair, tuple) = self.pair_list_layout(result, node)?;
                let fields = self.compiler.layouts[tuple].fields.clone();
                let (left_ty, right_ty) = (fields[0].ty, fields[1].ty);
                self.require_copyable_pair_item(left_ty, node)?;
                self.require_copyable_pair_item(right_ty, node)?;
                let other = values.pop().unwrap();
                let sequence = values.pop().unwrap();
                let right_list = self.collect_list(other, right_ty, node)?;
                let mut output = self.temporary(result, Rhs::ListNew(list, vec![]), node);
                let counter = self.local(Ty::Int, false, false);
                self.code.push(Statement::Set(
                    counter,
                    Rhs::Copy(Atom::Int(0)),
                    self.location(node),
                ));
                let length = Atom::CollectionLength(Box::new(right_list.atom.clone()));
                self.for_each(sequence, node, |builder, value, cell| {
                    let value = builder.freeze(value, node)?;
                    let inside = builder.temporary(
                        Ty::Bool,
                        Rhs::Binary("<".into(), Ty::Int, Atom::Slot(counter), length.clone()),
                        node,
                    );
                    let (_, body) = builder.isolated(|inner| {
                        let right = inner.temporary(
                            right_ty,
                            Rhs::Copy(Atom::ListItem(
                                Box::new(right_list.atom.clone()),
                                Box::new(Atom::Slot(counter)),
                                right_ty,
                            )),
                            node,
                        );
                        let pair_value = inner.temporary(
                            pair,
                            Rhs::Aggregate(
                                tuple,
                                None,
                                vec![(value.atom, left_ty), (right.atom, right_ty)],
                            ),
                            node,
                        );
                        inner.append_list(list, &mut output, pair_value, cell, node)?;
                        Ok(Value::unit())
                    })?;
                    builder.code.push(Statement::If(inside.atom, body, vec![]));
                    let next = builder.temporary(
                        Ty::Int,
                        Rhs::Binary("+".into(), Ty::Int, Atom::Slot(counter), Atom::Int(1)),
                        node,
                    );
                    builder.code.push(Statement::Set(
                        counter,
                        Rhs::Copy(next.atom),
                        builder.location(node),
                    ));
                    Ok(())
                })?;
                return Ok(output);
            }
            // A String's characters are walked by a List cursor over its one-character Strings.
            if short == "items"
                && let Ty::DynBox(iterator) = result
                && let Some(text) = values.first().cloned()
                && text.native == Ty::String
            {
                let characters = self.string_characters(text.atom, node)?;
                return self.collection_cursor(characters, iterator, node);
            }
            if short == "isNull"
                && values
                    .first()
                    .is_some_and(|value| matches!(value.native, Ty::Pointer(_)))
            {
                return Ok(self.temporary(
                    Ty::Bool,
                    Rhs::Builtin("cIsNull".into(), Ty::Unit, vec![values[0].atom.clone()]),
                    node,
                ));
            }
            if let Some(function) = name.strip_prefix("c:") {
                let mut target = name.clone();
                if values.iter().any(|value| value.native == Ty::String) {
                    let parameters: Vec<_> = values.iter().map(|value| value.native).collect();
                    let kinds = parameters
                        .iter()
                        .map(|ty| {
                            format!("{ty:?}")
                                .chars()
                                .filter(|c| c.is_ascii_alphanumeric())
                                .collect::<String>()
                        })
                        .collect::<Vec<_>>()
                        .join("_");
                    let wrapper = format!("{function}__kestrel_args_{kinds}");
                    self.compiler
                        .c_text_calls
                        .insert(wrapper.clone(), (parameters, result));
                    target = format!("c:{wrapper}");
                }
                let atoms = values.into_iter().map(|value| value.atom).collect();
                return Ok(self.temporary(result, Rhs::Builtin(target, Ty::Unit, atoms), node));
            }
            if let Some(index) = crate::interop::C_TYPES.iter().position(|t| *t == short)
                && values.len() == 1
            {
                let input = values[0].native;
                let atoms = values.into_iter().map(|value| value.atom).collect();
                return Ok(self.temporary(
                    result,
                    Rhs::Builtin(format!("cmake.{index}"), input, atoms),
                    node,
                ));
            }
            if name == "std.test.assertError" {
                let value = &values[0];
                let condition = if let Ty::Record(id) | Ty::Ref(id) = value.native {
                    if let LayoutKind::Union(parts, _) = &self.compiler.layouts[id].kind {
                        let case = parts
                            .iter()
                            .position(|part| self.compiler.is_error_type(part))
                            .unwrap();
                        self.temporary(
                            Ty::Bool,
                            Rhs::Test(Test::Equal(
                                Ty::Int,
                                Atom::Tag(Box::new(value.atom.clone())),
                                Atom::Int((case + 1) as i64),
                            )),
                            node,
                        )
                        .atom
                    } else {
                        Atom::Bool(self.compiler.is_error_type(&value.ty))
                    }
                } else {
                    Atom::Bool(value.native == Ty::Error)
                };
                self.code.push(Statement::Evaluate(
                    Rhs::Builtin("assertError".into(), Ty::Bool, vec![condition]),
                    self.location(node),
                ));
                return Ok(Value::unit());
            }
            // assertEq compares with `==` and shows both sides with debug() on failure.
            // assertNotEq fails when the two values are equal, and shows the value.
            if short == "assertNotEq" && values.len() == 2 {
                let right = values.pop().unwrap();
                let left = values.pop().unwrap();
                let equal = self.equal_values(left.clone(), right, node)?;
                let (_, failed) = self.isolated(|builder| {
                    let left = builder.render(left, true, node)?;
                    builder.code.push(Statement::Evaluate(
                        Rhs::Builtin("assertNotEqFailed".into(), Ty::Unit, vec![left.atom]),
                        builder.location(node),
                    ));
                    Ok(Value::unit())
                })?;
                self.code.push(Statement::If(equal.atom, failed, vec![]));
                return Ok(Value::unit());
            }
            if matches!(short, "assertTrue" | "assertFalse" | "assertNear")
                && name.starts_with("std.test.")
            {
                let atoms = values.into_iter().map(|value| value.atom).collect();
                self.code.push(Statement::Evaluate(
                    Rhs::Builtin(short.into(), Ty::Unit, atoms),
                    self.location(node),
                ));
                return Ok(Value::unit());
            }
            if short == "assertEq" && values.len() == 2 {
                let right = values.pop().unwrap();
                let left = values.pop().unwrap();
                let equal = self.equal_values(left.clone(), right.clone(), node)?;
                let (_, failed) = self.isolated(|builder| {
                    let left = builder.render(left, true, node)?;
                    let right = builder.render(right, true, node)?;
                    let atoms = vec![left.atom, right.atom];
                    builder.code.push(Statement::Evaluate(
                        Rhs::Builtin("assertFailed".into(), Ty::Unit, atoms),
                        builder.location(node),
                    ));
                    Ok(Value::unit())
                })?;
                self.code.push(Statement::If(equal.atom, vec![], failed));
                return Ok(Value::unit());
            }
            if short == "box" && values.len() == 1 {
                let value = values.pop().unwrap();
                let value = self.freeze_owned(value, node)?;
                return match result {
                    Ty::DynBox(_) => {
                        let rhs = self.transfer(value, result, node)?;
                        Ok(self.temporary(result, rhs, node))
                    }
                    Ty::Boxed(id) => {
                        let native = value.native;
                        Ok(self.temporary(result, Rhs::BoxNew(id, value.atom, native), node))
                    }
                    _ => self.unsupported(node, "this Box layout"),
                };
            }
            let array = match values.first().map(|value| value.native) {
                Some(Ty::Record(id) | Ty::Ref(id))
                    if matches!(self.compiler.layouts[id].kind, LayoutKind::Array(_)) =>
                {
                    Some(id)
                }
                _ => None,
            };
            if name == "std.array.empty"
                && let Ty::Record(id) = result
                && matches!(self.compiler.layouts[id].kind, LayoutKind::Array(_))
            {
                return Ok(self.temporary(
                    result,
                    Rhs::Builtin(format!("arrayAllocate.{id}"), Ty::Unit, vec![Atom::Int(0)]),
                    node,
                ));
            }
            if name == "std.array.generate"
                && let Ty::Record(id) = result
                && let LayoutKind::Array(element) = self.compiler.layouts[id].kind
            {
                let factory = values.pop().unwrap();
                let size = values.pop().unwrap();
                let output = self.temporary(
                    result,
                    Rhs::Builtin(
                        format!("arrayAllocate.{id}"),
                        Ty::Unit,
                        vec![size.atom.clone()],
                    ),
                    node,
                );
                let index = self.local(Ty::Int, false, false);
                self.code.push(Statement::Set(
                    index,
                    Rhs::Copy(Atom::Int(0)),
                    self.location(node),
                ));
                let (condition, condition_code) = self.isolated(|builder| {
                    Ok(builder.temporary(
                        Ty::Bool,
                        Rhs::Binary("<".into(), Ty::Int, Atom::Slot(index), size.atom),
                        node,
                    ))
                })?;
                let (_, body) = self.isolated(|builder| {
                    let value = builder.callback(
                        &factory,
                        vec![Value {
                            atom: Atom::Slot(index),
                            native: Ty::Int,
                            ty: Type::named("Int"),
                            ..Value::unit()
                        }],
                        node,
                    )?;
                    let rhs = builder.transfer(value, element, node)?;
                    builder.code.push(Statement::Store(
                        Atom::ArrayItem(
                            Box::new(output.atom.clone()),
                            Box::new(Atom::Slot(index)),
                            element,
                        ),
                        element,
                        rhs,
                        builder.location(node),
                    ));
                    Ok(Value::unit())
                })?;
                let step = vec![Statement::Set(
                    index,
                    Rhs::Binary("+".into(), Ty::Int, Atom::Slot(index), Atom::Int(1)),
                    self.location(node),
                )];
                let id = self.next_loop;
                self.next_loop += 1;
                self.code.push(Statement::Loop {
                    id,
                    condition: condition_code,
                    value: condition.atom,
                    body,
                    step,
                });
                return Ok(output);
            }
            if short == "Array"
                && let Ty::Record(id) = result
                && matches!(self.compiler.layouts[id].kind, LayoutKind::Array(_))
                && values.len() == 2
            {
                let atoms = values.into_iter().map(|value| value.atom).collect();
                return Ok(self.temporary(
                    result,
                    Rhs::Builtin(format!("arrayNew.{id}"), Ty::Unit, atoms),
                    node,
                ));
            }
            if short == "length"
                && values.len() == 1
                && matches!(values[0].native, Ty::Record(id) | Ty::Ref(id) if matches!(self.compiler.layouts[id].kind, LayoutKind::List(_) | LayoutKind::Map(_)))
            {
                return Ok(self.temporary(
                    Ty::Int,
                    Rhs::Copy(Atom::CollectionLength(Box::new(values[0].atom.clone()))),
                    node,
                ));
            }
            // append adds a slot at the end, growing the array in place. The index is read before the
            // growth, so the stored value lands in the slot the new length names.
            if matches!(short, "append" | "prepend")
                && values.len() == 2
                && let Some(id) = array
            {
                let LayoutKind::Array(element) = self.compiler.layouts[id].kind else {
                    return self.unsupported(node, "this Array layout");
                };
                let value = values.pop().unwrap();
                let owner = values.pop().unwrap();
                let rhs = self.transfer(value, element, node)?;
                // prepend stores at slot 0; append stores at the length read before growth.
                let (grow, index) = if short == "append" {
                    let index = self.temporary(
                        Ty::Int,
                        Rhs::Copy(Atom::ArrayLength(Box::new(owner.atom.clone()))),
                        node,
                    );
                    (format!("arrayAppend.{id}"), index.atom)
                } else {
                    (format!("arrayPrepend.{id}"), Atom::Int(0))
                };
                self.code.push(Statement::Evaluate(
                    Rhs::Builtin(grow, Ty::Unit, vec![owner.atom.clone()]),
                    self.location(node),
                ));
                self.code.push(Statement::Store(
                    Atom::ArrayItem(Box::new(owner.atom), Box::new(index), element),
                    element,
                    rhs,
                    self.location(node),
                ));
                return Ok(Value::unit());
            }
            // String.isEmpty compares the length with zero.
            if short == "isEmpty" && values.len() == 1 && values[0].native == Ty::String {
                let length = self.temporary(
                    Ty::Int,
                    Rhs::Builtin("length".into(), Ty::String, vec![values[0].atom.clone()]),
                    node,
                );
                return Ok(self.temporary(
                    Ty::Bool,
                    Rhs::Binary("==".into(), Ty::Int, length.atom, Atom::Int(0)),
                    node,
                ));
            }
            // Option.isSome and isNone test the tag. Some has tag 1, as render_option relies on.
            if matches!(short, "isSome" | "isNone")
                && values.len() == 1
                && let Ty::Record(option) | Ty::Ref(option) = values[0].native
                && self.compiler.layouts[option].kind.cases().is_some()
            {
                let present = self.temporary(
                    Ty::Bool,
                    Rhs::Test(Test::Equal(
                        Ty::Int,
                        Atom::Tag(Box::new(values[0].atom.clone())),
                        Atom::Int(1),
                    )),
                    node,
                );
                if short == "isSome" {
                    return Ok(present);
                }
                let absent = self.temporary(Ty::Bool, Rhs::Copy(Atom::Bool(true)), node);
                let Atom::Slot(slot) = absent.atom else {
                    unreachable!()
                };
                self.code.push(Statement::If(
                    present.atom,
                    vec![Statement::Set(
                        slot,
                        Rhs::Copy(Atom::Bool(false)),
                        self.location(node),
                    )],
                    vec![],
                ));
                return Ok(absent);
            }
            // first, last, min and max scan the items once. The Option keeps a copy of the item it chose.
            if (matches!(short, "first" | "last" | "min" | "max") && values.len() == 1
                || matches!(short, "minBy" | "maxBy") && values.len() == 2)
                && let Ty::Record(option) = result
                && let Some(cases) = self.compiler.layouts[option]
                    .kind
                    .cases()
                    .map(|cases| cases.to_vec())
            {
                let some = cases.iter().position(|case| case.name == "Some");
                let none = cases.iter().position(|case| case.name == "None");
                let (Some(some), Some(none)) = (some, none) else {
                    return self.unsupported(node, "this Option result");
                };
                let element = cases[some].fields[0];
                // A stored view of a class is a pointer, so the Option keeps a view of the item.
                let view = matches!(element, Ty::Ref(_));
                if !view && !crate::types::copies(&self.compiler.source_type(element)) {
                    return self.unsupported(node, "this method on items that cannot be copied");
                }
                // minBy and maxBy compare the keys of the item and of the best so far.
                let key = if values.len() == 2 {
                    values.pop()
                } else {
                    None
                };
                let sequence = values.pop().unwrap();
                let output =
                    self.temporary(result, Rhs::Aggregate(option, Some(none), vec![]), node);
                let Atom::Slot(slot) = output.atom else {
                    unreachable!()
                };
                let current = output.atom.clone();
                let method = short.to_owned();
                self.for_each(sequence, node, |builder, item, _| {
                    let loc = builder.location(node);
                    let (_, take) = builder.isolated(|inner| {
                        let payload = if view {
                            inner.borrow_value(item.clone())
                        } else {
                            inner.freeze_owned(item.clone(), node)?
                        };
                        inner.code.push(Statement::Set(
                            slot,
                            Rhs::Aggregate(
                                option,
                                Some(some),
                                vec![(payload.atom, payload.native)],
                            ),
                            loc.clone(),
                        ));
                        Ok(Value::unit())
                    })?;
                    match method.as_str() {
                        "first" => {
                            builder.code.extend(take);
                            let (id, depth) = *builder.loops.last().unwrap();
                            builder
                                .code
                                .push(Statement::Break(id, builder.cleanups(depth)));
                        }
                        "last" => builder.code.extend(take),
                        _ => {
                            let present = builder.temporary(
                                Ty::Bool,
                                Rhs::Test(Test::Equal(
                                    Ty::Int,
                                    Atom::Tag(Box::new(current.clone())),
                                    Atom::Int(1),
                                )),
                                node,
                            );
                            let payload_atom =
                                Atom::Payload(Box::new(current.clone()), some, 0, element);
                            let (_, replace) = builder.isolated(|inner| {
                                let order = if let Some(key) = &key {
                                    let best = inner.borrow_value(Value {
                                        keepalive: BTreeSet::new(),
                                        atom: payload_atom.clone(),
                                        native: element,
                                        ty: inner.compiler.source_type(element),
                                        temporary: false,
                                        callable: None,
                                    });
                                    let item_key = inner.callback(key, vec![item.clone()], node)?;
                                    let best_key = inner.callback(key, vec![best], node)?;
                                    let native = item_key.native;
                                    inner.compare_items(item_key, best_key.atom, native, node)?
                                } else {
                                    inner.compare_items(
                                        item.clone(),
                                        payload_atom.clone(),
                                        element,
                                        node,
                                    )?
                                };
                                let better = inner.temporary(
                                    Ty::Bool,
                                    Rhs::Binary(
                                        if matches!(method.as_str(), "min" | "minBy") {
                                            "<"
                                        } else {
                                            ">"
                                        }
                                        .into(),
                                        Ty::Int,
                                        order,
                                        Atom::Int(0),
                                    ),
                                    node,
                                );
                                inner
                                    .code
                                    .push(Statement::If(better.atom, take.clone(), vec![]));
                                Ok(Value::unit())
                            })?;
                            builder
                                .code
                                .push(Statement::If(present.atom, replace, take));
                        }
                    }
                    Ok(())
                })?;
                return Ok(output);
            }
            // any stops at the first item the predicate accepts, all at the first it rejects.
            if matches!(short, "any" | "all") && values.len() == 2 {
                let predicate = values.pop().unwrap();
                let sequence = values.pop().unwrap();
                let any = short == "any";
                let output = self.temporary(Ty::Bool, Rhs::Copy(Atom::Bool(!any)), node);
                let Atom::Slot(slot) = output.atom else {
                    unreachable!()
                };
                self.for_each(sequence, node, |builder, item, _| {
                    let verdict = builder.callback(&predicate, vec![item], node)?;
                    let loc = builder.location(node);
                    let (_, stop) = builder.isolated(|inner| {
                        inner.code.push(Statement::Set(
                            slot,
                            Rhs::Copy(Atom::Bool(any)),
                            loc.clone(),
                        ));
                        let (id, depth) = *inner.loops.last().unwrap();
                        inner.code.push(Statement::Break(id, inner.cleanups(depth)));
                        Ok(Value::unit())
                    })?;
                    if any {
                        builder.code.push(Statement::If(verdict.atom, stop, vec![]));
                    } else {
                        builder.code.push(Statement::If(verdict.atom, vec![], stop));
                    }
                    Ok(())
                })?;
                return Ok(output);
            }
            // isEmpty stops at the first item it finds.
            if short == "isEmpty" && values.len() == 1 {
                let sequence = values.pop().unwrap();
                let output = self.temporary(Ty::Bool, Rhs::Copy(Atom::Bool(true)), node);
                let Atom::Slot(slot) = output.atom else {
                    unreachable!()
                };
                self.for_each(sequence, node, |builder, _item, _| {
                    let loc = builder.location(node);
                    builder
                        .code
                        .push(Statement::Set(slot, Rhs::Copy(Atom::Bool(false)), loc));
                    let (id, depth) = *builder.loops.last().unwrap();
                    builder
                        .code
                        .push(Statement::Break(id, builder.cleanups(depth)));
                    Ok(())
                })?;
                return Ok(output);
            }
            // join concatenates each item's text, with the separator between items.
            if short == "join" && values.len() == 2 {
                let separator = values.pop().unwrap();
                let sequence = values.pop().unwrap();
                let output = self.temporary(Ty::String, Rhs::Copy(Atom::Text("".into())), node);
                let Atom::Slot(slot) = output.atom else {
                    unreachable!()
                };
                let first = self.local(Ty::Bool, false, false);
                self.code.push(Statement::Set(
                    first,
                    Rhs::Copy(Atom::Bool(true)),
                    self.location(node),
                ));
                // The accumulator is the only owner, so each step appends in
                // place instead of rebuilding the whole String.
                self.for_each(sequence, node, |builder, item, _| {
                    let text = builder.render(item, false, node)?;
                    let text = builder.freeze(text, node)?;
                    let loc = builder.location(node);
                    builder.code.push(Statement::If(
                        Atom::Slot(first),
                        vec![],
                        vec![Statement::Evaluate(
                            Rhs::StringAppend(slot, separator.atom.clone()),
                            loc.clone(),
                        )],
                    ));
                    builder.code.push(Statement::Evaluate(
                        Rhs::StringAppend(slot, text.atom),
                        loc.clone(),
                    ));
                    builder
                        .code
                        .push(Statement::Set(first, Rhs::Copy(Atom::Bool(false)), loc));
                    Ok(())
                })?;
                return Ok(output);
            }
            // index scans in order and stops at the first equal item, giving Some(position) or None.
            if short == "index"
                && values.len() == 2
                && let Ty::Record(option) = result
                && matches!(values[0].native, Ty::Record(id) | Ty::Ref(id) if matches!(self.compiler.layouts[id].kind, LayoutKind::List(_) | LayoutKind::Array(_)))
            {
                let cases = self.compiler.layouts[option]
                    .kind
                    .cases()
                    .map(|cases| cases.to_vec());
                let Some(cases) = cases else {
                    return self.unsupported(node, "this index result");
                };
                let some = cases.iter().position(|case| case.name == "Some");
                let none = cases.iter().position(|case| case.name == "None");
                let (Some(some), Some(none)) = (some, none) else {
                    return self.unsupported(node, "this index result");
                };
                let needle = values.pop().unwrap();
                let collection = values.pop().unwrap();
                let output =
                    self.temporary(result, Rhs::Aggregate(option, Some(none), vec![]), node);
                let Atom::Slot(slot) = output.atom else {
                    unreachable!()
                };
                let position = self.local(Ty::Int, false, false);
                self.code.push(Statement::Set(
                    position,
                    Rhs::Copy(Atom::Int(0)),
                    self.location(node),
                ));
                self.for_each(collection, node, |builder, item, _| {
                    let equal = builder.equal_values(item, needle.clone(), node)?;
                    let (id, depth) = *builder.loops.last().unwrap();
                    let loc = builder.location(node);
                    builder.code.push(Statement::If(
                        equal.atom,
                        vec![
                            Statement::Set(
                                slot,
                                Rhs::Aggregate(
                                    option,
                                    Some(some),
                                    vec![(Atom::Slot(position), Ty::Int)],
                                ),
                                loc.clone(),
                            ),
                            Statement::Break(id, builder.cleanups(depth)),
                        ],
                        vec![],
                    ));
                    builder.code.push(Statement::Set(
                        position,
                        Rhs::Binary("+".into(), Ty::Int, Atom::Slot(position), Atom::Int(1)),
                        loc,
                    ));
                    Ok(())
                })?;
                return Ok(output);
            }
            // pop hands the last item over to the Option. The native code moves it out of its slot.
            if short == "pop"
                && values.len() == 1
                && let Some(id) = array
                && matches!(result, Ty::Record(_))
            {
                let owner = values.pop().unwrap();
                return Ok(self.temporary(
                    result,
                    Rhs::Builtin(format!("arrayPop.{id}"), result, vec![owner.atom]),
                    node,
                ));
            }
            // sort(key = f) orders an Array in place by the key (Q135).
            if short == "sort"
                && values.len() == 2
                && let Some(id) = array
            {
                let LayoutKind::Array(element) = self.compiler.layouts[id].kind else {
                    unreachable!()
                };
                let key = values.pop().unwrap();
                let owner = values.pop().unwrap();
                let sequence = Value {
                    keepalive: BTreeSet::new(),
                    atom: owner.atom.clone(),
                    native: owner.native,
                    ty: owner.ty.clone(),
                    temporary: false,
                    callable: None,
                };
                let (sorted, _tuple, item) = self.sorted_pairs(sequence, key, element, node)?;
                let index = self.local(Ty::Int, false, false);
                self.code.push(Statement::Set(
                    index,
                    Rhs::Copy(Atom::Int(0)),
                    self.location(node),
                ));
                let owner_atom = owner.atom.clone();
                self.for_each(sorted, node, |builder, value, _| {
                    let item_value = Value {
                        keepalive: BTreeSet::new(),
                        atom: Atom::Field(Box::new(value.atom.clone()), 1, item),
                        native: item,
                        ty: builder.compiler.source_type(item),
                        temporary: false,
                        callable: None,
                    };
                    let rhs = builder.transfer(item_value, element, node)?;
                    builder.code.push(Statement::Store(
                        Atom::ArrayItem(
                            Box::new(owner_atom.clone()),
                            Box::new(Atom::Slot(index)),
                            element,
                        ),
                        element,
                        rhs,
                        builder.location(node),
                    ));
                    let next = builder.temporary(
                        Ty::Int,
                        Rhs::Binary("+".into(), Ty::Int, Atom::Slot(index), Atom::Int(1)),
                        node,
                    );
                    builder.code.push(Statement::Set(
                        index,
                        Rhs::Copy(next.atom),
                        builder.location(node),
                    ));
                    Ok(())
                })?;
                return Ok(Value::unit());
            }
            // sort() orders an Array in place, so the Array must be mutable (Q123).
            if short == "sort"
                && values.len() == 1
                && let Some(id) = array
            {
                let LayoutKind::Array(element) = self.compiler.layouts[id].kind else {
                    unreachable!()
                };
                self.ensure_sort_comparator(element, node)?;
                let owner = values.pop().unwrap();
                self.code.push(Statement::Evaluate(
                    Rhs::Builtin(format!("arraySort.{id}"), Ty::Unit, vec![owner.atom]),
                    self.location(node),
                ));
                return Ok(Value::unit());
            }
            if short == "swap"
                && values.len() == 3
                && let Some(id) = array
            {
                let right = values.pop().unwrap();
                let left = values.pop().unwrap();
                let owner = values.pop().unwrap();
                self.code.push(Statement::Evaluate(
                    Rhs::Builtin(
                        format!("arraySwap.{id}"),
                        Ty::Unit,
                        vec![owner.atom, left.atom, right.atom],
                    ),
                    self.location(node),
                ));
                return Ok(Value::unit());
            }
            if short == "splitArray"
                && values.len() == 2
                && values[0].native == Ty::String
                && let Ty::Record(array) = result
                && matches!(
                    self.compiler.layouts[array].kind,
                    LayoutKind::Array(Ty::String)
                )
            {
                let text = values[0].atom.clone();
                let separator = values[1].atom.clone();
                return Ok(self.temporary(
                    result,
                    Rhs::Builtin(
                        format!("arraySplit.{array}"),
                        Ty::Record(array),
                        vec![text, separator],
                    ),
                    node,
                ));
            }
            // get(i) on a String gives the i-th character, or None (chosen 2026-11-08).
            if short == "get" && values.len() == 2 && values[0].native == Ty::String {
                let text = values[0].atom.clone();
                let index = values[1].atom.clone();
                return Ok(self.temporary(
                    result,
                    Rhs::Builtin("stringGet".into(), Ty::String, vec![text, index]),
                    node,
                ));
            }
            // at(i) reads one item from a List without copying the whole List to an Array.
            if short == "at"
                && values.len() == 2
                && let Ty::Record(id) = values[0].native
                && let LayoutKind::List(element) = self.compiler.layouts[id].kind
                && matches!(
                    element,
                    Ty::Int | Ty::Nat | Ty::Double | Ty::Bool | Ty::String
                )
            {
                let owner = values[0].atom.clone();
                let index = values[1].atom.clone();
                return Ok(self.temporary(
                    result,
                    Rhs::Builtin(format!("listAt.{id}"), element, vec![owner, index]),
                    node,
                ));
            }
            if let Some(id) = array
                && matches!(short, "get" | "length")
            {
                let owner = values[0].atom.clone();
                if short == "length" {
                    return Ok(self.temporary(
                        Ty::Int,
                        Rhs::Copy(Atom::ArrayLength(Box::new(owner))),
                        node,
                    ));
                }
                let index = values[1].atom.clone();
                return Ok(self.temporary(
                    result,
                    Rhs::Builtin(format!("arrayGet.{id}"), Ty::Unit, vec![owner, index]),
                    node,
                ));
            }
            let map = if matches!(short, "Map" | "MutMap" | "Set" | "MutSet" | "SortedSet") {
                if let Ty::Record(id) = result {
                    Some(id)
                } else {
                    None
                }
            } else if matches!(
                short,
                "put" | "add" | "get" | "contains" | "update" | "remove"
            ) {
                values.first().and_then(|v| {
                    if let Ty::Record(id) | Ty::Ref(id) = v.native {
                        matches!(self.compiler.layouts[id].kind, LayoutKind::Map(_)).then_some(id)
                    } else {
                        None
                    }
                })
            } else {
                None
            };
            if let Some(id) = map {
                let LayoutKind::Map(info) = self.compiler.layouts[id].kind else {
                    unreachable!()
                };
                if matches!(short, "Map" | "MutMap" | "Set" | "MutSet" | "SortedSet") {
                    if !values.is_empty() {
                        self.ensure_map_equality(id, node)?;
                    }
                    let mut entries = vec![];
                    for value in values {
                        entries.push((value.atom, value.native));
                        if info.style != MapStyle::Map {
                            entries.push((Atom::Bool(true), Ty::Bool));
                        }
                    }
                    return Ok(self.temporary(result, Rhs::MapNew(id, entries), node));
                }
                self.ensure_map_equality(id, node)?;
                let receiver = values.remove(0);
                if matches!(short, "put" | "add") {
                    let value = if short == "put" {
                        values.pop().unwrap()
                    } else {
                        Value {
                            native: Ty::Bool,
                            atom: Atom::Bool(true),
                            ty: Type::named("Bool"),
                            ..Value::unit()
                        }
                    };
                    let key = values.pop().unwrap();
                    // A borrowed place, such as a loop item read from a List, keeps
                    // its storage: copy counted values instead of taking them.
                    let key = self.freeze_owned(key, node)?;
                    let value = self.freeze_owned(value, node)?;
                    return Ok(self.temporary(
                        result,
                        Rhs::MapPut(
                            id,
                            receiver.atom,
                            key.atom,
                            key.native,
                            value.atom,
                            value.native,
                        ),
                        node,
                    ));
                }
                // remove(key) drops the entry: a MutMap in place, a Map in its own copy.
                if short == "remove" && values.len() == 1 {
                    let key = values.pop().unwrap();
                    let key = self.borrow_value(key);
                    let operation = if info.mutable {
                        "mapRemove"
                    } else {
                        "mapRemoved"
                    };
                    return Ok(self.temporary(
                        result,
                        Rhs::Builtin(
                            format!("{operation}.{id}"),
                            Ty::Unit,
                            vec![receiver.atom, key.atom],
                        ),
                        node,
                    ));
                }
                // update(key, change) changes a stored object in place, or replaces a copyable
                // value with the change's result, and tells whether the key was present.
                if short == "update" && values.len() == 2 {
                    let change = values.pop().unwrap();
                    let key = values.pop().unwrap();
                    let stored = self.compiler.source_type(info.value);
                    let option = self.native_type(
                        &Type::applied("Option", vec![Type::StoredView(Box::new(stored.clone()))]),
                        node,
                    )?;
                    let Ty::Record(option_id) = option else {
                        return self.unsupported(node, "update on this map");
                    };
                    let found = self.temporary(
                        option,
                        Rhs::MapGet(id, receiver.atom.clone(), key.atom.clone(), false),
                        node,
                    );
                    let present = self.temporary(
                        Ty::Bool,
                        Rhs::Test(Test::Equal(
                            Ty::Int,
                            Atom::Tag(Box::new(found.atom.clone())),
                            Atom::Int(1),
                        )),
                        node,
                    );
                    let payload = self.compiler.layouts[option_id]
                        .kind
                        .cases()
                        .and_then(|cases| cases.first())
                        .and_then(|case| case.fields.first().copied())
                        .unwrap_or(Ty::Unit);
                    // The checker chose the form: a change that returns the new value
                    // replaces it, and one that returns void changes the object in place.
                    let copies_value = !matches!(&change.ty, Type::Function(function) if *function.result == Type::named("void"));
                    if copies_value && !crate::types::copies(&key.ty) {
                        return self.unsupported(
                            node,
                            "update of a copyable value under a key that does not copy",
                        );
                    }
                    let (_, code) = self.isolated(|builder| {
                        let item = Value {
                            keepalive: BTreeSet::new(),
                            native: payload,
                            atom: Atom::Payload(Box::new(found.atom.clone()), 0, 0, payload),
                            ty: stored.clone(),
                            temporary: false,
                            callable: None,
                        };
                        if copies_value {
                            let item = if crate::types::copies(&item.ty) {
                                builder.freeze(item, node)?
                            } else {
                                builder.borrow_value(item)
                            };
                            let changed = builder.callback(&change, vec![item], node)?;
                            let changed = builder.freeze_owned(changed, node)?;
                            let owned_key =
                                builder.temporary(key.native, Rhs::Copy(key.atom.clone()), node);
                            builder.temporary(
                                Ty::Unit,
                                Rhs::MapPut(
                                    id,
                                    receiver.atom.clone(),
                                    owned_key.atom,
                                    key.native,
                                    changed.atom,
                                    changed.native,
                                ),
                                node,
                            );
                        } else {
                            let item = builder.borrow_value(item);
                            builder.callback(&change, vec![item], node)?;
                        }
                        Ok(Value::unit())
                    })?;
                    self.code
                        .push(Statement::If(present.atom.clone(), code, vec![]));
                    return Ok(present);
                }
                // get(key, fallback) looks the key up, then takes the stored value or the fallback (Q127).
                if short == "get" && values.len() == 2 {
                    let fallback = values.pop().unwrap();
                    let key = values.pop().unwrap();
                    let option = self.native_type(
                        &Type::applied("Option", vec![Type::StoredView(Box::new(self.ty(node)))]),
                        node,
                    )?;
                    let found = self.temporary(
                        option,
                        Rhs::MapGet(id, receiver.atom.clone(), key.atom, false),
                        node,
                    );
                    return Ok(self.temporary(
                        result,
                        Rhs::Builtin("optionOr".into(), result, vec![found.atom, fallback.atom]),
                        node,
                    ));
                }
                let key = values.pop().unwrap();
                let mut owners = receiver.keepalive.clone();
                if receiver.temporary
                    && receiver.native.owns()
                    && let Atom::Slot(slot) = receiver.atom
                {
                    owners.insert(slot);
                }
                let mut value = self.temporary(
                    result,
                    Rhs::MapGet(id, receiver.atom, key.atom, short == "contains"),
                    node,
                );
                if self
                    .compiler
                    .holds_borrow(value.native, &mut BTreeSet::new())
                {
                    value.keepalive = owners;
                }
                return Ok(value);
            }
            if matches!(short, "map" | "filter") && values.len() == 2 {
                let callback = values.pop().unwrap();
                let sequence = values.pop().unwrap();
                let (source, mut steps) = if let Some(FunctionValue::Sequence { source, steps }) =
                    sequence.callable.as_deref()
                {
                    (source.as_ref().clone(), steps.clone())
                } else {
                    (sequence, vec![])
                };
                steps.push(if short == "map" {
                    SequenceStep::Map(callback)
                } else {
                    SequenceStep::Filter(callback)
                });
                return Ok(Value {
                    keepalive: source.keepalive.clone(),
                    native: Ty::Unit,
                    atom: Atom::Unit,
                    ty: result_type,
                    temporary: true,
                    callable: Some(Box::new(FunctionValue::Sequence {
                        source: Box::new(source),
                        steps,
                    })),
                });
            }
            if short == "toArray" && values.len() == 1 {
                let sequence = values.pop().unwrap();
                let Ty::Record(array) = result else {
                    return self.unsupported(node, "this Array layout");
                };
                let LayoutKind::Array(element) = self.compiler.layouts[array].kind else {
                    return self.unsupported(node, "this Array layout");
                };
                // The source runs exactly once, into a List. The Array then copies that
                // List's items, so a source with effects is never traversed twice.
                let element_type = self.compiler.source_type(element);
                let Ok(list_type) = self
                    .compiler
                    .layout_type(&Type::applied("List", vec![element_type]))
                else {
                    return self.unsupported(node, "this List layout");
                };
                let Ty::Record(list) = list_type else {
                    return self.unsupported(node, "this List layout");
                };
                let mut items = self.temporary(list_type, Rhs::ListNew(list, vec![]), node);
                self.for_each(sequence, node, |builder, value, cell| {
                    builder.append_list(list, &mut items, value, cell, node)
                })?;
                let output = self.temporary(
                    result,
                    Rhs::Builtin(
                        format!("arrayAllocate.{array}"),
                        Ty::Unit,
                        vec![Atom::CollectionLength(Box::new(items.atom.clone()))],
                    ),
                    node,
                );
                let index = self.local(Ty::Int, false, false);
                self.code.push(Statement::Set(
                    index,
                    Rhs::Copy(Atom::Int(0)),
                    self.location(node),
                ));
                self.for_each(items, node, |builder, value, _| {
                    let rhs = builder.transfer(value, element, node)?;
                    builder.code.push(Statement::Store(
                        Atom::ArrayItem(
                            Box::new(output.atom.clone()),
                            Box::new(Atom::Slot(index)),
                            element,
                        ),
                        element,
                        rhs,
                        builder.location(node),
                    ));
                    builder.code.push(Statement::Set(
                        index,
                        Rhs::Binary("+".into(), Ty::Int, Atom::Slot(index), Atom::Int(1)),
                        builder.location(node),
                    ));
                    Ok(())
                })?;
                return Ok(output);
            }
            if short == "toList" && values.len() == 1 {
                let sequence = values.pop().unwrap();
                if sequence.native == result {
                    return self.freeze(sequence, node);
                }
                let Ty::Record(id) = result else {
                    unreachable!()
                };
                let mut output = self.temporary(result, Rhs::ListNew(id, vec![]), node);
                self.for_each(sequence, node, |builder, value, cell| {
                    builder.append_list(id, &mut output, value, cell, node)
                })?;
                return Ok(output);
            }
            // slice(a, b) and reversed() build a new List that shares the cells.
            if matches!(
                (short, values.len()),
                ("slice", 3) | ("reversed", 1) | ("pick", 2)
            ) && let Ty::Record(list) | Ty::Ref(list) = values[0].native
                && matches!(self.compiler.layouts[list].kind, LayoutKind::List(_))
            {
                let atoms = values
                    .into_iter()
                    .map(|value| self.borrow_value(value).atom)
                    .collect::<Vec<_>>();
                return Ok(self.temporary(
                    Ty::Record(list),
                    Rhs::Builtin(format!("listShape.{short}.{list}"), Ty::Unit, atoms),
                    node,
                ));
            }
            // sorted(key = f) gives a new List ordered by the key (Q135).
            if short == "sorted" && values.len() == 2 {
                let Ty::Record(list) = result else {
                    unreachable!()
                };
                let LayoutKind::List(element) = self.compiler.layouts[list].kind else {
                    unreachable!()
                };
                // Objects such as records are not copied: sort (key, position) pairs and
                // share each original cell in the new order (REVIEW2 1.9).
                if !crate::types::copies(&self.compiler.source_type(element)) {
                    let key = values.pop().unwrap();
                    let sequence = values.pop().unwrap();
                    return self.sorted_by_position(sequence, key, list, element, node);
                }
                let key = values.pop().unwrap();
                let sequence = values.pop().unwrap();
                let (sorted, _, item) = self.sorted_pairs(sequence, key, element, node)?;
                let mut output = self.temporary(result, Rhs::ListNew(list, vec![]), node);
                self.for_each(sorted, node, |builder, value, cell| {
                    let item_value = Value {
                        keepalive: BTreeSet::new(),
                        atom: Atom::Field(Box::new(value.atom.clone()), 1, item),
                        native: item,
                        ty: builder.compiler.source_type(item),
                        temporary: false,
                        callable: None,
                    };
                    builder.append_list(list, &mut output, item_value, cell, node)
                })?;
                return Ok(output);
            }
            // sorted() gives a new List in ascending order (Q123). A List's cells are shared rather than copied,
            // and another Sortable receiver is first collected into a List.
            if short == "sorted" && values.len() == 1 {
                let Ty::Record(list) = result else {
                    unreachable!()
                };
                let LayoutKind::List(element) = self.compiler.layouts[list].kind else {
                    unreachable!()
                };
                self.ensure_sort_comparator(element, node)?;
                let sequence = values.pop().unwrap();
                let source = if sequence.native == result {
                    sequence
                } else {
                    let mut output = self.temporary(result, Rhs::ListNew(list, vec![]), node);
                    self.for_each(sequence, node, |builder, value, cell| {
                        builder.append_list(list, &mut output, value, cell, node)
                    })?;
                    output
                };
                return Ok(self.temporary(
                    result,
                    Rhs::Builtin(format!("listSorted.{list}"), Ty::Unit, vec![source.atom]),
                    node,
                ));
            }
            if short == "compare"
                && values.len() == 2
                && matches!(values[0].native, Ty::Record(_) | Ty::Ref(_))
            {
                let right = values.pop().unwrap();
                let left = values.pop().unwrap();
                return self.method_call(left, "compare", vec![right], Type::named("Int"), node);
            }
            if short == "equals" && values.len() == 2 {
                let right = values.pop().unwrap();
                let left = values.pop().unwrap();
                return self.equal_values(left, right, node);
            }
            if short == "contains"
                && values.len() == 2
                && matches!(values[0].native,Ty::Record(id) if matches!(self.compiler.layouts[id].kind,LayoutKind::List(_)))
            {
                let needle = values.pop().unwrap();
                let collection = values.pop().unwrap();
                let output = self.temporary(Ty::Bool, Rhs::Copy(Atom::Bool(false)), node);
                let Atom::Slot(slot) = output.atom else {
                    unreachable!()
                };
                self.for_each(collection, node, |builder, item, _| {
                    let equal = builder.equal_values(item, needle.clone(), node)?;
                    let (id, depth) = *builder.loops.last().unwrap();
                    builder.code.push(Statement::If(
                        equal.atom,
                        vec![
                            Statement::Set(
                                slot,
                                Rhs::Copy(Atom::Bool(true)),
                                builder.location(node),
                            ),
                            Statement::Break(id, builder.cleanups(depth)),
                        ],
                        vec![],
                    ));
                    Ok(())
                })?;
                return Ok(output);
            }
            if short == "range" && matches!(values.len(), 1..=3) {
                let (start, stop, step) = match values.as_slice() {
                    [stop] => (Atom::Int(0), stop.atom.clone(), Atom::Int(1)),
                    [start, stop] => (start.atom.clone(), stop.atom.clone(), Atom::Int(1)),
                    [start, stop, step] => {
                        (start.atom.clone(), stop.atom.clone(), step.atom.clone())
                    }
                    _ => unreachable!(),
                };
                let start = self.temporary(Ty::Int, Rhs::Copy(start), node);
                let stop = self.temporary(Ty::Int, Rhs::Copy(stop), node);
                let step = self.temporary(Ty::Int, Rhs::Copy(step), node);
                return Ok(Value {
                    keepalive: BTreeSet::new(),
                    native: Ty::Unit,
                    atom: Atom::Unit,
                    ty: result_type,
                    temporary: true,
                    callable: Some(Box::new(FunctionValue::Range {
                        start: start.atom,
                        stop: stop.atom,
                        step: step.atom,
                    })),
                });
            }
            if short == "print" && values.len() == 1 {
                let value = values.remove(0);
                let value = if matches!(
                    value.native,
                    Ty::Record(_)
                        | Ty::Ref(_)
                        | Ty::Boxed(_)
                        | Ty::Dyn(_)
                        | Ty::DynBox(_)
                        | Ty::Error
                        | Ty::C(_)
                ) {
                    self.render(value, false, node)?
                } else {
                    value
                };
                self.code.push(Statement::Print(value.native, value.atom));
                return Ok(Value::unit());
            }
            if matches!(short, "text" | "debug") && values.len() == 1 {
                let value = values.remove(0);
                return self.render(value, short == "debug", node);
            }
            let resource = if matches!(short, "Mutex" | "Channel" | "Atomic" | "Arena") {
                if let Ty::Record(id) = result {
                    Some((id, "new"))
                } else {
                    None
                }
            } else if matches!(short, "open" | "create") {
                let Type::Union(parts) = &result_type else {
                    unreachable!()
                };
                let ty = parts.iter().find(|p| **p != Type::named("Error")).unwrap();
                let Ty::Record(id) = self.native_type(ty, node)? else {
                    unreachable!()
                };
                Some((id, short))
            } else {
                values.first().and_then(|v| {
                    if let Ty::Record(id) | Ty::Ref(id) = v.native {
                        matches!(self.compiler.layouts[id].kind, LayoutKind::Resource(_))
                            .then_some((id, short))
                    } else {
                        None
                    }
                })
            };
            if let Some((id, method)) = resource {
                if let LayoutKind::Resource(Resource::Arena(item)) = self.compiler.layouts[id].kind
                    && matches!(method, "insert" | "replace")
                {
                    let index = if method == "insert" { 1 } else { 2 };
                    let mut value = values.remove(index);
                    if value.callable.is_some() {
                        value = self.materialize(
                            value,
                            &selected.signature.parameters[index - 1].ty,
                            node,
                        )?;
                    }
                    let rhs = self.transfer(value, item, node)?;
                    values.insert(index, self.temporary(item, rhs, node));
                }
                return Ok(self.temporary(
                    result,
                    Rhs::Builtin(
                        format!("resource.{id}.{method}"),
                        Ty::Unit,
                        values.into_iter().map(|v| v.atom).collect(),
                    ),
                    node,
                ));
            }
            let input = values
                .first()
                .map(|value| self.native_type(&value.ty, node))
                .transpose()?
                .unwrap_or(Ty::Unit);
            // A literal template's `.format(...)` joins its pieces as an f-string does.
            if name == "std.core.String.format" {
                let callee = &node.children[0];
                let literal = ungroup_node(callee)
                    .children
                    .first()
                    .map(|receiver| self.text(ungroup_node(receiver)).to_owned())
                    .unwrap_or_default();
                let template =
                    crate::strings::decode(&literal[1..literal.len() - 1], node.span.start, false)
                        .map_err(|diagnostic| LocatedDiagnostic {
                            module: self.module,
                            diagnostic,
                        })?;
                let pieces = crate::strings::template_pieces(&template).map_err(|message| {
                    LocatedDiagnostic {
                        module: self.module,
                        diagnostic: Diagnostic::new("E_FORMAT_TEMPLATE", message, node.span),
                    }
                })?;
                let arguments = values.into_iter().skip(1).collect::<Vec<_>>();
                let mut parts = vec![];
                for piece in pieces {
                    match piece {
                        crate::strings::TemplatePiece::Text(text) => parts.push(Atom::Text(text)),
                        crate::strings::TemplatePiece::Slot(index, spec) => {
                            let value = self.borrow_value(arguments[index].clone());
                            parts.push(self.format_piece(value, spec.as_deref(), node)?);
                        }
                    }
                }
                return Ok(self.temporary(Ty::String, Rhs::Join(parts), node));
            }
            // std.fs, std.io, std.time and the std.process functions run in the C runtime.
            if let Some(system) = name.strip_prefix("std.").filter(|system| {
                ["fs.", "io.", "time.", "process.", "rx.", "bytes."]
                    .iter()
                    .any(|prefix| system.starts_with(prefix))
                    && *system != "process.args"
            }) {
                self.native_type(&Type::named("std.core.RuntimeError"), node)?;
                let atoms = values.into_iter().map(|value| value.atom).collect();
                return Ok(self.temporary(
                    result,
                    Rhs::Builtin(format!("system.{system}"), input, atoms),
                    node,
                ));
            }
            if matches!(
                short,
                "checkedAdd" | "checkedSubtract" | "checkedMultiply" | "checkedDivide"
            ) && matches!(input, Ty::Int | Ty::Nat | Ty::Double)
            {
                self.native_type(&Type::named("std.core.RuntimeError"), node)?;
                return Ok(self.temporary(
                    result,
                    Rhs::Builtin(
                        short.into(),
                        input,
                        values.into_iter().map(|value| value.atom).collect(),
                    ),
                    node,
                ));
            }
            if short == "toBigInt" {
                return Ok(self.temporary(
                    result,
                    Rhs::Builtin(
                        "bigint.fromInt".into(),
                        result,
                        values.into_iter().map(|v| v.atom).collect(),
                    ),
                    node,
                ));
            }
            if matches!(short, "abs" | "min" | "max")
                && let Some(id) = values
                    .first()
                    .and_then(|value| self.compiler.bigint_layout(value.native))
            {
                return self.bigint_operation(short, id, values, Ty::Record(id), node);
            }
            if matches!(
                short,
                "parseInt" | "parseDouble" | "parseBigInt" | "checkedToInt"
            ) {
                self.native_type(&Type::named("std.core.RuntimeError"), node)?;
                return Ok(self.temporary(
                    result,
                    Rhs::Builtin(
                        short.into(),
                        input,
                        values.into_iter().map(|value| value.atom).collect(),
                    ),
                    node,
                ));
            }
            if values
                .first()
                .is_some_and(|value| value.native == Ty::String)
                && STRING_METHODS.contains(&short)
            {
                return Ok(self.temporary(
                    result,
                    Rhs::Builtin(
                        short.into(),
                        Ty::String,
                        values.into_iter().map(|value| value.atom).collect(),
                    ),
                    node,
                ));
            }
            if !matches!(
                short,
                "toDouble"
                    | "toInt"
                    | "isFinite"
                    | "abs"
                    | "sqrt"
                    | "pow"
                    | "assertEq"
                    | "length"
                    | "trim"
                    | "toUpper"
                    | "toLower"
                    | "contains"
                    | "min"
                    | "max"
                    | "split"
                    | "startsWith"
                    | "endsWith"
                    | "floor"
                    | "ceil"
                    | "round"
                    | "isEven"
                    | "isOdd"
                    | "chr"
                    | "sign"
                    | "isqrt"
                    | "popCount"
                    | "isPrime"
                    | "gcd"
                    | "lcm"
                    | "mod"
                    | "clamp"
                    | "isNaN"
                    | "trunc"
                    | "cbrt"
                    | "hypot"
                    | "atan2"
                    | "compare"
                    | "readLine"
                    | "__kesdasReadCsv"
                    | "__columnOrder"
                    | "__columnGather"
                    | "__columnCorrelation"
                    | "__columnRollingMean"
                    | "__columnCompareFloat"
                    | "__columnBoth"
                    | "__columnTile"
                    | "__columnConcat"
                    | "__columnIndex"
                    | "__columnRepeatNames"
                    | "__columnCodes"
                    | "__columnGroupStat"
                    | "__columnCodeCounts"
                    | "__columnArithmetic"
                    | "__factorCodes"
                    | "__mergeLeftPositions"
                    | "__columnTop"
            ) {
                return self.unsupported(node, &format!("builtin {name}"));
            }
            return Ok(self.temporary(
                result,
                Rhs::Builtin(
                    short.into(),
                    input,
                    values.into_iter().map(|value| value.atom).collect(),
                ),
                node,
            ));
        }
        // A function passed as an argument travels as a run-time closure.
        let mut converted = vec![];
        for (index, value) in arguments.into_iter().enumerate() {
            converted.push(match selected.signature.parameters.get(index) {
                Some(parameter) if value.callable.is_some() => {
                    let ty = parameter.ty.clone();
                    self.materialize(value, &ty, node)?
                }
                _ => value,
            });
        }
        let arguments = converted;
        for value in &arguments {
            protect(value, &mut owners);
        }
        let mut target = selected.target.clone();
        let receiver = receiver.map(|value| self.borrow_value(value));
        if let Some(receiver) = &receiver
            && let Ty::Dyn(trait_id) = receiver.native
            && let Callable::Declaration { name, .. } = &target
        {
            return self.dyn_call(trait_id, name, receiver.clone(), arguments, node);
        }
        if let Some(receiver) = &receiver
            && let Callable::Declaration { name, .. } = &target
        {
            let actual = self.compiler.source_type(receiver.native);
            let methods = self.compiler.methods(&actual, name);
            if let Some(method) = methods.into_iter().find(|f| {
                f.parameters.len() == arguments.len()
                    && f.parameters.iter().zip(&arguments).all(|(p, a)| {
                        p.ty == a.ty || matches!(p.ty, Type::Parameter(_) | Type::Rigid(_))
                    })
            }) && let Some(callable) = method.callable
            {
                target = callable;
            }
        }
        let Callable::Declaration {
            id,
            kind:
                DeclarationKind::Function
                | DeclarationKind::StaticMethod
                | DeclarationKind::Method
                | DeclarationKind::TraitMethod
                | DeclarationKind::Constructor,
            ..
        } = &target
        else {
            return self.unsupported(node, "this callable or dispatch mode");
        };
        if !self.compiler.definitions.contains_key(id)
            && let Some((module, declaration, _)) = self.compiler.local_functions.get(id).cloned()
        {
            self.compiler.definitions.insert(
                id.clone(),
                Definition {
                    module,
                    node: declaration,
                    signature: selected.signature.clone(),
                },
            );
        }
        let definition =
            self.compiler
                .definitions
                .get(id)
                .cloned()
                .ok_or_else(|| LocatedDiagnostic {
                    module: self.module,
                    diagnostic: Diagnostic::new(
                        "E_NATIVE_DECLARATION",
                        "This callable has no native declaration body.",
                        node.span,
                    ),
                })?;
        let constructor_call = definition.node.kind == K::Constructor;
        owners.clear();
        if let Some(value) = &receiver {
            protect(value, &mut owners);
        }
        for (parameter, value) in definition.signature.parameters.iter().zip(&arguments) {
            if parameter.passing == Passing::Move
                && !crate::types::copies(&value.ty)
                && !self
                    .compiler
                    .holds_borrow(value.native, &mut BTreeSet::new())
            {
                // The callee consumes the value, so its former slot holds no result storage.
                owners.extend(value.keepalive.iter().copied());
            } else {
                protect(value, &mut owners);
            }
        }
        if let Ty::Record(layout) | Ty::Ref(layout) = result {
            result = if !crate::types::copies(&result_type) && self.compiler.returns_borrow(&target)
            {
                Ty::Ref(layout)
            } else {
                Ty::Record(layout)
            };
        }
        let arguments = arguments
            .into_iter()
            .zip(&definition.signature.parameters)
            .map(|(value, parameter)| {
                if parameter.passing != Passing::Move {
                    self.borrow_value(value)
                } else {
                    value
                }
            })
            .collect::<Vec<_>>();
        let returned_parameter = if matches!(
            target,
            Callable::Declaration {
                kind: DeclarationKind::Function,
                ..
            }
        ) && let Ty::Ref(id) = result
            && let Some(body) = definition.node.children.last()
        {
            let source = &self.compiler.project.modules[definition.module]
                .parsed
                .source;
            definition
                .signature
                .parameters
                .iter()
                .zip(&arguments)
                .find_map(|(parameter, value)| {
                    (parameter.passing == Passing::Borrow
                        && parameter
                            .name
                            .as_deref()
                            .is_some_and(|name| returns_parameter(body, name, source))
                        && value.native == Ty::Ref(id))
                    .then(|| value.clone())
                })
        } else {
            None
        };
        // Keep a borrow's concrete class, including a finite set of subclasses.
        if let Ty::Ref(declared) = result {
            let owner = match &target {
                Callable::Declaration {
                    owner: Some(owner), ..
                } => Some(Type::named(owner)),
                _ => None,
            };
            let declared_receiver = selected.signature.receiver.as_deref().cloned().or(owner);
            let sources = receiver
                .iter()
                .filter_map(|value| {
                    declared_receiver
                        .clone()
                        .map(|formal| (value.native, self.mono.apply(&formal)))
                })
                .chain(
                    arguments
                        .iter()
                        .zip(&selected.signature.parameters)
                        .map(|(value, param)| (value.native, self.mono.apply(&param.ty))),
                )
                .collect();
            result = self.borrowed_class_result(declared, sources, node)?;
        }
        // A proof argument has no run-time value, so the call passes none (Q115).
        let arguments: Vec<Value> = arguments
            .into_iter()
            .enumerate()
            .filter(|(index, value)| {
                !(value.native == Ty::Unit
                    && selected
                        .signature
                        .parameters
                        .get(*index)
                        .is_some_and(|parameter| is_proof_type(&parameter.ty)))
            })
            .map(|(_, value)| value)
            .collect();
        let native_parameters = arguments
            .iter()
            .map(|value| value.native)
            .collect::<Vec<_>>();
        if native_parameters.contains(&Ty::Unit) {
            return self.unsupported(node, "void parameters");
        }
        let receiver_info = receiver
            .as_ref()
            .map(|value| (self.compiler.source_type(value.native), value.native));
        // A local function reads its enclosing function's locals through hidden borrows.
        let captured: Vec<(String, Value, bool)> = match self.compiler.local_functions.get(id) {
            Some((_, _, names)) => {
                let mut captured = vec![];
                for (name, assigned) in names.clone() {
                    let Some(value) = self.lookup(&name) else {
                        return self.unsupported(node, "a local function called where its enclosing variables are out of scope");
                    };
                    let value = if assigned {
                        value
                    } else {
                        self.borrow_value(value)
                    };
                    protect(&value, &mut owners);
                    captured.push((name, value, assigned));
                }
                captured
            }
            None => vec![],
        };
        let parameters = receiver
            .iter()
            .map(|v| v.native)
            .chain(native_parameters.iter().copied())
            .chain(captured.iter().map(|(_, value, _)| value.native))
            .collect::<Vec<_>>();
        let checked_contract = definition.node.child(K::Requires).is_some()
            && (target != selected.target
                || self.checked().contract_checks.get(&node.span)
                    != Some(&crate::contracts::Status::Proved));
        let skip_invariants = self.checking_invariants;
        let key = Key {
            checked_contract,
            skip_invariants,
            declaration: id.clone(),
            parameters,
            result,
        };
        let mut mono = Mono::default();
        for (name, ty) in &selected.type_arguments {
            mono.names.insert(name.clone(), self.mono.apply(ty));
        }
        if let Some((actual, _)) = &receiver_info
            && let Some(schema) = &definition.signature.receiver
        {
            let declared_owner = if let Type::Named(owner, _) = schema.as_ref() {
                self.compiler
                    .ancestor(actual, owner)
                    .unwrap_or_else(|| actual.clone())
            } else {
                actual.clone()
            };
            mono.bind(schema, &declared_owner);
        }
        // A method of a generic class sees the class's type arguments even when no
        // parameter names them, as in a `size()` that builds a local `Array[T]`.
        if let Some((actual, _)) = &receiver_info
            && let Callable::Declaration {
                owner: Some(owner), ..
            } = &target
            && let Some(class) = self.compiler.definition(owner)
        {
            let owner_type = self
                .compiler
                .ancestor(actual, owner)
                .unwrap_or_else(|| actual.clone());
            let owner_type = match owner_type {
                Type::StoredView(inner) | Type::Borrow(inner) => *inner,
                other => other,
            };
            if let Type::Named(_, arguments) = owner_type {
                for (name, argument) in class.parameters.iter().zip(arguments) {
                    mono.names.entry(name.clone()).or_insert(argument);
                }
            }
        }
        if self.compiler.is_trait_default(id) {
            if let (
                Some((actual, _)),
                Some(Callable::Declaration {
                    owner: Some(trait_owner),
                    ..
                }),
            ) = (&receiver_info, &definition.signature.callable)
                && let Some(instance) = self.compiler.trait_instance_for(actual, trait_owner)
            {
                let (trait_type, bindings) = match &instance {
                    Type::Associated(base, bindings) => (base.as_ref(), Some(bindings)),
                    other => (other, None),
                };
                if let (Type::Named(_, arguments), Some(trait_definition)) =
                    (trait_type, self.compiler.definition(trait_owner))
                {
                    for (name, argument) in trait_definition.parameters.iter().zip(arguments) {
                        mono.names.insert(name.clone(), self.mono.apply(argument));
                    }
                }
                if let Some(bindings) = bindings {
                    for (name, value) in bindings {
                        mono.names.insert(name.clone(), self.mono.apply(value));
                    }
                }
            }
            for (declared, selected) in definition
                .signature
                .parameters
                .iter()
                .zip(&selected.signature.parameters)
            {
                mono.bind(&declared.ty, &self.mono.apply(&selected.ty));
            }
            mono.bind(
                &definition.signature.result,
                &self.mono.apply(&selected.signature.result),
            );
        }
        for (parameter, argument) in definition.signature.parameters.iter().zip(&arguments) {
            mono.bind(&parameter.ty, &argument.ty);
        }
        mono.bind(&definition.signature.result, &result_type);
        let mut signature = definition.signature.map(&mut |ty| match ty {
            Type::Parameter(name) | Type::Rigid(name) => mono.names.get(name).cloned(),
            Type::Variable(id) => mono.variables.get(id).cloned(),
            _ => None,
        });
        signature.result = Box::new(result_type);
        let modes = signature
            .parameters
            .iter()
            .map(|parameter| parameter.passing)
            .collect::<Vec<_>>();
        let index = if let Some(index) = self.compiler.instances.get(&key) {
            *index
        } else {
            let index = self.compiler.functions.len();
            self.compiler.instances.insert(key, index);
            self.compiler.functions.push(None);
            self.compiler.pending.push(Pending {
                index,
                checked_contract,
                skip_invariants,
                captures: captured
                    .iter()
                    .map(|(name, value, assigned)| {
                        (name.clone(), value.native, value.ty.clone(), *assigned)
                    })
                    .collect(),
                native_parameters,
                native_result: result,
                receiver: receiver_info,
                definition,
                signature,
                mono,
            });
            index
        };
        let lends_result = self.compiler.holds_borrow(result, &mut BTreeSet::new());
        let mut atoms = receiver
            .into_iter()
            .map(|v| (v.atom, Passing::Borrow))
            .chain(arguments.into_iter().zip(modes).map(|(value, mode)| {
                let keeps_capture = matches!(value.atom, Atom::EnvField(..));
                (
                    value.atom,
                    if matches!(value.native, Ty::Ref(_))
                        || mode != Passing::Mut
                            && (lends_result || keeps_capture)
                            && crate::types::copies(&value.ty)
                    {
                        Passing::Borrow
                    } else {
                        mode
                    },
                )
            }))
            .collect::<Vec<_>>();
        atoms.extend(captured.into_iter().map(|(_, value, assigned)| {
            (
                value.atom,
                if assigned {
                    Passing::Mut
                } else {
                    Passing::Borrow
                },
            )
        }));
        let result_box = if let Ty::Ref(id) = result
            && !constructor_call
        {
            let slot = self.local(Ty::Boxed(id), false, true);
            self.result_boxes.insert(slot);
            atoms.push((Atom::Slot(slot), Passing::Mut));
            self.result_box_calls.insert(slot, index);
            Some(slot)
        } else {
            None
        };
        let mut value = self.temporary(result, Rhs::Call(index, atoms), node);
        if self
            .compiler
            .holds_borrow(value.native, &mut BTreeSet::new())
        {
            value.keepalive = owners;
        }
        value.keepalive.extend(result_box);
        if let Some(parameter) = returned_parameter
            && value.native == parameter.native
        {
            // Keep the call and its contracts, but preserve its exact pointer alias.
            value.atom = parameter.atom;
            value.keepalive = parameter.keepalive;
        }
        Ok(value)
    }

    fn lambda_call(
        &mut self,
        module: usize,
        lambda: &Node,
        captures: &BTreeMap<String, Value>,
        arguments: Vec<Value>,
        call: &Node,
    ) -> Result<Value> {
        let body = lambda.children.last().unwrap();
        if body.descendants().any(|node| node.kind == K::Return) {
            // `return` leaves the lambda, so it needs its own function.
            let ty = self.compiler.types[module]
                .get(&lambda.span)
                .cloned()
                .unwrap_or(Type::Invalid);
            let value = Value {
                ty: ty.clone(),
                callable: Some(Box::new(FunctionValue::Lambda {
                    module,
                    node: lambda.clone(),
                    captures: captures.clone(),
                })),
                ..Value::unit()
            };
            let previous = self.module;
            self.module = module;
            let closure = self.materialize(value, &ty, call);
            self.module = previous;
            return self.closure_call(closure?, arguments, call);
        }
        let old_module = self.module;
        self.module = module;
        let old_mono = self.mono.clone();
        if let Some(Type::Function(schema)) = self.compiler.types[module].get(&lambda.span) {
            for (parameter, argument) in schema.parameters.iter().zip(&arguments) {
                self.mono.bind(&parameter.ty, &argument.ty);
            }
        }
        let old_inline = std::mem::replace(&mut self.inline_lambda, true);
        self.scopes.push(Scope {
            finalizers: vec![],
            names: captures.clone(),
            owned: vec![],
        });
        if let Some(parameters) = lambda.child(K::LambdaParameters) {
            for (parameter, value) in parameters.children.iter().zip(arguments) {
                let name = parameter
                    .child(K::Name)
                    .map(|node| self.text(node))
                    .unwrap_or("")
                    .to_owned();
                self.scopes.last_mut().unwrap().names.insert(name, value);
            }
        }
        let value = self.expression(body)?;
        let value = if value.native.owns() {
            let slot = self.local(value.native, false, false);
            let outer = self.scopes.len() - 2;
            self.scopes[outer].owned.push(slot);
            if !slot.global {
                self.locals[slot.id].owned = true;
            }
            self.assign(slot, value, call)?;
            Value {
                keepalive: BTreeSet::new(),
                native: self.slot_type(slot),
                atom: Atom::Slot(slot),
                ty: self.compiler.source_type(self.slot_type(slot)),
                temporary: true,
                callable: None,
            }
        } else {
            value
        };
        self.finish_scope();
        self.module = old_module;
        self.mono = old_mono;
        self.inline_lambda = old_inline;
        Ok(value)
    }

    fn constant(
        &mut self,
        id: &DeclarationId,
        visiting: &mut BTreeSet<DeclarationId>,
    ) -> Result<Value> {
        let module = self
            .compiler
            .project
            .modules
            .iter()
            .position(|module| module.parsed.source.name == id.file)
            .unwrap();
        let node = self.compiler.nodes[module].get(&id.span).copied().unwrap();
        if !visiting.insert(id.clone()) {
            return self.error(
                node,
                "E_NATIVE_CONST_CYCLE",
                "This constant depends on itself.",
            );
        }
        let previous = self.module;
        self.module = module;
        let value = node.children.last().unwrap();
        let result = match value.kind {
            K::Literal => self.literal(value),
            K::One => self.one(value),
            K::Name => match self.checked().references.get(&value.span).cloned() {
                Some(Reference::Constant(other)) => self.constant(&other, visiting),
                _ => self.expression(value),
            },
            // Constants are pure, so each use can evaluate the initializer.
            _ => self.expression(value),
        };
        self.module = previous;
        visiting.remove(id);
        result
    }
}

// C's mandatory tail call requires matching calling conventions and argument layouts.
// Tail cycles outside that subset are refused until a typed trampoline is available.
fn prepare_tail_calls(
    program: &mut Program,
    project: &ProjectCheck,
) -> std::result::Result<(), Vec<LocatedDiagnostic>> {
    /// Target, call site, whether the signatures match, and whether the call can end this frame.
    type Edge = (usize, Location, bool, bool);
    fn signature(f: &Function) -> (Vec<(Ty, bool)>, Ty) {
        (
            f.parameters
                .iter()
                .map(|(_, ty, mode)| (*ty, *mode == Passing::Mut))
                .collect(),
            f.result,
        )
    }
    /// A tail call replaces this frame, so each pointer argument must reach the caller's
    /// data: a parameter, a field path under one, or a local that only ever copies those.
    fn outer(atom: &Atom, derived: &BTreeSet<Slot>) -> bool {
        match atom {
            Atom::Slot(slot) => derived.contains(slot),
            Atom::Address(inner, _) => place(inner, derived),
            Atom::Field(inner, _, Ty::Ref(_) | Ty::Boxed(_))
            | Atom::Payload(inner, _, _, Ty::Ref(_) | Ty::Boxed(_)) => place(inner, derived),
            _ => false,
        }
    }
    fn place(atom: &Atom, derived: &BTreeSet<Slot>) -> bool {
        match atom {
            Atom::Slot(slot) => derived.contains(slot),
            // `&parameter->field` still reaches the caller's data (REVIEW2 1.6).
            Atom::Field(inner, ..) | Atom::Payload(inner, ..) | Atom::Address(inner, _) => {
                place(inner, derived)
            }
            _ => false,
        }
    }
    fn derived(function: &Function) -> BTreeSet<Slot> {
        fn assignments<'c>(code: &'c [Statement], out: &mut Vec<(Slot, Option<&'c Atom>)>) {
            for statement in code {
                match statement {
                    Statement::Set(slot, Rhs::Copy(atom) | Rhs::Take(atom), _) => {
                        out.push((*slot, Some(atom)));
                    }
                    Statement::Set(slot, _, _) => out.push((*slot, None)),
                    Statement::If(_, a, b) => {
                        assignments(a, out);
                        assignments(b, out);
                    }
                    Statement::Match(_, arms) => {
                        for arm in arms {
                            assignments(&arm.test, out);
                            if let Some((code, _)) = &arm.guard {
                                assignments(code, out);
                            }
                            assignments(&arm.body, out);
                        }
                    }
                    Statement::Loop {
                        condition,
                        body,
                        step,
                        ..
                    } => {
                        assignments(condition, out);
                        assignments(body, out);
                        assignments(step, out);
                    }
                    _ => {}
                }
            }
        }
        let parameters = function
            .parameters
            .iter()
            .filter(|(_, ty, mode)| matches!(ty, Ty::Ref(_)) || *mode == Passing::Mut)
            .map(|(slot, _, _)| *slot);
        let mut found = vec![];
        assignments(&function.body, &mut found);
        let mut derived = parameters.collect::<BTreeSet<_>>();
        let candidates = found
            .iter()
            .map(|(slot, _)| *slot)
            .filter(|slot| matches!(function.locals[slot.id].ty, Ty::Ref(_)))
            .filter(|slot| !derived.contains(slot))
            .collect::<BTreeSet<_>>();
        let mut assumed = derived.union(&candidates).copied().collect::<BTreeSet<_>>();
        loop {
            let refuted = candidates
                .iter()
                .filter(|slot| assumed.contains(slot))
                .filter(|slot| {
                    found.iter().any(|(target, atom)| {
                        target == *slot && !atom.is_some_and(|atom| outer(atom, &assumed))
                    })
                })
                .copied()
                .collect::<Vec<_>>();
            if refuted.is_empty() {
                break;
            }
            for slot in refuted {
                assumed.remove(&slot);
            }
        }
        derived.extend(assumed);
        derived
    }
    /// Closures whose environment points at this frame: a capture by reference, or a view
    /// that does not reach the caller's data, and every slot that copies one of them.
    fn frame_closures(function: &Function, derived: &BTreeSet<Slot>) -> BTreeSet<Slot> {
        fn collect<'c>(code: &'c [Statement], out: &mut Vec<(Slot, &'c Rhs)>) {
            for statement in code {
                match statement {
                    Statement::Set(slot, rhs, _) => out.push((*slot, rhs)),
                    Statement::If(_, a, b) => {
                        collect(a, out);
                        collect(b, out);
                    }
                    Statement::Match(_, arms) => {
                        for arm in arms {
                            collect(&arm.test, out);
                            if let Some((code, _)) = &arm.guard {
                                collect(code, out);
                            }
                            collect(&arm.body, out);
                        }
                    }
                    Statement::Loop {
                        condition,
                        body,
                        step,
                        ..
                    } => {
                        collect(condition, out);
                        collect(body, out);
                        collect(step, out);
                    }
                    _ => {}
                }
            }
        }
        let mut sets = vec![];
        collect(&function.body, &mut sets);
        let mut escaping = sets
            .iter()
            .filter(|(_, rhs)| {
                matches!(rhs, Rhs::ClosureNew(_, _, Some((_, values))) if values.iter().any(|(atom, ty, by_ref)| {
                    *by_ref && !place(atom, derived) || matches!(ty, Ty::Ref(_)) && !outer(atom, derived)
                }))
            })
            .map(|(slot, _)| *slot)
            .collect::<BTreeSet<_>>();
        loop {
            let before = escaping.len();
            for (slot, rhs) in &sets {
                if let Rhs::Copy(Atom::Slot(source)) | Rhs::Take(Atom::Slot(source)) = rhs
                    && escaping.contains(source)
                {
                    escaping.insert(*slot);
                }
            }
            if escaping.len() == before {
                return escaping;
            }
        }
    }
    fn safe(
        signatures: &[(Vec<(Ty, bool)>, Ty)],
        current: usize,
        target: usize,
        args: &[(Atom, Passing)],
        derived: &BTreeSet<Slot>,
        escaping: &BTreeSet<Slot>,
    ) -> bool {
        // A closure that points at this frame's locals cannot outlive the frame.
        !args
            .iter()
            .any(|(atom, _)| matches!(atom, Atom::Slot(slot) if escaping.contains(slot)))
            && signatures[current].1 == signatures[target].1
            && signatures[target].0.len() == args.len()
            && signatures[target]
                .0
                .iter()
                .zip(args)
                .all(|((ty, pointer), (atom, _))| {
                    !(*pointer || matches!(ty, Ty::Ref(_))) || outer(atom, derived)
                })
    }
    fn visit(
        code: &mut Vec<Statement>,
        current: usize,
        signatures: &[(Vec<(Ty, bool)>, Ty)],
        derived: &BTreeSet<Slot>,
        escaping: &BTreeSet<Slot>,
        edges: &mut Vec<Edge>,
    ) {
        let mut i = 0;
        while i < code.len() {
            if let Statement::Return(mut returned, cleanups) = code[i].clone() {
                if cleanups.iter().any(|c| !matches!(c, Cleanup::Drop(_))) {
                    i += 1;
                    continue;
                }
                let mut j = i;
                // Drops between the joined value and the return still run on each new path.
                let mut skipped = vec![];
                while j > 0 {
                    j -= 1;
                    match &code[j] {
                        Statement::Drop(slots) => {
                            skipped.splice(0..0, slots.iter().copied().map(Cleanup::Drop));
                            continue;
                        }
                        Statement::Set(
                            slot,
                            Rhs::Copy(Atom::Slot(source)) | Rhs::Take(Atom::Slot(source)),
                            _,
                        ) if matches!(returned, Atom::Slot(found) if found == *slot) => {
                            returned = Atom::Slot(*source);
                        }
                        Statement::Set(slot, Rhs::Call(target, args), loc) if matches!(returned, Atom::Slot(found) if found == *slot) =>
                        {
                            let identical = signatures[current] == signatures[*target];
                            let safe = safe(signatures, current, *target, args, derived, escaping);
                            edges.push((*target, loc.clone(), identical, safe));
                            if safe {
                                let tail = Statement::TailCall(*target, args.clone());
                                code.splice(j..=i, [tail]);
                                i = j;
                            }
                            break;
                        }
                        Statement::Evaluate(Rhs::Call(target, args), loc)
                            if matches!(returned, Atom::Unit) =>
                        {
                            let identical = signatures[current] == signatures[*target];
                            let safe = safe(signatures, current, *target, args, derived, escaping);
                            edges.push((*target, loc.clone(), identical, safe));
                            if safe {
                                code.splice(j..=i, [Statement::TailCall(*target, args.clone())]);
                                i = j;
                            }
                            break;
                        }
                        Statement::Match(id, arms) => {
                            let cleanups = skipped
                                .iter()
                                .cloned()
                                .chain(cleanups.iter().cloned())
                                .collect::<Vec<_>>();
                            let mut arms = arms.clone();
                            for arm in &mut arms {
                                if !ends_with_return(&arm.body) {
                                    arm.body.extend(arm.cleanup.clone());
                                    arm.body.push(Statement::Return(
                                        returned.clone(),
                                        cleanups.clone(),
                                    ));
                                }
                            }
                            code.splice(j..=i, [Statement::Match(*id, arms)]);
                            i = j;
                            break;
                        }
                        Statement::If(condition, left, right) => {
                            let cleanups = skipped
                                .iter()
                                .cloned()
                                .chain(cleanups.iter().cloned())
                                .collect::<Vec<_>>();
                            let mut left = left.clone();
                            let mut right = right.clone();
                            for branch in [&mut left, &mut right] {
                                if !ends_with_return(branch) {
                                    branch.push(Statement::Return(
                                        returned.clone(),
                                        cleanups.clone(),
                                    ));
                                }
                            }
                            code.splice(j..=i, [Statement::If(condition.clone(), left, right)]);
                            i = j;
                            break;
                        }
                        _ => break,
                    }
                }
            }
            match &mut code[i] {
                Statement::If(_, left, right) => {
                    visit(left, current, signatures, derived, escaping, edges);
                    visit(right, current, signatures, derived, escaping, edges);
                }
                Statement::Match(_, arms) => {
                    for arm in arms {
                        visit(&mut arm.test, current, signatures, derived, escaping, edges);
                        if let Some((code, _)) = &mut arm.guard {
                            visit(code, current, signatures, derived, escaping, edges);
                        }
                        visit(&mut arm.body, current, signatures, derived, escaping, edges);
                    }
                }
                Statement::Loop {
                    condition,
                    body,
                    step,
                    ..
                } => {
                    visit(condition, current, signatures, derived, escaping, edges);
                    visit(body, current, signatures, derived, escaping, edges);
                    visit(step, current, signatures, derived, escaping, edges);
                }
                _ => {}
            }
            i += 1;
        }
    }
    let signatures = program.functions.iter().map(signature).collect::<Vec<_>>();
    let mut graph = vec![Vec::new(); signatures.len()];
    for (index, function) in program.functions.iter_mut().enumerate().skip(1) {
        let derived = derived(function);
        let escaping = frame_closures(function, &derived);
        visit(
            &mut function.body,
            index,
            &signatures,
            &derived,
            &escaping,
            &mut graph[index],
        );
    }
    fn reaches(
        graph: &[Vec<Edge>],
        from: usize,
        target: usize,
        only_safe: bool,
        seen: &mut BTreeSet<usize>,
    ) -> bool {
        from == target
            || seen.insert(from)
                && graph[from].iter().any(|(next, _, _, safe)| {
                    (*safe || !only_safe) && reaches(graph, *next, target, only_safe, seen)
                })
    }
    // Aggregate arguments can be passed indirectly by the C ABI. A tail call
    // must not depend on local aggregate storage surviving the replaced frame,
    // even when both signatures match. Keep those values in the shared block.
    let mut grouped = BTreeSet::new();
    for (caller, edges) in graph.iter().enumerate() {
        let aggregate = signatures[caller].0.iter().any(|(ty, by_ref)| {
            !*by_ref
                && matches!(
                    ty,
                    Ty::Record(_)
                        | Ty::String
                        | Ty::Error
                        | Ty::Dyn(_)
                        | Ty::DynBox(_)
                        | Ty::ArenaRef(_)
                )
        });
        for (target, _, identical, safe) in edges {
            if *identical && !aggregate || !*safe || grouped.contains(&caller) {
                continue;
            }
            if !reaches(&graph, *target, caller, true, &mut BTreeSet::new()) {
                continue;
            }
            let group = (0..graph.len())
                .filter(|member| {
                    reaches(&graph, caller, *member, true, &mut BTreeSet::new())
                        && reaches(&graph, *member, caller, true, &mut BTreeSet::new())
                })
                .collect::<Vec<_>>();
            grouped.extend(group.iter().copied());
            program.tail_groups.push(group);
        }
    }
    let mut errors = vec![];
    for (caller, edges) in graph.iter().enumerate() {
        for (target, location, _, safe) in edges {
            if !safe && reaches(&graph, *target, caller, false, &mut BTreeSet::new()) {
                let module = project
                    .modules
                    .iter()
                    .position(|m| m.parsed.source.name == location.file)
                    .unwrap();
                errors.push(LocatedDiagnostic { module, diagnostic: Diagnostic::new(
                    "E_NATIVE_TAIL_CALL", "This tail-recursive cycle needs a calling convention the native backend does not support yet.", location.span
                ).with_hint("A tail call ends this frame, so a borrowed or mut argument must come from this function's own parameters, not a local. Functions in the cycle also need matching parameter and result layouts.") });
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Whether every path through this body returns the named parameter. Used to
/// preserve an exact pointer alias through an `if` or `match` helper.
fn returns_parameter(node: &Node, name: &str, source: &crate::source::SourceFile) -> bool {
    let node = ungroup_node(node);
    match node.kind {
        K::Name => source.slice(node.span) == name,
        K::Group => node
            .children
            .first()
            .is_some_and(|child| returns_parameter(child, name, source)),
        K::If => {
            node.children.len() == 3
                && returns_parameter(&node.children[1], name, source)
                && returns_parameter(&node.children[2], name, source)
        }
        K::Match => {
            !node.children.is_empty()
                && node.children[1..].iter().all(|arm| {
                    arm.children
                        .last()
                        .is_some_and(|body| returns_parameter(body, name, source))
                })
        }
        K::Block => {
            !node.descendants().any(|child| child.kind == K::Return)
                && node
                    .children
                    .last()
                    .is_some_and(|last| returns_parameter(last, name, source))
        }
        _ => false,
    }
}

/// The root name of a field path with no calls, such as `p.point` -> `p`.
fn place_root_name<'n>(node: &'n Node, source: &'n crate::source::SourceFile) -> Option<&'n str> {
    let node = ungroup_node(node);
    match node.kind {
        K::Name => Some(source.slice(node.span)),
        K::Member => node
            .children
            .first()
            .and_then(|root| place_root_name(root, source)),
        // A method call's receiver is the value the result may borrow.
        K::Call => node.children.first().and_then(|callee| {
            (callee.kind == K::Member)
                .then(|| place_root_name(&callee.children[0], source))
                .flatten()
        }),
        _ => None,
    }
}

fn ungroup_node(mut node: &Node) -> &Node {
    while node.kind == K::Group {
        node = &node.children[0];
    }
    node
}

fn protect(value: &Value, protected: &mut BTreeSet<Slot>) {
    fn slots(atom: &Atom, protected: &mut BTreeSet<Slot>) {
        match atom {
            Atom::Slot(slot) => {
                protected.insert(*slot);
            }
            Atom::Field(inner, ..)
            | Atom::Address(inner, _)
            | Atom::Payload(inner, ..)
            | Atom::Tag(inner)
            | Atom::ErrorType(inner)
            | Atom::ErrorPayload(inner, _)
            | Atom::ResourceValue(inner, _)
            | Atom::CollectionLength(inner)
            | Atom::LengthLess(inner, _)
            | Atom::ArrayLength(inner)
            | Atom::MapCapacity(inner)
            | Atom::StepOwner(inner)
            | Atom::OwnerValue(inner, ..)
            | Atom::EnvField(inner, ..) => slots(inner, protected),
            Atom::ListItem(a, b, _)
            | Atom::ArrayItem(a, b, _)
            | Atom::MapOccupied(a, b)
            | Atom::MapItem(a, b, _)
            | Atom::SetItem(a, b, ..) => {
                slots(a, protected);
                slots(b, protected);
            }
            _ => {}
        }
    }
    slots(&value.atom, protected);
    protected.extend(value.keepalive.iter().copied());
    match value.callable.as_deref() {
        Some(FunctionValue::Partial {
            captures, receiver, ..
        }) => {
            for capture in captures.iter().flatten().chain(receiver.as_deref()) {
                protect(capture, protected);
            }
        }
        Some(FunctionValue::Lambda { captures, .. }) => {
            for capture in captures.values() {
                protect(capture, protected);
            }
        }
        Some(FunctionValue::Sequence { source, steps }) => {
            protect(source, protected);
            for step in steps {
                match step {
                    SequenceStep::Map(value) | SequenceStep::Filter(value) => {
                        protect(value, protected)
                    }
                }
            }
        }
        Some(FunctionValue::Range { .. }) | Some(FunctionValue::Named) | None => {}
    }
}

/// A proof parameter has the type Proof. Proofs have no run-time value, so calls pass none (Q115).
fn is_proof_type(ty: &Type) -> bool {
    matches!(ty, Type::Named(name, args) if name == "Proof" && args.is_empty())
}

include!("core_tensor.rs");

/// The trait name, its arguments and its associated bindings in a declared trait type, such as
/// `Text`, `Equals[T]` or `Iterator(Item = Int)`.
fn trait_name_and_args(ty: &Type) -> Option<(String, Vec<Type>, BTreeMap<String, Type>)> {
    match ty {
        Type::Named(name, args) => Some((name.clone(), args.clone(), BTreeMap::new())),
        Type::Associated(base, bindings) => {
            let (name, args, _) = trait_name_and_args(base)?;
            Some((name, args, bindings.clone()))
        }
        _ => None,
    }
}

include!("core_variable.rs");
include!("core_numerical.rs");
