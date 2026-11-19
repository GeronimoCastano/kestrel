use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Type {
    Named(String, Vec<Type>),
    Parameter(String),
    Rigid(String),
    Variable(usize),
    Tuple(Vec<Type>),
    Function(FunctionType),
    Overloaded(Vec<FunctionType>),
    Union(Vec<Type>),
    Borrow(Box<Type>),
    Dynamic(Box<Type>),
    Indexed(Box<Type>, Index),
    Lazy(Box<Type>, Traversal),
    Cursor(Box<Type>, bool),
    /// The view a `using g := m.lock()` block gets of a Mutex value.
    Guard(Box<Type>),
    StoredView(Box<Type>),
    Associated(Box<Type>, BTreeMap<String, Type>),
    Constructor(String, Vec<Type>),
    /// A compile-time module namespace; it cannot be stored or passed as a value.
    Namespace(String),
    Never,
    Invalid,
}

/// Permissions delayed until a lazy sequence is traversed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Traversal {
    pub effects: bool,
    pub mutates: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FunctionType {
    pub parameters: Vec<Parameter>,
    pub result: Box<Type>,
    pub bounds: BTreeMap<String, Vec<Type>>,
    pub requirements: Vec<(Type, Type)>,
    pub effects: bool,
    pub receiver_mutates: bool,
    pub callable: Option<crate::resolution::Callable>,
    pub receiver: Option<Box<Type>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parameter {
    pub name: Option<String>,
    pub ty: Type,
    pub default: bool,
    pub passing: Passing,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Passing {
    #[default]
    Borrow,
    Mut,
    Move,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Index {
    Number(u64),
    Variable(String),
    Rigid(String),
    Add(Box<Index>, Box<Index>),
    Scale(u64, Box<Index>),
}

/// The marker a trait signature writes as `Self`: the implementing class, bound
/// where the trait is implemented. A class body resolves it to the class itself.
pub fn self_type() -> Type {
    Type::Parameter("Self".into())
}

pub fn is_self_type(ty: &Type) -> bool {
    matches!(ty, Type::Parameter(name) | Type::Rigid(name) if name == "Self")
}

pub fn is_tensor(ty: &Type) -> bool {
    match ty {
        Type::Borrow(t) | Type::StoredView(t) => is_tensor(t),
        Type::Named(n, _) => matches!(n.as_str(), "Tensor" | "DynTensor"),
        _ => false,
    }
}

/// A KesNet differentiable value. Variables copy: every copy shares one native
/// node, so parameters, views and optimizer state see the same storage.
pub fn is_variable(ty: &Type) -> bool {
    match ty {
        Type::Borrow(t) | Type::StoredView(t) => is_variable(t),
        Type::Named(n, args) => n == "Variable" && args.is_empty(),
        _ => false,
    }
}

pub fn tensor_shape(ty: &Type) -> Option<Vec<Index>> {
    match ty {
        Type::Borrow(t) | Type::StoredView(t) => tensor_shape(t),
        Type::Named(n, args) if n == "Tensor" && args.len() == 2 => {
            let Type::Tuple(parts) = &args[1] else {
                return None;
            };
            parts
                .iter()
                .map(|part| match part {
                    Type::Indexed(_, i) => Some(i.clone()),
                    _ => None,
                })
                .collect()
        }
        _ => None,
    }
}

pub fn tensor_type(shape: Option<Vec<Index>>) -> Type {
    match shape {
        Some(shape) => Type::applied(
            "Tensor",
            vec![
                Type::named("double"),
                Type::Tuple(
                    shape
                        .into_iter()
                        .map(|i| Type::Indexed(Box::new(Type::named("Nat")), i))
                        .collect(),
                ),
            ],
        ),
        None => Type::applied("DynTensor", vec![Type::named("double")]),
    }
}

pub fn scalar(ty: &Type) -> bool {
    matches!(ty, Type::Named(name, args) if args.is_empty() && matches!(name.as_str(), "Int" | "double" | "Bool" | "void" | "Nat" | "Proof"))
}

/// A copy-rule definition: its type parameters and every field or payload type.
type CopyShape = (Vec<String>, Vec<Type>);

thread_local! {
    // Type parameters bounded by `Copy` in the function bodies being checked, innermost last.
    static COPY_PARAMETERS: std::cell::RefCell<Vec<BTreeSet<String>>> = const { std::cell::RefCell::new(Vec::new()) };
    static COPY_DEFINITIONS: std::cell::RefCell<BTreeMap<String, CopyShape>> = const { std::cell::RefCell::new(BTreeMap::new()) };
}

pub(crate) struct CopyDefinitions(BTreeMap<String, CopyShape>);
impl Drop for CopyDefinitions {
    fn drop(&mut self) {
        COPY_DEFINITIONS
            .with(|definitions| *definitions.borrow_mut() = std::mem::take(&mut self.0));
    }
}
pub(crate) fn copy_definitions(
    definitions: &BTreeMap<String, crate::checker::Definition>,
) -> CopyDefinitions {
    let values = definitions
        .iter()
        .filter(|(_, d)| {
            d.kind == crate::syntax::SyntaxKind::Enum && d.parent.is_none() || d.record
        })
        .map(|(name, d)| {
            (
                name.clone(),
                (
                    d.parameters.clone(),
                    if d.record {
                        d.fields.values().cloned().collect()
                    } else {
                        d.variants.iter().flat_map(|v| v.fields.clone()).collect()
                    },
                ),
            )
        })
        .collect();
    CopyDefinitions(
        COPY_DEFINITIONS
            .with(|definitions| std::mem::replace(&mut *definitions.borrow_mut(), values)),
    )
}

fn aggregate_copies(ty: &Type, seen: &mut BTreeSet<String>) -> bool {
    if seen.len() > 64 {
        return false;
    }
    if let Type::Tuple(parts) = ty {
        return parts
            .iter()
            .all(|part| builtin_copies(part) || aggregate_copies(part, seen));
    }
    let Type::Named(name, arguments) = ty else {
        return false;
    };
    if name == "Option" && arguments.len() == 1 {
        return builtin_copies(&arguments[0]) || aggregate_copies(&arguments[0], seen);
    }
    let definition = COPY_DEFINITIONS.with(|definitions| definitions.borrow().get(name).cloned());
    let Some((parameters, fields)) = definition else {
        return false;
    };
    let key = ty.to_string();
    if !seen.insert(key.clone()) {
        return true;
    }
    let replacements = parameters
        .into_iter()
        .zip(arguments.iter().cloned())
        .collect();
    let result = fields.into_iter().all(|field| {
        let field = field.replace_parameters(&replacements);
        aggregate_copies(&field, seen) || builtin_copies(&field)
    });
    seen.remove(&key);
    result
}

/// Starts a function body whose `T : Copy` parameters copy like the types they stand for.
pub fn push_copy_parameters(names: BTreeSet<String>) {
    COPY_PARAMETERS.with(|stack| stack.borrow_mut().push(names));
}
pub fn pop_copy_parameters() {
    COPY_PARAMETERS.with(|stack| stack.borrow_mut().pop());
}
pub fn copy_parameter(name: &str) -> bool {
    COPY_PARAMETERS.with(|stack| stack.borrow().iter().any(|names| names.contains(name)))
}

/// The types that satisfy `T : Copy`: they copy, and nothing inside them can borrow, so a
/// closure type is excluded. A List of Ints is Copy; a List of closures or of classes is not.
pub fn plain_copy(ty: &Type) -> bool {
    fn visit(ty: &Type, seen: &mut BTreeSet<String>) -> bool {
        if seen.len() > 64 {
            return false;
        }
        match ty {
            Type::Function(_) | Type::Borrow(_) | Type::StoredView(_) => false,
            Type::Tuple(parts) => parts.iter().all(|part| visit(part, seen)),
            Type::Named(name, args) => {
                if matches!(name.as_str(), "ArenaRef" | "CPointer" | "CConstPointer") {
                    return true;
                }
                if !copies(ty) || !args.iter().all(|part| visit(part, seen)) {
                    return false;
                }
                let definition =
                    COPY_DEFINITIONS.with(|definitions| definitions.borrow().get(name).cloned());
                let Some((parameters, fields)) = definition else {
                    return true;
                };
                let key = ty.to_string();
                if !seen.insert(key.clone()) {
                    return true;
                }
                let replacements = parameters.into_iter().zip(args.iter().cloned()).collect();
                let result = fields
                    .iter()
                    .all(|field| visit(&field.replace_parameters(&replacements), seen));
                seen.remove(&key);
                result
            }
            _ => copies(ty),
        }
    }
    visit(ty, &mut BTreeSet::new())
}

/// Values that copy instead of moving: scalars, and immutable built-ins whose
/// copies share storage behind a count (confirmed 2026-11-03, Q28).
pub fn copies(ty: &Type) -> bool {
    builtin_copies(ty) || aggregate_copies(ty, &mut BTreeSet::new())
}
fn builtin_copies(ty: &Type) -> bool {
    scalar(ty)
        || matches!(ty, Type::Rigid(name) | Type::Parameter(name) if copy_parameter(name))
        || matches!(ty,Type::Named(name,_) if matches!(name.as_str(),"CPointer"|"CConstPointer"|"ArenaRef") || crate::interop::C_TYPES.contains(&name.as_str()))
        || matches!(ty, Type::Named(name, _) if matches!(name.as_str(), "String" | "Bytes" | "List" | "Map" | "Set" | "SortedSet" | "Variable"))
        // A closure that changes what it captured moves; other closures share their storage (Q40).
        || matches!(ty, Type::Function(function) if !function.receiver_mutates)
}

/// Stored arena callables must retain the permission to change captured state.
pub(crate) fn same_capture_permissions(a: &Type, b: &Type) -> bool {
    match (a, b) {
        (Type::Function(a), Type::Function(b)) => {
            a.receiver_mutates == b.receiver_mutates
                && a.parameters
                    .iter()
                    .zip(&b.parameters)
                    .all(|(a, b)| same_capture_permissions(&a.ty, &b.ty))
                && same_capture_permissions(&a.result, &b.result)
        }
        (Type::Named(_, a), Type::Named(_, b))
        | (Type::Tuple(a), Type::Tuple(b))
        | (Type::Union(a), Type::Union(b)) => {
            a.iter().zip(b).all(|(a, b)| same_capture_permissions(a, b))
        }
        (Type::Borrow(a), Type::Borrow(b))
        | (Type::StoredView(a), Type::StoredView(b))
        | (Type::Dynamic(a), Type::Dynamic(b))
        | (Type::Guard(a), Type::Guard(b))
        | (Type::Lazy(a, _), Type::Lazy(b, _))
        | (Type::Cursor(a, _), Type::Cursor(b, _))
        | (Type::Indexed(a, _), Type::Indexed(b, _)) => same_capture_permissions(a, b),
        _ => true,
    }
}

impl Type {
    pub fn named(name: &str) -> Self {
        Self::Named(name.into(), vec![])
    }
    pub fn applied(name: &str, args: Vec<Type>) -> Self {
        Self::Named(name.into(), args)
    }
    pub fn function(parameters: Vec<Type>, result: Type) -> Self {
        Self::Function(FunctionType {
            parameters: parameters
                .into_iter()
                .map(|ty| Parameter {
                    name: None,
                    ty,
                    default: false,
                    passing: Passing::Borrow,
                })
                .collect(),
            result: Box::new(result),
            bounds: BTreeMap::new(),
            requirements: vec![],
            effects: false,
            receiver_mutates: false,
            callable: None,
            receiver: None,
        })
    }
    pub fn union(left: Type, right: Type) -> Self {
        let mut members = vec![];
        for ty in [left, right] {
            let parts = if let Type::Union(parts) = ty {
                parts
            } else {
                vec![ty]
            };
            for part in parts {
                if !members.contains(&part) {
                    members.push(part);
                }
            }
        }
        Self::Union(members)
    }
    /// Whether a named type appears anywhere inside this one.
    pub fn mentions(&self, name: &str) -> bool {
        let mut found = false;
        self.map(&mut |ty| {
            found |= matches!(ty, Type::Named(n, _) if n == name);
            None
        });
        found
    }
    pub fn map(&self, f: &mut impl FnMut(&Type) -> Option<Type>) -> Type {
        if let Some(ty) = f(self) {
            return ty;
        }
        match self {
            Self::Named(name, args) => {
                Self::Named(name.clone(), args.iter().map(|t| t.map(f)).collect())
            }
            Self::Tuple(args) => Self::Tuple(args.iter().map(|t| t.map(f)).collect()),
            Self::Union(args) => Self::Union(args.iter().map(|t| t.map(f)).collect()),
            Self::Borrow(t) => Self::Borrow(Box::new(t.map(f))),
            Self::Dynamic(t) => Self::Dynamic(Box::new(t.map(f))),
            Self::Lazy(t, traversal) => Self::Lazy(Box::new(t.map(f)), *traversal),
            Self::Cursor(t, effects) => Self::Cursor(Box::new(t.map(f)), *effects),
            Self::Guard(t) => Self::Guard(Box::new(t.map(f))),
            Self::StoredView(t) => Self::StoredView(Box::new(t.map(f))),
            Self::Associated(base, bindings) => Self::Associated(
                Box::new(base.map(f)),
                bindings
                    .iter()
                    .map(|(key, ty)| (key.clone(), ty.map(f)))
                    .collect(),
            ),
            Self::Indexed(t, index) => Self::Indexed(Box::new(t.map(f)), index.clone()),
            Self::Constructor(name, args) => {
                Self::Constructor(name.clone(), args.iter().map(|t| t.map(f)).collect())
            }
            Self::Function(fun) => Self::Function(fun.map(f)),
            Self::Overloaded(funs) => Self::Overloaded(funs.iter().map(|fun| fun.map(f)).collect()),
            _ => self.clone(),
        }
    }
    pub fn replace_parameters(&self, replacements: &BTreeMap<String, Type>) -> Type {
        self.map(&mut |ty| {
            if let Type::Parameter(name) = ty {
                replacements.get(name).cloned()
            } else {
                None
            }
        })
    }
    pub(crate) fn without_parameter_names(&self) -> Type {
        self.map(&mut |part| {
            if let Type::Function(function) = part {
                let mut function = function.map(&mut |inner| Some(inner.without_parameter_names()));
                for parameter in &mut function.parameters {
                    parameter.name = None;
                }
                Some(Type::Function(function))
            } else {
                None
            }
        })
    }
    pub(crate) fn same_type(&self, other: &Type) -> bool {
        self == other || self.without_parameter_names() == other.without_parameter_names()
    }
    pub fn variables(&self) -> BTreeSet<usize> {
        let mut variables = BTreeSet::new();
        self.map(&mut |ty| {
            if let Type::Variable(id) = ty {
                variables.insert(*id);
            }
            None
        });
        variables
    }
    pub fn map_indices(&self, f: &mut impl FnMut(&Index) -> Index) -> Type {
        self.map(&mut |ty| {
            if let Type::Indexed(inner, index) = ty {
                Some(Type::Indexed(Box::new(inner.map_indices(f)), index.map(f)))
            } else {
                None
            }
        })
    }
}

impl Index {
    pub fn map(&self, f: &mut impl FnMut(&Index) -> Index) -> Index {
        let mapped = match self {
            Self::Add(a, b) => Self::Add(Box::new(a.map(f)), Box::new(b.map(f))),
            Self::Scale(n, index) => Self::Scale(*n, Box::new(index.map(f))),
            _ => self.clone(),
        };
        f(&mapped)
    }
    fn contains(&self, name: &str) -> bool {
        match self {
            Self::Variable(variable) => variable == name,
            Self::Add(a, b) => a.contains(name) || b.contains(name),
            Self::Scale(n, index) => *n != 0 && index.contains(name),
            _ => false,
        }
    }

    pub(crate) fn scaled(factor: u64, index: Index) -> Option<Self> {
        let scaled = match (factor, index) {
            (0, _) => Self::Number(0),
            (1, index) => index,
            (factor, Self::Number(n)) => Self::Number(factor.checked_mul(n)?),
            (factor, Self::Scale(n, index)) => Self::Scale(factor.checked_mul(n)?, index),
            (factor, index) => Self::Scale(factor, Box::new(index)),
        };
        scaled.representable().then_some(scaled)
    }

    pub(crate) fn representable(&self) -> bool {
        index_sum(self).is_some()
    }
}

impl FunctionType {
    pub fn map(&self, f: &mut impl FnMut(&Type) -> Option<Type>) -> Self {
        Self {
            parameters: self
                .parameters
                .iter()
                .map(|p| Parameter {
                    ty: p.ty.map(f),
                    ..p.clone()
                })
                .collect(),
            result: Box::new(self.result.map(f)),
            bounds: self
                .bounds
                .iter()
                .map(|(key, bounds)| (key.clone(), bounds.iter().map(|t| t.map(f)).collect()))
                .collect(),
            effects: self.effects,
            receiver_mutates: self.receiver_mutates,
            callable: self.callable.clone(),
            receiver: self.receiver.as_ref().map(|ty| Box::new(ty.map(f))),

            requirements: self
                .requirements
                .iter()
                .map(|(ty, requirement)| (ty.map(f), requirement.map(f)))
                .collect(),
        }
    }
    pub fn same_parameters(&self, other: &Self) -> bool {
        self.parameters.len() == other.parameters.len()
            && self
                .parameters
                .iter()
                .zip(&other.parameters)
                .all(|(a, b)| a.ty.same_type(&b.ty))
    }
}

#[derive(Clone, Debug, Default)]
pub struct Inference {
    bindings: Vec<Option<Type>>,
    index_bindings: BTreeMap<String, Index>,
    next_index: usize,
    /// What a match arm learned about a fixed length: in `Cons(x, rest)`, the
    /// function's `n` is `k + 1` for a fresh fixed `k`. The checker restores
    /// these after each arm.
    rigid_equations: BTreeMap<String, Index>,
    linear_equations: Vec<(Index, Index)>,
}

/// Facts saved and restored around a length-indexed match arm.
#[derive(Clone, Debug)]
pub struct IndexFacts {
    fixed: BTreeMap<String, Index>,
    linear: Vec<(Index, Index)>,
}

impl Inference {
    pub fn fresh(&mut self) -> Type {
        let id = self.bindings.len();
        self.bindings.push(None);
        Type::Variable(id)
    }
    pub fn resolve(&self, ty: &Type) -> Type {
        ty.map(&mut |t| {
            if let Type::Variable(id) = t {
                self.bindings
                    .get(*id)
                    .and_then(|bound| bound.as_ref())
                    .map(|bound| self.resolve(bound))
            } else if let Type::StoredView(inner) = t {
                let inner = self.resolve(inner);
                Some(if copies(&inner) || inner == Type::Invalid {
                    inner
                } else if is_tensor(&inner)
                    || matches!(
                        inner,
                        Type::Variable(_) | Type::Parameter(_) | Type::Rigid(_)
                    )
                {
                    Type::StoredView(Box::new(inner))
                } else {
                    Type::Borrow(Box::new(inner))
                })
            } else if let Type::Indexed(inner, index) = t {
                Some(Type::Indexed(
                    Box::new(self.resolve(inner)),
                    self.resolve_index(index),
                ))
            } else {
                None
            }
        })
    }
    pub fn instantiate(&mut self, ty: &Type) -> Type {
        self.instantiate_with_parameters(ty).0
    }
    pub fn instantiate_with_parameters(&mut self, ty: &Type) -> (Type, BTreeMap<String, Type>) {
        let mut parameters = BTreeMap::new();
        let ty = ty.map(&mut |t| {
            if let Type::Parameter(name) = t {
                Some(
                    parameters
                        .entry(name.clone())
                        .or_insert_with(|| self.fresh())
                        .clone(),
                )
            } else {
                None
            }
        });
        let mut indices = BTreeMap::new();
        let ty = ty.map_indices(&mut |index| {
            if let Index::Variable(name) = index {
                indices
                    .entry(name.clone())
                    .or_insert_with(|| {
                        let id = self.next_index;
                        self.next_index += 1;
                        Index::Variable(format!("@{id}:{name}"))
                    })
                    .clone()
            } else {
                index.clone()
            }
        });
        (ty, parameters)
    }
    pub fn instantiate_variables(&mut self, ty: &Type, variables: &BTreeSet<usize>) -> Type {
        let mut replacements = BTreeMap::new();
        ty.map(&mut |t| {
            if let Type::Variable(id) = t
                && variables.contains(id)
            {
                return Some(
                    replacements
                        .entry(*id)
                        .or_insert_with(|| self.fresh())
                        .clone(),
                );
            }
            None
        })
    }
    /// Transactional: a failed overload candidate must not constrain later ones.
    pub fn unify(&mut self, expected: &Type, found: &Type) -> bool {
        let saved = self.clone();
        if self.unify_inner(expected, found) {
            true
        } else {
            *self = saved;
            false
        }
    }
    fn unify_inner(&mut self, expected: &Type, found: &Type) -> bool {
        let expected = self.resolve(expected);
        let found = self.resolve(found);
        if expected == found
            || matches!(expected, Type::Invalid | Type::Never)
            || matches!(found, Type::Invalid | Type::Never)
        {
            return true;
        }
        match (&expected, &found) {
            (Type::Variable(id), ty) | (ty, Type::Variable(id)) => {
                if ty.variables().contains(id) {
                    return false;
                }
                self.bindings[*id] = Some(ty.clone());
                true
            }
            (Type::Named(a, xs), Type::Named(b, ys)) if a == b => {
                (!matches!(a.as_str(), "Arena" | "ArenaRef")
                    || same_capture_permissions(&expected, &found))
                    && self.unify_list(xs, ys)
            }
            (Type::Tuple(xs), Type::Tuple(ys)) => self.unify_list(xs, ys),
            (Type::Borrow(x), Type::Borrow(y))
            | (Type::Dynamic(x), Type::Dynamic(y))
            | (Type::Guard(x), Type::Guard(y)) => self.unify_inner(x, y),
            (Type::Lazy(x, a), Type::Lazy(y, b)) if a == b => self.unify_inner(x, y),
            (Type::Cursor(x, a), Type::Cursor(y, b)) if a == b => self.unify_inner(x, y),
            (Type::StoredView(x), Type::StoredView(y)) => self.unify_inner(x, y),
            (Type::StoredView(x), Type::Borrow(y)) | (Type::Borrow(y), Type::StoredView(x))
                if !copies(y) =>
            {
                self.unify_inner(x, y)
            }
            (Type::StoredView(x), y) | (y, Type::StoredView(x)) if copies(y) => {
                self.unify_inner(x, y)
            }
            (Type::Associated(a, xs), Type::Associated(b, ys)) => {
                xs.keys().eq(ys.keys())
                    && self.unify_inner(a, b)
                    && xs
                        .iter()
                        .all(|(key, value)| self.unify_inner(value, &ys[key]))
            }
            (Type::Indexed(x, a), Type::Indexed(y, b)) => {
                self.unify_indices(a, b) && self.unify_inner(x, y)
            }
            (Type::Function(a), Type::Function(b)) => {
                a.parameters.len() == b.parameters.len()
                    && a.effects == b.effects
                    && a.receiver_mutates == b.receiver_mutates
                    && a.parameters
                        .iter()
                        .zip(&b.parameters)
                        .all(|(x, y)| x.passing == y.passing && self.unify_inner(&x.ty, &y.ty))
                    && self.unify_inner(&a.result, &b.result)
            }
            _ => false,
        }
    }
    fn unify_list(&mut self, xs: &[Type], ys: &[Type]) -> bool {
        xs.len() == ys.len() && xs.iter().zip(ys).all(|(x, y)| self.unify_inner(x, y))
    }
    pub fn accepts(&mut self, expected: &Type, found: &Type) -> bool {
        if self.unify(expected, found) {
            return true;
        }
        let expected = self.resolve(expected);
        let found = self.resolve(found);
        match (&expected, &found) {
            (Type::Union(expected), Type::Union(found)) => {
                let saved = self.clone();
                for member in found {
                    if !expected.iter().any(|e| self.accepts(e, member)) {
                        *self = saved;
                        return false;
                    }
                }
                true
            }
            (Type::Union(members), _) => members.iter().any(|member| self.accepts(member, &found)),
            (Type::Borrow(inner), _) => self.unify(inner, &found),
            // A pure function may stand where one with outside effects is expected (Q50).
            (Type::Function(wanted), Type::Function(given))
                if (wanted.effects || !given.effects)
                    && (wanted.receiver_mutates || !given.receiver_mutates) =>
            {
                let mut given = given.clone();
                given.effects = wanted.effects;
                given.receiver_mutates = wanted.receiver_mutates;
                self.unify(&expected, &Type::Function(given))
            }
            _ => false,
        }
    }
}

impl Inference {
    /// The arm-local length equations, saved before a match arm.
    pub fn equations(&self) -> IndexFacts {
        IndexFacts {
            fixed: self.rigid_equations.clone(),
            linear: self.linear_equations.clone(),
        }
    }
    /// Restores the length equations after a match arm.
    pub fn restore_equations(&mut self, equations: IndexFacts) {
        self.rigid_equations = equations.fixed;
        self.linear_equations = equations.linear;
    }

    /// Matches a case's length against the matched value's length and learns
    /// its exact equation. The case's own length variables become fresh
    /// fixed lengths, so later code cannot choose them.
    pub fn refine_index(&mut self, scrutinee: &Index, case: &Index) -> bool {
        if self.unify_indices(scrutinee, case) {
            return true;
        }
        let scrutinee = self.resolve_index(scrutinee);
        let Some((terms, offset)) = index_sum(&scrutinee) else {
            return false;
        };
        let case = self.resolve_index(case);
        let Some((case_terms, _)) = index_sum(&case) else {
            return false;
        };
        for ((rigid, name), _) in case_terms {
            if !rigid {
                let id = self.next_index;
                self.next_index += 1;
                let base = name.rsplit(':').next().unwrap_or(&name).to_owned();
                self.index_bindings
                    .insert(name, Index::Rigid(format!("@{id}:{base}'")));
            }
        }
        let case = self.resolve_index(&case);
        let Some((case_terms, case_offset)) = index_sum(&case) else {
            return false;
        };
        // Nonnegative summands of a zero total are individually zero.
        for (summands, total) in [(&scrutinee, &case), (&case, &scrutinee)] {
            if matches!(total, Index::Number(0))
                && let Some((terms, 0)) = index_sum(summands)
            {
                for ((rigid, name), _) in terms {
                    if rigid {
                        self.rigid_equations.insert(name, Index::Number(0));
                    } else {
                        self.index_bindings.insert(name, Index::Number(0));
                    }
                }
            }
        }
        // Keep the cheap substitution for the common single-name case.
        if let [((true, fixed), 1)] = terms.iter().collect::<Vec<_>>().as_slice()
            && let Some(remaining) = case_offset.checked_sub(offset)
            && !case_terms.contains_key(&(true, (*fixed).clone()))
        {
            self.rigid_equations
                .insert((*fixed).clone(), sum_index(&case_terms, remaining));
        } else {
            self.linear_equations.push((scrutinee, case));
        }
        true
    }

    fn resolve_index(&self, index: &Index) -> Index {
        match index {
            Index::Variable(name) => self
                .index_bindings
                .get(name)
                .map(|index| self.resolve_index(index))
                .unwrap_or(index.clone()),
            Index::Rigid(name) => self
                .rigid_equations
                .get(name)
                .map(|index| self.resolve_index(index))
                .unwrap_or(index.clone()),
            Index::Add(a, b) => {
                let a = self.resolve_index(a);
                let b = self.resolve_index(b);
                if let (Index::Number(a), Index::Number(b)) = (&a, &b)
                    && let Some(number) = a.checked_add(*b)
                {
                    return Index::Number(number);
                }
                Index::Add(Box::new(a), Box::new(b))
            }
            Index::Scale(factor, index) => {
                let index = self.resolve_index(index);
                Index::scaled(*factor, index.clone())
                    .unwrap_or_else(|| Index::Scale(*factor, Box::new(index)))
            }
            _ => index.clone(),
        }
    }

    fn unify_indices(&mut self, left: &Index, right: &Index) -> bool {
        let left = self.resolve_index(left);
        let right = self.resolve_index(right);
        if left == right {
            return true;
        }
        if let (Some(a), Some(b)) = (index_sum(&left), index_sum(&right))
            && a == b
        {
            return true;
        }
        let Some((left_base, left_number)) = linear_index(&left) else {
            return self.solve_indices(&left, &right);
        };
        let Some((right_base, right_number)) = linear_index(&right) else {
            return self.solve_indices(&left, &right);
        };
        if left_base == right_base {
            return left_number == right_number;
        }
        if let Some(Index::Variable(name)) = &left_base
            && right_number >= left_number
        {
            let value = index_from_parts(right_base.clone(), right_number - left_number);
            if !value.contains(name) {
                self.index_bindings.insert(name.clone(), value);
                return true;
            }
        }
        if let Some(Index::Variable(name)) = &right_base
            && left_number >= right_number
        {
            let value = index_from_parts(left_base, left_number - right_number);
            if !value.contains(name) {
                self.index_bindings.insert(name.clone(), value);
                return true;
            }
        }
        self.solve_indices(&left, &right)
    }

    /// Cancels what both sides share; a lone length variable left on one side
    /// takes the rest of the other side, as `@c + 1 = k + m + 1` gives `@c = k + m`.
    fn solve_indices(&mut self, left: &Index, right: &Index) -> bool {
        let (Some((mut a, mut x)), Some((mut b, mut y))) = (index_sum(left), index_sum(right))
        else {
            return false;
        };
        for (key, count) in a.iter_mut() {
            if let Some(other) = b.get_mut(key) {
                let shared = (*count).min(*other);
                *count -= shared;
                *other -= shared;
            }
        }
        a.retain(|_, n| *n > 0);
        b.retain(|_, n| *n > 0);
        let shared = x.min(y);
        x -= shared;
        y -= shared;
        for (lone, lone_offset, rest, rest_offset) in [(&a, x, &b, y), (&b, y, &a, x)] {
            if lone_offset == 0
                && let [((false, name), coefficient)] = lone.iter().collect::<Vec<_>>().as_slice()
                && **coefficient > 0
                && !rest.contains_key(&(false, (*name).clone()))
                && rest_offset % **coefficient == 0
                && rest.values().all(|count| count % **coefficient == 0)
            {
                let divided = rest
                    .iter()
                    .map(|(key, count)| (key.clone(), count / **coefficient))
                    .collect();
                let value = sum_index(&divided, rest_offset / **coefficient);
                self.index_bindings.insert((*name).clone(), value);
                return true;
            }
        }
        self.solve_index_facts(left, right)
    }

    fn index_difference(&self, left: &Index, right: &Index) -> Option<IndexEquation> {
        let goal = IndexEquation::between(&self.resolve_index(left), &self.resolve_index(right))?;
        let mut basis: Vec<IndexEquation> = vec![];
        for (left, right) in &self.linear_equations {
            let row = IndexEquation::between(&self.resolve_index(left), &self.resolve_index(right))
                .and_then(|row| row.reduce(&basis))?;
            if !row.terms.is_empty() {
                basis.push(row);
                basis.sort_by(|a, b| index_key_order(a.pivot().unwrap().0, b.pivot().unwrap().0));
            }
        }
        goal.reduce(&basis)
    }

    /// A case is impossible when its equation contradicts nonnegative integer
    /// lengths, including facts established by enclosing patterns.
    pub fn impossible_lengths(&self, left: &Index, right: &Index) -> bool {
        self.index_difference(left, right)
            .is_some_and(|row| row.impossible())
    }

    fn solve_index_facts(&mut self, left: &Index, right: &Index) -> bool {
        let Some(goal) = self.index_difference(left, right) else {
            return false;
        };
        if goal.terms.is_empty() {
            return goal.constant == 0;
        }
        // Only solve a flexible name when the equation gives a representable
        // nonnegative sum. Fixed names are facts, never inference choices.
        for ((rigid, name), coefficient) in &goal.terms {
            if *rigid {
                continue;
            }
            let divide = |value: i128| {
                (value.checked_rem(*coefficient) == Some(0))
                    .then(|| value.checked_neg()?.checked_div(*coefficient))
                    .flatten()
                    .and_then(|n| u64::try_from(n).ok())
            };
            let Some(number) = divide(goal.constant) else {
                continue;
            };
            let Some(terms) = goal
                .terms
                .iter()
                .filter(|(key, _)| *key != &(*rigid, name.clone()))
                .map(|(key, count)| Some((key.clone(), divide(*count)?)))
                .collect::<Option<BTreeMap<_, _>>>()
            else {
                continue;
            };
            self.index_bindings
                .insert(name.clone(), sum_index(&terms, number));
            return true;
        }
        false
    }
}

/// An exact signed linear equation. Elimination scales whole rows by integers;
/// no rounding, floating point or choice of fixed length is involved.
#[derive(Clone, Debug)]
struct IndexEquation {
    terms: BTreeMap<(bool, String), i128>,
    constant: i128,
}

impl IndexEquation {
    fn pivot(&self) -> Option<(&(bool, String), &i128)> {
        self.terms
            .iter()
            .min_by(|(a, _), (b, _)| index_key_order(a, b))
    }

    fn impossible(&self) -> bool {
        if self.terms.is_empty() {
            return self.constant != 0;
        }
        if self.constant > 0 && self.terms.values().all(|n| *n > 0)
            || self.constant < 0 && self.terms.values().all(|n| *n < 0)
        {
            return true;
        }
        // Even a sum with both signs must have an integer solution.
        let gcd = self
            .terms
            .values()
            .fold(0_u128, |a, n| gcd(a, n.unsigned_abs()));
        !self.constant.unsigned_abs().is_multiple_of(gcd)
    }

    fn between(left: &Index, right: &Index) -> Option<Self> {
        let ((left, a), (right, b)) = (index_sum(left)?, index_sum(right)?);
        let mut terms = left
            .into_iter()
            .map(|(key, n)| (key, i128::from(n)))
            .collect::<BTreeMap<_, _>>();
        for (key, n) in right {
            *terms.entry(key).or_default() -= i128::from(n);
        }
        let mut row = Self {
            terms,
            constant: i128::from(a) - i128::from(b),
        };
        row.normalize()?;
        Some(row)
    }

    fn normalize(&mut self) -> Option<()> {
        self.terms.retain(|_, n| *n != 0);
        let gcd = self
            .terms
            .values()
            .chain(std::iter::once(&self.constant))
            .fold(0_u128, |a, n| gcd(a, n.unsigned_abs()));
        if gcd > 1 {
            let gcd = i128::try_from(gcd).ok()?;
            self.constant /= gcd;
            for n in self.terms.values_mut() {
                *n /= gcd;
            }
        }
        Some(())
    }

    fn reduce(mut self, basis: &[Self]) -> Option<Self> {
        for row in basis {
            let (key, coefficient) = row.pivot()?;
            let Some(found) = self.terms.get(key).copied() else {
                continue;
            };
            let divisor =
                i128::try_from(gcd(found.unsigned_abs(), coefficient.unsigned_abs())).ok()?;
            let scale = coefficient / divisor;
            let subtract = found / divisor;
            // Normalization after each cancellation keeps common factors small.
            let mut terms = BTreeMap::new();
            for key in self.terms.keys().chain(row.terms.keys()) {
                let a = self
                    .terms
                    .get(key)
                    .copied()
                    .unwrap_or(0)
                    .checked_mul(scale)?;
                let b = row
                    .terms
                    .get(key)
                    .copied()
                    .unwrap_or(0)
                    .checked_mul(subtract)?;
                terms.insert(key.clone(), a.checked_sub(b)?);
            }
            self.constant = self
                .constant
                .checked_mul(scale)?
                .checked_sub(row.constant.checked_mul(subtract)?)?;
            self.terms = terms;
            self.normalize()?;
        }
        Some(self)
    }
}

fn index_key_order(a: &(bool, String), b: &(bool, String)) -> std::cmp::Ordering {
    // Eliminate the signature's names before fresh case witnesses. This exposes
    // a positive case length such as k + 1 when checking nested coverage.
    (a.1.starts_with('@'), a).cmp(&(b.1.starts_with('@'), b))
}

fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// Rebuilds a length from its terms and number, in a stable order.
fn sum_index(terms: &BTreeMap<(bool, String), u64>, number: u64) -> Index {
    let mut parts = vec![];
    for ((rigid, name), count) in terms {
        if *count > 0 {
            let index = if *rigid {
                Index::Rigid(name.clone())
            } else {
                Index::Variable(name.clone())
            };
            parts.push(if *count == 1 {
                index
            } else {
                Index::Scale(*count, Box::new(index))
            });
        }
    }
    if number > 0 || parts.is_empty() {
        parts.push(Index::Number(number));
    }
    let mut parts = parts.into_iter();
    let first = parts.next().unwrap();
    parts.fold(first, |sum, part| Index::Add(Box::new(sum), Box::new(part)))
}

/// Whether a case's length can never equal the matched length: one side is a
/// plain number smaller than the other side's fixed part, since lengths are Nats.
pub fn impossible_lengths(left: &Index, right: &Index) -> bool {
    let (Some((a, x)), Some((b, y))) = (index_sum(left), index_sum(right)) else {
        return false;
    };
    let a_vars = a.values().any(|n| *n > 0);
    let b_vars = b.values().any(|n| *n > 0);
    (!a_vars && !b_vars && x != y) || (!a_vars && y > x) || (!b_vars && x > y)
}

/// Exact additive identities need no external solver or fresh-variable choices.
type IndexSum = (BTreeMap<(bool, String), u64>, u64);
fn index_sum(index: &Index) -> Option<IndexSum> {
    match index {
        Index::Number(n) => Some((BTreeMap::new(), *n)),
        Index::Variable(n) => Some((BTreeMap::from([((false, n.clone()), 1)]), 0)),
        Index::Rigid(n) => Some((BTreeMap::from([((true, n.clone()), 1)]), 0)),
        Index::Add(a, b) => {
            let (mut terms, n) = index_sum(a)?;
            let (other, m) = index_sum(b)?;
            for (key, count) in other {
                let previous = terms.entry(key).or_default();
                *previous = previous.checked_add(count)?;
            }
            Some((terms, n.checked_add(m)?))
        }
        Index::Scale(factor, index) => {
            if *factor == 0 {
                return Some((BTreeMap::new(), 0));
            }
            let (mut terms, number) = index_sum(index)?;
            for count in terms.values_mut() {
                *count = count.checked_mul(*factor)?;
            }
            Some((terms, number.checked_mul(*factor)?))
        }
    }
}

fn linear_index(index: &Index) -> Option<(Option<Index>, u64)> {
    match index {
        Index::Number(number) => Some((None, *number)),
        Index::Variable(_) | Index::Rigid(_) => Some((Some(index.clone()), 0)),
        Index::Add(a, b) => {
            let (a_base, a_number) = linear_index(a)?;
            let (b_base, b_number) = linear_index(b)?;
            if a_base.is_some() && b_base.is_some() {
                return None;
            }
            Some((a_base.or(b_base), a_number.checked_add(b_number)?))
        }
        Index::Scale(_, _) => None,
    }
}

fn index_from_parts(base: Option<Index>, number: u64) -> Index {
    match base {
        None => Index::Number(number),
        Some(base) if number == 0 => base,
        Some(base) => Index::Add(Box::new(base), Box::new(Index::Number(number))),
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Named(name, args) if name == "Tensor" && tensor_shape(self).is_some() => {
                let shape = tensor_shape(self).unwrap();
                write!(
                    f,
                    "Tensor[{}, ({}{})]",
                    args[0],
                    shape
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", "),
                    if shape.len() == 1 { "," } else { "" }
                )
            }
            Self::Named(name, args) | Self::Constructor(name, args) => {
                write!(f, "{}", name.rsplit('.').next().unwrap_or(name))?;
                if !args.is_empty() {
                    write!(f, "[{}]", joined(args, ", "))?;
                }
                Ok(())
            }
            Self::Parameter(name) | Self::Rigid(name) => write!(f, "{name}"),
            Self::Variable(id) => write!(f, "?{id}"),
            Self::Namespace(name) => write!(f, "module {name}"),
            Self::Tuple(parts) => write!(f, "({})", joined(parts, ", ")),
            Self::Union(parts) => write!(f, "{}", joined(parts, " or ")),
            Self::Borrow(inner) => write!(f, "&{inner}"),
            Self::Dynamic(inner) => write!(f, "dyn {inner}"),
            Self::Lazy(inner, _) => write!(f, "lazy sequence of {inner}"),
            Self::Cursor(inner, _) => write!(f, "collection cursor of {inner}"),
            Self::Guard(inner) => write!(f, "lock on Mutex[{inner}]"),
            Self::StoredView(inner) => write!(f, "stored view of {inner}"),
            Self::Associated(base, bindings) => write!(
                f,
                "{base}({})",
                bindings
                    .iter()
                    .map(|(name, ty)| format!("{name} = {ty}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Indexed(inner, index) => write!(f, "{inner}({index})"),
            Self::Function(fun) => {
                if fun.receiver_mutates {
                    write!(f, "mut (")?;
                }
                write!(
                    f,
                    "({}) -> {}{}",
                    fun.parameters
                        .iter()
                        .map(|parameter| format!(
                            "{}{}{}",
                            match parameter.passing {
                                Passing::Borrow => "",
                                Passing::Mut => "mut ",
                                Passing::Move => "move ",
                            },
                            parameter
                                .name
                                .as_ref()
                                .map(|name| format!("{name} : "))
                                .unwrap_or_default(),
                            parameter.ty
                        ))
                        .collect::<Vec<_>>()
                        .join(", "),
                    fun.result,
                    if fun.effects { " with Effects" } else { "" }
                )?;
                if fun.receiver_mutates {
                    write!(f, ")")?;
                }
                Ok(())
            }
            Self::Overloaded(_) => write!(f, "overloaded function"),
            Self::Never => write!(f, "a value that does not return"),
            Self::Invalid => write!(f, "an earlier error"),
        }
    }
}

impl fmt::Display for Index {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Number(n) => write!(f, "{n}"),
            Self::Variable(n) | Self::Rigid(n) => {
                write!(f, "{}", n.rsplit(':').next().unwrap_or(n))
            }
            Self::Add(a, b) => write!(f, "{a} + {b}"),
            Self::Scale(n, index) => match index.as_ref() {
                Self::Add(_, _) => write!(f, "{n} * ({index})"),
                _ => write!(f, "{n} * {index}"),
            },
        }
    }
}

fn joined(parts: &[Type], separator: &str) -> String {
    parts
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(separator)
}
