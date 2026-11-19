//! Declaration identities and the call decisions made by the type checker.

use crate::{
    source::Span,
    types::{FunctionType, Type},
};
use std::collections::BTreeMap;

/// Stable within a compilation, including declarations imported from another file.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DeclarationId {
    pub file: String,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeclarationKind {
    Function,
    Method,
    StaticMethod,
    TraitMethod,
    Constructor,
    Variant,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Callable {
    Declaration {
        id: DeclarationId,
        name: String,
        owner: Option<String>,
        kind: DeclarationKind,
    },
    Builtin(String),
    Closure(DeclarationId),
    /// A function parameter or another callable whose code is selected at run time.
    Indirect,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Argument {
    Supplied { expression: Span, hole: bool },
    Default,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reference {
    Constant(DeclarationId),
    ScriptBinding(DeclarationId),
    Builtin(String),
}

/// Parameter order is separate from source evaluation order in named calls.
#[derive(Clone, Debug)]
pub struct ResolvedCall {
    pub target: Callable,
    pub callee: Option<Span>,
    pub overload: usize,
    pub signature: FunctionType,
    pub type_arguments: BTreeMap<String, Type>,
    pub arguments: Vec<Argument>,
    pub evaluation_order: Vec<Span>,
    pub receiver: Option<Span>,
    pub partial: bool,
}

/// A merged or stored function value has a type, but not a fixed code address.
pub(crate) fn erase_callable_identities(ty: &Type) -> Type {
    ty.map(&mut |part| {
        if let Type::Function(function) = part {
            let mut function = function.map(&mut |nested| {
                matches!(nested, Type::Function(_)).then(|| erase_callable_identities(nested))
            });
            function.callable = None;
            function.receiver = None;
            Some(Type::Function(function))
        } else if let Type::Overloaded(functions) = part {
            Some(Type::Overloaded(
                functions
                    .iter()
                    .map(|function| {
                        match erase_callable_identities(&Type::Function(function.clone())) {
                            Type::Function(function) => function,
                            _ => unreachable!(),
                        }
                    })
                    .collect(),
            ))
        } else {
            None
        }
    })
}
