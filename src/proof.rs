//! Stage 7 kernel: checks a theorem's proof against its statement, then the
//! compiler erases it. Proofs use explicit terms, induction, simp or optional smt.
//!
//! Terms are first-order: variables, numerals, enum cases, calls to pure
//! functions, arithmetic, `if` and `match`. Two terms are equal by definition
//! when they normalize to the same term. Normalization unfolds a call only when
//! the unfolded body makes progress; a call stuck on a variable stays folded, so
//! `add(n, Zero)` remains `add(n, Zero)` until a proof splits `n` into cases.
use crate::{
    Parsed,
    checker::Checked,
    diagnostic::Diagnostic,
    resolution::{Callable, DeclarationId, DeclarationKind},
    source::Span,
    syntax::{Node, SyntaxKind as K},
    types::Type,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};
#[path = "proof_bounds.rs"]
mod bounds;
#[path = "proof_class_bindings.rs"]
mod class_bindings;
#[path = "proof_defined.rs"]
mod definedness;
#[path = "proof_types.rs"]
pub(crate) mod generic;
#[path = "proof_linear.rs"]
mod linear;
#[path = "proof_runtime_evidence.rs"]
mod runtime_evidence;
#[path = "proof_sequences.rs"]
mod sequences;
pub(crate) use runtime_evidence::{check_runtime_evidence, runtime_theorem};
#[path = "proof_project.rs"]
mod project_dispatch;
pub(crate) use project_dispatch::check_project_dispatches;
#[path = "proof_requirements.rs"]
mod requirements;
#[path = "proof_simp.rs"]
mod simplification;
#[path = "proof_smt.rs"]
mod smt;
#[path = "proof_tactic.rs"]
mod tactics;
use generic::{Holes, compatible, instantiate, settle, unify};
pub use tactics::{GoalReport, ProofGoal, goals_at};
use tactics::{pretty_prop, unify_prop};

/// Reduction steps allowed for one comparison, so a huge numeral cannot hang the compiler.
const FUEL: usize = 200_000;

/// A numeral on the left of `+` or `*` unfolds to its `Succ` chain. Above this
/// many steps the operation stays stuck rather than expanding without bound.
const NUMERAL_EXPANSION_LIMIT: u64 = 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Term {
    Var(String),
    /// A Nat numeral; also the normal form of any closed Nat.
    Nat(u64),
    Int(i64),
    Bool(bool),
    Ctor {
        ty: String,
        case: String,
        args: Vec<Term>,
    },
    /// A call to a visible function; the identity includes its source file.
    Call {
        function: DeclarationId,
        name: String,
        args: Vec<Term>,
        /// The static receiver type for a method call. Keeping it in the term
        /// lets imported theorems recheck virtual dispatch against subclasses
        /// declared by the importing program.
        dispatch: Option<Box<Type>>,
    },
    /// A call the kernel cannot unfold, such as an imported or built-in function.
    Opaque {
        name: String,
        args: Vec<Term>,
    },
    Op(String, Vec<Term>),
    If(Box<Term>, Box<Term>, Box<Term>),
    Match(Box<Term>, Vec<(Pat, Term)>),
    /// Application of a function value to arguments, `f(a, b)`.
    App(Box<Term>, Vec<Term>),
    /// A lambda value, `(x : T) => body`.
    Lam(Vec<(String, Type)>, Box<Term>),
    /// A proposition used as a value, so a `Prop`-typed parameter can be
    /// instantiated at a compound proposition.
    Prop(Box<Prop>),
    /// `⋃₀ F`, where `F` is a family and `element` is the universe each member
    /// shares. The element type travels with the term because membership
    /// unfolds to a quantifier over it.
    FamilyUnion {
        set: Box<Term>,
        element: Type,
    },
    /// `⋂₀ F`, after the same reading.
    FamilyInter {
        set: Box<Term>,
        element: Type,
    },
    /// `{name : element | body}`, a set written with a predicate.
    Builder {
        name: String,
        element: Type,
        body: Box<Prop>,
    },
    /// `{a, b}`, `{a}` or `∅` (`Finite(vec![])`).
    Finite(Vec<Term>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Pat {
    Any,
    Bind(String),
    Ctor(String, Vec<Pat>),
    Lit(Term),
    /// `p | q`: the value matches when any alternative does.
    Alt(Vec<Pat>),
    /// An integer range: `low <= value < high`, or `<= high` when closed.
    Range {
        low: i64,
        high: i64,
        inclusive: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Prop {
    Eq(Term, Term),
    /// A Bool term that must be true, such as `n > 0`.
    Holds(Term),
    /// `x ∈ A` for an abstract set `A`.
    Mem(Term, Term),
    /// `A ⊆ B`, kept as written so the proof state shows the surface form.
    Subset {
        left: Term,
        right: Term,
        element: Type,
    },
    And(Box<Prop>, Box<Prop>),
    Or(Box<Prop>, Box<Prop>),
    Not(Box<Prop>),
    /// `P -> Q`, written in a statement with the ordinary arrow.
    Implies(Box<Prop>, Box<Prop>),
    /// `P ↔ Q`, if and only if.
    Iff(Box<Prop>, Box<Prop>),
    /// `forall n : T, P`.
    Forall {
        name: String,
        ty: Type,
        body: Box<Prop>,
    },
    /// `exists n : T, P`.
    Exists {
        name: String,
        ty: Type,
        body: Box<Prop>,
    },
    True,
    False,
    /// An abstract proposition supplied as a `Prop` parameter. The term is the
    /// parameter itself, usually a variable; substitution renames it like any
    /// other argument. The kernel treats it as an opaque atom: no tactic can
    /// break it apart.
    Atom(Term),
}

fn shown(name: &str) -> &str {
    name.split('#').next().unwrap_or(name)
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

fn list(items: &[Term]) -> String {
    items
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

/// The universe of a `Set[U]`, if this is one.
fn set_element_type(ty: &Type) -> Option<Type> {
    match ty {
        Type::Named(id, args) if id == "Set" && args.len() == 1 => Some(args[0].clone()),
        _ => None,
    }
}

/// A readable binder for the witness of a family membership. The name avoids
/// the free variables of the set and the element so substitution cannot capture
/// them; `cases` can then introduce it directly.
fn family_binder(value: &Term, set: &Term) -> String {
    let mut free = BTreeSet::new();
    value.free(&mut free);
    set.free(&mut free);
    let mut binder = "s".to_owned();
    let mut suffix = 0;
    while free.contains(&binder) {
        suffix += 1;
        binder = format!("s{suffix}");
    }
    binder
}

/// `x ∈ A ∪ B` is definitionally `x ∈ A or x ∈ B`, and similarly for
/// intersection and complement. Expanding here lets the ordinary logical
/// tactics work without a separate set-membership rule for every tactic.
/// One-step unfolding for tactics: only the outer set operation changes.
fn set_membership_head(value: Term, set: Term) -> Prop {
    match set {
        Term::Op(op, args) if op == "∪" && args.len() == 2 => {
            let mut args = args.into_iter();
            let left = args.next().unwrap();
            let right = args.next().unwrap();
            Prop::Or(
                Box::new(Prop::Mem(value.clone(), left)),
                Box::new(Prop::Mem(value, right)),
            )
        }
        Term::Op(op, args) if op == "∩" && args.len() == 2 => {
            let mut args = args.into_iter();
            let left = args.next().unwrap();
            let right = args.next().unwrap();
            Prop::And(
                Box::new(Prop::Mem(value.clone(), left)),
                Box::new(Prop::Mem(value, right)),
            )
        }
        Term::Op(op, args) if op == "ᶜ" && args.len() == 1 => {
            let inner = args.into_iter().next().unwrap();
            Prop::Not(Box::new(Prop::Mem(value, inner)))
        }
        Term::FamilyUnion { set, element } => {
            let binder = family_binder(&value, &set);
            Prop::Exists {
                name: binder.clone(),
                ty: Type::applied("Set", vec![element]),
                body: Box::new(Prop::And(
                    Box::new(Prop::Mem(Term::Var(binder.clone()), set.as_ref().clone())),
                    Box::new(Prop::Mem(value, Term::Var(binder))),
                )),
            }
        }
        Term::FamilyInter { set, element } => {
            let binder = family_binder(&value, &set);
            Prop::Forall {
                name: binder.clone(),
                ty: Type::applied("Set", vec![element]),
                body: Box::new(Prop::Implies(
                    Box::new(Prop::Mem(Term::Var(binder.clone()), set.as_ref().clone())),
                    Box::new(Prop::Mem(value, Term::Var(binder))),
                )),
            }
        }
        Term::Builder {
            name,
            body,
            element: _,
        } => {
            let map = BTreeMap::from([(name.clone(), value)]);
            body.subst_prop(&map, &mut 0)
        }
        Term::Finite(elements) => finite_membership(value, &elements),
        other => Prop::Mem(value, other),
    }
}

/// `x ∈ {a, b}` is `x = a or x = b`; a singleton is one equation and the empty
/// set is false.
fn finite_membership(value: Term, elements: &[Term]) -> Prop {
    match elements {
        [] => Prop::False,
        [only] => Prop::Eq(value, only.clone()),
        [first, rest @ ..] => {
            rest.iter()
                .fold(Prop::Eq(value.clone(), first.clone()), |left, element| {
                    Prop::Or(
                        Box::new(left),
                        Box::new(Prop::Eq(value.clone(), element.clone())),
                    )
                })
        }
    }
}

fn set_membership(value: Term, set: Term) -> Prop {
    match set {
        Term::Op(op, args) if op == "∪" && args.len() == 2 => {
            let mut args = args.into_iter();
            let left = args.next().unwrap();
            let right = args.next().unwrap();
            Prop::Or(
                Box::new(set_membership(value.clone(), left)),
                Box::new(set_membership(value, right)),
            )
        }
        Term::Op(op, args) if op == "∩" && args.len() == 2 => {
            let mut args = args.into_iter();
            let left = args.next().unwrap();
            let right = args.next().unwrap();
            Prop::And(
                Box::new(set_membership(value.clone(), left)),
                Box::new(set_membership(value, right)),
            )
        }
        Term::Op(op, args) if op == "ᶜ" && args.len() == 1 => {
            let inner = args.into_iter().next().unwrap();
            Prop::Not(Box::new(set_membership(value, inner)))
        }
        Term::FamilyUnion { set, element } => {
            let binder = family_binder(&value, &set);
            let member = set_membership(Term::Var(binder.clone()), set.as_ref().clone());
            let inside = set_membership(value, Term::Var(binder.clone()));
            Prop::Exists {
                name: binder,
                ty: Type::applied("Set", vec![element]),
                body: Box::new(Prop::And(Box::new(member), Box::new(inside))),
            }
        }
        Term::FamilyInter { set, element } => {
            let binder = family_binder(&value, &set);
            let member = set_membership(Term::Var(binder.clone()), set.as_ref().clone());
            let inside = set_membership(value, Term::Var(binder.clone()));
            Prop::Forall {
                name: binder,
                ty: Type::applied("Set", vec![element]),
                body: Box::new(Prop::Implies(Box::new(member), Box::new(inside))),
            }
        }
        Term::Builder {
            name,
            body,
            element: _,
        } => {
            let map = BTreeMap::from([(name.clone(), value)]);
            expand_prop(&body.subst_prop(&map, &mut 0))
        }
        Term::Finite(elements) => finite_membership(value, &elements),
        other => Prop::Mem(value, other),
    }
}

/// Rebuild a proposition after changing a set term, keeping membership of a
/// compound set in its logical form.
fn expand_prop(prop: &Prop) -> Prop {
    match prop {
        Prop::Mem(value, set) => set_membership(value.clone(), set.clone()),
        Prop::Subset {
            left,
            right,
            element,
        } => {
            let binder = "@x".to_owned();
            let value = Term::Var(binder.clone());
            let body = Prop::Implies(
                Box::new(Prop::Mem(value.clone(), left.clone())),
                Box::new(Prop::Mem(value, right.clone())),
            );
            Prop::Forall {
                name: binder,
                ty: element.clone(),
                body: Box::new(expand_prop(&body)),
            }
        }
        Prop::Eq(a, b) => Prop::Eq(a.clone(), b.clone()),
        Prop::Holds(t) => Prop::Holds(t.clone()),
        Prop::And(a, b) => Prop::And(Box::new(expand_prop(a)), Box::new(expand_prop(b))),
        Prop::Or(a, b) => Prop::Or(Box::new(expand_prop(a)), Box::new(expand_prop(b))),
        Prop::Not(p) => Prop::Not(Box::new(expand_prop(p))),
        Prop::Implies(a, b) => Prop::Implies(Box::new(expand_prop(a)), Box::new(expand_prop(b))),
        Prop::Iff(a, b) => Prop::Iff(Box::new(expand_prop(a)), Box::new(expand_prop(b))),
        Prop::Forall { name, ty, body } => Prop::Forall {
            name: name.clone(),
            ty: ty.clone(),
            body: Box::new(expand_prop(body)),
        },
        Prop::Exists { name, ty, body } => Prop::Exists {
            name: name.clone(),
            ty: ty.clone(),
            body: Box::new(expand_prop(body)),
        },
        Prop::True => Prop::True,
        Prop::False => Prop::False,
        Prop::Atom(Term::Prop(inner)) => expand_prop(inner),
        Prop::Atom(term) => Prop::Atom(term.clone()),
    }
}

impl fmt::Display for Term {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Term::Var(name) => write!(f, "{}", shown(name)),
            Term::Nat(n) => write!(f, "{n}"),
            Term::Int(n) => write!(f, "{n}"),
            Term::Bool(b) => write!(f, "{b}"),
            Term::Ctor { ty, case, args } if ty == "List" => {
                if case == sequences::NIL {
                    write!(f, "[]")
                } else {
                    write!(f, "[{}, ..{}]", args[0], args[1])
                }
            }
            Term::Ctor { case, args, .. } if args.is_empty() => write!(f, "{case}"),
            Term::Ctor { case, args, .. } => write!(f, "{case}({})", list(args)),
            Term::Call { name, args, .. } | Term::Opaque { name, args } => {
                write!(f, "{name}({})", list(args))
            }
            Term::Op(op, args) if op.starts_with("$field:") && args.len() == 1 => {
                let (_, field) = field_marker_parts(op).unwrap_or((vec![], "?"));
                write!(f, "{}.{}", args[0], field)
            }
            Term::Op(op, args) if args.len() == 1 && op == "ᶜ" => write!(f, "{}ᶜ", args[0]),
            Term::Op(op, args) if op == "$list.concat" => write!(f, "({} + {})", args[0], args[1]),
            Term::Op(op, args) if op.starts_with("$list.") || op.starts_with("$option.") => {
                let method = op.split('.').nth(1).unwrap_or(op);
                write!(f, "{}.{method}({})", args[0], list(&args[1..]))
            }
            Term::Op(op, args) if args.len() == 1 => write!(f, "{op}{}", args[0]),
            Term::Op(op, args) => write!(f, "({} {op} {})", args[0], args[1]),
            Term::If(c, a, b) => write!(f, "if {c} then {a} else {b}"),
            Term::Match(s, _) => write!(f, "match {s} {{ ... }}"),
            Term::App(function, args) => write!(f, "{function}({})", list(args)),
            Term::Lam(params, body) => {
                let names = params
                    .iter()
                    .map(|(name, ty)| format!("{name} : {ty}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "fun {names} => {body}")
            }
            Term::Prop(prop) => write!(f, "({prop})"),
            Term::FamilyUnion { set, .. } => write!(f, "⋃₀ {set}"),
            Term::FamilyInter { set, .. } => write!(f, "⋂₀ {set}"),
            Term::Builder {
                name,
                element,
                body,
            } => {
                write!(f, "{{{} : {element} | {body}}}", shown(name))
            }
            Term::Finite(elements) => match elements.as_slice() {
                [] => write!(f, "∅"),
                [only] => write!(f, "{{{only}}}"),
                _ => write!(f, "{{{}}}", list(elements)),
            },
        }
    }
}

impl fmt::Display for Prop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Prop::Eq(a, b) => write!(f, "{a} = {b}"),
            Prop::Holds(t) => write!(f, "{t}"),
            Prop::Mem(a, b) => write!(f, "{a} ∈ {b}"),
            Prop::Subset { left, right, .. } => write!(f, "{left} ⊆ {right}"),
            Prop::And(a, b) => write!(f, "({a} and {b})"),
            Prop::Or(a, b) => write!(f, "({a} or {b})"),
            Prop::Not(inner) => match inner.as_ref() {
                Prop::Mem(a, b) => write!(f, "{a} ∉ {b}"),
                p => write!(f, "not {p}"),
            },
            Prop::Implies(a, b) => write!(f, "({a} -> {b})"),
            Prop::Iff(a, b) => write!(f, "({a} ↔ {b})"),
            Prop::Forall { name, ty, body } => write!(f, "forall {} : {ty}, {body}", shown(name)),
            Prop::Exists { name, ty, body } => write!(f, "exists {} : {ty}, {body}", shown(name)),
            Prop::True => write!(f, "true"),
            Prop::False => write!(f, "a contradiction"),
            Prop::Atom(term) => write!(f, "{term}"),
        }
    }
}

impl Term {
    fn free(&self, out: &mut BTreeSet<String>) {
        match self {
            Term::Var(name) => {
                out.insert(name.clone());
            }
            Term::Nat(_) | Term::Int(_) | Term::Bool(_) => {}
            Term::Ctor { args, .. }
            | Term::Call { args, .. }
            | Term::Opaque { args, .. }
            | Term::Op(_, args) => args.iter().for_each(|a| a.free(out)),
            Term::Prop(prop) => {
                prop.map(&mut |term| {
                    term.free(out);
                    term.clone()
                });
            }
            Term::App(function, args) => {
                function.free(out);
                args.iter().for_each(|a| a.free(out));
            }
            Term::Lam(params, body) => {
                let mut inner = BTreeSet::new();
                body.free(&mut inner);
                for (name, _) in params {
                    inner.remove(name);
                }
                out.extend(inner);
            }
            Term::If(c, a, b) => {
                c.free(out);
                a.free(out);
                b.free(out);
            }
            Term::Match(s, arms) => {
                s.free(out);
                for (pat, body) in arms {
                    let mut inner = BTreeSet::new();
                    body.free(&mut inner);
                    let mut bound = BTreeSet::new();
                    pat.binders(&mut bound);
                    out.extend(inner.difference(&bound).cloned());
                }
            }
            Term::FamilyUnion { set, .. } | Term::FamilyInter { set, .. } => set.free(out),
            Term::Builder { name, body, .. } => {
                let mut inner = BTreeSet::new();
                body.free(&mut inner);
                out.extend(inner.into_iter().filter(|free| free != name));
            }
            Term::Finite(elements) => elements.iter().for_each(|element| element.free(out)),
        }
    }

    pub(crate) fn is_closed(&self) -> bool {
        let mut free = BTreeSet::new();
        self.free(&mut free);
        free.is_empty()
    }

    /// Capture-avoiding substitution of variables by terms.
    pub(crate) fn subst(&self, map: &BTreeMap<String, Term>, fresh: &mut usize) -> Term {
        match self {
            Term::Var(name) => map.get(name).cloned().unwrap_or_else(|| self.clone()),
            Term::Nat(_) | Term::Int(_) | Term::Bool(_) => self.clone(),
            Term::Ctor { ty, case, args } => Term::Ctor {
                ty: ty.clone(),
                case: case.clone(),
                args: args.iter().map(|a| a.subst(map, fresh)).collect(),
            },
            Term::Call {
                function,
                name,
                args,
                dispatch,
            } => Term::Call {
                function: function.clone(),
                name: name.clone(),
                args: args.iter().map(|a| a.subst(map, fresh)).collect(),
                dispatch: dispatch.clone(),
            },
            Term::Opaque { name, args } => Term::Opaque {
                name: name.clone(),
                args: args.iter().map(|a| a.subst(map, fresh)).collect(),
            },
            Term::Op(op, args) => Term::Op(
                op.clone(),
                args.iter().map(|a| a.subst(map, fresh)).collect(),
            ),
            Term::App(function, args) => Term::App(
                Box::new(function.subst(map, fresh)),
                args.iter().map(|a| a.subst(map, fresh)).collect(),
            ),
            Term::Lam(params, body) => {
                let mut inner = map.clone();
                let mut renamed = params.clone();
                let mut local = BTreeSet::new();
                for (name, _) in params {
                    inner.remove(name);
                    local.insert(name.clone());
                }
                let captured = inner.values().any(|term| {
                    let mut free = BTreeSet::new();
                    term.free(&mut free);
                    free.iter().any(|name| local.contains(name))
                });
                if captured {
                    let mut renamed_body = body.as_ref().clone();
                    for (name, _) in renamed.iter_mut() {
                        *fresh += 1;
                        let replacement = format!("{name}#{}", *fresh);
                        let swap = BTreeMap::from([(name.clone(), Term::Var(replacement.clone()))]);
                        renamed_body = renamed_body.subst(&swap, fresh);
                        *name = replacement;
                    }
                    return Term::Lam(renamed, Box::new(renamed_body.subst(&inner, fresh)));
                }
                Term::Lam(renamed, Box::new(body.subst(&inner, fresh)))
            }
            Term::Prop(prop) => Term::Prop(Box::new(prop.subst_prop(map, fresh))),
            Term::If(c, a, b) => Term::If(
                Box::new(c.subst(map, fresh)),
                Box::new(a.subst(map, fresh)),
                Box::new(b.subst(map, fresh)),
            ),
            Term::Match(s, arms) => {
                let mut incoming = BTreeSet::new();
                for term in map.values() {
                    term.free(&mut incoming);
                }
                let arms = arms
                    .iter()
                    .map(|(pat, body)| {
                        let mut bound = BTreeSet::new();
                        pat.binders(&mut bound);
                        let mut inner = map.clone();
                        let mut renames = BTreeMap::new();
                        for name in &bound {
                            inner.remove(name);
                            if incoming.contains(name) {
                                *fresh += 1;
                                let new = format!("{}#{}", shown(name), fresh);
                                renames.insert(name.clone(), new.clone());
                                inner.insert(name.clone(), Term::Var(new));
                            }
                        }
                        (pat.rename(&renames), body.subst(&inner, fresh))
                    })
                    .collect();
                Term::Match(Box::new(s.subst(map, fresh)), arms)
            }
            Term::FamilyUnion { set, element } => Term::FamilyUnion {
                set: Box::new(set.subst(map, fresh)),
                element: element.clone(),
            },
            Term::FamilyInter { set, element } => Term::FamilyInter {
                set: Box::new(set.subst(map, fresh)),
                element: element.clone(),
            },
            Term::Builder {
                name,
                element,
                body,
            } => {
                let mut inner = map.clone();
                inner.remove(name);
                let captured = inner.values().any(|term| {
                    let mut free = BTreeSet::new();
                    term.free(&mut free);
                    free.contains(name)
                });
                let (bound, body) = if captured {
                    *fresh += 1;
                    let renamed = format!("{name}#{}", *fresh);
                    (renamed.clone(), body.rename_free(name, &renamed, fresh))
                } else {
                    (name.clone(), body.as_ref().clone())
                };
                Term::Builder {
                    name: bound,
                    element: element.clone(),
                    body: Box::new(body.subst_prop(&inner, fresh)),
                }
            }
            Term::Finite(elements) => {
                Term::Finite(elements.iter().map(|e| e.subst(map, fresh)).collect())
            }
        }
    }
}

fn field_marker(path: &str, indices: &[usize]) -> String {
    format!(
        "$field:{}|{path}",
        indices
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(".")
    )
}

fn field_marker_parts(op: &str) -> Option<(Vec<usize>, &str)> {
    let rest = op.strip_prefix("$field:")?;
    let (indices, path) = rest.split_once('|')?;
    let indices = indices
        .split('.')
        .map(str::parse::<usize>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    Some((indices, path))
}

impl Pat {
    fn binders(&self, out: &mut BTreeSet<String>) {
        match self {
            Pat::Bind(name) => {
                out.insert(name.clone());
            }
            Pat::Ctor(_, pats) => pats.iter().for_each(|p| p.binders(out)),
            Pat::Alt(pats) => pats.iter().for_each(|p| p.binders(out)),
            Pat::Any | Pat::Lit(_) | Pat::Range { .. } => {}
        }
    }
    fn rename(&self, renames: &BTreeMap<String, String>) -> Pat {
        match self {
            Pat::Bind(name) => {
                Pat::Bind(renames.get(name).cloned().unwrap_or_else(|| name.clone()))
            }
            Pat::Ctor(case, pats) => Pat::Ctor(
                case.clone(),
                pats.iter().map(|p| p.rename(renames)).collect(),
            ),
            Pat::Alt(pats) => Pat::Alt(pats.iter().map(|p| p.rename(renames)).collect()),
            other => other.clone(),
        }
    }
}

impl Prop {
    /// Every variable that occurs free, accounting for quantifier and builder
    /// binders.
    pub(crate) fn free(&self, out: &mut BTreeSet<String>) {
        match self {
            Prop::Eq(a, b) => {
                a.free(out);
                b.free(out);
            }
            Prop::Holds(t) | Prop::Atom(t) => t.free(out),
            Prop::Mem(a, b) => {
                a.free(out);
                b.free(out);
            }
            Prop::Subset { left, right, .. } => {
                left.free(out);
                right.free(out);
            }
            Prop::And(a, b) | Prop::Or(a, b) | Prop::Implies(a, b) | Prop::Iff(a, b) => {
                a.free(out);
                b.free(out);
            }
            Prop::Not(p) => p.free(out),
            Prop::Forall { name, body, .. } | Prop::Exists { name, body, .. } => {
                let mut inner = BTreeSet::new();
                body.free(&mut inner);
                out.extend(inner.into_iter().filter(|free| free != name));
            }
            Prop::True | Prop::False => {}
        }
    }

    pub(crate) fn map(&self, f: &mut impl FnMut(&Term) -> Term) -> Prop {
        match self {
            Prop::Eq(a, b) => Prop::Eq(f(a), f(b)),
            Prop::Holds(t) => Prop::Holds(f(t)),
            Prop::Mem(a, b) => Prop::Mem(f(a), f(b)),
            Prop::Subset {
                left,
                right,
                element,
            } => Prop::Subset {
                left: f(left),
                right: f(right),
                element: element.clone(),
            },
            Prop::And(a, b) => Prop::And(Box::new(a.map(f)), Box::new(b.map(f))),
            Prop::Or(a, b) => Prop::Or(Box::new(a.map(f)), Box::new(b.map(f))),
            Prop::Not(p) => Prop::Not(Box::new(p.map(f))),
            Prop::Implies(a, b) => Prop::Implies(Box::new(a.map(f)), Box::new(b.map(f))),
            Prop::Iff(a, b) => Prop::Iff(Box::new(a.map(f)), Box::new(b.map(f))),
            Prop::Forall { name, ty, body } => Prop::Forall {
                name: name.clone(),
                ty: ty.clone(),
                body: Box::new(body.map(f)),
            },
            Prop::Exists { name, ty, body } => Prop::Exists {
                name: name.clone(),
                ty: ty.clone(),
                body: Box::new(body.map(f)),
            },
            Prop::True => Prop::True,
            Prop::False => Prop::False,
            Prop::Atom(term) => Prop::Atom(f(term)),
        }
    }
    /// Renames the free occurrences of a quantifier binder, stopping at a
    /// nested binder of the same name.
    fn rename_free(&self, old: &str, new: &str, fresh: &mut usize) -> Prop {
        let swap = BTreeMap::from([(old.to_owned(), Term::Var(new.to_owned()))]);
        match self {
            Prop::Eq(a, b) => Prop::Eq(a.subst(&swap, fresh), b.subst(&swap, fresh)),
            Prop::Holds(t) => Prop::Holds(t.subst(&swap, fresh)),
            Prop::Mem(a, b) => Prop::Mem(a.subst(&swap, fresh), b.subst(&swap, fresh)),
            Prop::Subset {
                left,
                right,
                element,
            } => Prop::Subset {
                left: left.subst(&swap, fresh),
                right: right.subst(&swap, fresh),
                element: element.clone(),
            },
            Prop::And(a, b) => Prop::And(
                Box::new(a.rename_free(old, new, fresh)),
                Box::new(b.rename_free(old, new, fresh)),
            ),
            Prop::Or(a, b) => Prop::Or(
                Box::new(a.rename_free(old, new, fresh)),
                Box::new(b.rename_free(old, new, fresh)),
            ),
            Prop::Not(p) => Prop::Not(Box::new(p.rename_free(old, new, fresh))),
            Prop::Implies(a, b) => Prop::Implies(
                Box::new(a.rename_free(old, new, fresh)),
                Box::new(b.rename_free(old, new, fresh)),
            ),
            Prop::Iff(a, b) => Prop::Iff(
                Box::new(a.rename_free(old, new, fresh)),
                Box::new(b.rename_free(old, new, fresh)),
            ),
            Prop::Forall { name, ty, body } if name != old => Prop::Forall {
                name: name.clone(),
                ty: ty.clone(),
                body: Box::new(body.rename_free(old, new, fresh)),
            },
            Prop::Exists { name, ty, body } if name != old => Prop::Exists {
                name: name.clone(),
                ty: ty.clone(),
                body: Box::new(body.rename_free(old, new, fresh)),
            },
            Prop::Forall { .. } | Prop::Exists { .. } => self.clone(),
            Prop::True => Prop::True,
            Prop::False => Prop::False,
            Prop::Atom(term) => Prop::Atom(term.subst(&swap, fresh)),
        }
    }

    /// A canonical copy where each quantifier binder is named by its depth.
    /// Two alpha-equivalent propositions become identical.
    fn alpha_normalize(&self) -> Prop {
        self.alpha_at(0)
    }

    fn alpha_at(&self, depth: usize) -> Prop {
        match self {
            Prop::Forall { name, ty, body } | Prop::Exists { name, ty, body } => {
                let renamed = format!("@{depth}");
                let body = body.rename_free(name, &renamed, &mut 0).alpha_at(depth + 1);
                if matches!(self, Prop::Forall { .. }) {
                    Prop::Forall {
                        name: renamed,
                        ty: ty.clone(),
                        body: Box::new(body),
                    }
                } else {
                    Prop::Exists {
                        name: renamed,
                        ty: ty.clone(),
                        body: Box::new(body),
                    }
                }
            }
            Prop::Eq(a, b) => Prop::Eq(alpha_term_at(a, depth), alpha_term_at(b, depth)),
            Prop::Holds(t) => Prop::Holds(alpha_term_at(t, depth)),
            Prop::Mem(a, b) => Prop::Mem(alpha_term_at(a, depth), alpha_term_at(b, depth)),
            Prop::Subset {
                left,
                right,
                element,
            } => Prop::Subset {
                left: alpha_term_at(left, depth),
                right: alpha_term_at(right, depth),
                element: element.clone(),
            },
            Prop::And(a, b) => Prop::And(Box::new(a.alpha_at(depth)), Box::new(b.alpha_at(depth))),
            Prop::Or(a, b) => Prop::Or(Box::new(a.alpha_at(depth)), Box::new(b.alpha_at(depth))),
            Prop::Not(p) => Prop::Not(Box::new(p.alpha_at(depth))),
            Prop::Implies(a, b) => {
                Prop::Implies(Box::new(a.alpha_at(depth)), Box::new(b.alpha_at(depth)))
            }
            Prop::Iff(a, b) => Prop::Iff(Box::new(a.alpha_at(depth)), Box::new(b.alpha_at(depth))),
            Prop::True => Prop::True,
            Prop::False => Prop::False,
            Prop::Atom(term) => Prop::Atom(alpha_term_at(term, depth)),
        }
    }

    /// Replaces theorem type variables after citation unifies them with the
    /// argument types. Terms do not carry types, but quantifier binders do.
    fn specialize_types(prop: &Prop, variables: &Holes, holes: &Holes) -> Prop {
        match prop {
            Prop::Forall { name, ty, body } => Prop::Forall {
                name: name.clone(),
                ty: settle(&instantiate(ty, variables), holes),
                body: Box::new(Self::specialize_types(body, variables, holes)),
            },
            Prop::Exists { name, ty, body } => Prop::Exists {
                name: name.clone(),
                ty: settle(&instantiate(ty, variables), holes),
                body: Box::new(Self::specialize_types(body, variables, holes)),
            },
            Prop::Eq(a, b) => Prop::Eq(
                specialize_term_types(a, variables, holes),
                specialize_term_types(b, variables, holes),
            ),
            Prop::Holds(t) => Prop::Holds(specialize_term_types(t, variables, holes)),
            Prop::Mem(a, b) => Prop::Mem(
                specialize_term_types(a, variables, holes),
                specialize_term_types(b, variables, holes),
            ),
            Prop::Subset {
                left,
                right,
                element,
            } => Prop::Subset {
                left: specialize_term_types(left, variables, holes),
                right: specialize_term_types(right, variables, holes),
                element: settle(&instantiate(element, variables), holes),
            },
            Prop::And(a, b) => Prop::And(
                Box::new(Self::specialize_types(a, variables, holes)),
                Box::new(Self::specialize_types(b, variables, holes)),
            ),
            Prop::Or(a, b) => Prop::Or(
                Box::new(Self::specialize_types(a, variables, holes)),
                Box::new(Self::specialize_types(b, variables, holes)),
            ),
            Prop::Not(p) => Prop::Not(Box::new(Self::specialize_types(p, variables, holes))),
            Prop::Implies(a, b) => Prop::Implies(
                Box::new(Self::specialize_types(a, variables, holes)),
                Box::new(Self::specialize_types(b, variables, holes)),
            ),
            Prop::Iff(a, b) => Prop::Iff(
                Box::new(Self::specialize_types(a, variables, holes)),
                Box::new(Self::specialize_types(b, variables, holes)),
            ),
            Prop::True => Prop::True,
            Prop::False => Prop::False,
            Prop::Atom(term) => Prop::Atom(specialize_term_types(term, variables, holes)),
        }
    }
}

/// Replaces theorem type variables in the types that term forms carry. Only the
/// proof-set forms store a `Type`; the rest recurse so a nested builder is
/// found too.
fn specialize_term_types(term: &Term, variables: &Holes, holes: &Holes) -> Term {
    let specialize = |t: &Term| specialize_term_types(t, variables, holes);
    match term {
        Term::Var(_) | Term::Nat(_) | Term::Int(_) | Term::Bool(_) => term.clone(),
        Term::Ctor { ty, case, args } => Term::Ctor {
            ty: ty.clone(),
            case: case.clone(),
            args: args.iter().map(specialize).collect(),
        },
        Term::Call {
            function,
            name,
            args,
            dispatch,
        } => Term::Call {
            function: function.clone(),
            name: name.clone(),
            args: args.iter().map(specialize).collect(),
            dispatch: dispatch.clone(),
        },
        Term::Opaque { name, args } => Term::Opaque {
            name: name.clone(),
            args: args.iter().map(specialize).collect(),
        },
        Term::Op(op, args) => Term::Op(op.clone(), args.iter().map(specialize).collect()),
        Term::If(c, a, b) => Term::If(
            Box::new(specialize(c)),
            Box::new(specialize(a)),
            Box::new(specialize(b)),
        ),
        Term::Match(value, arms) => Term::Match(
            Box::new(specialize(value)),
            arms.iter()
                .map(|(pat, body)| (pat.clone(), specialize(body)))
                .collect(),
        ),
        Term::App(function, args) => Term::App(
            Box::new(specialize(function)),
            args.iter().map(specialize).collect(),
        ),
        Term::Lam(params, body) => Term::Lam(
            params
                .iter()
                .map(|(name, ty)| (name.clone(), settle(&instantiate(ty, variables), holes)))
                .collect(),
            Box::new(specialize(body)),
        ),
        Term::Prop(prop) => Term::Prop(Box::new(Prop::specialize_types(prop, variables, holes))),
        Term::FamilyUnion { set, element } => Term::FamilyUnion {
            set: Box::new(specialize(set)),
            element: settle(&instantiate(element, variables), holes),
        },
        Term::FamilyInter { set, element } => Term::FamilyInter {
            set: Box::new(specialize(set)),
            element: settle(&instantiate(element, variables), holes),
        },
        Term::Builder {
            name,
            element,
            body,
        } => Term::Builder {
            name: name.clone(),
            element: settle(&instantiate(element, variables), holes),
            body: Box::new(Prop::specialize_types(body, variables, holes)),
        },
        Term::Finite(elements) => Term::Finite(elements.iter().map(specialize).collect()),
    }
}

impl Prop {
    /// Capture-avoiding substitution. A binder of the same name shadows the
    /// substitution inside its body.
    fn subst_prop(&self, map: &BTreeMap<String, Term>, fresh: &mut usize) -> Prop {
        match self {
            Prop::Eq(a, b) => Prop::Eq(a.subst(map, fresh), b.subst(map, fresh)),
            Prop::Holds(t) => Prop::Holds(t.subst(map, fresh)),
            Prop::Mem(a, b) => Prop::Mem(a.subst(map, fresh), b.subst(map, fresh)),
            Prop::Subset {
                left,
                right,
                element,
            } => Prop::Subset {
                left: left.subst(map, fresh),
                right: right.subst(map, fresh),
                element: element.clone(),
            },
            Prop::And(a, b) => Prop::And(
                Box::new(a.subst_prop(map, fresh)),
                Box::new(b.subst_prop(map, fresh)),
            ),
            Prop::Or(a, b) => Prop::Or(
                Box::new(a.subst_prop(map, fresh)),
                Box::new(b.subst_prop(map, fresh)),
            ),
            Prop::Not(p) => Prop::Not(Box::new(p.subst_prop(map, fresh))),
            Prop::Implies(a, b) => Prop::Implies(
                Box::new(a.subst_prop(map, fresh)),
                Box::new(b.subst_prop(map, fresh)),
            ),
            Prop::Iff(a, b) => Prop::Iff(
                Box::new(a.subst_prop(map, fresh)),
                Box::new(b.subst_prop(map, fresh)),
            ),
            Prop::Forall { name, ty, body } | Prop::Exists { name, ty, body } => {
                let mut inner = map.clone();
                inner.remove(name);
                // A substitution term that mentions the binder would be
                // captured under it. Rename the binder first.
                let captured = inner.values().any(|term| {
                    let mut free = BTreeSet::new();
                    term.free(&mut free);
                    free.contains(name)
                });
                let (bound, body) = if captured {
                    *fresh += 1;
                    let renamed = format!("{name}#{}", *fresh);
                    (renamed.clone(), body.rename_free(name, &renamed, fresh))
                } else {
                    (name.clone(), body.as_ref().clone())
                };
                let body = Box::new(body.subst_prop(&inner, fresh));
                match self {
                    Prop::Forall { .. } => Prop::Forall {
                        name: bound,
                        ty: ty.clone(),
                        body,
                    },
                    _ => Prop::Exists {
                        name: bound,
                        ty: ty.clone(),
                        body,
                    },
                }
            }
            Prop::True => Prop::True,
            Prop::False => Prop::False,
            Prop::Atom(term) => Prop::Atom(term.subst(map, fresh)),
        }
    }
    fn mentions(&self, name: &str) -> bool {
        let mut found = false;
        self.map(&mut |t| {
            let mut free = BTreeSet::new();
            t.free(&mut free);
            found |= free.contains(name);
            t.clone()
        });
        found
    }
}

/// Alpha-normalizes the binders that terms carry. A builder renames its binder
/// to a depth name and normalizes the predicate at the next depth, so two
/// builders that differ only in the binder name become identical.
fn alpha_term_at(term: &Term, depth: usize) -> Term {
    match term {
        Term::Var(_) | Term::Nat(_) | Term::Int(_) | Term::Bool(_) => term.clone(),
        Term::Ctor { ty, case, args } => Term::Ctor {
            ty: ty.clone(),
            case: case.clone(),
            args: args.iter().map(|a| alpha_term_at(a, depth)).collect(),
        },
        Term::Call {
            function,
            name,
            args,
            dispatch,
        } => Term::Call {
            function: function.clone(),
            name: name.clone(),
            args: args.iter().map(|a| alpha_term_at(a, depth)).collect(),
            dispatch: dispatch.clone(),
        },
        Term::Opaque { name, args } => Term::Opaque {
            name: name.clone(),
            args: args.iter().map(|a| alpha_term_at(a, depth)).collect(),
        },
        Term::Op(op, args) => Term::Op(
            op.clone(),
            args.iter().map(|a| alpha_term_at(a, depth)).collect(),
        ),
        Term::If(c, a, b) => Term::If(
            Box::new(alpha_term_at(c, depth)),
            Box::new(alpha_term_at(a, depth)),
            Box::new(alpha_term_at(b, depth)),
        ),
        Term::Match(value, arms) => Term::Match(
            Box::new(alpha_term_at(value, depth)),
            arms.iter()
                .map(|(pat, body)| (pat.clone(), alpha_term_at(body, depth)))
                .collect(),
        ),
        Term::App(function, args) => Term::App(
            Box::new(alpha_term_at(function, depth)),
            args.iter().map(|a| alpha_term_at(a, depth)).collect(),
        ),
        Term::Lam(params, body) => {
            Term::Lam(params.clone(), Box::new(alpha_term_at(body, depth + 1)))
        }
        Term::Prop(prop) => Term::Prop(Box::new(prop.alpha_at(depth))),
        Term::FamilyUnion { set, element } => Term::FamilyUnion {
            set: Box::new(alpha_term_at(set, depth)),
            element: element.clone(),
        },
        Term::FamilyInter { set, element } => Term::FamilyInter {
            set: Box::new(alpha_term_at(set, depth)),
            element: element.clone(),
        },
        Term::Builder {
            name,
            element,
            body,
        } => {
            let renamed = format!("@{depth}");
            let body = body.rename_free(name, &renamed, &mut 0);
            Term::Builder {
                name: renamed,
                element: element.clone(),
                body: Box::new(body.alpha_at(depth + 1)),
            }
        }
        Term::Finite(elements) => {
            Term::Finite(elements.iter().map(|e| alpha_term_at(e, depth)).collect())
        }
    }
}

/// An unfoldable function: its parameters and its body as a term.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Definition {
    pub params: Vec<String>,
    pub body: Term,
    /// Theorems the function requires, with arguments over its parameters. A call
    /// unfolds only where each theorem's assumptions hold at those arguments.
    pub requires: Vec<(DeclarationId, Vec<Term>)>,
    /// Run-time `requires` conditions over its parameters. A proof must establish
    /// them at each call, which then stands for the success value (Q80).
    pub preconditions: Vec<Prop>,
}

fn small_term(term: &Term) -> bool {
    let mut pending = vec![term];
    let mut budget = 8192;
    while let Some(term) = pending.pop() {
        if budget == 0 {
            return false;
        }
        budget -= 1;
        match term {
            Term::Ctor { args, .. }
            | Term::Call { args, .. }
            | Term::Opaque { args, .. }
            | Term::Op(_, args) => pending.extend(args),
            Term::If(a, b, c) => pending.extend([a.as_ref(), b.as_ref(), c.as_ref()]),
            Term::Match(value, arms) => {
                pending.push(value);
                pending.extend(arms.iter().map(|(_, body)| body));
            }
            Term::App(function, args) => {
                pending.push(function);
                pending.extend(args);
            }
            Term::Lam(_, body) => pending.push(body),
            Term::Prop(_) => {}
            Term::FamilyUnion { set, .. } | Term::FamilyInter { set, .. } => pending.push(set),
            Term::Finite(elements) => pending.extend(elements),
            // A builder body lives in a proposition; count it as one node here.
            // Proof-only terms do not occur in the run-time definitions this
            // bounds, so the estimate only needs to stay finite.
            Term::Builder { .. } => {}
            _ => {}
        }
    }
    true
}

/// Checked, first-order function bodies. Private helper bodies can be unfolded
/// through a public function, but are not added to an importer's visible names.
/// Checked theorems travel the same way; an importer sees only those its imports name.
#[derive(Clone, Debug, Default)]
pub(crate) struct Definitions {
    signatures: BTreeMap<DeclarationId, Signature>,
    constraints: BTreeMap<DeclarationId, Vec<(Type, Type)>>,
    functions: BTreeMap<DeclarationId, Definition>,
    opaque: BTreeMap<DeclarationId, String>,
    theorems: BTreeMap<DeclarationId, Exported>,
}
impl Definitions {
    pub fn extend(&mut self, other: &Self) {
        self.signatures.extend(other.signatures.clone());
        self.constraints.extend(other.constraints.clone());
        self.functions.extend(other.functions.clone());
        self.opaque.extend(other.opaque.clone());
        self.theorems.extend(other.theorems.clone());
    }
    pub(crate) fn hide_cycle_files(&mut self, files: &BTreeSet<String>) {
        for id in self.signatures.keys().filter(|id| files.contains(&id.file)) {
            self.functions.remove(id);
            self.opaque.insert(
                id.clone(),
                "proof unfolding across an import cycle is not supported yet".into(),
            );
        }
    }
}

/// A theorem that checked in its own module, as an importer cites it. An @simp
/// theorem imported by name joins the importer's plain `simp` (Q79).
#[derive(Clone, Debug)]
pub(crate) struct Exported {
    pub theorem: Theorem,
    pub simp: bool,
}

/// Normalizes terms against the module's unfoldable functions.
pub(crate) struct Reducer<'a> {
    pub functions: &'a BTreeMap<DeclarationId, Definition>,
    definitions: &'a BTreeMap<String, crate::checker::Definition>,
    pub fuel: usize,
    pub fresh: usize,
}

/// A value at the head of a term, which a pattern can test.
fn is_value(t: &Term) -> bool {
    matches!(
        t,
        Term::Nat(_)
            | Term::Int(_)
            | Term::Bool(_)
            | Term::Ctor { .. }
            | Term::Lam(..)
            | Term::Prop(..)
    )
}

impl Reducer<'_> {
    fn spend(&mut self) -> bool {
        if self.fuel == 0 {
            return false;
        }
        self.fuel -= 1;
        true
    }

    /// Reduces a structural field projection, expanding only calls needed to
    /// reveal the receiver's constructor. `seq` is kept around the projected
    /// value so constructor and method locals remain part of the checked work.
    fn project(&mut self, value: &Term, path: &[usize]) -> Term {
        if path.is_empty() || !self.spend() {
            return value.clone();
        }
        match value {
            Term::Ctor { args, .. } => {
                let Some(next) = args.get(path[0]) else {
                    return value.clone();
                };
                let mut projected = if path.len() == 1 {
                    next.clone()
                } else {
                    self.project(next, &path[1..])
                };
                // Constructor arguments are evaluated eagerly, including
                // fields that the projection discards. Keep their checked
                // work visible to definedness and SMT encoding.
                for arg in args.iter().rev() {
                    projected = Term::Op("seq".into(), vec![arg.clone(), projected]);
                }
                projected
            }
            Term::Op(op, args) if op == "seq" && args.len() == 2 => Term::Op(
                "seq".into(),
                vec![args[0].clone(), self.project(&args[1], path)],
            ),
            Term::Op(op, args) if op.starts_with("$field:") && args.len() == 1 => {
                let Some((mut nested, field)) = field_marker_parts(op) else {
                    return value.clone();
                };
                nested.extend_from_slice(path);
                Term::Op(field_marker(field, &nested), vec![args[0].clone()])
            }
            Term::Call { function, args, .. } => {
                let Some(definition) = self.functions.get(function) else {
                    return Term::Op(field_marker("?", path), vec![value.clone()]);
                };
                let map = definition
                    .params
                    .iter()
                    .cloned()
                    .zip(args.iter().cloned())
                    .collect();
                let body = definition.body.subst(&map, &mut self.fresh);
                self.project(&body, path)
            }
            _ => Term::Op(field_marker("?", path), vec![value.clone()]),
        }
    }

    /// Weak head normal form: reduce until the head is a value or stuck.
    pub(crate) fn whnf(&mut self, t: &Term) -> Term {
        if !self.spend() {
            return t.clone();
        }
        match t {
            Term::Ctor { ty, case, args } if ty == "Nat" => {
                match (case.as_str(), args.as_slice()) {
                    ("Zero", []) => Term::Nat(0),
                    ("Succ", [inner]) => match self.whnf(inner) {
                        Term::Nat(n) if n < u64::MAX => Term::Nat(n + 1),
                        inner => Term::Ctor {
                            ty: ty.clone(),
                            case: case.clone(),
                            args: vec![inner],
                        },
                    },
                    _ => t.clone(),
                }
            }
            Term::Call { function, args, .. } => {
                let Some(definition) = self.functions.get(function) else {
                    return t.clone();
                };
                let map = definition
                    .params
                    .iter()
                    .cloned()
                    .zip(args.iter().cloned())
                    .collect();
                let mut body = definition.body.subst(&map, &mut self.fresh);
                if let Term::Call {
                    dispatch: Some(receiver),
                    ..
                } = t
                    && trait_method_default(function, self.definitions)
                {
                    body = specialize_trait_calls(
                        &body,
                        receiver,
                        args.first(),
                        self.functions,
                        self.definitions,
                    );
                }
                let reduced = self.whnf(&body);
                // Keep calls stuck on variables folded. A match on a known
                // constructor can make progress to a conditional result; list
                // operations must be able to inspect that result.
                // A stuck match stays folded. A conditional that the match
                // reached is a real step: the branches are known, and a tactic
                // such as byCases can reduce the condition.
                if matches!(reduced, Term::Match(..)) {
                    t.clone()
                } else {
                    reduced
                }
            }
            Term::Match(scrutinee, arms) => {
                let value = self.whnf(scrutinee);
                if !is_value(&value) {
                    return Term::Match(Box::new(value), arms.clone());
                }
                for (pat, body) in arms {
                    match self.bind(pat, &value) {
                        Some(Some(map)) => {
                            let body = body.subst(&map, &mut self.fresh);
                            return self.whnf(&body);
                        }
                        Some(None) => continue,
                        None => return Term::Match(Box::new(value), arms.clone()),
                    }
                }
                Term::Match(Box::new(value), arms.clone())
            }
            Term::If(c, a, b) => match self.whnf(c) {
                Term::Bool(true) => self.whnf(a),
                Term::Bool(false) => self.whnf(b),
                c => Term::If(Box::new(c), a.clone(), b.clone()),
            },
            Term::Op(op, args) if op == "seq" => self.whnf(&args[1]),
            Term::Op(op, args) if op.starts_with("$field:") && args.len() == 1 => {
                let Some((path, _)) = field_marker_parts(op) else {
                    return t.clone();
                };
                self.project(&args[0], &path)
            }
            Term::Op(op, args) => {
                let args = args.iter().map(|a| self.whnf(a)).collect::<Vec<_>>();
                if let Some(next) = sequences::step(op, &args) {
                    self.whnf(&next)
                } else if let Some(value) = compute(op, &args) {
                    value
                } else if let Some(next) = nat_step(op, &args) {
                    // The order rules produce a smaller comparison, so drive the
                    // step to a value instead of leaving one reduction per call.
                    self.whnf(&next)
                } else {
                    Term::Op(op.clone(), args)
                }
            }
            Term::Prop(prop) => Term::Prop(Box::new(self.nf_prop(prop))),
            Term::App(function, args) => {
                let function = self.whnf(function);
                if let Term::Lam(params, body) = function {
                    if args.len() >= params.len() {
                        let mut map = BTreeMap::new();
                        for ((name, _), arg) in params.iter().zip(args.iter()) {
                            map.insert(name.clone(), arg.clone());
                        }
                        let reduced = body.subst(&map, &mut self.fresh);
                        let rest = args[params.len()..].to_vec();
                        if rest.is_empty() {
                            return self.whnf(&reduced);
                        }
                        return self.whnf(&Term::App(Box::new(reduced), rest));
                    }
                    return Term::App(
                        Box::new(Term::Lam(params, body)),
                        args.iter().map(|a| self.whnf(a)).collect(),
                    );
                }
                Term::App(
                    Box::new(function),
                    args.iter().map(|a| self.whnf(a)).collect(),
                )
            }
            Term::FamilyUnion { set, element } => {
                let reduced = self.whnf(set);
                if &reduced == set.as_ref() {
                    t.clone()
                } else {
                    Term::FamilyUnion {
                        set: Box::new(reduced),
                        element: element.clone(),
                    }
                }
            }
            Term::FamilyInter { set, element } => {
                let reduced = self.whnf(set);
                if &reduced == set.as_ref() {
                    t.clone()
                } else {
                    Term::FamilyInter {
                        set: Box::new(reduced),
                        element: element.clone(),
                    }
                }
            }
            // A builder is a set written by its predicate. The proof kernel
            // compares it structurally or unfolds its membership; reducing the
            // predicate here would lose the binder and the surface form.
            Term::Builder { .. } => t.clone(),
            Term::Finite(elements) => {
                let reduced = elements.iter().map(|e| self.whnf(e)).collect::<Vec<_>>();
                if &reduced == elements {
                    t.clone()
                } else {
                    Term::Finite(reduced)
                }
            }
            _ => t.clone(),
        }
    }

    /// Matches a pattern against a value. `None` means the value is not known
    /// enough to decide, `Some(None)` a mismatch, `Some(Some(map))` a match.
    fn bind(&mut self, pat: &Pat, value: &Term) -> Option<Option<BTreeMap<String, Term>>> {
        match pat {
            Pat::Any => Some(Some(BTreeMap::new())),
            Pat::Bind(name) => Some(Some(BTreeMap::from([(name.clone(), value.clone())]))),
            Pat::Lit(lit) => {
                let value = self.whnf(value);
                if !is_value(&value) {
                    return None;
                }
                Some((&value == lit).then(BTreeMap::new))
            }
            Pat::Alt(pats) => {
                let mut unknown = false;
                for pat in pats {
                    match self.bind(pat, value) {
                        Some(Some(map)) => return Some(Some(map)),
                        Some(None) => {}
                        None => unknown = true,
                    }
                }
                if unknown { None } else { Some(None) }
            }
            Pat::Range {
                low,
                high,
                inclusive,
            } => {
                let value = self.whnf(value);
                let number = match &value {
                    Term::Int(n) => *n,
                    Term::Nat(n) => i64::try_from(*n).ok()?,
                    _ => return None,
                };
                Some(
                    (*low <= number && (number < *high || *inclusive && number == *high))
                        .then(BTreeMap::new),
                )
            }
            Pat::Ctor(case, pats) => {
                let value = self.whnf(value);
                let (found, args) = match &value {
                    Term::Nat(0) => ("Zero".to_owned(), vec![]),
                    Term::Nat(n) => ("Succ".to_owned(), vec![Term::Nat(n - 1)]),
                    Term::Bool(b) => (b.to_string(), vec![]),
                    Term::Ctor { case, args, .. } => (case.clone(), args.clone()),
                    _ => return None,
                };
                if &found != case || args.len() != pats.len() {
                    return Some(None);
                }
                let mut map = BTreeMap::new();
                for (pat, arg) in pats.iter().zip(&args) {
                    match self.bind(pat, arg)? {
                        Some(inner) => map.extend(inner),
                        None => return Some(None),
                    }
                }
                Some(Some(map))
            }
        }
    }

    /// Full normal form: the head and every argument reduced.
    pub(crate) fn nf(&mut self, t: &Term) -> Term {
        match self.whnf(t) {
            Term::Ctor { ty, case, args } => {
                let args = args.iter().map(|a| self.nf(a)).collect();
                self.whnf(&Term::Ctor { ty, case, args })
            }
            Term::Call {
                function,
                name,
                args,
                dispatch,
            } => Term::Call {
                function,
                name,
                args: args.iter().map(|a| self.nf(a)).collect(),
                dispatch,
            },
            Term::Opaque { name, args } => Term::Opaque {
                name,
                args: args.iter().map(|a| self.nf(a)).collect(),
            },
            Term::Op(op, args) => {
                let args = args.iter().map(|a| self.nf(a)).collect::<Vec<_>>();
                self.whnf(&Term::Op(op, args))
            }
            Term::If(c, a, b) => Term::If(
                Box::new(self.nf(&c)),
                Box::new(self.nf(&a)),
                Box::new(self.nf(&b)),
            ),
            Term::Match(s, arms) => Term::Match(
                Box::new(self.nf(&s)),
                arms.iter().map(|(p, b)| (p.clone(), self.nf(b))).collect(),
            ),
            Term::FamilyUnion { set, element } => Term::FamilyUnion {
                set: Box::new(self.nf(&set)),
                element: element.clone(),
            },
            Term::FamilyInter { set, element } => Term::FamilyInter {
                set: Box::new(self.nf(&set)),
                element: element.clone(),
            },
            Term::Builder {
                name,
                element,
                body,
            } => Term::Builder {
                name,
                element,
                body: Box::new(body.map(&mut |t| self.nf(t))),
            },
            Term::Finite(elements) => Term::Finite(elements.iter().map(|e| self.nf(e)).collect()),
            Term::Prop(prop) => Term::Prop(Box::new(self.nf_prop(&prop))),
            other => other,
        }
    }

    pub(crate) fn nf_prop(&mut self, p: &Prop) -> Prop {
        if let Prop::Atom(Term::Prop(inner)) = p {
            return self.nf_prop(inner);
        }
        let mapped = p.map(&mut |t| self.nf(t));
        if let Prop::Atom(Term::Prop(inner)) = &mapped {
            self.nf_prop(inner)
        } else {
            mapped
        }
    }

    /// Evaluates a closed proposition; `None` when it is not closed or not computable.
    pub(crate) fn decide(&mut self, p: &Prop) -> Option<bool> {
        match self.nf_prop(p) {
            Prop::Eq(a, b) => {
                (a.is_closed() && b.is_closed() && is_value(&a) && is_value(&b)).then_some(a == b)
            }
            Prop::Holds(Term::Bool(b)) => Some(b),
            Prop::Holds(_) | Prop::Mem(..) | Prop::Subset { .. } => None,
            Prop::And(a, b) => Some(self.decide(&a)? && self.decide(&b)?),
            Prop::Or(a, b) => Some(self.decide(&a)? || self.decide(&b)?),
            Prop::Not(p) => Some(!self.decide(&p)?),
            Prop::Implies(a, b) => Some(!self.decide(&a)? || self.decide(&b)?),
            Prop::Iff(a, b) => Some(self.decide(&a)? == self.decide(&b)?),
            Prop::True => Some(true),
            Prop::Forall { .. } | Prop::Exists { .. } => None,
            Prop::False => Some(false),
            Prop::Atom(_) => None,
        }
    }
}

/// Arithmetic and comparisons on literal arguments. Int arithmetic that would
/// overflow stays unreduced, matching the run-time stop.
fn compute(op: &str, args: &[Term]) -> Option<Term> {
    Some(match (op, args) {
        ("!", [Term::Bool(a)]) => Term::Bool(!a),
        ("-", [Term::Int(a)]) => Term::Int(a.checked_neg()?),
        ("+", [Term::Int(a), Term::Int(b)]) => Term::Int(a.checked_add(*b)?),
        ("-", [Term::Int(a), Term::Int(b)]) => Term::Int(a.checked_sub(*b)?),
        ("*", [Term::Int(a), Term::Int(b)]) => Term::Int(a.checked_mul(*b)?),
        // A zero divisor or an overflow leaves the term unreduced, as for `+`.
        ("/", [Term::Int(a), Term::Int(b)]) => Term::Int(a.checked_div(*b)?),
        ("%", [Term::Int(a), Term::Int(b)]) => Term::Int(a.checked_rem(*b)?),
        ("**", [Term::Int(a), Term::Int(b)]) => Term::Int(a.checked_pow(u32::try_from(*b).ok()?)?),
        ("*", [Term::Nat(a), Term::Nat(b)]) => Term::Nat(a.checked_mul(*b)?),
        ("/", [Term::Nat(a), Term::Nat(b)]) => Term::Nat(a.checked_div(*b)?),
        ("%", [Term::Nat(a), Term::Nat(b)]) => Term::Nat(a.checked_rem(*b)?),
        ("+", [Term::Nat(a), Term::Nat(b)]) => Term::Nat(a.checked_add(*b)?),
        ("-", [Term::Nat(a), Term::Nat(b)]) => Term::Nat(a.checked_sub(*b)?),
        ("&&", [Term::Bool(a), Term::Bool(b)]) => Term::Bool(*a && *b),
        ("||", [Term::Bool(a), Term::Bool(b)]) => Term::Bool(*a || *b),
        (op, [a, b]) if matches!(op, "<" | "<=" | ">" | ">=" | "==" | "!=") => {
            let ordering = match (a, b) {
                (Term::Int(x), Term::Int(y)) => x.cmp(y),
                (Term::Nat(x), Term::Nat(y)) => x.cmp(y),
                (Term::Bool(x), Term::Bool(y)) if matches!(op, "==" | "!=") => x.cmp(y),
                _ => return None,
            };
            Term::Bool(match op {
                "<" => ordering.is_lt(),
                "<=" => ordering.is_le(),
                ">" => ordering.is_gt(),
                ">=" => ordering.is_ge(),
                "==" => ordering.is_eq(),
                _ => ordering.is_ne(),
            })
        }
        _ => return None,
    })
}

/// Map portable math spellings to the operator the kernel knows.
fn canonical_operator(op: &str) -> &str {
    match op {
        "≥" | "\\ge" => ">=",
        "≤" | "\\le" => "<=",
        "≠" | "\\ne" => "!=",
        "\\cup" => "∪",
        "\\cap" => "∩",
        "\\compl" => "ᶜ",
        "\\union0" => "⋃₀",
        "\\inter0" => "⋂₀",
        "\\emptyset" => "∅",
        _ => op,
    }
}

/// Structural steps for the built-in Nat operators. They mirror the
/// constructors of Nat, so proofs can reason about `+` and `*` as well as a
/// user-written recursive function.
fn nat_step(op: &str, args: &[Term]) -> Option<Term> {
    // A numeral is `Succ(Succ(...Zero))` (std.core), but the kernel
    // folds a closed Nat to `Term::Nat`. Peel one constructor so the Zero and
    // Succ equations fire for a numeral on the left, as they do for the
    // unfolded spelling. The cap keeps a literal near 2^64 from expanding into
    // an unbounded term; above it the operation stays stuck and `rfl` reports
    // the mismatch instead of hanging the checker.
    let zero = |t: &Term| match t {
        Term::Nat(0) => true,
        Term::Ctor { ty, case, .. } => ty == "Nat" && case == "Zero",
        _ => false,
    };
    let successor = |t: &Term| match t {
        Term::Ctor { ty, case, args } if ty == "Nat" && case == "Succ" && args.len() == 1 => {
            Some(args[0].clone())
        }
        Term::Nat(n) if *n > 0 && *n <= NUMERAL_EXPANSION_LIMIT => Some(Term::Nat(n - 1)),
        _ => None,
    };
    match (op, args) {
        ("+", [a, b]) if zero(a) => Some(b.clone()),
        ("+", [a, b]) => successor(a).map(|inner| Term::Ctor {
            ty: "Nat".into(),
            case: "Succ".into(),
            args: vec![Term::Op("+".into(), vec![inner, b.clone()])],
        }),
        ("*", [a, _]) if zero(a) => Some(Term::Nat(0)),
        ("*", [a, b]) => successor(a).map(|inner| {
            Term::Op(
                "+".into(),
                vec![b.clone(), Term::Op("*".into(), vec![inner, b.clone()])],
            )
        }),
        // Nat order follows the constructors: `Zero <= b`, not `Succ <= Zero`,
        // and `Succ(a) <= Succ(b)` reduces to `a <= b`. Two identical sides are
        // true, which is what `leRefl` needs. `a < b` means `Succ(a) <= b`.
        ("<=", [a, b]) => {
            if a == b {
                return Some(Term::Bool(true));
            }
            match nat_shape(a) {
                None => None,
                Some(NatShape::Zero) => Some(Term::Bool(true)),
                Some(NatShape::Succ(x)) => match nat_shape(b) {
                    None => None,
                    Some(NatShape::Zero) => Some(Term::Bool(false)),
                    Some(NatShape::Succ(y)) => Some(Term::Op("<=".into(), vec![x, y])),
                },
            }
        }
        // The strict forms rewrite to `<=` only when a side is visibly Nat.
        // An Int comparison such as `s.side > 0` must stay untouched.
        ("<", [a, b]) if nat_shape(a).is_some() || nat_shape(b).is_some() => Some(Term::Op(
            "<=".into(),
            vec![
                Term::Ctor {
                    ty: "Nat".into(),
                    case: "Succ".into(),
                    args: vec![a.clone()],
                },
                b.clone(),
            ],
        )),
        (">=", [a, b]) if nat_shape(a).is_some() || nat_shape(b).is_some() => {
            Some(Term::Op("<=".into(), vec![b.clone(), a.clone()]))
        }
        (">", [a, b]) if nat_shape(a).is_some() || nat_shape(b).is_some() => Some(Term::Op(
            "<=".into(),
            vec![
                Term::Ctor {
                    ty: "Nat".into(),
                    case: "Succ".into(),
                    args: vec![b.clone()],
                },
                a.clone(),
            ],
        )),
        _ => None,
    }
}

#[allow(clippy::large_enum_variant)]
enum NatShape {
    Zero,
    Succ(Term),
}

/// A Nat term seen through its constructors, including the folded numeral.
fn nat_shape(t: &Term) -> Option<NatShape> {
    match t {
        Term::Nat(0) => Some(NatShape::Zero),
        Term::Nat(n) if *n <= NUMERAL_EXPANSION_LIMIT => Some(NatShape::Succ(Term::Nat(n - 1))),
        Term::Ctor { ty, case, args } if ty == "Nat" && case == "Zero" && args.is_empty() => {
            Some(NatShape::Zero)
        }
        Term::Ctor { ty, case, args } if ty == "Nat" && case == "Succ" && args.len() == 1 => {
            Some(NatShape::Succ(args[0].clone()))
        }
        _ => None,
    }
}

/// A checked theorem: what citing it states and what the citation must establish.
#[derive(Clone, Debug)]
pub(crate) struct Theorem {
    pub params: Vec<(String, Type)>,
    pub bounds: BTreeMap<String, Vec<Type>>,
    pub requires: Vec<(String, Prop)>,
    pub statement: Prop,
    /// Virtual method calls used directly or through functions in this
    /// statement. Importers validate them against their own class hierarchy.
    pub dispatches: Vec<Dispatch>,
    /// Some assumption relies on its own arithmetic succeeding, which earlier
    /// assumptions do not establish. Citations must establish it; a function
    /// cannot require such a theorem (Q82).
    pub presupposes: bool,
    /// The proof used `sorry`. The theorem is usable for sketching but never
    /// discharges a function requirement.
    pub admitted: bool,
    /// This proof or a cited lemma relies on an unchecked solver result.
    pub trusted_smt: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Dispatch {
    pub receiver: Type,
    pub method: String,
    pub function: DeclarationId,
}

fn class_as_supertype(
    mut current: Type,
    target_id: &str,
    definitions: &BTreeMap<String, crate::checker::Definition>,
) -> Option<Type> {
    for _ in 0..96 {
        let (id, args) = Kernel::class_type(&current)?;
        if id == target_id {
            return Some(Type::Named(id.to_owned(), args.to_vec()));
        }
        let parent = definitions.get(id)?.parent.as_ref()?;
        let owner = definitions.get(id)?;
        if owner.parameters.len() != args.len() {
            return None;
        }
        let substitutions = owner
            .parameters
            .iter()
            .cloned()
            .zip(args.iter().cloned())
            .collect::<BTreeMap<_, _>>();
        current = parent.replace_parameters(&substitutions);
    }
    None
}

fn universalize_type(ty: &Type, prefix: &str) -> Type {
    ty.map(&mut |part| match part {
        Type::Parameter(name) if !name.starts_with('?') => {
            Some(Type::Parameter(format!("?{prefix}:{name}")))
        }
        Type::Rigid(name) => Some(Type::Parameter(format!("?{prefix}:{name}"))),
        _ => None,
    })
}

fn specialize_method(
    signature: &crate::types::FunctionType,
    owner: &str,
    receiver: &Type,
    definitions: &BTreeMap<String, crate::checker::Definition>,
) -> Option<crate::types::FunctionType> {
    let definition = definitions.get(owner)?;
    let (found_owner, args) = Kernel::class_type(receiver)?;
    if found_owner != owner || definition.parameters.len() != args.len() {
        return None;
    }
    let substitutions = definition
        .parameters
        .iter()
        .cloned()
        .zip(args.iter().cloned())
        .collect::<BTreeMap<_, _>>();
    match Type::Function(signature.clone()).replace_parameters(&substitutions) {
        Type::Function(found) => Some(found),
        _ => None,
    }
}

fn overridden_by(
    dispatch: &Dispatch,
    definitions: &BTreeMap<String, crate::checker::Definition>,
) -> Option<String> {
    let (target_id, target_args) = Kernel::class_type(&dispatch.receiver)?;
    let normalized_receiver = Type::Named(target_id.to_owned(), target_args.to_vec());
    // A method resolved on Child can be declared on Parent. Find the actual
    // declaration owner first, then specialize its signature through the
    // receiver's ancestor path before comparing candidate overrides.
    let target_owner = definitions.iter().find_map(|(class_id, class)| {
        class
            .methods
            .get(&dispatch.method)?
            .iter()
            .find_map(|signature| {
                matches!(
                    &signature.callable,
                    Some(Callable::Declaration { id, .. }) if id == &dispatch.function
                )
                .then(|| match &signature.callable {
                    Some(Callable::Declaration {
                        owner: Some(owner), ..
                    }) => owner.clone(),
                    _ => class_id.clone(),
                })
            })
    })?;
    let target_signature = definitions
        .get(&target_owner)?
        .methods
        .get(&dispatch.method)?
        .iter()
        .find(|signature| {
            matches!(&signature.callable, Some(Callable::Declaration { id, .. }) if id == &dispatch.function)
        })?;
    for (candidate, definition) in definitions {
        if candidate == target_id || definition.kind != K::Class {
            continue;
        }
        let candidate_type = Type::Named(
            candidate.clone(),
            definition
                .parameters
                .iter()
                .map(|name| Type::Parameter(format!("?dispatch:{candidate}:{name}")))
                .collect(),
        );
        let Some(static_ancestor) =
            class_as_supertype(candidate_type.clone(), target_id, definitions)
        else {
            continue;
        };
        let target_pattern =
            universalize_type(&normalized_receiver, &format!("receiver:{target_id}"));
        if !generic::compatible(&static_ancestor, &target_pattern) {
            continue;
        }
        let Some(candidate_owner_type) =
            class_as_supertype(candidate_type.clone(), &target_owner, definitions)
        else {
            continue;
        };
        let Some(candidate_target_method) = specialize_method(
            target_signature,
            &target_owner,
            &candidate_owner_type,
            definitions,
        ) else {
            continue;
        };
        let candidate_substitutions = definition
            .parameters
            .iter()
            .cloned()
            .zip(match &candidate_type {
                Type::Named(_, args) => args.clone(),
                _ => vec![],
            })
            .collect::<BTreeMap<_, _>>();
        let Some(methods) = definition.methods.get(&dispatch.method) else {
            continue;
        };
        let overridden = methods.iter().any(|signature| {
            let Some(Callable::Declaration {
                id,
                owner,
                kind: DeclarationKind::Method | DeclarationKind::StaticMethod,
                ..
            }) = &signature.callable
            else {
                return false;
            };
            if id == &dispatch.function || owner.as_deref() != Some(candidate.as_str()) {
                return false;
            }
            let specialized = match Type::Function(signature.clone())
                .replace_parameters(&candidate_substitutions)
            {
                Type::Function(found) => found,
                _ => return false,
            };
            specialized.same_parameters(&candidate_target_method)
        });
        if overridden {
            return Some(candidate.clone());
        }
    }
    None
}

fn trait_method_default(
    id: &DeclarationId,
    definitions: &BTreeMap<String, crate::checker::Definition>,
) -> bool {
    definitions.values().any(|definition| {
        definition.kind == K::Trait
            && definition.file == id.file
            && definition.default_methods.contains(&id.span)
    })
}

fn trait_method_signature<'a>(
    id: &DeclarationId,
    name: &str,
    definitions: &'a BTreeMap<String, crate::checker::Definition>,
) -> Option<&'a crate::types::FunctionType> {
    definitions.values().find_map(|definition| {
        (definition.kind == K::Trait)
            .then(|| definition.methods.get(name))
            .flatten()?
            .iter()
            .find(|signature| {
                matches!(&signature.callable, Some(Callable::Declaration { id: method, kind: DeclarationKind::TraitMethod, .. }) if method == id)
            })
    })
}

fn concrete_trait_method(
    receiver: &Type,
    trait_method: &DeclarationId,
    name: &str,
    definitions: &BTreeMap<String, crate::checker::Definition>,
) -> Option<(DeclarationId, crate::types::FunctionType)> {
    let contract = trait_method_signature(trait_method, name, definitions)?;
    let argument_count = contract.parameters.len();
    let mut current = Type::Named(
        Kernel::class_type(receiver)?.0.to_owned(),
        Kernel::class_type(receiver)?.1.to_vec(),
    );
    for _ in 0..96 {
        let (id, args) = Kernel::class_type(&current)?;
        let definition = definitions.get(id)?;
        if definition.kind != K::Class || definition.parameters.len() != args.len() {
            return None;
        }
        let substitutions = definition
            .parameters
            .iter()
            .cloned()
            .zip(args.iter().cloned())
            .collect::<BTreeMap<_, _>>();
        if let Some(methods) = definition.methods.get(name) {
            for signature in methods {
                if signature.parameters.len() != argument_count {
                    continue;
                }
                let Some(Callable::Declaration {
                    id: implementation,
                    owner: Some(owner),
                    kind: DeclarationKind::Method | DeclarationKind::StaticMethod,
                    ..
                }) = &signature.callable
                else {
                    continue;
                };
                let implementation_signature =
                    match Type::Function(signature.clone()).replace_parameters(&substitutions) {
                        Type::Function(found) => found,
                        _ => continue,
                    };
                let contract_signature =
                    match Type::Function(contract.clone()).replace_parameters(&substitutions) {
                        Type::Function(found) => found,
                        _ => continue,
                    };
                if implementation == trait_method
                    || implementation_signature.same_parameters(&contract_signature)
                {
                    let _ = owner;
                    return Some((implementation.clone(), implementation_signature));
                }
            }
        }
        let Some(parent) = &definition.parent else {
            return None;
        };
        current = parent.replace_parameters(&substitutions);
    }
    None
}

fn own_field_names(definition: &crate::checker::Definition) -> Vec<String> {
    let mut names = definition.fields.keys().cloned().collect::<Vec<_>>();
    if definition.record
        && let Some(Callable::Builtin(constructor)) = definition
            .constructors
            .first()
            .and_then(|constructor| constructor.callable.as_ref())
        && let Some((_, declared)) = constructor.split_once('|')
    {
        let declared = declared
            .split(',')
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if declared.len() == names.len()
            && declared
                .iter()
                .all(|name| definition.fields.contains_key(name))
        {
            names = declared;
        }
    }
    names
}

fn concrete_field_path(
    ty: &Type,
    field: &str,
    definitions: &BTreeMap<String, crate::checker::Definition>,
) -> Option<(Vec<usize>, Type)> {
    fn find(
        ty: &Type,
        field: &str,
        definitions: &BTreeMap<String, crate::checker::Definition>,
        depth: usize,
    ) -> Option<(Vec<usize>, Type)> {
        if depth >= 96 {
            return None;
        }
        let (id, args) = Kernel::class_type(ty)?;
        let definition = definitions.get(id)?;
        if definition.kind != K::Class || definition.parameters.len() != args.len() {
            return None;
        }
        let substitutions = definition
            .parameters
            .iter()
            .cloned()
            .zip(args.iter().cloned())
            .collect::<BTreeMap<_, _>>();
        let parent = definition
            .parent
            .as_ref()
            .map(|parent| parent.replace_parameters(&substitutions))
            .filter(|parent| *parent != Type::named("Object"));
        let own = own_field_names(definition);
        if let Some(index) = own.iter().position(|name| name == field) {
            let field_ty = definition
                .fields
                .get(field)?
                .replace_parameters(&substitutions);
            return Some((vec![usize::from(parent.is_some()) + index], field_ty));
        }
        let parent = parent?;
        let (mut path, field_ty) = find(&parent, field, definitions, depth + 1)?;
        path.insert(0, 0);
        Some((path, field_ty))
    }

    find(ty, field, definitions, 0)
}

fn concrete_receiver_projection(
    suffix: &str,
    receiver: &Type,
    value: &Term,
    definitions: &BTreeMap<String, crate::checker::Definition>,
) -> Option<Term> {
    let mut current = receiver.clone();
    let mut path = vec![];
    for field in suffix.split('.') {
        let (part, field_type) = concrete_field_path(&current, field, definitions)?;
        path.extend(part);
        current = field_type;
    }
    Some(Term::Op(field_marker(suffix, &path), vec![value.clone()]))
}

fn specialize_trait_calls(
    term: &Term,
    receiver: &Type,
    receiver_value: Option<&Term>,
    functions: &BTreeMap<DeclarationId, Definition>,
    definitions: &BTreeMap<String, crate::checker::Definition>,
) -> Term {
    match term {
        Term::Call {
            function,
            name,
            args,
            dispatch,
        } => {
            let same_receiver = args
                .first()
                .zip(receiver_value)
                .is_some_and(|(call_receiver, expected)| call_receiver == expected);
            let args = args
                .iter()
                .map(|arg| {
                    specialize_trait_calls(arg, receiver, receiver_value, functions, definitions)
                })
                .collect::<Vec<_>>();
            let (function, dispatch, args) = if dispatch
                .as_ref()
                .and_then(|ty| Kernel::class_type(ty))
                .is_some_and(|(id, _)| definitions.get(id).is_some_and(|d| d.kind == K::Trait))
                && same_receiver
            {
                concrete_trait_method(receiver, function, name, definitions)
                    .and_then(|(implementation, signature)| {
                        let definition = functions.get(&implementation)?;
                        let visible_count = signature.parameters.len() + 1;
                        if args.len() < visible_count || visible_count > definition.params.len() {
                            return None;
                        }
                        let mut specialized_args = args[..visible_count].to_vec();
                        for parameter in definition.params.iter().skip(visible_count) {
                            let (index, suffix) = definition
                                .params
                                .iter()
                                .take(visible_count)
                                .enumerate()
                                .find_map(|(index, root)| {
                                    parameter
                                        .strip_prefix(&format!("{root}."))
                                        .map(|suffix| (index, suffix))
                                })?;
                            let ty = if index == 0 {
                                receiver
                            } else {
                                &signature.parameters.get(index - 1)?.ty
                            };
                            specialized_args.push(concrete_receiver_projection(
                                suffix,
                                ty,
                                &args[index],
                                definitions,
                            )?);
                        }
                        (specialized_args.len() == definition.params.len()).then(|| {
                            (
                                implementation,
                                Some(Box::new(receiver.clone())),
                                specialized_args,
                            )
                        })
                    })
                    .unwrap_or_else(|| (function.clone(), dispatch.clone(), args.clone()))
            } else {
                (function.clone(), dispatch.clone(), args.clone())
            };
            Term::Call {
                function,
                name: name.clone(),
                args,
                dispatch,
            }
        }
        Term::Ctor { ty, case, args } => Term::Ctor {
            ty: ty.clone(),
            case: case.clone(),
            args: args
                .iter()
                .map(|arg| {
                    specialize_trait_calls(arg, receiver, receiver_value, functions, definitions)
                })
                .collect(),
        },
        Term::Opaque { name, args } => Term::Opaque {
            name: name.clone(),
            args: args
                .iter()
                .map(|arg| {
                    specialize_trait_calls(arg, receiver, receiver_value, functions, definitions)
                })
                .collect(),
        },
        Term::Op(op, args) => Term::Op(
            op.clone(),
            args.iter()
                .map(|arg| {
                    specialize_trait_calls(arg, receiver, receiver_value, functions, definitions)
                })
                .collect(),
        ),
        Term::If(c, a, b) => Term::If(
            Box::new(specialize_trait_calls(
                c,
                receiver,
                receiver_value,
                functions,
                definitions,
            )),
            Box::new(specialize_trait_calls(
                a,
                receiver,
                receiver_value,
                functions,
                definitions,
            )),
            Box::new(specialize_trait_calls(
                b,
                receiver,
                receiver_value,
                functions,
                definitions,
            )),
        ),
        Term::Match(value, arms) => Term::Match(
            Box::new(specialize_trait_calls(
                value,
                receiver,
                receiver_value,
                functions,
                definitions,
            )),
            arms.iter()
                .map(|(pat, body)| {
                    (
                        pat.clone(),
                        specialize_trait_calls(
                            body,
                            receiver,
                            receiver_value,
                            functions,
                            definitions,
                        ),
                    )
                })
                .collect(),
        ),
        _ => term.clone(),
    }
}

/// Revalidates virtual calls recorded by checked theorems against every class
/// in a loaded program, including classes hidden from a module's imports.
pub(crate) fn dispatch_override_error(
    dispatches: &[Dispatch],
    definitions: &BTreeMap<String, crate::checker::Definition>,
    span: Span,
) -> Option<Diagnostic> {
    dispatches.iter().find_map(|dispatch| {
        overridden_by(dispatch, definitions).map(|subclass| {
            Diagnostic::new(
                "E_PROOF_OVERRIDDEN",
                format!(
                    "Theorem dispatch to `{}.{}` is unsafe because subclass `{subclass}` overrides it.",
                    dispatch.receiver,
                    dispatch.method
                ),
                span,
            )
            .with_hint("Prove the claim for the subclass's implementation, or avoid relying on virtual dispatch.")
        })
    })
}

/// Names visible while elaborating: each maps to a term and its type.
#[derive(Clone, Default)]
struct Scope {
    terms: Vec<(String, Term, Type)>,
    bounds: BTreeMap<String, Vec<Type>>,
}

impl Scope {
    fn get(&self, name: &str) -> Option<&(String, Term, Type)> {
        self.terms.iter().rev().find(|(n, _, _)| n == name)
    }
    fn with(&self, name: &str, term: Term, ty: Type) -> Scope {
        let mut next = self.clone();
        next.terms.push((name.to_owned(), term, ty));
        next
    }
}

type Found<T> = Result<T, Diagnostic>;
/// A function's name, parameter names and types, and result type.
type Signature = (String, Vec<(String, Type)>, Type);

fn fail<T>(span: Span, code: &'static str, message: impl Into<String>, hint: &str) -> Found<T> {
    Err(Diagnostic::new(code, message, span).with_hint(hint))
}

fn fail_with<T>(
    span: Span,
    code: &'static str,
    message: impl Into<String>,
    hint: &str,
    fixes: Vec<crate::actions::Fix>,
) -> Found<T> {
    Err(Diagnostic::new(code, message, span)
        .with_hint(hint)
        .with_fixes(fixes))
}

fn nat() -> Type {
    Type::named("Nat")
}

pub(crate) struct Kernel<'a> {
    parsed: &'a Parsed,
    checked: &'a Checked,
    /// Local callable syntax, indexed once. A file and its only declaration can
    /// share a span, so only callable nodes belong in this index.
    local_nodes: BTreeMap<Span, &'a Node>,
    /// Every free function of this module: name, parameter names and types, result.
    signatures: BTreeMap<DeclarationId, Signature>,
    constraints: BTreeMap<DeclarationId, Vec<(Type, Type)>>,
    /// The owning class for each instance method. Its synthetic receiver fields
    /// are passed along when the method term is unfolded.
    method_infos: BTreeMap<DeclarationId, MethodInfo>,
    pub functions: BTreeMap<DeclarationId, Definition>,
    /// Why a function cannot be unfolded, for the message when a theorem names it.
    opaque: BTreeMap<DeclarationId, String>,
    /// Functions declared with `opaque function`. A theorem may name them; the
    /// kernel never unfolds them.
    declared_opaque: BTreeSet<DeclarationId>,
    declared_opaque_names: BTreeSet<String>,
    pub theorems: BTreeMap<String, Theorem>,
    simp_theorems: Vec<String>,
    /// Checked theorems by declaration: those imported, then this module's own.
    exported: BTreeMap<DeclarationId, Exported>,
    /// Functions whose every call is defined, so a proof need not unfold them to check.
    total: BTreeSet<DeclarationId>,
    /// Functions every call of which is defined over the integers and naturals:
    /// `Succ`, `+` and `*` cannot fail there (REVIEW 2.8).
    total_math: BTreeSet<DeclarationId>,
    fresh: usize,
    warnings: Vec<Diagnostic>,
    /// Goal snapshots while an editor asks for the live proof state.
    tactic_states: Vec<crate::proof::tactics::StepState>,
    record_tactics: bool,
    smt_trusted: std::cell::Cell<bool>,
    /// Class whose body is currently being elaborated; private members are
    /// available there but never from a top-level theorem.
    access_owner: Option<String>,
    /// Proofs treat Int as the integers and Nat as the naturals, so a theorem
    /// carries no overflow conditions (REVIEW 2.8). Facts that reach run-time
    /// code, such as a function's theorem requirement, keep the 64-bit bounds.
    pub(crate) runtime_bounds: std::cell::Cell<bool>,
}

#[derive(Clone, Debug)]
struct MethodInfo {
    owner: String,
    receiver: Type,
    name: String,
    is_static: bool,
}

impl<'a> Kernel<'a> {
    fn text(&self, node: &Node) -> &'a str {
        self.parsed.source.slice(node.span)
    }

    fn fresh(&mut self, name: &str) -> String {
        self.fresh += 1;
        format!("{}#{}", shown(name), self.fresh)
    }

    pub(crate) fn new(parsed: &'a Parsed, checked: &'a Checked) -> Self {
        let mut kernel = Kernel {
            parsed,
            checked,
            local_nodes: parsed
                .root
                .descendants()
                .filter(|node| matches!(node.kind, K::Function | K::Constructor))
                .map(|node| (node.span, node))
                .collect(),
            signatures: checked.environment.proofs.signatures.clone(),
            constraints: checked.environment.proofs.constraints.clone(),
            method_infos: BTreeMap::new(),
            functions: checked.environment.proofs.functions.clone(),
            opaque: checked.environment.proofs.opaque.clone(),
            declared_opaque: BTreeSet::new(),
            declared_opaque_names: BTreeSet::new(),
            theorems: BTreeMap::new(),
            simp_theorems: vec![],
            exported: checked.environment.proofs.theorems.clone(),
            total: BTreeSet::new(),
            total_math: BTreeSet::new(),
            fresh: 0,
            warnings: vec![],
            tactic_states: vec![],
            record_tactics: false,
            smt_trusted: std::cell::Cell::new(false),
            access_owner: None,
            runtime_bounds: std::cell::Cell::new(false),
        };
        for (owner, class) in &checked.environment.definitions {
            for (name, methods) in &class.methods {
                for signature in methods {
                    let Some(Callable::Declaration {
                        id,
                        kind,
                        owner: callable_owner,
                        ..
                    }) = &signature.callable
                    else {
                        continue;
                    };
                    let method_owner = callable_owner.as_deref().unwrap_or(owner);
                    let receiver = checked
                        .environment
                        .definitions
                        .get(method_owner)
                        .map(|definition| {
                            Type::Named(
                                method_owner.to_owned(),
                                definition
                                    .parameters
                                    .iter()
                                    .cloned()
                                    .map(Type::Parameter)
                                    .collect(),
                            )
                        })
                        .unwrap_or_else(|| Type::Named(method_owner.to_owned(), vec![]));
                    let info = MethodInfo {
                        owner: method_owner.to_owned(),
                        receiver: receiver.clone(),
                        name: name.clone(),
                        is_static: *kind == DeclarationKind::StaticMethod,
                    };
                    if class.kind == K::Trait {
                        // A synthesized default keeps its trait declaration
                        // identity in every implementing class. The body is
                        // loaded once with trait `this`; concrete dispatch is
                        // specialized only at each call site.
                        kernel.method_infos.insert(id.clone(), info);
                    } else {
                        kernel.method_infos.entry(id.clone()).or_insert(info);
                    }
                }
            }
        }
        // Imported theorems precede every local one, so any local proof may cite them.
        for (name, id) in &checked.environment.theorems {
            if let Some(exported) = kernel.exported.get(id) {
                kernel
                    .theorems
                    .insert(name.clone(), exported.theorem.clone());
                // A namespace import names `facts.rule`; only a rule imported by
                // name joins a plain simp (Q79).
                if exported.simp && !name.contains('.') {
                    kernel.simp_theorems.push(name.clone());
                }
            }
        }
        for node in parsed
            .root
            .children
            .iter()
            .filter(|n| n.kind == K::Function)
        {
            let name = node
                .child(K::Name)
                .map(|n| kernel.text(n).to_owned())
                .unwrap_or_default();
            let mut params = vec![];
            for parameter in node
                .child(K::ParameterList)
                .map(|p| p.children.as_slice())
                .unwrap_or(&[])
            {
                let pname = parameter
                    .child(K::Name)
                    .map(|n| kernel.text(n).to_owned())
                    .unwrap_or_default();
                params.push((
                    pname,
                    checked
                        .binding_types
                        .get(&parameter.span)
                        .cloned()
                        .unwrap_or(Type::Invalid),
                ));
            }
            let mut result = kernel.signature_result(&name, node.span);
            if node
                .children
                .iter()
                .any(|n| n.kind == K::Requires && !checked.theorem_requirements.contains(&n.span))
                && let Some(success) = kernel.success_type(&result)
            {
                // A failed run-time condition is the only Error the kernel admits, and a
                // proof must rule it out, so the call stands for its success value (Q80).
                result = success;
            }
            if params.iter().any(|(_, t)| *t == Type::Invalid) {
                // Parameter types come from the checker's signature when bindings lack them.
                if let Some((p, _)) = kernel.signature_params(&name, node.span) {
                    params = p;
                }
            }
            let id = DeclarationId {
                file: parsed.source.name.clone(),
                span: node.span,
            };
            if let Some(signature) = kernel.signature_entry(&name, node.span) {
                let mut constraints = signature.requirements;
                for (name, required) in signature.bounds {
                    constraints.extend(
                        required
                            .into_iter()
                            .map(|bound| (Type::Parameter(name.clone()), bound)),
                    );
                }
                if !constraints.is_empty() {
                    kernel.constraints.insert(id.clone(), constraints);
                }
            }
            kernel.signatures.insert(id, (name, params, result));
        }
        for wrapper in parsed.root.children.iter().filter(|n| n.kind == K::Opaque) {
            let Some(function) = wrapper.children.first().filter(|n| n.kind == K::Function) else {
                continue;
            };
            let name = function
                .child(K::Name)
                .map(|n| kernel.text(n).to_owned())
                .unwrap_or_default();
            if name.is_empty() {
                continue;
            }
            let id = DeclarationId {
                file: parsed.source.name.clone(),
                span: function.span,
            };
            let Some(symbol) = checked.environment.values.get(&name) else {
                continue;
            };
            let signatures = match &symbol.ty {
                Type::Function(signature) => vec![signature.clone()],
                Type::Overloaded(list) => list.clone(),
                _ => continue,
            };
            let Some(signature) = signatures.into_iter().find(|signature| {
                matches!(
                    &signature.callable,
                    Some(Callable::Declaration { id: callable, .. }) if callable == &id
                )
            }) else {
                continue;
            };
            let params = signature
                .parameters
                .iter()
                .map(|parameter| {
                    (
                        parameter.name.clone().unwrap_or_default(),
                        parameter.ty.clone(),
                    )
                })
                .collect::<Vec<_>>();
            kernel.signatures.insert(
                id.clone(),
                (name.clone(), params, signature.result.as_ref().clone()),
            );
            kernel.opaque.insert(
                id.clone(),
                "it is an opaque declaration with no body".into(),
            );
            kernel.declared_opaque.insert(id);
            kernel.declared_opaque_names.insert(name.clone());
            kernel
                .declared_opaque_names
                .insert(crate::lexer::path_name(&name).to_owned());
        }
        for node in parsed
            .root
            .children
            .iter()
            .filter(|n| matches!(n.kind, K::Class | K::Trait))
        {
            let Some(source_name) = node.child(K::Name).map(|n| kernel.text(n)) else {
                continue;
            };
            let owner = checked
                .environment
                .names
                .get(source_name)
                .cloned()
                .unwrap_or_else(|| source_name.to_owned());
            let Some(class) = checked.environment.definitions.get(&owner) else {
                continue;
            };
            for (name, methods) in &class.methods {
                for signature in methods {
                    let Some(Callable::Declaration {
                        id,
                        kind,
                        owner: callable_owner,
                        ..
                    }) = &signature.callable
                    else {
                        continue;
                    };
                    if id.file != parsed.source.name {
                        continue;
                    }
                    if !node
                        .descendants()
                        .any(|member| member.kind == K::Function && member.span == id.span)
                    {
                        continue;
                    }
                    let method_owner = callable_owner.as_deref().unwrap_or(&owner);
                    if method_owner != owner {
                        continue;
                    }
                    let receiver = Type::Named(
                        owner.clone(),
                        class
                            .parameters
                            .iter()
                            .cloned()
                            .map(Type::Parameter)
                            .collect(),
                    );
                    let is_static = *kind == DeclarationKind::StaticMethod;
                    let mut params = Vec::with_capacity(signature.parameters.len() + 1);
                    if !is_static {
                        params.push(("this".to_owned(), receiver.clone()));
                    }
                    params.extend(signature.parameters.iter().map(|parameter| {
                        (
                            parameter.name.clone().unwrap_or_default(),
                            parameter.ty.clone(),
                        )
                    }));
                    kernel.signatures.insert(
                        id.clone(),
                        (name.clone(), params, signature.result.as_ref().clone()),
                    );
                    let mut constraints = signature.requirements.clone();
                    for (parameter, bounds) in &signature.bounds {
                        constraints.extend(
                            bounds
                                .iter()
                                .cloned()
                                .map(|bound| (Type::Parameter(parameter.clone()), bound)),
                        );
                    }
                    if !constraints.is_empty() {
                        kernel.constraints.insert(id.clone(), constraints);
                    }
                    kernel.method_infos.insert(
                        id.clone(),
                        MethodInfo {
                            owner: method_owner.to_owned(),
                            receiver: receiver.clone(),
                            name: name.clone(),
                            is_static,
                        },
                    );
                }
            }
            for constructor in &class.constructors {
                let Some(Callable::Declaration { id, .. }) = &constructor.callable else {
                    continue;
                };
                if id.file != parsed.source.name {
                    continue;
                }
                let params = constructor
                    .parameters
                    .iter()
                    .map(|parameter| {
                        (
                            parameter.name.clone().unwrap_or_default(),
                            parameter.ty.clone(),
                        )
                    })
                    .collect();
                kernel.signatures.insert(
                    id.clone(),
                    (
                        source_name.to_owned(),
                        params,
                        constructor.result.as_ref().clone(),
                    ),
                );
                let mut constraints = constructor.requirements.clone();
                for (parameter, bounds) in &constructor.bounds {
                    constraints.extend(
                        bounds
                            .iter()
                            .cloned()
                            .map(|bound| (Type::Parameter(parameter.clone()), bound)),
                    );
                }
                if !constraints.is_empty() {
                    kernel.constraints.insert(id.clone(), constraints);
                }
            }
        }
        let spans = kernel
            .signatures
            .keys()
            .filter(|id| id.file == parsed.source.name)
            .cloned()
            .collect::<Vec<_>>();
        for span in &spans {
            kernel.load(span.clone());
        }
        // Class readability can depend on declarations anywhere in the file.
        // Rebuild in reverse source order until the local function bodies stop
        // changing; a declaration-count bound prevents an accidental cycle.
        let reload_fresh = kernel.fresh;
        for _ in 0..=spans.len() {
            // Match binders use fresh names. Reuse the same range on each
            // rebuild so those implementation details do not prevent the
            // fixed point from stabilizing.
            kernel.fresh = reload_fresh;
            let before = spans
                .iter()
                .filter_map(|id| {
                    kernel
                        .functions
                        .get(id)
                        .cloned()
                        .map(|body| (id.clone(), body))
                })
                .collect::<BTreeMap<_, _>>();
            for span in spans.iter().rev() {
                kernel.load(span.clone());
            }
            let after = spans
                .iter()
                .filter_map(|id| {
                    kernel
                        .functions
                        .get(id)
                        .cloned()
                        .map(|body| (id.clone(), body))
                })
                .collect::<BTreeMap<_, _>>();
            if before == after {
                break;
            }
        }
        kernel.reject_mutual_recursion();
        kernel.reject_opaque_dependencies();
        kernel.total = total_functions(&kernel.functions, false);
        kernel.total_math = total_functions(&kernel.functions, true);
        kernel
    }

    fn signature_entry(&self, name: &str, span: Span) -> Option<crate::types::FunctionType> {
        let symbol = self.checked.environment.values.get(name)?;
        let candidates = match &symbol.ty {
            Type::Function(f) => vec![f.clone()],
            Type::Overloaded(fs) => fs.clone(),
            _ => vec![],
        };
        candidates.into_iter().find(
            |f| matches!(&f.callable, Some(Callable::Declaration { id, .. }) if id.span == span && id.file == self.parsed.source.name),
        )
    }

    fn signature_result(&self, name: &str, span: Span) -> Type {
        self.signatures
            .get(&DeclarationId {
                file: self.parsed.source.name.clone(),
                span,
            })
            .map(|(_, _, result)| result.clone())
            .or_else(|| self.signature_entry(name, span).map(|f| *f.result))
            .unwrap_or(Type::Invalid)
    }

    fn signature_params(&self, name: &str, span: Span) -> Option<(Vec<(String, Type)>, Type)> {
        let f = self.signature_entry(name, span)?;
        Some((
            f.parameters
                .iter()
                .map(|p| (p.name.clone().unwrap_or_default(), p.ty.clone()))
                .collect(),
            *f.result,
        ))
    }

    fn visible_functions(&self, name: &str) -> Vec<DeclarationId> {
        let Some(symbol) = self.checked.environment.values.get(name) else {
            return vec![];
        };
        let signatures = match &symbol.ty {
            Type::Function(f) => std::slice::from_ref(f),
            Type::Overloaded(fs) => fs.as_slice(),
            _ => return vec![],
        };
        signatures
            .iter()
            .filter_map(|f| match &f.callable {
                Some(Callable::Declaration {
                    id,
                    kind: DeclarationKind::Function,
                    ..
                }) => Some(id.clone()),
                _ => None,
            })
            .collect()
    }

    /// Elaborates one function body, or records why the kernel cannot unfold it.
    fn load(&mut self, span: DeclarationId) {
        let owner = self
            .method_infos
            .get(&span)
            .map(|method| method.owner.clone())
            .or_else(|| {
                self.local_nodes
                    .get(&span.span)
                    .filter(|node| node.kind == K::Constructor)
                    .and_then(|_| self.signatures.get(&span))
                    .and_then(|(_, _, result)| Self::class_type(result))
                    .map(|(owner, _)| owner.to_owned())
            });
        let previous = std::mem::replace(&mut self.access_owner, owner);
        self.load_inner(span);
        self.access_owner = previous;
    }

    fn load_inner(&mut self, span: DeclarationId) {
        self.functions.remove(&span);
        self.opaque.remove(&span);
        let Some(node) = self.local_nodes.get(&span.span).copied() else {
            return;
        };
        if node.kind == K::Constructor {
            self.load_constructor(span.clone(), node);
            if let Some(reason) = self.opaque.get_mut(&span) {
                let (line, column) = self.parsed.source.position(node.span.start);
                *reason = format!("{}:{line}:{column}: {reason}", self.parsed.source.name);
            }
            return;
        }
        let reason = if node.child(K::Effects).is_some() {
            Some("it does outside work (with Effects)")
        } else if node.child(K::Ensures).is_some() {
            Some("its postcondition has not been proved by the kernel")
        } else if self.method_infos.contains_key(&span)
            && !self.method_infos[&span].is_static
            && node
                .children
                .iter()
                .any(|child| child.kind == K::Modifier && self.text(child) == "mut")
        {
            Some("it mutates its receiver")
        } else if node
            .children
            .iter()
            .any(|n| n.kind == K::Requires && !self.checked.theorem_requirements.contains(&n.span))
            && {
                let name = self.signatures[&span].0.as_str();
                let declared_result = self
                    .signature_entry(name, span.span)
                    .map(|signature| *signature.result)
                    .or_else(|| {
                        let info = self.method_infos.get(&span)?;
                        let definition = self.checked.environment.definitions.get(&info.owner)?;
                        definition.methods.get(&info.name)?.iter().find_map(|signature| {
                            matches!(&signature.callable, Some(Callable::Declaration { id, .. }) if id == &span)
                                .then(|| *signature.result.clone())
                        })
                    })
                    .unwrap_or_else(|| self.signature_result(name, span.span));
                self.success_type(&declared_result).is_none()
            }
        {
            Some("its preconditions guard a result that is not one value or Error")
        } else if node.child(K::ParameterList).is_some_and(|ps| {
            ps.children.iter().any(|p| {
                p.children
                    .iter()
                    .any(|c| c.kind == K::Modifier && self.text(c) == "mut")
            })
        }) {
            // A move parameter consumes the caller's value; nothing the caller
            // still holds can change, so the function stays a pure function.
            Some("a mut parameter can change the caller's value")
        } else {
            None
        };
        if let Some(reason) = reason {
            self.opaque.insert(span, reason.into());
            return;
        }
        let (_, params, _) = self.signatures[&span].clone();
        let mut scope = Scope::default();
        // An unbounded type parameter is still a type the body may quantify
        // over, so register every declared parameter, not only the bounded ones.
        if let Some(parameters) = node.child(K::TypeParameters) {
            for parameter in &parameters.children {
                let Some(name) = parameter.child(K::Name) else {
                    continue;
                };
                let name = self.text(name).to_owned();
                scope.bounds.entry(name).or_default();
            }
        }
        if let Some(constraints) = self.constraints.get(&span) {
            for (subject, bound) in constraints {
                if let Type::Parameter(name) | Type::Rigid(name) = subject {
                    scope
                        .bounds
                        .entry(name.clone())
                        .or_default()
                        .push(bound.clone());
                }
            }
        }
        for (name, ty) in &params {
            let value = if name == "this" {
                Term::Var(name.clone())
            } else if self.plain_class(ty).is_ok() {
                self.symbolic_value(name, ty)
                    .unwrap_or_else(|| Term::Var(name.clone()))
            } else {
                Term::Var(name.clone())
            };
            scope = scope.with(name, value, ty.clone());
        }
        let mut definition_params = params
            .iter()
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();
        for (name, ty) in &params {
            if self.plain_class(ty).is_ok() {
                for (projected, projected_type, _) in self.projection_specs(ty, name) {
                    scope = scope.with(&projected, Term::Var(projected.clone()), projected_type);
                    definition_params.push(projected);
                }
            }
        }
        let mut requires = vec![];
        let mut preconditions = vec![];
        for requirement in node.children.iter().filter(|n| n.kind == K::Requires) {
            if !self
                .checked
                .theorem_requirements
                .contains(&requirement.span)
            {
                match self.prop(&requirement.children[0], &scope) {
                    Ok(prop) => preconditions.push(prop),
                    Err(error) => {
                        self.opaque.insert(
                            span,
                            format!(
                                "its precondition uses a form the kernel cannot read ({})",
                                error.message
                            ),
                        );
                        return;
                    }
                }
                continue;
            }
            match self.required_theorem(requirement, &scope) {
                Ok(required) => requires.push(required),
                Err(reason) => {
                    self.opaque.insert(span, reason);
                    return;
                }
            }
        }
        let parameter_end = node
            .child(K::ParameterList)
            .map(|n| n.span.end)
            .unwrap_or(0);
        let Some(body) = node.children.iter().rev().find(|n| {
            n.span.start >= parameter_end
                && !matches!(
                    n.kind,
                    K::ReturnType | K::Effects | K::Requires | K::Ensures | K::Modifier
                )
        }) else {
            self.opaque.insert(span, "it has no body".into());
            return;
        };
        let result = self.signatures[&span].2.clone();
        match self.term(body, Some(&result), &scope) {
            Ok((body, _)) if !small_term(&body) => {
                self.opaque.insert(
                    span,
                    "its expanded body exceeds the proof term limit".into(),
                );
            }
            Ok((body, _)) => match structural(&span, &definition_params, &body) {
                Ok(()) => {
                    self.functions.insert(
                        span,
                        Definition {
                            params: definition_params,
                            body,
                            requires,
                            preconditions,
                        },
                    );
                }
                Err(reason) => {
                    self.opaque.insert(span, reason);
                }
            },
            Err(error) => {
                self.opaque.insert(
                    span,
                    format!(
                        "its body uses a form the kernel cannot read ({})",
                        error.message
                    ),
                );
            }
        }
    }

    fn load_constructor(&mut self, span: DeclarationId, node: &Node) {
        let Some((_class_name, params, result)) = self.signatures.get(&span).cloned() else {
            self.opaque
                .insert(span, "it has no checked signature".into());
            return;
        };
        let Some((owner, type_args)) = Self::class_type(&result) else {
            self.opaque
                .insert(span, "its result is not a class value".into());
            return;
        };
        let Some(class) = self.checked.environment.definitions.get(owner) else {
            self.opaque
                .insert(span, "its class definition is unavailable".into());
            return;
        };
        if node.child(K::Effects).is_some()
            || node.child(K::Ensures).is_some()
            || node.children.iter().any(|child| child.kind == K::Requires)
            || node.child(K::ParameterList).is_some_and(|parameters| {
                parameters.children.iter().any(|parameter| {
                    parameter
                        .children
                        .iter()
                        .any(|child| child.kind == K::Modifier && self.text(child) == "mut")
                })
            })
        {
            self.opaque.insert(
                span,
                "it has effects, mutable parameters or unproved conditions".into(),
            );
            return;
        }
        let mut scope = Scope::default();
        let mut definition_params = vec![];
        for (name, ty) in &params {
            let value = if self.plain_class(ty).is_ok() {
                self.symbolic_value(name, ty)
                    .unwrap_or_else(|| Term::Var(name.clone()))
            } else {
                Term::Var(name.clone())
            };
            scope = scope.with(name, value, ty.clone());
            definition_params.push(name.clone());
        }
        for (name, ty) in &params {
            if self.plain_class(ty).is_ok() {
                for (projected, projected_type, _) in self.projection_specs(ty, name) {
                    scope = scope.with(&projected, Term::Var(projected.clone()), projected_type);
                    definition_params.push(projected);
                }
            }
        }
        let substitutions = class
            .parameters
            .iter()
            .cloned()
            .zip(type_args.iter().cloned())
            .collect::<BTreeMap<_, _>>();
        let receiver = result.clone();
        scope = scope.with("this", Term::Var("this".into()), receiver.clone());
        let Some(body) = node.child(K::Block) else {
            self.opaque.insert(span, "it has no body".into());
            return;
        };
        let mut assigned = BTreeMap::<String, Term>::new();
        let mut evaluated = vec![];
        if let Ok(own_fields) = self.own_class_fields(&receiver) {
            for (field, field_type) in own_fields {
                match self.class_initializer(&receiver, &field, &scope) {
                    Ok(Some(value)) => {
                        evaluated.push(value.clone());
                        assigned.insert(field.clone(), value.clone());
                        scope = scope.with(&format!("this.{field}"), value, field_type);
                    }
                    Ok(None) => {}
                    Err(error) => {
                        self.opaque.insert(
                            span,
                            format!(
                                "field `{field}` has an unreadable initializer ({})",
                                error.message
                            ),
                        );
                        return;
                    }
                }
            }
        }
        let mut parent_value = None;
        for (index, statement) in body.children.iter().enumerate() {
            if statement.kind == K::Call
                && statement
                    .children
                    .first()
                    .is_some_and(|callee| callee.kind == K::Name && self.text(callee) == "super")
            {
                if index != 0 || parent_value.is_some() {
                    self.opaque.insert(
                        span,
                        "its parent constructor call is not the first statement".into(),
                    );
                    return;
                }
                let Some(parent) = class
                    .parent
                    .as_ref()
                    .map(|ty| ty.replace_parameters(&substitutions))
                else {
                    self.opaque
                        .insert(span, "it calls `super` without a parent class".into());
                    return;
                };
                match self.call(statement, Some(&parent), &scope) {
                    Ok((term, _)) => parent_value = Some(term),
                    Err(error) => {
                        self.opaque.insert(
                            span,
                            format!("its parent constructor is unreadable ({})", error.message),
                        );
                        return;
                    }
                }
                continue;
            }
            if statement.kind == K::Binding && statement.children.len() >= 2 {
                let pattern = &statement.children[0];
                if pattern.kind != K::PatternName
                    || statement
                        .children
                        .iter()
                        .any(|child| child.kind == K::Modifier && self.text(child) != "final")
                {
                    self.opaque
                        .insert(span, "it uses a non-immutable local binding".into());
                    return;
                }
                let name = self.text(pattern).to_owned();
                let declared = self.checked.binding_types.get(&pattern.span).cloned();
                match self.term(
                    statement.children.last().unwrap(),
                    declared.as_ref(),
                    &scope,
                ) {
                    Ok((value, ty)) => {
                        evaluated.push(value.clone());
                        scope = scope.with(&name, value, ty);
                        continue;
                    }
                    Err(error) => {
                        self.opaque.insert(
                            span,
                            format!("a local binding is unreadable ({})", error.message),
                        );
                        return;
                    }
                }
            }
            if statement.kind == K::FieldDeclaration {
                let Some(member) = statement.child(K::Member) else {
                    self.opaque
                        .insert(span, "it declares an unreadable constructor field".into());
                    return;
                };
                let Some(field_node) = member.children.get(1) else {
                    return;
                };
                let field = self.text(field_node).to_owned();
                let Some((_, field_type)) = self
                    .own_class_fields(&receiver)
                    .ok()
                    .and_then(|fs| fs.into_iter().find(|(name, _)| name == &field))
                else {
                    self.opaque
                        .insert(span, format!("field `{field}` has no checked type"));
                    return;
                };
                let Some(value_node) = statement.children.last() else {
                    self.opaque
                        .insert(span, format!("field `{field}` has no readable initializer"));
                    return;
                };
                match self.term(value_node, Some(&field_type), &scope) {
                    Ok((value, _)) => {
                        evaluated.push(value.clone());
                        assigned.insert(field.clone(), value.clone());
                        scope = scope.with(&format!("this.{field}"), value, field_type);
                        continue;
                    }
                    Err(error) => {
                        self.opaque.insert(
                            span,
                            format!(
                                "field `{field}` has an unreadable initializer ({})",
                                error.message
                            ),
                        );
                        return;
                    }
                }
            }
            if statement.kind == K::Assignment && statement.children.len() == 2 {
                let lhs = &statement.children[0];
                if lhs.kind != K::Member
                    || lhs.children.len() != 2
                    || lhs.children[0].kind != K::Name
                    || self.text(&lhs.children[0]) != "this"
                {
                    self.opaque.insert(
                        span,
                        "it assigns something other than one of its fields".into(),
                    );
                    return;
                }
                let field = self.text(&lhs.children[1]).to_owned();
                let Some((_, field_type)) = self
                    .own_class_fields(&receiver)
                    .ok()
                    .and_then(|fs| fs.into_iter().find(|(name, _)| name == &field))
                else {
                    self.opaque
                        .insert(span, format!("field `{field}` has no checked type"));
                    return;
                };
                match self.term(&statement.children[1], Some(&field_type), &scope) {
                    Ok((value, _)) => {
                        evaluated.push(value.clone());
                        assigned.insert(field.clone(), value.clone());
                        scope = scope.with(&format!("this.{field}"), value, field_type);
                        continue;
                    }
                    Err(error) => {
                        self.opaque.insert(
                            span,
                            format!(
                                "field `{field}` has an unreadable assignment ({})",
                                error.message
                            ),
                        );
                        return;
                    }
                }
            }
            self.opaque.insert(
                span,
                format!(
                    "statement `{}` is not a readable field assignment",
                    self.text(statement)
                ),
            );
            return;
        }
        let parent_type = class
            .parent
            .as_ref()
            .map(|parent| parent.replace_parameters(&substitutions))
            .filter(|parent| *parent != Type::named("Object"));
        let parent = match (parent_type, parent_value) {
            (Some(_), Some(value)) => Some(value),
            (Some(parent), None) => {
                if Self::class_type(&parent).is_none() {
                    self.opaque
                        .insert(span, "its parent type is unreadable".into());
                    return;
                }
                match self.default_instance(&parent, &scope, &mut BTreeSet::new()) {
                    Ok(Some(value)) => Some(value),
                    Ok(None) => {
                        self.opaque.insert(
                            span,
                            "it does not pass readable arguments to `super`".into(),
                        );
                        return;
                    }
                    Err(error) => {
                        self.opaque.insert(
                            span,
                            format!(
                                "its parent's default fields are unreadable ({})",
                                error.message
                            ),
                        );
                        return;
                    }
                }
            }
            (None, value) => value,
        };
        let own_fields = match self.own_class_fields(&receiver) {
            Ok(fields) => fields,
            Err(reason) => {
                self.opaque.insert(span, reason);
                return;
            }
        };
        let mut values = Vec::with_capacity(own_fields.len() + usize::from(parent.is_some()));
        if let Some(parent) = parent {
            values.push(parent);
        }
        for (field, field_type) in own_fields {
            let Some(value) = assigned.get(&field).cloned() else {
                self.opaque.insert(
                    span,
                    format!("field `{field}` is not initialized by this constructor"),
                );
                return;
            };
            if !self.readable(&field_type) {
                self.opaque.insert(
                    span,
                    format!("field `{field}` has unreadable type {field_type}"),
                );
                return;
            }
            values.push(value);
        }
        let mut body = self.ctor(owner, "new", values);
        // Constructors evaluate each statement in order, even if a later write
        // makes an earlier value unobservable in the final field layout.
        for value in evaluated.into_iter().rev() {
            body = Term::Op("seq".into(), vec![value, body]);
        }
        if !small_term(&body) {
            self.opaque.insert(
                span,
                "its expanded body exceeds the proof term limit".into(),
            );
            return;
        }
        match structural(&span, &definition_params, &body) {
            Ok(()) => {
                self.functions.insert(
                    span,
                    Definition {
                        params: definition_params,
                        body,
                        requires: vec![],
                        preconditions: vec![],
                    },
                );
            }
            Err(reason) => {
                self.opaque.insert(span, reason);
            }
        }
    }

    /// The value type of `T or Error`, when exactly one member is not an error.
    fn success_type(&self, ty: &Type) -> Option<Type> {
        let Type::Union(members) = ty else {
            return None;
        };
        let error = |t: &Type| match t {
            Type::Named(id, _) => {
                id == "Error"
                    || self
                        .checked
                        .environment
                        .definitions
                        .get(id)
                        .is_some_and(|d| d.parent == Some(Type::named("Error")))
            }
            _ => false,
        };
        let values = members.iter().filter(|m| !error(m)).collect::<Vec<_>>();
        match values.as_slice() {
            [value] if values.len() < members.len() => Some((*value).clone()),
            _ => None,
        }
    }

    /// A function's `requires theorem(args)`: the theorem's declaration and the
    /// arguments as terms over the function's parameters.
    /// Whether a theorem's proof mentions `sorry`, cites an imported admitted
    /// theorem, or cites a local theorem that does. Function requirements run
    /// before local theorems are checked, so this conservative scan keeps a
    /// sketch from removing runtime checks.
    fn proof_depends_on_sorry(&self, theorem: &Node) -> bool {
        let locals = self
            .parsed
            .root
            .children
            .iter()
            .filter(|node| node.kind == K::Theorem)
            .filter_map(|node| {
                node.child(K::Name)
                    .map(|name| (self.text(name).to_owned(), node))
            })
            .collect::<BTreeMap<_, _>>();
        fn visit(
            node: &Node,
            text: &str,
            admitted: &BTreeMap<String, Theorem>,
            locals: &BTreeMap<String, &Node>,
            visited: &mut BTreeSet<String>,
        ) -> bool {
            if node.kind == K::Name && text.get(node.span.range()) == Some("sorry") {
                return true;
            }
            if matches!(node.kind, K::Name | K::Member | K::Call) {
                let name = if node.kind == K::Call {
                    node.children
                        .first()
                        .map(|callee| crate::lexer::path_name(&text[callee.span.range()]))
                } else {
                    Some(crate::lexer::path_name(&text[node.span.range()]))
                };
                if let Some(name) = name {
                    if admitted.get(&name).is_some_and(|theorem| theorem.admitted) {
                        return true;
                    }
                    if let Some(local) = locals.get(&name)
                        && visited.insert(name.clone())
                        && visit(local, text, admitted, locals, visited)
                    {
                        return true;
                    }
                }
            }
            node.children
                .iter()
                .any(|child| visit(child, text, admitted, locals, visited))
        }
        visit(
            theorem,
            &self.parsed.source.text,
            &self.theorems,
            &locals,
            &mut BTreeSet::new(),
        )
    }

    fn required_theorem(
        &mut self,
        requirement: &Node,
        scope: &Scope,
    ) -> Result<(DeclarationId, Vec<Term>), String> {
        let call = &requirement.children[0];
        let name = crate::lexer::path_name(self.text(&call.children[0]));
        if self
            .theorems
            .get(&name)
            .is_some_and(|theorem| theorem.admitted)
        {
            return Err(format!(
                "`{name}` uses `sorry`, so a function cannot require it."
            ));
        }
        let local = self.parsed.root.children.iter().find(|n| {
            n.kind == K::Theorem && n.child(K::Name).is_some_and(|c| self.text(c) == name)
        });
        if let Some(theorem) = local
            && self.proof_depends_on_sorry(theorem)
        {
            return Err(format!(
                "`{name}` uses `sorry`, so a function cannot require it."
            ));
        }
        let (id, types, constraints) = if let Some(theorem) = local {
            let types = theorem
                .child(K::ParameterList)
                .map(|p| p.children.as_slice())
                .unwrap_or(&[])
                .iter()
                .map(|p| {
                    self.checked
                        .binding_types
                        .get(&p.span)
                        .cloned()
                        .unwrap_or(Type::Invalid)
                })
                .collect::<Vec<_>>();
            let id = DeclarationId {
                file: self.parsed.source.name.clone(),
                span: theorem.span,
            };
            let bounds = self
                .checked
                .theorem_bounds
                .get(&theorem.span)
                .cloned()
                .unwrap_or_default();
            (id, types, Self::bounds_constraints(&bounds))
        } else if let Some(id) = self.checked.environment.theorems.get(&name)
            && let Some(exported) = self.exported.get(id)
        {
            let types = exported.theorem.params.iter().map(|(_, t)| t.clone());
            (
                id.clone(),
                types.collect(),
                Self::theorem_constraints(&exported.theorem),
            )
        } else {
            return Err(format!(
                "it requires `{name}`, which is not a checked theorem"
            ));
        };
        let args = self.arguments(call);
        if args.len() != types.len() {
            return Err(format!(
                "its requirement passes the wrong number of arguments to `{name}`"
            ));
        }
        let mut terms = vec![];
        let mut all_types = types.iter().collect::<Vec<_>>();
        for (ty, bound) in &constraints {
            all_types.extend([ty, bound]);
        }
        let variables = self.holes_for(&all_types);
        let mut holes = Holes::new();
        for (arg, ty) in args.iter().zip(&types) {
            let want = generic::resolve(&instantiate(ty, &variables), &holes);
            match self.term(arg, Some(&want), scope) {
                Ok((term, found)) if unify(&want, &found, &mut holes) => terms.push(term),
                Ok(_) => {
                    return Err(format!(
                        "its requirement passes the wrong type of argument to `{name}`"
                    ));
                }
                Err(error) => {
                    return Err(format!(
                        "its requirement uses a form the kernel cannot read ({})",
                        error.message
                    ));
                }
            }
        }
        self.check_bounds(&constraints, &variables, &holes, scope, requirement.span)
            .map_err(|error| error.message)?;
        Ok((id, terms))
    }

    /// Functions that call each other in a cycle cannot be unfolded (first version).
    fn reject_mutual_recursion(&mut self) {
        let calls = self
            .functions
            .iter()
            .map(|(span, d)| {
                let mut out = BTreeSet::new();
                callees(&d.body, &mut out);
                out.remove(span);
                (span.clone(), out)
            })
            .collect::<BTreeMap<_, _>>();
        let mut cyclic = vec![];
        for start in calls.keys() {
            let mut stack = calls[start].iter().cloned().collect::<Vec<_>>();
            let mut seen = BTreeSet::new();
            while let Some(next) = stack.pop() {
                if next == *start {
                    cyclic.push(start.clone());
                    break;
                }
                if seen.insert(next.clone())
                    && let Some(more) = calls.get(&next)
                {
                    stack.extend(more.iter().cloned());
                }
            }
        }
        for span in cyclic {
            self.functions.remove(&span);
            self.opaque
                .insert(span, "it calls itself through another function".into());
        }
    }

    /// A transparent wrapper cannot hide a nonterminating or unreadable callee.
    fn reject_opaque_dependencies(&mut self) {
        fn opaque(term: &Term) -> Option<String> {
            match term {
                Term::Opaque { name, .. } => Some(name.clone()),
                Term::Call { args, .. } | Term::Ctor { args, .. } | Term::Op(_, args) => {
                    args.iter().find_map(opaque)
                }
                Term::If(a, b, c) => [a.as_ref(), b.as_ref(), c.as_ref()]
                    .into_iter()
                    .find_map(opaque),
                Term::Match(value, arms) => {
                    opaque(value).or_else(|| arms.iter().find_map(|(_, body)| opaque(body)))
                }
                _ => None,
            }
        }
        loop {
            let broken = self
                .functions
                .iter()
                .filter_map(|(span, definition)| {
                    let mut calls = BTreeSet::new();
                    callees(&definition.body, &mut calls);
                    let missing = calls
                        .iter()
                        .find(|callee| {
                            !self.functions.contains_key(callee)
                                && !self.checked.environment.definitions.values().any(|definition| {
                                    definition.kind == K::Trait
                                        && !definition.default_methods.contains(&callee.span)
                                        && definition.methods.values().flatten().any(|method| {
                                            matches!(&method.callable, Some(Callable::Declaration { id, kind: DeclarationKind::TraitMethod, .. }) if id == *callee)
                                        })
                                })
                        });
                    let reason = opaque(&definition.body).or_else(|| {
                        missing.map(|callee| {
                            self.signatures
                                .get(callee)
                                .map(|signature| signature.0.clone())
                                .unwrap_or_else(|| "an unavailable function".into())
                        })
                    });
                    reason.map(|reason| (span.clone(), reason))
                })
                .collect::<Vec<_>>();
            if broken.is_empty() {
                break;
            }
            for (span, name) in broken {
                self.functions.remove(&span);
                self.opaque.insert(
                    span,
                    format!("it depends on `{name}`, which the kernel cannot unfold"),
                );
            }
        }
    }

    /// The enum that owns a case, the case's field types and the type it builds.
    /// A generic enum's parameters come from the expected type, or are holes.
    fn case_owner(
        &mut self,
        case: &str,
        expected: Option<&Type>,
    ) -> Option<(String, Vec<Type>, Type)> {
        let checked = self.checked;
        let qualified = crate::lexer::path_name(case);
        let (owner, case) = match qualified.rsplit_once('.') {
            Some((owner, case)) => (Some(checked.environment.names.get(owner)?.as_str()), case),
            None => (None, qualified.as_str()),
        };
        let wanted = expected.and_then(|t| match t {
            Type::Named(id, _) => Some(id.as_str()),
            _ => None,
        });
        let mut found = vec![];
        // Option constructors are built-ins, not declarations in the enum map.
        if owner.is_none_or(|owner| owner == "Option") && matches!(case, "Some" | "None") {
            found.push((
                "Option".into(),
                if case == "Some" {
                    vec![Type::Parameter("T".into())]
                } else {
                    vec![]
                },
                vec!["T".into()],
            ));
        }
        for (id, definition) in &checked.environment.definitions {
            if !checked
                .environment
                .names
                .values()
                .any(|visible| visible == id)
            {
                continue;
            }
            if owner.is_some_and(|owner| owner != id) {
                continue;
            }
            if definition.kind != K::Enum {
                continue;
            }
            if let Some(variant) = definition.variants.iter().find(|v| v.name == case) {
                found.push((
                    id.clone(),
                    variant.fields.clone(),
                    definition.parameters.clone(),
                ));
            }
        }
        let (id, fields, parameters) =
            match wanted.and_then(|wanted| found.iter().position(|(id, _, _)| id == wanted)) {
                Some(index) => found.swap_remove(index),
                None if found.len() == 1 => found.remove(0),
                None => return None,
            };
        let arguments = match expected {
            Some(Type::Named(eid, args)) if *eid == id && args.len() == parameters.len() => {
                args.clone()
            }
            _ => parameters
                .iter()
                .map(|_| {
                    self.fresh += 1;
                    Type::Parameter(format!("?{}", self.fresh))
                })
                .collect(),
        };
        let map: Holes = parameters
            .into_iter()
            .zip(arguments.iter().cloned())
            .collect();
        let fields = fields.iter().map(|f| instantiate(f, &map)).collect();
        Some((id.clone(), fields, Type::Named(id, arguments)))
    }

    fn arguments<'n>(&self, call: &'n Node) -> Vec<&'n Node> {
        call.children
            .iter()
            .skip(1)
            .map(|a| {
                if a.kind == K::Argument {
                    a.children.last().unwrap_or(a)
                } else {
                    a
                }
            })
            .collect()
    }

    /// `{name : U | P}`. The binder has type `U` inside the predicate, and the
    /// whole term is a set of `U`.
    fn set_builder(
        &mut self,
        node: &Node,
        _expected: Option<&Type>,
        scope: &Scope,
    ) -> Found<(Term, Type)> {
        let name = self.text(&node.children[0]).to_owned();
        let element = self.quantified_type(&node.children[1], scope)?;
        let inner = scope.with(&name, Term::Var(name.clone()), element.clone());
        let body = self.prop(&node.children[2], &inner)?;
        let ty = Type::applied("Set", vec![element.clone()]);
        Ok((
            Term::Builder {
                name,
                element,
                body: Box::new(body),
            },
            ty,
        ))
    }

    /// `{a, b}`, `{a}` or `∅`. The elements supply the universe; the empty set
    /// takes it from the expected type.
    fn set_literal(
        &mut self,
        node: &Node,
        expected: Option<&Type>,
        scope: &Scope,
    ) -> Found<(Term, Type)> {
        let expected_element = expected.and_then(set_element_type);
        let Some((first, rest)) = node.children.split_first() else {
            let Some(element) = expected_element else {
                return fail(
                    node.span,
                    "E_PROOF_TYPE",
                    "The empty set needs a type, and nothing here supplies one.",
                    "Write ∅ where the expected type is Set[U], such as x ∈ ∅.",
                );
            };
            return Ok((Term::Finite(vec![]), Type::applied("Set", vec![element])));
        };
        let (first, element_type) = self.term(first, expected_element.as_ref(), scope)?;
        let mut elements = vec![first];
        for element in rest {
            elements.push(self.term(element, Some(&element_type), scope)?.0);
        }
        let ty = Type::applied("Set", vec![element_type]);
        Ok((Term::Finite(elements), ty))
    }

    /// Elaborates an expression into a term and its type.
    fn term(&mut self, node: &Node, expected: Option<&Type>, scope: &Scope) -> Found<(Term, Type)> {
        // A `Prop` parameter is instantiated at a proposition, which may be
        // compound: quote the proposition into a term.
        if matches!(expected, Some(Type::Named(id, args)) if id == "Prop" && args.is_empty()) {
            let prop = self.prop(node, scope)?;
            return Ok(match prop {
                Prop::Atom(term) => (term, Type::named("Prop")),
                other => (Term::Prop(Box::new(other)), Type::named("Prop")),
            });
        }
        let (term, ty) = self.elaborate_term(node, expected, scope)?;
        if let Some(expected) = expected
            && *expected != Type::Invalid
            && !compatible(expected, &ty)
        {
            return fail(
                node.span,
                "E_PROOF_TYPE",
                format!("This term needs {expected}, but has {ty}."),
                "Use a term of the required type.",
            );
        }
        Ok((term, ty))
    }

    fn elaborate_term(
        &mut self,
        node: &Node,
        expected: Option<&Type>,
        scope: &Scope,
    ) -> Found<(Term, Type)> {
        match node.kind {
            K::Group => self.term(&node.children[0], expected, scope),
            K::List => self.list_literal(node, expected, scope),
            K::SetBuilder => self.set_builder(node, expected, scope),
            K::SetLiteral => self.set_literal(node, expected, scope),
            K::Literal => {
                let text = self.text(node).replace('_', "");
                match text.as_str() {
                    "true" => Ok((Term::Bool(true), Type::named("Bool"))),
                    "false" => Ok((Term::Bool(false), Type::named("Bool"))),
                    _ if text.starts_with('"') || crate::lexer::is_double_literal(&text) => fail(
                        node.span,
                        "E_PROOF_UNSUPPORTED",
                        "The kernel reasons about Nat, Int, Bool and enums; this literal is none of them.",
                        "State the theorem about numbers, truth values or enum cases.",
                    ),
                    _ if expected == Some(&nat()) => match text.parse::<u64>() {
                        Ok(n) => Ok((Term::Nat(n), nat())),
                        Err(_) => fail(
                            node.span,
                            "E_PROOF_UNSUPPORTED",
                            "This Nat numeral is too large.",
                            "Use a numeral below 2^64.",
                        ),
                    },
                    _ => match text.parse::<i64>() {
                        Ok(n) => Ok((Term::Int(n), Type::named("Int"))),
                        Err(_) => fail(
                            node.span,
                            "E_PROOF_UNSUPPORTED",
                            "This Int literal is out of range.",
                            "Use a literal that fits a 64-bit Int.",
                        ),
                    },
                }
            }
            K::Name => {
                let name = self.text(node);
                if let Some((_, term, ty)) = scope.get(name) {
                    return Ok((term.clone(), ty.clone()));
                }
                if let Some((ty, fields, result)) = self.case_owner(name, expected)
                    && fields.is_empty()
                {
                    return Ok((self.ctor(&ty, name, vec![]), result));
                }
                fail(
                    node.span,
                    "E_PROOF_UNKNOWN",
                    format!("`{name}` is not a parameter, a bound name or an enum case here."),
                    "Theorems mention their parameters, enum cases and pure functions.",
                )
            }
            K::Member if node.children.len() == 2 => {
                let case = self.text(&node.children[1]);
                let qualified = crate::lexer::path_name(self.text(node));
                let shadowed = qualified
                    .split('.')
                    .next()
                    .is_some_and(|root| scope.get(root).is_some());
                let base_name = self.text(&node.children[0]);
                if let Ok((base_term, base_type)) = self.term(&node.children[0], None, scope)
                    && Self::class_type(&base_type).is_some()
                {
                    if let Some((owner, _)) = self.class_field_visibility(&base_type, case)
                        && !self.proof_can_access(&owner, case)
                    {
                        return fail(
                            node.span,
                            "E_MEMBER_PRIVATE",
                            format!("Field `{case}` is not visible here."),
                            "Use a public field, or package visibility within the same folder.",
                        );
                    }
                    if let Err(reason) = self.plain_class(&base_type) {
                        return fail(
                            node.span,
                            "E_PROOF_UNSUPPORTED",
                            format!("The kernel cannot read field `{case}` because {reason}."),
                            "Use a class whose fields and constructors are readable and immutable.",
                        );
                    }
                    if let Some((_, projected, field_type)) =
                        scope.get(&format!("{base_name}.{case}"))
                    {
                        return Ok((projected.clone(), field_type.clone()));
                    }
                    return self
                        .project_value(base_term, &base_type, case)
                        .map_err(|reason| {
                            Diagnostic::new(
                                "E_PROOF_UNSUPPORTED",
                                format!("The kernel cannot read this member access: {reason}."),
                                node.span,
                            )
                            .with_hint("Use a declared field of a readable class or record.")
                        });
                }
                match (!shadowed)
                    .then(|| self.case_owner(&qualified, expected))
                    .flatten()
                {
                    Some((ty, fields, result)) if fields.is_empty() => {
                        Ok((self.ctor(&ty, case, vec![]), result))
                    }
                    _ => fail(
                        node.span,
                        "E_PROOF_UNSUPPORTED",
                        "The kernel cannot read this member access.",
                        "Use enum cases and pure functions.",
                    ),
                }
            }
            K::Lambda => self.lambda(node, expected, scope),
            K::Call => {
                // Applying a function-valued parameter is a term application,
                // not a call to a named function.
                let callee = &node.children[0];
                if callee.kind == K::Name {
                    let callee_name = self.text(callee);
                    if let Some((_, value, ty)) = scope.get(callee_name)
                        && let Type::Function(function) = ty
                    {
                        let args = self.arguments(node);
                        if args.len() != function.parameters.len() {
                            return fail(
                                node.span,
                                "E_PROOF_SHAPE",
                                format!(
                                    "`{callee_name}` takes {} argument(s), but {} were given.",
                                    function.parameters.len(),
                                    args.len()
                                ),
                                "Pass one argument for each function parameter.",
                            );
                        }
                        let mut terms = vec![];
                        for (arg, parameter) in args.iter().zip(&function.parameters) {
                            let (term, found) = self.term(arg, Some(&parameter.ty), scope)?;
                            if !compatible(&parameter.ty, &found) {
                                return fail(
                                    arg.span,
                                    "E_PROOF_TYPE",
                                    format!(
                                        "This argument is {found}, but the function needs {}.",
                                        parameter.ty
                                    ),
                                    "Pass an argument of the declared type.",
                                );
                            }
                            terms.push(term);
                        }
                        return Ok((
                            Term::App(Box::new(value.clone()), terms),
                            function.result.as_ref().clone(),
                        ));
                    }
                }
                // A callee that is itself an expression of function type:
                // `f(n)(m)` or `makeAdder(n)(m)`.
                if !matches!(callee.kind, K::Name | K::Member)
                    && let Ok((function, Type::Function(signature))) =
                        self.term(callee, None, scope)
                {
                    let args = self.arguments(node);
                    if args.len() == signature.parameters.len() {
                        let mut terms = vec![];
                        for (arg, parameter) in args.iter().zip(&signature.parameters) {
                            let (term, found) = self.term(arg, Some(&parameter.ty), scope)?;
                            if !compatible(&parameter.ty, &found) {
                                return fail(
                                    arg.span,
                                    "E_PROOF_TYPE",
                                    format!(
                                        "This argument is {found}, but the function needs {}.",
                                        parameter.ty
                                    ),
                                    "Pass an argument of the declared type.",
                                );
                            }
                            terms.push(term);
                        }
                        return Ok((
                            Term::App(Box::new(function), terms),
                            signature.result.as_ref().clone(),
                        ));
                    }
                }
                self.call(node, expected, scope)
            }
            K::Unary => {
                let op = canonical_operator(self.text(&node.children[0])).to_owned();
                if matches!(op.as_str(), "⋃₀" | "⋂₀") {
                    let operand_expected = expected.and_then(set_element_type).map(|element| {
                        Type::applied("Set", vec![Type::applied("Set", vec![element])])
                    });
                    let (family, family_type) =
                        self.term(&node.children[1], operand_expected.as_ref(), scope)?;
                    let Some(member_type) = set_element_type(&family_type) else {
                        return fail(
                            node.span,
                            "E_PROOF_TYPE",
                            format!("`{op}` needs a family of sets, but this is {family_type}."),
                            "Write ⋃₀ F or ⋂₀ F where F has type Set[Set[U]].",
                        );
                    };
                    let Some(element) = set_element_type(&member_type) else {
                        return fail(
                            node.span,
                            "E_PROOF_TYPE",
                            format!("`{op}` needs a family of sets, but this is {family_type}."),
                            "Write ⋃₀ F or ⋂₀ F where F has type Set[Set[U]].",
                        );
                    };
                    let term = if op == "⋃₀" {
                        Term::FamilyUnion {
                            set: Box::new(family),
                            element: element.clone(),
                        }
                    } else {
                        Term::FamilyInter {
                            set: Box::new(family),
                            element: element.clone(),
                        }
                    };
                    return Ok((term, Type::applied("Set", vec![element])));
                }
                if op == "ᶜ" {
                    let (value, ty) = self.term(&node.children[1], expected, scope)?;
                    if set_element_type(&ty).is_none() {
                        return fail(
                            node.span,
                            "E_PROOF_TYPE",
                            format!("The complement needs a set, but this is {ty}."),
                            "Write Aᶜ where A has type Set[U].",
                        );
                    }
                    return Ok((Term::Op("ᶜ".into(), vec![value]), ty));
                }
                if op == "-"
                    && node.children[1].kind == K::Literal
                    && self
                        .text(&node.children[1])
                        .replace('_', "")
                        .parse::<u64>()
                        .ok()
                        == Some(1_u64 << 63)
                {
                    return Ok((Term::Int(i64::MIN), Type::named("Int")));
                }
                let (value, ty) = self.term(&node.children[1], expected, scope)?;
                if op == "-" && ty == Type::named("Int") || op == "!" && ty == Type::named("Bool") {
                    Ok((Term::Op(op, vec![value]), ty))
                } else {
                    fail(
                        node.span,
                        "E_PROOF_UNSUPPORTED",
                        format!("The kernel does not compute `{op}` on {ty}."),
                        "Use - on Int or ! on Bool.",
                    )
                }
            }
            K::Binary => {
                let op = canonical_operator(self.text(&node.children[1])).to_owned();
                if matches!(op.as_str(), "=" | "and" | "or") {
                    return fail(
                        node.span,
                        "E_PROOF_STATEMENT",
                        format!("`{op}` belongs in a statement, not inside a term."),
                        "Write equalities and connectives at the statement level.",
                    );
                }
                if matches!(op.as_str(), "∪" | "∩") {
                    // `∅ ∪ A` has no type on its own, so the non-empty side is
                    // read first; either side can supply the universe.
                    let left_node = &node.children[0];
                    let right_node = &node.children[2];
                    let (left, left_type, right, right_type) = if left_node.kind == K::SetLiteral
                        && right_node.kind != K::SetLiteral
                    {
                        let right = self.term(right_node, None, scope)?;
                        let left = self.term(left_node, Some(&right.1), scope)?;
                        (left.0, left.1, right.0, right.1)
                    } else if let Some(want) = expected.filter(|ty| set_element_type(ty).is_some())
                    {
                        let left = self.term(left_node, Some(want), scope)?;
                        let right = self.term(right_node, Some(&left.1), scope)?;
                        (left.0, left.1, right.0, right.1)
                    } else {
                        let left = self.term(left_node, None, scope)?;
                        let right = self.term(right_node, Some(&left.1), scope)?;
                        (left.0, left.1, right.0, right.1)
                    };
                    let Some(_) = set_element_type(&left_type) else {
                        return fail(
                            node.span,
                            "E_PROOF_TYPE",
                            format!("`{op}` needs sets, but the left side is {left_type}."),
                            "Write A ∪ B or A ∩ B with sets A and B.",
                        );
                    };
                    if !compatible(&left_type, &right_type) {
                        return fail(
                            node.span,
                            "E_PROOF_TYPE",
                            format!(
                                "`{op}` combines a set of {left_type} with a set of {right_type}."
                            ),
                            "Use sets over the same universe.",
                        );
                    }
                    return Ok((Term::Op(op, vec![left, right]), left_type));
                }
                let ((left, lt), (right, rt)) =
                    if matches!(node.children[0].kind, K::Literal | K::List)
                        && !matches!(node.children[2].kind, K::Literal | K::List)
                    {
                        let right = self.term(&node.children[2], None, scope)?;
                        (self.term(&node.children[0], Some(&right.1), scope)?, right)
                    } else {
                        let operand_type =
                            if matches!(op.as_str(), "+" | "-" | "*" | "/" | "%" | "**") {
                                expected
                            } else {
                                None
                            };
                        let left = self.term(&node.children[0], operand_type, scope)?;
                        let right = self.term(&node.children[2], Some(&left.1), scope)?;
                        (left, right)
                    };
                if !compatible(&lt, &rt) {
                    return fail(
                        node.span,
                        "E_PROOF_TYPE",
                        format!("`{op}` compares {lt} with {rt}."),
                        "Give both sides the same type.",
                    );
                }
                let ty = match op.as_str() {
                    "+" if sequences::list_item(&lt).is_some() => {
                        return Ok((Term::Op("$list.concat".into(), vec![left, right]), lt));
                    }
                    "+" | "-" | "*" | "/" | "%" | "**" if lt == Type::named("Int") => lt,
                    "+" | "-" | "*" | "/" | "%" if lt == nat() => lt,
                    "<" | "<=" | ">" | ">=" if lt == Type::named("Int") || lt == nat() => {
                        Type::named("Bool")
                    }
                    "==" | "!=" if matches!(lt, Type::Named(ref n, _) if matches!(n.as_str(), "Int" | "Nat" | "Bool")) => {
                        Type::named("Bool")
                    }
                    "&&" | "||" if lt == Type::named("Bool") => lt,
                    _ => {
                        return fail(
                            node.span,
                            "E_PROOF_UNSUPPORTED",
                            format!("The kernel does not compute `{op}` on {lt}."),
                            "Use + - * / % ** on Int, + - * / % on Nat, comparisons on Int or Nat, and && || on Bool.",
                        );
                    }
                };
                Ok((Term::Op(op, vec![left, right]), ty))
            }
            K::If => {
                let (condition, _) =
                    self.term(&node.children[0], Some(&Type::named("Bool")), scope)?;
                let (then, ty) = self.term(&node.children[1], expected, scope)?;
                let Some(other) = node.children.get(2) else {
                    return fail(
                        node.span,
                        "E_PROOF_UNSUPPORTED",
                        "An `if` without `else` has no value to reason about.",
                        "Add an else branch.",
                    );
                };
                let (otherwise, _) = self.term(other, Some(&ty), scope)?;
                Ok((
                    Term::If(Box::new(condition), Box::new(then), Box::new(otherwise)),
                    ty,
                ))
            }
            K::Match => {
                let (scrutinee, sty) = self.term(&node.children[0], None, scope)?;
                let mut arms = vec![];
                let mut ty = expected.cloned();
                for arm in node.children.iter().skip(1) {
                    if arm.child(K::Guard).is_some() {
                        return fail(
                            arm.span,
                            "E_PROOF_UNSUPPORTED",
                            "The kernel cannot read guarded match arms.",
                            "Move the condition into an if expression.",
                        );
                    }
                    let mut inner = scope.clone();
                    let pat = self.pattern(&arm.children[0], &sty, &mut inner)?;
                    let (body, bty) =
                        self.term(arm.children.last().unwrap(), ty.as_ref(), &inner)?;
                    ty.get_or_insert(bty);
                    arms.push((pat, body));
                }
                Ok((
                    Term::Match(Box::new(scrutinee), arms),
                    ty.unwrap_or(Type::Invalid),
                ))
            }
            K::Block => {
                let mut inner = scope.clone();
                let mut evaluated = vec![];
                let Some((last, lets)) = node.children.split_last() else {
                    return fail(
                        node.span,
                        "E_PROOF_UNSUPPORTED",
                        "An empty block has no value.",
                        "End the block with an expression.",
                    );
                };
                for statement in lets {
                    if statement.kind != K::Binding
                        || statement.children.iter().any(|c| c.kind == K::Modifier)
                    {
                        return fail(
                            statement.span,
                            "E_PROOF_UNSUPPORTED",
                            "The kernel reads only immutable `let` bindings before a block's value.",
                            "Remove mutation and statements from functions used in theorems.",
                        );
                    }
                    let pattern = &statement.children[0];
                    let written_type = self
                        .checked
                        .binding_types
                        .get(&pattern.span)
                        .cloned()
                        .or_else(|| {
                            (statement.children.len() == 3
                                && statement.children[1].kind == K::TypeName)
                                .then(|| {
                                    let name =
                                        crate::lexer::path_name(self.text(&statement.children[1]));
                                    let id =
                                        self.checked.environment.names.get(&name).unwrap_or(&name);
                                    self.checked
                                        .environment
                                        .definitions
                                        .get(id)
                                        .and_then(|d| d.alias.clone())
                                        .unwrap_or_else(|| Type::named(id))
                                })
                        });
                    if statement.children.len() > 2 && written_type.is_none() {
                        return fail(
                            statement.span,
                            "E_PROOF_UNSUPPORTED",
                            "This proof binding needs a supported named type.",
                            "Use Int, Nat, Bool or a concrete enum type.",
                        );
                    }
                    let (value, vty) = self.term(
                        statement.children.last().unwrap(),
                        written_type.as_ref(),
                        &inner,
                    )?;
                    if !small_term(&value) {
                        return fail(
                            statement.span,
                            "E_PROOF_LIMIT",
                            "Expanding this binding exceeds the proof term limit.",
                            "Split the calculation into named functions or smaller lemmas.",
                        );
                    }
                    if pattern.kind != K::PatternName {
                        return fail(
                            pattern.span,
                            "E_PROOF_UNSUPPORTED",
                            "The kernel reads `let name := value` only.",
                            "Bind a single name.",
                        );
                    }
                    evaluated.push(value.clone());
                    inner = inner.with(self.text(pattern), value, vty);
                }
                let (mut result, ty) = self.term(last, expected, &inner)?;
                // Substitution alone loses an unused binding's checked work.
                // seq preserves its evaluation for definedness, while reduction
                // still compares the final value of the block.
                for value in evaluated.into_iter().rev() {
                    result = Term::Op("seq".into(), vec![value, result]);
                }
                Ok((result, ty))
            }
            _ => fail(
                node.span,
                "E_PROOF_UNSUPPORTED",
                "The kernel cannot read this expression.",
                "Theorems and the functions they name use cases, calls, arithmetic, if and match.",
            ),
        }
    }

    fn ctor(&self, ty: &str, case: &str, args: Vec<Term>) -> Term {
        if ty == "Bool" && args.is_empty() && matches!(case, "true" | "false") {
            return Term::Bool(case == "true");
        }
        let term = Term::Ctor {
            ty: ty.to_owned(),
            case: case.to_owned(),
            args,
        };
        // Closed Nat cases become numerals at once.
        match &term {
            Term::Ctor { ty, case, args } if ty == "Nat" => {
                match (case.as_str(), args.as_slice()) {
                    ("Zero", []) => Term::Nat(0),
                    ("Succ", [Term::Nat(n)]) if *n < u64::MAX => Term::Nat(n + 1),
                    _ => term,
                }
            }
            _ => term,
        }
    }

    fn proof_method_candidates(&self, ty: &Type, name: &str) -> Vec<crate::types::FunctionType> {
        fn collect(
            kernel: &Kernel<'_>,
            ty: &Type,
            name: &str,
            seen: &mut BTreeSet<String>,
        ) -> Vec<crate::types::FunctionType> {
            let Some((id, args)) = Kernel::class_type(ty) else {
                return vec![];
            };
            if !seen.insert(id.to_owned()) {
                return vec![];
            }
            let Some(definition) = kernel.checked.environment.definitions.get(id) else {
                return vec![];
            };
            let substitutions = definition
                .parameters
                .iter()
                .cloned()
                .zip(args.iter().cloned())
                .collect::<BTreeMap<_, _>>();
            let own = definition
                .methods
                .get(name)
                .into_iter()
                .flatten()
                .filter_map(|signature| {
                    let Type::Function(mut signature) =
                        Type::Function(signature.clone()).replace_parameters(&substitutions)
                    else {
                        return None;
                    };
                    if !matches!(
                        signature.callable,
                        Some(Callable::Declaration {
                            kind: DeclarationKind::StaticMethod,
                            ..
                        })
                    ) {
                        signature.receiver = Some(Box::new(ty.clone()));
                    }
                    Some(signature)
                })
                .collect::<Vec<_>>();
            let mut inherited = vec![];
            if let Some(parent) = &definition.parent {
                inherited.extend(collect(
                    kernel,
                    &parent.replace_parameters(&substitutions),
                    name,
                    seen,
                ));
            }
            if definition.kind == K::Trait {
                for trait_type in &definition.traits {
                    inherited.extend(collect(
                        kernel,
                        &trait_type.replace_parameters(&substitutions),
                        name,
                        seen,
                    ));
                }
            }
            let mut methods = own.clone();
            for signature in inherited {
                if !methods
                    .iter()
                    .any(|current| current.same_parameters(&signature))
                {
                    methods.push(signature);
                }
            }
            methods
        }
        let mut candidates = collect(self, ty, name, &mut BTreeSet::new());
        for candidate in &mut candidates {
            if !matches!(
                candidate.callable,
                Some(Callable::Declaration {
                    kind: DeclarationKind::StaticMethod,
                    ..
                })
            ) {
                candidate.receiver = Some(Box::new(ty.clone()));
            }
        }
        candidates
    }

    fn method_call(
        &mut self,
        node: &Node,
        callee: &Node,
        args: &[&Node],
        expected: Option<&Type>,
        scope: &Scope,
        resolved: &crate::resolution::ResolvedCall,
    ) -> Found<(Term, Type)> {
        let Callable::Declaration { id, kind, .. } = resolved.target.clone() else {
            return fail(
                node.span,
                "E_PROOF_OPAQUE",
                "This is not a declared method.",
                "Use a pure method on a readable class.",
            );
        };
        let Some((_, raw_signature_params, raw_result)) = self.signatures.get(&id).cloned() else {
            return fail(
                node.span,
                "E_PROOF_OPAQUE",
                "The kernel has no readable signature for this method.",
                "Use a pure method declared on a readable class.",
            );
        };
        let is_static = kind == DeclarationKind::StaticMethod;
        let mut signature_params = vec![];
        let explicit_raw = raw_signature_params.iter().skip(usize::from(!is_static));
        if !is_static {
            let Some((this_name, raw_receiver_type)) = raw_signature_params.first() else {
                return fail(
                    node.span,
                    "E_PROOF_OPAQUE",
                    "The method signature has no receiver.",
                    "Use an instance method on a readable class.",
                );
            };
            let receiver_type = resolved
                .signature
                .receiver
                .as_deref()
                .cloned()
                .or_else(|| self.method_infos.get(&id).map(|info| info.receiver.clone()))
                .unwrap_or_else(|| raw_receiver_type.clone());
            signature_params.push((this_name.clone(), receiver_type));
        }
        let resolved_params = resolved.signature.parameters.iter();
        let mut resolved_params = resolved_params.peekable();
        for (name, raw_type) in explicit_raw {
            let ty = resolved_params
                .next()
                .map(|parameter| parameter.ty.clone())
                .unwrap_or_else(|| raw_type.clone());
            signature_params.push((name.clone(), ty));
        }
        let result = resolved.signature.result.as_ref().clone();
        let result = if result == Type::Invalid {
            raw_result
        } else {
            result
        };
        let method_name = self
            .method_infos
            .get(&id)
            .map(|info| info.name.clone())
            .unwrap_or_else(|| self.text(&callee.children[1]).to_owned());
        let callable_owner = match &resolved.target {
            Callable::Declaration { owner, .. } => owner.clone(),
            _ => None,
        }
        .or_else(|| self.method_infos.get(&id).map(|info| info.owner.clone()));
        if let Some(owner) = callable_owner
            && self
                .checked
                .environment
                .definitions
                .get(&owner)
                .is_some_and(|definition| definition.kind == K::Class)
            && !self.proof_can_access(&owner, &method_name)
        {
            return fail(
                callee.span,
                "E_MEMBER_PRIVATE",
                format!("Method `{method_name}` is not visible here."),
                "Use a public method, or package visibility within the same folder.",
            );
        }
        let mut visible = vec![];
        let mut actual_terms = vec![];
        let mut dispatch = None;
        if !is_static {
            let Some(receiver_node) = callee.children.first() else {
                return fail(
                    node.span,
                    "E_PROOF_UNSUPPORTED",
                    "This method call has no receiver.",
                    "Call the method on a readable class value.",
                );
            };
            let direct_super = receiver_node.kind == K::Name && self.text(receiver_node) == "super";
            let (receiver_term, inferred_type) = if direct_super {
                let Some((_, term, ty)) = scope.get("this") else {
                    return fail(
                        node.span,
                        "E_PROOF_UNSUPPORTED",
                        "`super` is only readable inside a class method.",
                        "Use an explicit receiver.",
                    );
                };
                (term.clone(), ty.clone())
            } else {
                self.term(receiver_node, None, scope)?
            };
            let receiver_type = resolved
                .signature
                .receiver
                .as_deref()
                .cloned()
                .unwrap_or(inferred_type.clone());
            let actual_receiver_type = inferred_type.clone();
            let static_receiver = if direct_super {
                receiver_type.clone()
            } else {
                inferred_type
            };
            let trait_receiver = Self::class_type(&receiver_type)
                .and_then(|(id, _)| self.checked.environment.definitions.get(id))
                .is_some_and(|definition| definition.kind == K::Trait);
            if let Err(reason) = self.plain_class(&receiver_type)
                && !trait_receiver
            {
                return fail(
                    receiver_node.span,
                    "E_PROOF_UNSUPPORTED",
                    format!("The kernel cannot unfold `{method_name}` because {reason}."),
                    "Use a pure method on a class with readable final fields and constructors.",
                );
            }
            let dep = Dispatch {
                receiver: static_receiver.clone(),
                method: method_name.clone(),
                function: id.clone(),
            };
            if !direct_super
                && !trait_receiver
                && let Some(subclass) = overridden_by(&dep, &self.checked.environment.definitions)
            {
                return fail(
                    receiver_node.span,
                    "E_PROOF_OVERRIDDEN",
                    format!(
                        "Theorem dispatch to `{static_receiver}.{method_name}` is unsafe because subclass `{subclass}` overrides it."
                    ),
                    "Prove the claim for the subclass's implementation, or avoid relying on virtual dispatch.",
                );
            }
            if !direct_super {
                dispatch = Some(Box::new(static_receiver));
            }
            let Some((this_name, _)) = signature_params.first() else {
                return fail(
                    node.span,
                    "E_PROOF_OPAQUE",
                    "The method signature has no receiver.",
                    "Use an instance method on a readable class.",
                );
            };
            visible.push((
                this_name.clone(),
                actual_receiver_type,
                receiver_term.clone(),
            ));
            actual_terms.push(receiver_term);
        }
        let explicit_params = signature_params.iter().skip(usize::from(!is_static));
        if explicit_params.len() != args.len() {
            return fail(
                node.span,
                "E_PROOF_UNSUPPORTED",
                "The kernel cannot read this method call's defaults or named arguments.",
                "Pass every method argument positionally.",
            );
        }
        for (arg, (name, ty)) in args.iter().zip(explicit_params) {
            let (term, found) = self.term(arg, Some(ty), scope)?;
            if !compatible(ty, &found) {
                return fail(
                    arg.span,
                    "E_PROOF_TYPE",
                    format!("Method argument needs {ty}, but this is {found}."),
                    "Pass a term of the method parameter's type.",
                );
            }
            visible.push((name.clone(), ty.clone(), term.clone()));
            actual_terms.push(term);
        }
        let hidden = self.hidden_arguments(&id, &visible, scope);
        actual_terms.extend(hidden);
        let mut result = result;
        if let Some(expected) = expected {
            if !compatible(&result, expected) {
                return fail(
                    node.span,
                    "E_PROOF_TYPE",
                    format!("This method returns {result}, not {expected}."),
                    "Use the result type the method declares.",
                );
            }
            result = expected.clone();
        }
        Ok((
            Term::Call {
                function: id,
                name: method_name,
                args: actual_terms,
                dispatch,
            },
            result,
        ))
    }

    /// `(x : T, y : U) => e` or `x => e`. Parameter types come from the
    /// annotation or from the expected function type; the result type comes
    /// from the expected type.
    fn lambda(
        &mut self,
        node: &Node,
        expected: Option<&Type>,
        scope: &Scope,
    ) -> Found<(Term, Type)> {
        let expected_function = match expected {
            Some(Type::Function(function)) => Some(function.clone()),
            _ => None,
        };
        let mut params: Vec<(String, Type)> = vec![];
        let mut body = None;
        for child in &node.children {
            match child.kind {
                K::Modifier | K::ReturnType => {}
                K::LambdaParameters => {
                    for (index, parameter) in child.children.iter().enumerate() {
                        let name = parameter
                            .child(K::Name)
                            .map(|n| self.text(n).to_owned())
                            .unwrap_or_else(|| format!("x{index}"));
                        let written = parameter
                            .children
                            .iter()
                            .skip(1)
                            .find(|n| !matches!(n.kind, K::Modifier | K::Name));
                        let ty = match written {
                            Some(ty) => self.quantified_type(ty, scope)?,
                            None => expected_function
                                .as_ref()
                                .and_then(|function| function.parameters.get(index))
                                .map(|parameter| parameter.ty.clone())
                                .ok_or_else(|| {
                                    Diagnostic::new(
                                        "E_PROOF_TYPE",
                                        format!("The kernel cannot infer a type for `{name}`."),
                                        parameter.span,
                                    )
                                    .with_hint(
                                        "Write the parameter type, as in (x : Nat) => x + 1.",
                                    )
                                })?,
                        };
                        params.push((name, ty));
                    }
                }
                _ => body = Some(child),
            }
        }
        let Some(body_node) = body else {
            return fail(
                node.span,
                "E_PROOF_STATEMENT",
                "A lambda needs a body.",
                "Write (x : Nat) => x + 1.",
            );
        };
        let result_expected = expected_function
            .as_ref()
            .map(|function| *function.result.clone());
        let mut inner = scope.clone();
        for (name, ty) in &params {
            inner = inner.with(name, Term::Var(name.clone()), ty.clone());
        }
        let (body, body_type) = self.term(body_node, result_expected.as_ref(), &inner)?;
        let ty = Type::Function(crate::types::FunctionType {
            parameters: params
                .iter()
                .map(|(name, ty)| crate::types::Parameter {
                    name: Some(name.clone()),
                    ty: ty.clone(),
                    default: false,
                    passing: crate::types::Passing::Borrow,
                })
                .collect(),
            result: Box::new(body_type),
            bounds: BTreeMap::new(),
            requirements: vec![],
            effects: false,
            receiver_mutates: false,
            callable: None,
            receiver: None,
        });
        Ok((Term::Lam(params, Box::new(body)), ty))
    }

    fn call(&mut self, node: &Node, expected: Option<&Type>, scope: &Scope) -> Found<(Term, Type)> {
        let callee = &node.children[0];
        let args = self.arguments(node);
        if let Some(result) = self.sequence_method(callee, &args, scope) {
            return result;
        }
        let qualified = crate::lexer::path_name(self.text(callee));
        if let Some(resolved) = self.checked.resolved_calls.get(&node.span).cloned()
            && let Callable::Declaration {
                kind:
                    DeclarationKind::Method
                    | DeclarationKind::StaticMethod
                    | DeclarationKind::TraitMethod,
                ..
            } = resolved.target.clone()
        {
            return self.method_call(node, callee, &args, expected, scope, &resolved);
        }
        // The checker records calls in function bodies, but theorem
        // propositions are elaborated separately. Resolve class methods from
        // the checked class signatures when that call-site record is absent.
        if callee.kind == K::Member && callee.children.len() == 2 {
            let receiver_node = &callee.children[0];
            let method_name = self.text(&callee.children[1]).to_owned();
            let direct_super = receiver_node.kind == K::Name && self.text(receiver_node) == "super";
            let static_class = (!direct_super
                && receiver_node.kind == K::Name
                && scope.get(self.text(receiver_node)).is_none())
            .then(|| {
                let name = self.text(receiver_node);
                let id = self
                    .checked
                    .environment
                    .names
                    .get(name)
                    .map(String::as_str)
                    .unwrap_or(name);
                self.checked
                    .environment
                    .definitions
                    .get(id)
                    .filter(|definition| definition.kind == K::Class)
                    .map(|definition| {
                        Type::Named(
                            id.to_owned(),
                            definition
                                .parameters
                                .iter()
                                .cloned()
                                .map(Type::Parameter)
                                .collect(),
                        )
                    })
            })
            .flatten();
            let (receiver_type, receiver_span) = if direct_super {
                let Some((_, _, this_type)) = scope.get("this") else {
                    return fail(
                        node.span,
                        "E_PROOF_UNSUPPORTED",
                        "`super` is only readable inside a class method.",
                        "Use an explicit receiver.",
                    );
                };
                let Some((owner, args)) = Self::class_type(this_type) else {
                    return fail(
                        node.span,
                        "E_PROOF_UNSUPPORTED",
                        "`super` has no class parent here.",
                        "Use an explicit receiver.",
                    );
                };
                let Some(definition) = self.checked.environment.definitions.get(owner) else {
                    return fail(
                        node.span,
                        "E_PROOF_UNSUPPORTED",
                        "The parent class definition is unavailable.",
                        "Use an explicit receiver.",
                    );
                };
                let substitutions = definition
                    .parameters
                    .iter()
                    .cloned()
                    .zip(args.iter().cloned())
                    .collect();
                let Some(parent) = definition
                    .parent
                    .as_ref()
                    .map(|parent| parent.replace_parameters(&substitutions))
                else {
                    return fail(
                        node.span,
                        "E_PROOF_UNSUPPORTED",
                        "This class has no readable parent.",
                        "Use an explicit receiver.",
                    );
                };
                (parent, receiver_node.span)
            } else if let Some(ty) = static_class.clone() {
                (ty, receiver_node.span)
            } else if let Ok((_, ty)) = self.term(receiver_node, None, scope) {
                (ty, receiver_node.span)
            } else {
                (Type::Invalid, receiver_node.span)
            };
            if receiver_type != Type::Invalid {
                let candidates = self.proof_method_candidates(&receiver_type, &method_name);
                let had_candidates = !candidates.is_empty();
                let mut matches = vec![];
                let mut best_error = None;
                for mut signature in candidates {
                    let Some(Callable::Declaration { id, kind, .. }) = signature.callable.clone()
                    else {
                        continue;
                    };
                    if !matches!(
                        kind,
                        DeclarationKind::Method
                            | DeclarationKind::StaticMethod
                            | DeclarationKind::TraitMethod
                    ) || signature.parameters.len() != args.len()
                        || static_class.is_some() != (kind == DeclarationKind::StaticMethod)
                    {
                        continue;
                    }
                    let mut valid = true;
                    for (arg, parameter) in args.iter().zip(&signature.parameters) {
                        match self.term(arg, Some(&parameter.ty), scope) {
                            Ok((_, found)) if compatible(&parameter.ty, &found) => {}
                            Ok((_, found)) => {
                                valid = false;
                                best_error = Some(
                                    Diagnostic::new(
                                        "E_PROOF_TYPE",
                                        format!(
                                            "Method argument needs {}, but this is {found}.",
                                            parameter.ty
                                        ),
                                        arg.span,
                                    )
                                    .with_hint("Pass an argument of the method's declared type."),
                                );
                                break;
                            }
                            Err(error) => {
                                valid = false;
                                best_error = Some(error);
                                break;
                            }
                        }
                    }
                    if valid {
                        if kind == DeclarationKind::StaticMethod {
                            signature.receiver = None;
                        } else {
                            signature.receiver = Some(Box::new(receiver_type.clone()));
                        }
                        matches.push((id, kind, signature));
                    }
                }
                if matches.len() == 1 {
                    let (id, kind, signature) = matches.pop().unwrap();
                    let resolved = crate::resolution::ResolvedCall {
                        target: Callable::Declaration {
                            id: id.clone(),
                            name: method_name,
                            owner: signature
                                .callable
                                .as_ref()
                                .and_then(|callable| match callable {
                                    Callable::Declaration { owner, .. } => owner.clone(),
                                    _ => None,
                                })
                                .or_else(|| {
                                    self.method_infos.get(&id).map(|info| info.owner.clone())
                                }),
                            kind: kind.clone(),
                        },
                        callee: Some(callee.span),
                        overload: 0,
                        signature,
                        type_arguments: BTreeMap::new(),
                        arguments: vec![],
                        evaluation_order: args.iter().map(|arg| arg.span).collect(),
                        receiver: (!matches!(kind, DeclarationKind::StaticMethod))
                            .then_some(receiver_span),
                        partial: false,
                    };
                    return self.method_call(node, callee, &args, expected, scope, &resolved);
                }
                if matches.len() > 1 {
                    return fail(
                        node.span,
                        "E_PROOF_TYPE",
                        format!("The call to `{method_name}` is ambiguous in this proof."),
                        "Choose a method overload with a unique parameter list.",
                    );
                }
                if let Some(error) = best_error
                    && had_candidates
                {
                    return Err(error);
                }
            }
        }
        if callee.kind == K::Member
            && qualified
                .split('.')
                .next()
                .is_some_and(|root| scope.get(root).is_some())
        {
            return fail(
                node.span,
                "E_PROOF_UNSUPPORTED",
                "The kernel cannot unfold a call through a value's member.",
                "Use a pure free function or an enum constructor.",
            );
        }
        let name = qualified.rsplit('.').next().unwrap_or("").to_owned();
        if (callee.kind == K::Name && scope.get(&name).is_none() || callee.kind == K::Member)
            && let Some((ty, fields, result)) = self.case_owner(&qualified, expected)
            && fields.len() == args.len()
            && !fields.is_empty()
        {
            let mut holes = Holes::new();
            let mut terms = vec![];
            for (arg, field) in args.iter().zip(&fields) {
                let want = generic::resolve(field, &holes);
                let (term, found) = self.term(arg, Some(&want), scope)?;
                if !unify(&want, &found, &mut holes) {
                    return fail(
                        arg.span,
                        "E_PROOF_TYPE",
                        format!("Case `{name}` needs {want} here, but this is {found}."),
                        "Give every field of the case a consistent type.",
                    );
                }
                terms.push(term);
            }
            return Ok((self.ctor(&ty, &name, terms), settle(&result, &holes)));
        }
        // Function bodies have resolved calls; theorem statements resolve only
        // through visible symbols, never a same-spelled private helper.
        let resolved = self
            .checked
            .resolved_calls
            .get(&node.span)
            .map(|c| c.target.clone());
        let is_resolved = resolved.is_some();
        if let Some(Callable::Builtin(builtin)) = &resolved
            && builtin.starts_with("record.")
        {
            let (constructor, field_names) = builtin.split_once('|').unwrap_or((builtin, ""));
            let owner = constructor.strip_prefix("record.").unwrap_or("");
            let definition = self.checked.environment.definitions.get(owner);
            let Some(_definition) = definition.filter(|definition| definition.record) else {
                return fail(
                    node.span,
                    "E_PROOF_OPAQUE",
                    "The record constructor has no checked field layout.",
                    "Use a declared record class.",
                );
            };
            if args.len()
                != field_names
                    .split(',')
                    .filter(|name| !name.trim().is_empty())
                    .count()
            {
                return fail(
                    node.span,
                    "E_PROOF_TYPE",
                    "This record constructor has the wrong number of fields.",
                    "Pass each record field in declaration order.",
                );
            }
            let result = self
                .checked
                .expression_types
                .iter()
                .rev()
                .find(|(span, _)| *span == node.span)
                .map(|(_, ty)| ty.clone())
                .unwrap_or_else(|| Type::Named(owner.to_owned(), vec![]));
            if let Err(reason) = self.plain_class(&result) {
                return fail(
                    node.span,
                    "E_PROOF_UNSUPPORTED",
                    format!("This record is unreadable because {reason}."),
                    "Use a record with supported immutable field types.",
                );
            }
            let fields = self
                .own_class_fields(&result)
                .map_err(|reason| Diagnostic::new("E_PROOF_UNSUPPORTED", reason, node.span))?;
            let mut holes = Holes::new();
            let mut terms = vec![];
            let names = field_names
                .split(',')
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .collect::<Vec<_>>();
            for (arg, name) in args.iter().zip(names) {
                let field_type = fields
                    .iter()
                    .find(|(field, _)| field == name)
                    .map(|(_, ty)| ty)
                    .unwrap_or(&Type::Invalid);
                let (term, found) = self.term(arg, Some(field_type), scope)?;
                if !unify(field_type, &found, &mut holes) {
                    return fail(
                        arg.span,
                        "E_PROOF_TYPE",
                        format!("Record field `{name}` needs {field_type}, but this is {found}."),
                        "Pass a value of the field's declared type.",
                    );
                }
                terms.push(term);
            }
            let result = settle(&result, &holes);
            return Ok((self.ctor(owner, "new", terms), result));
        }
        if !is_resolved && callee.kind == K::Name && scope.get(&qualified).is_none() {
            let type_name = qualified.rsplit('.').next().unwrap_or(&qualified);
            let class_id = self
                .checked
                .environment
                .names
                .get(&qualified)
                .or_else(|| self.checked.environment.names.get(type_name))
                .map(String::as_str)
                .unwrap_or(type_name);
            if let Some(class) = self
                .checked
                .environment
                .definitions
                .get(class_id)
                .filter(|definition| definition.kind == K::Class)
            {
                let generic_args = class
                    .parameters
                    .iter()
                    .map(|parameter| Type::Parameter(format!("?ctor:{class_id}:{parameter}")))
                    .collect::<Vec<_>>();
                let generic_type = Type::Named(class_id.to_owned(), generic_args.clone());
                let mut initial_holes = Holes::new();
                if let Some(expected) = expected
                    && !unify(&generic_type, expected, &mut initial_holes)
                {
                    return fail(
                        node.span,
                        "E_PROOF_TYPE",
                        format!("This constructor creates {generic_type}, not {expected}."),
                        "Use a constructor for the expected class type.",
                    );
                }
                if class.record {
                    let field_types = self.own_class_fields(&generic_type).map_err(|reason| {
                        Diagnostic::new("E_PROOF_UNSUPPORTED", reason, node.span)
                    })?;
                    if field_types.len() != args.len() {
                        return fail(
                            node.span,
                            "E_PROOF_TYPE",
                            format!("`{type_name}` takes {} record fields.", field_types.len()),
                            "Pass every record field in declaration order.",
                        );
                    }
                    let mut holes = initial_holes;
                    let mut terms = vec![];
                    for (arg, (field, ty)) in args.iter().zip(&field_types) {
                        let want = generic::resolve(ty, &holes);
                        let (term, found) = self.term(arg, Some(&want), scope)?;
                        if !unify(&want, &found, &mut holes) {
                            return fail(
                                arg.span,
                                "E_PROOF_TYPE",
                                format!(
                                    "Record field `{field}` needs {want}, but this is {found}."
                                ),
                                "Pass a value of the field's declared type.",
                            );
                        }
                        terms.push(term);
                    }
                    let result = generic::resolve(&generic_type, &holes);
                    return Ok((self.ctor(class_id, "new", terms), result));
                }
                let mut last_error = None;
                for constructor in &class.constructors {
                    let Some(Callable::Declaration {
                        id,
                        kind: DeclarationKind::Constructor,
                        ..
                    }) = &constructor.callable
                    else {
                        continue;
                    };
                    let Some((_, parameters, _)) = self.signatures.get(id).cloned() else {
                        continue;
                    };
                    if parameters.len() != args.len() {
                        continue;
                    }
                    let class_substitutions = class
                        .parameters
                        .iter()
                        .cloned()
                        .zip(generic_args.iter().cloned())
                        .collect::<BTreeMap<_, _>>();
                    let mut holes = initial_holes.clone();
                    let mut actual_terms = vec![];
                    let mut visible = vec![];
                    let mut valid = true;
                    for (arg, (parameter, raw_ty)) in args.iter().zip(&parameters) {
                        let parameter_ty = raw_ty.replace_parameters(&class_substitutions);
                        let want = generic::resolve(&parameter_ty, &holes);
                        match self.term(arg, Some(&want), scope) {
                            Ok((term, found)) if unify(&want, &found, &mut holes) => {
                                visible.push((
                                    parameter.clone(),
                                    generic::resolve(&want, &holes),
                                    term.clone(),
                                ));
                                actual_terms.push(term);
                            }
                            Ok((_, found)) => {
                                last_error = Some(Diagnostic::new(
                                    "E_PROOF_TYPE",
                                    format!("Constructor argument needs {want}, but this is {found}."),
                                    arg.span,
                                ).with_hint("Pass a value of the constructor parameter's type."));
                                valid = false;
                                break;
                            }
                            Err(error) => {
                                last_error = Some(error);
                                valid = false;
                                break;
                            }
                        }
                    }
                    if !valid {
                        continue;
                    }
                    let result = generic::resolve(&generic_type, &holes);
                    let hidden = self.hidden_arguments(id, &visible, scope);
                    actual_terms.extend(hidden);
                    return Ok((
                        Term::Call {
                            function: id.clone(),
                            name: type_name.to_owned(),
                            args: actual_terms,
                            dispatch: None,
                        },
                        result,
                    ));
                }
                if let Some(error) = last_error {
                    return Err(error);
                }
            }
        }
        let candidates = match resolved {
            Some(Callable::Declaration {
                id,
                kind: DeclarationKind::Function | DeclarationKind::Constructor,
                ..
            }) => vec![id],
            Some(
                Callable::Declaration { .. }
                | Callable::Builtin(_)
                | Callable::Indirect
                | Callable::Closure(_),
            ) => vec![],
            None if scope.get(&qualified).is_none() => self.visible_functions(&qualified),
            None => vec![],
        };
        if candidates.is_empty() {
            if !is_resolved {
                return fail(
                    node.span,
                    "E_PROOF_OPAQUE",
                    format!("`{qualified}` is not a visible pure function the kernel can unfold."),
                    "Import a public function or use an enum constructor.",
                );
            }
            let terms = args
                .iter()
                .map(|arg| self.term(arg, None, scope).map(|(t, _)| t))
                .collect::<Found<Vec<_>>>()?;
            let ty = self
                .checked
                .expression_types
                .iter()
                .rev()
                .find(|(s, _)| *s == node.span)
                .map(|(_, t)| t.clone())
                .unwrap_or(Type::Invalid);
            return Ok((
                Term::Opaque {
                    name: qualified,
                    args: terms,
                },
                ty,
            ));
        }
        let mut bound_failure = None;
        for span in candidates {
            let Some((_, params, result)) = self.signatures.get(&span).cloned() else {
                return fail(
                    node.span,
                    "E_PROOF_OPAQUE",
                    format!("The kernel has no readable definition for `{qualified}`."),
                    "Use a supported pure function.",
                );
            };
            if params.len() != args.len() {
                continue;
            }
            // The function's own type variables are filled at each call.
            let mut types = params.iter().map(|(_, t)| t).collect::<Vec<_>>();
            types.push(&result);
            let constraints = self.constraints.get(&span).cloned().unwrap_or_default();
            for (ty, bound) in &constraints {
                types.extend([ty, bound]);
            }
            let variables = self.holes_for(&types);
            let mut holes = Holes::new();
            if let Some(expected) = expected {
                unify(&instantiate(&result, &variables), expected, &mut holes);
            }
            let mut terms = vec![];
            let mut visible = vec![];
            let mut fits = true;
            for (arg, (parameter_name, pty)) in args.iter().zip(&params) {
                let want = generic::resolve(&instantiate(pty, &variables), &holes);
                match self.term(arg, Some(&want), scope) {
                    Ok((term, ty)) if unify(&want, &ty, &mut holes) => {
                        visible.push((parameter_name.clone(), want, term.clone()));
                        terms.push(term);
                    }
                    _ => {
                        fits = false;
                        break;
                    }
                }
            }
            if fits {
                if let Err(error) =
                    self.check_bounds(&constraints, &variables, &holes, scope, node.span)
                {
                    bound_failure = Some(error);
                    continue;
                }
                let result = settle(&instantiate(&result, &variables), &holes);
                terms.extend(self.hidden_arguments(&span, &visible, scope));
                if self.declared_opaque.contains(&span) {
                    return Ok((
                        Term::Opaque {
                            name: name.clone(),
                            args: terms,
                        },
                        result,
                    ));
                }
                return Ok((
                    Term::Call {
                        function: span,
                        name: name.clone(),
                        args: terms,
                        dispatch: None,
                    },
                    result,
                ));
            }
        }
        if let Some(error) = bound_failure {
            return Err(error);
        }
        fail(
            node.span,
            "E_PROOF_UNKNOWN",
            format!(
                "`{name}` with these arguments is not a function of this module or an enum case."
            ),
            "Theorems can name the pure functions declared in the same file.",
        )
    }

    /// Elaborates a match pattern, adding its names to the scope.
    fn pattern(&mut self, node: &Node, ty: &Type, scope: &mut Scope) -> Found<Pat> {
        match node.kind {
            K::PatternList => self.list_pattern(node, ty, scope),
            K::PatternWildcard => Ok(Pat::Any),
            K::PatternName => {
                let name = self.text(node).to_owned();
                let unique = self.fresh(&name);
                *scope = scope.with(&name, Term::Var(unique.clone()), ty.clone());
                Ok(Pat::Bind(unique))
            }
            K::PatternLiteral => {
                let literal = self
                    .term(&Node::leaf(K::Literal, node.span), Some(ty), scope)?
                    .0;
                Ok(match literal {
                    Term::Bool(value) => Pat::Ctor(value.to_string(), vec![]),
                    other => Pat::Lit(other),
                })
            }
            K::PatternRange | K::PatternRangeInclusive => {
                let (Some(low), Some(high)) = (
                    integer_literal(self.text(&node.children[0])),
                    integer_literal(self.text(&node.children[1])),
                ) else {
                    return fail(
                        node.span,
                        "E_PROOF_UNSUPPORTED",
                        "The kernel reads integer range bounds only.",
                        "Write the bounds as whole numbers.",
                    );
                };
                Ok(Pat::Range {
                    low,
                    high,
                    inclusive: node.kind == K::PatternRangeInclusive,
                })
            }
            K::PatternAlt => {
                let mut pats = vec![];
                let mut first = None;
                for alternative in &node.children {
                    let mut inner = scope.clone();
                    pats.push(self.pattern(alternative, ty, &mut inner)?);
                    if first.is_none() {
                        first = Some(inner);
                    }
                }
                let names = |pat: &Pat| {
                    let mut binders = BTreeSet::new();
                    pat.binders(&mut binders);
                    binders
                        .iter()
                        .map(|name| shown(name).to_owned())
                        .collect::<BTreeSet<_>>()
                };
                let expected = pats.first().map(&names).unwrap_or_default();
                for pat in pats.iter().skip(1) {
                    if names(pat) != expected {
                        return fail(
                            node.span,
                            "E_PROOF_SHAPE",
                            "Every alternative must bind the same names.",
                            "Bind the same names, with the same types, in every alternative.",
                        );
                    }
                }
                // Each alternative freshens its binders on its own. Align them on the
                // first one so a match through any alternative substitutes the same
                // names in the shared body.
                if let Some(first) = pats.first() {
                    let mut binders = BTreeSet::new();
                    first.binders(&mut binders);
                    let canonical = binders
                        .into_iter()
                        .map(|name| (shown(&name).to_owned(), name))
                        .collect::<BTreeMap<_, _>>();
                    for pat in pats.iter_mut().skip(1) {
                        let mut binders = BTreeSet::new();
                        pat.binders(&mut binders);
                        let renames = binders
                            .into_iter()
                            .map(|name| {
                                let target = canonical
                                    .get(shown(&name))
                                    .cloned()
                                    .unwrap_or_else(|| name.clone());
                                (name, target)
                            })
                            .collect::<BTreeMap<_, _>>();
                        *pat = pat.rename(&renames);
                    }
                }
                if let Some(first) = first {
                    *scope = first;
                }
                Ok(Pat::Alt(pats))
            }
            K::PatternConstructor => {
                let qualified = crate::lexer::path_name(self.text(&node.children[0]));
                let case = qualified.rsplit('.').next().unwrap_or("").to_owned();
                let Some((_, fields, result)) = self.case_owner(&qualified, Some(ty)) else {
                    return fail(
                        node.span,
                        "E_PROOF_UNKNOWN",
                        format!("`{case}` is not a case of {ty}."),
                        "Use a case of the matched enum.",
                    );
                };
                if !compatible(&result, ty) {
                    return fail(
                        node.span,
                        "E_PROOF_TYPE",
                        format!("`{case}` constructs {result}, not {ty}."),
                        "Use a case of the matched type.",
                    );
                }
                if fields.len() != node.children.len() - 1 {
                    return fail(
                        node.span,
                        "E_PROOF_SHAPE",
                        format!(
                            "Case `{case}` needs {} fields in this pattern.",
                            fields.len()
                        ),
                        "Bind one pattern for each case field.",
                    );
                }
                let mut pats = vec![];
                for (child, field) in node.children.iter().skip(1).zip(&fields) {
                    pats.push(self.pattern(child, field, scope)?);
                }
                Ok(Pat::Ctor(case, pats))
            }
            _ => fail(
                node.span,
                "E_PROOF_UNSUPPORTED",
                "The kernel reads case, name, wildcard and literal patterns.",
                "Rewrite this pattern.",
            ),
        }
    }
}

/// Functions that cannot fail: no arithmetic, no Succ, no requirements, and calls
/// only to such functions or, structurally, to themselves. Unfoldable functions
/// never call each other in a cycle, so removing failures until nothing changes
/// leaves exactly these.
fn total_functions(
    functions: &BTreeMap<DeclarationId, Definition>,
    math: bool,
) -> BTreeSet<DeclarationId> {
    fn safe(t: &Term, me: &DeclarationId, total: &BTreeSet<DeclarationId>, math: bool) -> bool {
        let all = |ts: &[Term]| ts.iter().all(|t| safe(t, me, total, math));
        match t {
            Term::Var(_) | Term::Nat(_) | Term::Int(_) | Term::Bool(_) => true,
            Term::Lam(_, body) => safe(body, me, total, math),
            Term::Prop(prop) => {
                let mut all_safe = true;
                prop.map(&mut |term| {
                    all_safe &= safe(term, me, total, math);
                    term.clone()
                });
                all_safe
            }
            Term::App(function, args) => safe(function, me, total, math) && all(args),
            Term::Opaque { .. } => false,
            Term::Ctor { ty, case, args } => {
                (math || !(ty == "Nat" && case == "Succ")) && all(args)
            }
            Term::Call { function, args, .. } => {
                (function == me || total.contains(function)) && all(args)
            }
            Term::Op(op, args) => {
                (matches!(
                    op.as_str(),
                    "<" | "<=" | ">" | ">=" | "==" | "!=" | "&&" | "||" | "!" | "seq"
                ) || op.starts_with("$list.")
                    || op.starts_with("$option.")
                    || math && (matches!(op.as_str(), "+" | "*") || op == "-" && args.len() == 1))
                    && all(args)
            }
            Term::If(c, a, b) => {
                safe(c, me, total, math) && safe(a, me, total, math) && safe(b, me, total, math)
            }
            Term::Match(value, arms) => {
                safe(value, me, total, math)
                    && arms.iter().all(|(_, body)| safe(body, me, total, math))
            }
            // Proof-only set forms have no run-time evaluation, so they never
            // make a function total.
            Term::FamilyUnion { .. }
            | Term::FamilyInter { .. }
            | Term::Builder { .. }
            | Term::Finite(_) => false,
        }
    }
    let mut total = functions
        .iter()
        .filter(|(_, d)| d.requires.is_empty() && d.preconditions.is_empty())
        .map(|(id, _)| id.clone())
        .collect::<BTreeSet<_>>();
    loop {
        let kept = total
            .iter()
            .filter(|id| safe(&functions[*id].body, id, &total, math))
            .cloned()
            .collect::<BTreeSet<_>>();
        if kept.len() == total.len() {
            return kept;
        }
        total = kept;
    }
}

fn callees(t: &Term, out: &mut BTreeSet<DeclarationId>) {
    match t {
        Term::Call { function, args, .. } => {
            out.insert(function.clone());
            args.iter().for_each(|a| callees(a, out));
        }
        Term::Ctor { args, .. } | Term::Opaque { args, .. } | Term::Op(_, args) => {
            args.iter().for_each(|a| callees(a, out))
        }
        Term::If(c, a, b) => [c, a, b].iter().for_each(|x| callees(x, out)),
        Term::Match(s, arms) => {
            callees(s, out);
            arms.iter().for_each(|(_, b)| callees(b, out));
        }
        _ => {}
    }
}

/// Every call a function makes to itself must pass, at one fixed position, a
/// variable that a match took out of that parameter. That keeps unfolding finite.
fn structural(span: &DeclarationId, params: &[String], body: &Term) -> Result<(), String> {
    fn walk(
        t: &Term,
        span: &DeclarationId,
        params: &[String],
        smaller: &BTreeMap<String, usize>,
        candidates: &mut Option<BTreeSet<usize>>,
    ) {
        match t {
            Term::Call { function, args, .. } => {
                if function == span {
                    let here = args
                        .iter()
                        .enumerate()
                        .filter(|(i, a)| matches!(a, Term::Var(v) if smaller.get(v) == Some(i)))
                        .map(|(i, _)| i)
                        .collect::<BTreeSet<_>>();
                    *candidates = Some(match candidates.take() {
                        Some(previous) => previous.intersection(&here).copied().collect(),
                        None => here,
                    });
                }
                args.iter()
                    .for_each(|a| walk(a, span, params, smaller, candidates));
            }
            Term::Ctor { args, .. } | Term::Opaque { args, .. } | Term::Op(_, args) => args
                .iter()
                .for_each(|a| walk(a, span, params, smaller, candidates)),
            Term::If(c, a, b) => [c, a, b]
                .iter()
                .for_each(|x| walk(x, span, params, smaller, candidates)),
            Term::Match(s, arms) => {
                walk(s, span, params, smaller, candidates);
                let origin = match s.as_ref() {
                    Term::Var(v) => params
                        .iter()
                        .position(|p| p == v)
                        .or_else(|| smaller.get(v).copied()),
                    _ => None,
                };
                for (pat, body) in arms {
                    let mut inner = smaller.clone();
                    if let (Some(index), Pat::Ctor(..)) = (origin, pat) {
                        let mut bound = BTreeSet::new();
                        pat.binders(&mut bound);
                        for name in bound {
                            inner.insert(name, index);
                        }
                    }
                    walk(body, span, params, &inner, candidates);
                }
            }
            _ => {}
        }
    }
    let mut candidates = None;
    walk(body, span, params, &BTreeMap::new(), &mut candidates);
    match candidates {
        Some(set) if set.is_empty() => Err("it is not structurally recursive: a recursive call must pass a piece that a match took out of one fixed parameter".into()),
        _ => Ok(()),
    }
}

/// What a proof may use at one point: names, assumptions, and which variables
/// came out of a theorem parameter (for structural self-citation).
#[derive(Clone, Default)]
struct Context {
    scope: Scope,
    hyps: Vec<(String, Prop)>,
    /// The theorem's `requires` assumptions and proposition parameters. An
    /// induction hypothesis is conditional on these at the smaller value (Q81);
    /// local facts, including earlier induction hypotheses, are not.
    assumptions: Vec<(String, Prop)>,
    smaller: BTreeMap<String, usize>,
    /// Induction hypotheses that hold only where their assumptions hold at the
    /// smaller value: name, those assumptions, statement (Q81).
    conditional: Vec<(String, Vec<Prop>, Prop)>,
    /// Names of the induction hypotheses in `hyps`. Rewriting a variable one of
    /// them mentions would invalidate the hypothesis; a plain `cases` binder
    /// has none and may be rewritten.
    induction_hyps: Vec<String>,
}

/// The theorem being proved, so its proof can cite itself on smaller values.
struct Current {
    name: String,
    start: usize,
    theorem: Theorem,
    candidates: Option<BTreeSet<usize>>,
    admitted: bool,
}

/// Decomposes `Ctor(args) = Ctor(args)` into one equation per argument pair.
/// `changed` records that at least one constructor layer was removed.
fn peel_equality(left: &Term, right: &Term, out: &mut Vec<(Term, Term)>, changed: &mut bool) {
    match (left, right) {
        (
            Term::Ctor {
                ty: a_ty,
                case: a_case,
                args: a_args,
            },
            Term::Ctor {
                ty: b_ty,
                case: b_case,
                args: b_args,
            },
        ) if a_ty == b_ty && a_case == b_case && a_args.len() == b_args.len() => {
            *changed = true;
            for (a, b) in a_args.iter().zip(b_args) {
                peel_equality(a, b, out, changed);
            }
        }
        _ => out.push((left.clone(), right.clone())),
    }
}

impl Kernel<'_> {
    fn reducer(&self) -> Reducer<'_> {
        Reducer {
            functions: &self.functions,
            definitions: &self.checked.environment.definitions,
            fuel: FUEL,
            fresh: self.fresh,
        }
    }

    fn normal(&mut self, p: &Prop) -> Prop {
        let mut reducer = self.reducer();
        let result = reducer.nf_prop(p);
        let fresh = reducer.fresh;
        self.fresh = fresh;
        result
    }

    fn same(&mut self, a: &Prop, b: &Prop) -> bool {
        if a == b {
            return true;
        }
        let a = self.normal(a);
        let b = self.normal(b);
        if a == b || a.alpha_normalize() == b.alpha_normalize() {
            return true;
        }
        // A constructor is injective: `Succ(a) = Succ(b)` is the same claim as
        // `a = b`, and an enum case is the conjunction of its field equations.
        // This is the Peano rule the inductive types already promise, used only
        // when at least one side is a constructor pair, so it terminates.
        if let (Prop::Eq(left, right), Prop::Eq(other, beyond)) = (&a, &b) {
            let mut ours = vec![];
            let mut theirs = vec![];
            let mut peeled = false;
            peel_equality(left, right, &mut ours, &mut peeled);
            peel_equality(other, beyond, &mut theirs, &mut peeled);
            if peeled
                && ours.len() == theirs.len()
                && ours.iter().zip(&theirs).all(|((x, y), (u, v))| {
                    let one = self.normal(&Prop::Eq(x.clone(), y.clone()));
                    let two = self.normal(&Prop::Eq(u.clone(), v.clone()));
                    one == two
                        || one.alpha_normalize() == two.alpha_normalize()
                        || expand_prop(&one) == expand_prop(&two)
                        || expand_prop(&one).alpha_normalize()
                            == expand_prop(&two).alpha_normalize()
                })
            {
                return true;
            }
        }
        // Definitions unfold for equality checks, but not for the goal the
        // editor displays.
        let a = expand_prop(&a);
        let b = expand_prop(&b);
        a == b || a.alpha_normalize() == b.alpha_normalize()
    }

    /// Unfold only the outer definition the next tactic needs. This is the
    /// demand-driven step: `A ⊆ B` becomes a quantified implication, and
    /// `x ∈ A ∪ B` becomes `x ∈ A or x ∈ B`, but their children stay as
    /// written.
    fn unfold_head(&mut self, prop: &Prop) -> Prop {
        match prop {
            Prop::Subset {
                left,
                right,
                element,
            } => {
                let binder = self.fresh("x");
                let value = Term::Var(binder.clone());
                Prop::Forall {
                    name: binder,
                    ty: element.clone(),
                    body: Box::new(Prop::Implies(
                        Box::new(Prop::Mem(value.clone(), left.clone())),
                        Box::new(Prop::Mem(value, right.clone())),
                    )),
                }
            }
            Prop::Mem(value, set) => set_membership_head(value.clone(), set.clone()),
            _ => prop.clone(),
        }
    }

    fn decide(&mut self, p: &Prop) -> Option<bool> {
        let mut reducer = self.reducer();
        let result = reducer.decide(p);
        let fresh = reducer.fresh;
        self.fresh = fresh;
        result
    }

    /// Elaborates a statement written after a theorem's `:` or `requires h :`.
    fn prop(&mut self, node: &Node, scope: &Scope) -> Found<Prop> {
        match node.kind {
            K::Group | K::Proposition => self.prop(&node.children[0], scope),
            K::Name => {
                let name = self.text(node);
                if let Some((_, term, ty)) = scope.get(name)
                    && matches!(ty, Type::Named(id, args) if id == "Prop" && args.is_empty())
                {
                    return Ok(Prop::Atom(term.clone()));
                }
                self.holds(node, scope)
            }
            K::Literal if self.text(node) == "false" => Ok(Prop::False),
            K::Literal if self.text(node) == "true" => Ok(Prop::True),
            K::Forall => self.quantified(node, true, scope),
            K::Exists => self.quantified(node, false, scope),
            K::Binary => {
                let op = self.text(&node.children[1]);
                match op {
                    "∈" | "\\in" => self.membership(node, scope),
                    "∉" | "\\notin" => Ok(Prop::Not(Box::new(self.membership(node, scope)?))),
                    "⊆" | "\\subseteq" => self.subset(node, scope),
                    "->" | "→" | "\\to" => Ok(Prop::Implies(
                        Box::new(self.prop(&node.children[0], scope)?),
                        Box::new(self.prop(&node.children[2], scope)?),
                    )),
                    "↔" | "\\iff" => Ok(Prop::Iff(
                        Box::new(self.prop(&node.children[0], scope)?),
                        Box::new(self.prop(&node.children[2], scope)?),
                    )),
                    "and" | "∧" | "\\land" => Ok(Prop::And(
                        Box::new(self.prop(&node.children[0], scope)?),
                        Box::new(self.prop(&node.children[2], scope)?),
                    )),
                    "or" | "∨" | "\\lor" => Ok(Prop::Or(
                        Box::new(self.prop(&node.children[0], scope)?),
                        Box::new(self.prop(&node.children[2], scope)?),
                    )),
                    "=" => {
                        let (left, right) = (&node.children[0], &node.children[2]);
                        // A bare literal or an empty set takes its type from
                        // the other side.
                        let (a, b) = if matches!(left.kind, K::Literal | K::SetLiteral)
                            && right.kind != K::SetLiteral
                        {
                            let (b, ty) = self.term(right, None, scope)?;
                            (self.term(left, Some(&ty), scope)?, (b, ty))
                        } else {
                            let a = self.term(left, None, scope)?;
                            let b = self.term(right, Some(&a.1), scope)?;
                            (a, b)
                        };
                        if !compatible(&a.1, &b.1) {
                            return fail(
                                node.span,
                                "E_PROOF_TYPE",
                                format!("This equality compares {} with {}.", a.1, b.1),
                                "Both sides of `=` need the same type.",
                            );
                        }
                        Ok(Prop::Eq(a.0, b.0))
                    }
                    _ => self.holds_or_atom(node, scope),
                }
            }
            K::Unary
                if matches!(
                    self.text(&node.children[0]),
                    "not" | "¬" | "\\not" | "\\neg"
                ) =>
            {
                Ok(Prop::Not(Box::new(self.prop(&node.children[1], scope)?)))
            }
            _ => self.holds_or_atom(node, scope),
        }
    }

    /// A term of sort `Prop` is an opaque proposition; anything else is a Bool
    /// condition that must hold.
    fn holds_or_atom(&mut self, node: &Node, scope: &Scope) -> Found<Prop> {
        if let Ok((term, ty)) = self.term(node, None, scope)
            && matches!(&ty, Type::Named(id, args) if id == "Prop" && args.is_empty())
        {
            // A defined predicate call unfolds now: the tactics need the
            // quantifier or connective, not an opaque atom. An `opaque`
            // predicate keeps its arguments as written, so `rw` can still
            // rewrite inside them.
            let reduced = if matches!(term, Term::Opaque { .. }) {
                term
            } else {
                let mut reducer = self.reducer();
                let reduced = reducer.nf(&term);
                self.fresh = reducer.fresh;
                reduced
            };
            return Ok(match reduced {
                Term::Prop(prop) => *prop,
                other => Prop::Atom(other),
            });
        }
        self.holds(node, scope)
    }

    /// `x ∈ A`: a proof-only membership fact about an abstract set.
    fn membership(&mut self, node: &Node, scope: &Scope) -> Found<Prop> {
        let (value, value_type) = self.term(&node.children[0], None, scope)?;
        // The universe comes from the element, so `x ∈ ∅` and `x ∈ {a}` are
        // typed without an annotation on the set.
        let set_expected = Type::applied("Set", vec![value_type.clone()]);
        let (set, set_type) = self.term(&node.children[2], Some(&set_expected), scope)?;
        let Type::Named(id, args) = &set_type else {
            return fail(
                node.span,
                "E_PROOF_TYPE",
                format!("The right side of ∈ is {set_type}, not a set."),
                "Write x ∈ A where A has type Set[U].",
            );
        };
        if id != "Set" || args.len() != 1 {
            return fail(
                node.span,
                "E_PROOF_TYPE",
                format!("The right side of ∈ is {set_type}, not a set."),
                "Write x ∈ A where A has type Set[U].",
            );
        }
        if !compatible(&args[0], &value_type) {
            return fail(
                node.span,
                "E_PROOF_TYPE",
                format!(
                    "`{value}` is {value_type}, but this set holds {} values.",
                    args[0]
                ),
                "Use an element of the set's universe.",
            );
        }
        Ok(Prop::Mem(value, set))
    }

    /// `A ⊆ B` means every element of A is an element of B. It expands to a
    /// quantified implication, so intro, apply and exact work as usual.
    fn subset(&mut self, node: &Node, scope: &Scope) -> Found<Prop> {
        // `∅ ⊆ A` and `A ⊆ ∅` need the other side to supply the universe.
        let written_left = &node.children[0];
        let written_right = &node.children[2];
        let (left, left_type, right, right_type) =
            if written_left.kind == K::SetLiteral && written_right.kind != K::SetLiteral {
                let (right, right_type) = self.term(written_right, None, scope)?;
                let (left, left_type) = self.term(written_left, Some(&right_type), scope)?;
                (left, left_type, right, right_type)
            } else {
                let (left, left_type) = self.term(written_left, None, scope)?;
                let (right, right_type) = self.term(written_right, Some(&left_type), scope)?;
                (left, left_type, right, right_type)
            };
        let (Type::Named(left_name, left_args), Type::Named(right_name, right_args)) =
            (&left_type, &right_type)
        else {
            return fail(
                node.span,
                "E_PROOF_TYPE",
                "Both sides of ⊆ need to be sets.",
                "Write A ⊆ B where A and B have type Set[U].",
            );
        };
        if left_name != "Set"
            || left_args.len() != 1
            || right_name != "Set"
            || right_args.len() != 1
        {
            return fail(
                node.span,
                "E_PROOF_TYPE",
                "Both sides of ⊆ need to be sets.",
                "Write A ⊆ B where A and B have type Set[U].",
            );
        }
        if !compatible(&left_args[0], &right_args[0]) {
            return fail(
                node.span,
                "E_PROOF_TYPE",
                format!(
                    "These sets have different element types: {} and {}.",
                    left_args[0], right_args[0]
                ),
                "Compare sets over the same universe.",
            );
        }
        Ok(Prop::Subset {
            left,
            right,
            element: left_args[0].clone(),
        })
    }

    /// `A ⊆ B` built from already-elaborated terms, for tactics that invent a
    /// subset goal (set extensionality).
    fn subset_prop(&mut self, left: Term, right: Term, element: Type) -> Prop {
        Prop::Subset {
            left,
            right,
            element,
        }
    }

    /// Reads the one type a quantifier binds. The kernel reasons about the
    /// types it can decide and about the theorem's own type variables.
    fn quantified_type(&self, node: &Node, scope: &Scope) -> Found<Type> {
        match node.kind {
            K::Group => self.quantified_type(&node.children[0], scope),
            K::TypeName => {
                let name = self.text(node);
                match name {
                    "Nat" => Ok(nat()),
                    "Int" => Ok(Type::named("Int")),
                    "Bool" => Ok(Type::named("Bool")),
                    "Prop" => Ok(Type::named("Prop")),
                    _ if scope.bounds.contains_key(name) => Ok(Type::Parameter(name.to_owned())),
                    _ => {
                        let id = self
                            .checked
                            .environment
                            .names
                            .get(name)
                            .map(String::as_str)
                            .unwrap_or(name);
                        match self.checked.environment.definitions.get(id) {
                            Some(definition)
                                if definition.kind == K::Enum
                                    && definition.parameters.is_empty() =>
                            {
                                Ok(Type::named(id))
                            }
                            Some(definition)
                                if definition.kind == K::Class
                                    && definition.parameters.is_empty() =>
                            {
                                let ty = Type::named(id);
                                self.plain_class(&ty).map(|_| ty).map_err(|reason| {
                                    Diagnostic::new(
                                        "E_PROOF_UNSUPPORTED",
                                        format!("The kernel cannot quantify over `{name}` because {reason}."),
                                        node.span,
                                    ).with_hint("Quantify over a readable class or record, or use Nat, Int, Bool or an enum type.")
                                })
                            }
                            _ => fail(
                                node.span,
                                "E_PROOF_UNSUPPORTED",
                                format!("The kernel cannot quantify over `{name}`."),
                                "Quantify over Nat, Int, Bool, an enum or a readable class.",
                            ),
                        }
                    }
                }
            }
            K::FunctionType => {
                if node.children.len() != 2 {
                    return fail(
                        node.span,
                        "E_PROOF_UNSUPPORTED",
                        "The kernel cannot quantify over this function type.",
                        "Write a pure function type such as Nat -> Nat or (Int, Int) -> Bool.",
                    );
                }
                let mut input = &node.children[0];
                while input.kind == K::TupleType
                    && input.children.len() == 1
                    && input.children[0].kind == K::TupleType
                {
                    input = &input.children[0];
                }
                let mut parameters = vec![];
                if input.kind == K::TupleType
                    && input
                        .children
                        .iter()
                        .any(|child| child.kind == K::Parameter)
                {
                    for parameter in &input.children {
                        let (name, ty) = if parameter.kind == K::Parameter {
                            (
                                parameter
                                    .child(K::Name)
                                    .map(|n| self.text(n).to_owned())
                                    .unwrap_or_else(|| format!("x{}", parameters.len())),
                                self.quantified_type(parameter.children.last().unwrap(), scope)?,
                            )
                        } else {
                            (
                                format!("x{}", parameters.len()),
                                self.quantified_type(parameter, scope)?,
                            )
                        };
                        parameters.push(crate::types::Parameter {
                            name: Some(name),
                            ty,
                            default: false,
                            passing: crate::types::Passing::Borrow,
                        });
                    }
                } else if input.kind == K::TupleType {
                    for item in &input.children {
                        parameters.push(crate::types::Parameter {
                            name: Some(format!("x{}", parameters.len())),
                            ty: self.quantified_type(item, scope)?,
                            default: false,
                            passing: crate::types::Passing::Borrow,
                        });
                    }
                } else {
                    parameters.push(crate::types::Parameter {
                        name: Some("x".into()),
                        ty: self.quantified_type(input, scope)?,
                        default: false,
                        passing: crate::types::Passing::Borrow,
                    });
                }
                let result = self.quantified_type(&node.children[1], scope)?;
                Ok(Type::Function(crate::types::FunctionType {
                    parameters,
                    result: Box::new(result),
                    bounds: BTreeMap::new(),
                    requirements: vec![],
                    effects: false,
                    receiver_mutates: false,
                    callable: None,
                    receiver: None,
                }))
            }
            K::TypeApplication if !node.children.is_empty() => {
                let base = &node.children[0];
                let name = self.text(base);
                let id = self
                    .checked
                    .environment
                    .names
                    .get(name)
                    .map(String::as_str)
                    .unwrap_or(name);
                if matches!(id, "List" | "Option" | "Set") && node.children.len() == 2 {
                    let item = self.quantified_type(&node.children[1], scope)?;
                    return Ok(Type::applied(id, vec![item]));
                }
                // `Set[U]` is a proof-only type, so its universe only needs the
                // same reach the quantifier itself has.
                if id == "Set" && node.children.len() == 2 {
                    let item = self.quantified_type(&node.children[1], scope)?;
                    return Ok(Type::applied("Set", vec![item]));
                }
                let Some(definition) = self.checked.environment.definitions.get(id) else {
                    return fail(
                        node.span,
                        "E_PROOF_UNSUPPORTED",
                        format!("The kernel cannot quantify over `{name}`."),
                        "Use a readable generic class or record type.",
                    );
                };
                if definition.kind != K::Class
                    || definition.parameters.len() != node.children.len() - 1
                {
                    return fail(
                        node.span,
                        "E_PROOF_UNSUPPORTED",
                        format!("The kernel cannot quantify over this application of `{name}`."),
                        "Use a readable class with all type arguments specified.",
                    );
                }
                let args = node.children[1..]
                    .iter()
                    .map(|arg| self.quantified_type(arg, scope))
                    .collect::<Found<Vec<_>>>()?;
                let ty = Type::Named(id.to_owned(), args);
                self.plain_class(&ty).map(|_| ty.clone()).map_err(|reason| {
                    Diagnostic::new(
                        "E_PROOF_UNSUPPORTED",
                        format!("The kernel cannot quantify over `{ty}` because {reason}."),
                        node.span,
                    )
                    .with_hint("Use a readable class or record with supported field types.")
                })
            }
            _ => fail(
                node.span,
                "E_PROOF_UNSUPPORTED",
                "The kernel cannot quantify over this type expression.",
                "Quantify over Nat, Int, Bool, an enum or a readable class.",
            ),
        }
    }

    fn quantified(&mut self, node: &Node, forall: bool, scope: &Scope) -> Found<Prop> {
        let (body, binders) = node.children.split_last().ok_or_else(|| {
            Diagnostic::new("E_PROOF_STATEMENT", "A quantifier needs a body.", node.span)
        })?;
        let mut inner = scope.clone();
        let mut bound = vec![];
        for binder in binders {
            let name = binder
                .child(K::Name)
                .map(|n| self.text(n).to_owned())
                .ok_or_else(|| {
                    Diagnostic::new(
                        "E_PROOF_STATEMENT",
                        "A quantifier needs a name.",
                        binder.span,
                    )
                })?;
            let ty_node = binder
                .children
                .iter()
                .find(|n| n.kind != K::Name)
                .ok_or_else(|| {
                    Diagnostic::new(
                        "E_PROOF_STATEMENT",
                        "A quantifier needs a type.",
                        binder.span,
                    )
                })?;
            let ty = self.quantified_type(ty_node, scope)?;
            // Keep projections as operations on the binder itself. This lets
            // substitution at a theorem citation replace the whole class value
            // (and its fields together) without leaving free `x.field` names.
            inner = inner.with(&name, Term::Var(name.clone()), ty.clone());
            bound.push((name, ty));
        }
        let mut result = self.prop(body, &inner)?;
        for (name, ty) in bound.into_iter().rev() {
            result = if forall {
                Prop::Forall {
                    name,
                    ty,
                    body: Box::new(result),
                }
            } else {
                Prop::Exists {
                    name,
                    ty,
                    body: Box::new(result),
                }
            };
        }
        Ok(result)
    }

    fn holds(&mut self, node: &Node, scope: &Scope) -> Found<Prop> {
        let (term, ty) = self.term(node, Some(&Type::named("Bool")), scope)?;
        if ty != Type::named("Bool") {
            return fail(
                node.span,
                "E_PROOF_TYPE",
                format!("A statement must be true or false, but this is {ty}."),
                "Write an equality with `=`, a comparison, or a Bool.",
            );
        }
        Ok(Prop::Holds(term))
    }

    fn dispatches_in_prop(&self, prop: &Prop) -> Vec<Dispatch> {
        fn visit(
            term: &Term,
            kernel: &Kernel<'_>,
            found: &mut Vec<Dispatch>,
            seen: &mut BTreeSet<(DeclarationId, String)>,
        ) {
            match term {
                Term::Call {
                    function,
                    name,
                    args,
                    dispatch,
                } => {
                    if let Some(receiver) = dispatch
                        && Kernel::class_type(receiver).is_some_and(|(id, _)| {
                            kernel
                                .checked
                                .environment
                                .definitions
                                .get(id)
                                .is_some_and(|definition| definition.kind == K::Class)
                        })
                    {
                        let dependency = Dispatch {
                            receiver: receiver.as_ref().clone(),
                            method: name.clone(),
                            function: function.clone(),
                        };
                        if !found.contains(&dependency) {
                            found.push(dependency);
                        }
                    }
                    args.iter().for_each(|arg| visit(arg, kernel, found, seen));
                    let receiver = dispatch
                        .as_ref()
                        .map(ToString::to_string)
                        .unwrap_or_default();
                    let key = (function.clone(), receiver);
                    if let Some(definition) = kernel.functions.get(function)
                        && seen.insert(key)
                    {
                        let map = definition
                            .params
                            .iter()
                            .cloned()
                            .zip(args.iter().cloned())
                            .collect();
                        let mut body = definition.body.subst(&map, &mut 0);
                        if trait_method_default(function, &kernel.checked.environment.definitions)
                            && let Some(receiver) = dispatch
                        {
                            body = specialize_trait_calls(
                                &body,
                                receiver,
                                args.first(),
                                &kernel.functions,
                                &kernel.checked.environment.definitions,
                            );
                        }
                        visit(&body, kernel, found, seen);
                        for (required, _) in &definition.requires {
                            if let Some(theorem) = kernel.exported.get(required) {
                                for dependency in &theorem.theorem.dispatches {
                                    if !found.contains(dependency) {
                                        found.push(dependency.clone());
                                    }
                                }
                            }
                        }
                    }
                }
                Term::Ctor { args, .. } | Term::Opaque { args, .. } | Term::Op(_, args) => {
                    args.iter().for_each(|arg| visit(arg, kernel, found, seen));
                }
                Term::If(condition, yes, no) => {
                    visit(condition, kernel, found, seen);
                    visit(yes, kernel, found, seen);
                    visit(no, kernel, found, seen);
                }
                Term::Match(value, arms) => {
                    visit(value, kernel, found, seen);
                    arms.iter()
                        .for_each(|(_, body)| visit(body, kernel, found, seen));
                }
                Term::FamilyUnion { set, .. } | Term::FamilyInter { set, .. } => {
                    visit(set, kernel, found, seen);
                }
                Term::Finite(elements) => {
                    elements
                        .iter()
                        .for_each(|element| visit(element, kernel, found, seen));
                }
                Term::Builder { body, .. } => {
                    body.map(&mut |inner| {
                        visit(inner, kernel, found, seen);
                        inner.clone()
                    });
                }
                _ => {}
            }
        }
        let mut found = vec![];
        let mut seen = BTreeSet::new();
        prop.map(&mut |term| {
            visit(term, self, &mut found, &mut seen);
            term.clone()
        });
        found
    }

    /// A statement may name only functions the kernel can unfold (settled under Theorems).
    fn transparent(&self, p: &Prop, span: Span) -> Found<()> {
        let mut problem = None;
        let mut seen = BTreeSet::new();
        p.map(&mut |t| {
            fn visit(
                t: &Term,
                kernel: &Kernel<'_>,
                problem: &mut Option<String>,
                seen: &mut BTreeSet<(DeclarationId, String)>,
            ) {
                match t {
                    Term::Call { function, name, args, dispatch } => {
                        if let Some(reason) = kernel.opaque.get(function) {
                            problem.get_or_insert(format!("names `{name}`, which the kernel cannot unfold: {reason}"));
                        } else if !kernel.functions.contains_key(function) {
                            problem.get_or_insert(format!("names `{name}`, whose definition is unavailable to the kernel"));
                        }
                        args.iter().for_each(|a| visit(a, kernel, problem, seen));
                        let receiver = dispatch.as_ref().map(ToString::to_string).unwrap_or_default();
                        if let Some(definition) = kernel.functions.get(function)
                            && seen.insert((function.clone(), receiver))
                        {
                            let map = definition.params.iter().cloned()
                                .zip(args.iter().cloned()).collect();
                            let mut body = definition.body.subst(&map, &mut 0);
                            if trait_method_default(function, &kernel.checked.environment.definitions)
                                && let Some(receiver) = dispatch
                            {
                                body = specialize_trait_calls(
                                    &body,
                                    receiver,
                                    args.first(),
                                    &kernel.functions,
                                    &kernel.checked.environment.definitions,
                                );
                            }
                            visit(&body, kernel, problem, seen);
                        }
                    }
                    Term::Opaque { name, args } => {
                        if !kernel.declared_opaque_names.contains(name) {
                            problem.get_or_insert(format!("names `{name}`, which is not a supported visible pure function, so the kernel cannot unfold it"));
                        }
                        args.iter().for_each(|a| visit(a, kernel, problem, seen));
                    }
                    Term::Ctor { args, .. } | Term::Op(_, args) => args.iter().for_each(|a| visit(a, kernel, problem, seen)),
                    Term::App(function, args) => {
                        visit(function, kernel, problem, seen);
                        args.iter().for_each(|a| visit(a, kernel, problem, seen));
                    }
                    Term::Lam(_, body) => visit(body, kernel, problem, seen),
                    Term::Prop(prop) => {
                        prop.map(&mut |inner| {
                            visit(inner, kernel, problem, seen);
                            inner.clone()
                        });
                    }
                    Term::If(c, a, b) => [c, a, b].iter().for_each(|x| visit(x, kernel, problem, seen)),
                    Term::Match(s, arms) => {
                        visit(s, kernel, problem, seen);
                        arms.iter().for_each(|(_, b)| visit(b, kernel, problem, seen));
                    }
                    Term::FamilyUnion { set, .. } | Term::FamilyInter { set, .. } => {
                        visit(set, kernel, problem, seen);
                    }
                    Term::Finite(elements) => {
                        elements.iter().for_each(|element| visit(element, kernel, problem, seen));
                    }
                    Term::Builder { body: inner, .. } => {
                        inner.map(&mut |term| {
                            visit(term, kernel, problem, seen);
                            term.clone()
                        });
                    }
                    _ => {}
                }
            }
            visit(t, self, &mut problem, &mut seen);
            t.clone()
        });
        match problem {
            Some(problem) => fail(
                span,
                "E_PROOF_OPAQUE",
                format!("This statement {problem}."),
                "A claim about anything else is a test: write it as @test.",
            ),
            None => Ok(()),
        }
    }

    fn theorem_name(&self, node: &Node) -> String {
        node.child(K::Name)
            .map(|n| self.text(n).to_owned())
            .unwrap_or_else(|| format!("#example{}", node.span.start))
    }

    /// The header of a theorem: its name, parameters, assumptions and statement.
    fn theorem_parts(&mut self, node: &Node) -> Found<(String, Theorem, Context, Prop)> {
        let name = self.theorem_name(node);
        let mut ctx = Context::default();
        ctx.scope.bounds = self
            .checked
            .theorem_bounds
            .get(&node.span)
            .cloned()
            .unwrap_or_default();
        let mut params = vec![];
        let mut requires = vec![];
        let mut presupposes = false;
        for parameter in node
            .child(K::ParameterList)
            .map(|p| p.children.as_slice())
            .unwrap_or(&[])
        {
            let pname = parameter
                .child(K::Name)
                .map(|n| self.text(n).to_owned())
                .unwrap_or_default();
            if let Some(claim) = parameter.child(K::Proposition) {
                let claim = claim.children.first().unwrap_or(claim);
                let prop = self.prop(claim, &ctx.scope)?;
                self.transparent(&prop, claim.span)?;
                presupposes |= self
                    .with_runtime_bounds(|kernel| kernel.defined_prop(&prop, &ctx, claim.span))
                    .is_err();
                ctx.hyps.push((pname.clone(), prop.clone()));
                requires.push((pname, prop));
                continue;
            }
            let ty = self
                .checked
                .binding_types
                .get(&parameter.span)
                .cloned()
                .unwrap_or(Type::Invalid);
            if !self.readable(&ty) {
                return fail(
                    parameter.span,
                    "E_PROOF_UNSUPPORTED",
                    format!(
                        "The kernel reasons about Nat, Int, Bool, List, Option, enums, class values and type variables, not {ty}."
                    ),
                    "Change the parameter type, or write the claim as @test.",
                );
            }
            self.bind_class_parameter(&mut ctx.scope, &pname, &ty);
            params.push((pname, ty));
        }
        for requirement in node.children.iter().filter(|n| n.kind == K::Requires) {
            let hname = self.text(&requirement.children[0]).to_owned();
            let prop = self.prop(&requirement.children[1], &ctx.scope)?;
            self.transparent(&prop, requirement.span)?;
            // An assumption carries its own definedness (Q82): inside the proof,
            // `add(a, b) < m` also means add(a, b) succeeds.
            presupposes |= self
                .with_runtime_bounds(|kernel| kernel.defined_prop(&prop, &ctx, requirement.span))
                .is_err();
            ctx.hyps.push((hname.clone(), prop.clone()));
            requires.push((hname, prop));
        }
        let Some(statement) = node.child(K::Proposition).and_then(|p| p.children.first()) else {
            return fail(
                node.span,
                "E_PROOF_STATEMENT",
                "This theorem has no statement.",
                "Write `: statement` after the parameters.",
            );
        };
        let statement_span = statement.span;
        let statement = self.prop(statement, &ctx.scope)?;
        self.transparent(&statement, statement_span)?;
        ctx.hyps = requires.clone();
        let mut dispatches = self.dispatches_in_prop(&statement);
        for (_, requirement) in &requires {
            for dispatch in self.dispatches_in_prop(requirement) {
                if !dispatches.contains(&dispatch) {
                    dispatches.push(dispatch);
                }
            }
        }
        ctx.assumptions = requires.clone();
        let theorem = Theorem {
            params,
            bounds: ctx.scope.bounds.clone(),
            requires,
            statement: statement.clone(),
            dispatches,
            presupposes,
            admitted: false,
            trusted_smt: false,
        };
        if let Some(error) = dispatch_override_error(
            &theorem.dispatches,
            &self.checked.environment.definitions,
            node.span,
        ) {
            return Err(error);
        }
        Ok((name, theorem, ctx, statement))
    }

    pub(crate) fn theorem(&mut self, node: &Node) -> Found<()> {
        let name = self.theorem_name(node);
        if self.theorems.contains_key(&name) {
            let imported = self.checked.environment.theorems.contains_key(&name);
            return fail(
                node.span,
                "E_DUPLICATE_NAME",
                if imported {
                    format!("The theorem `{name}` is already imported.")
                } else {
                    format!("The theorem `{name}` is already defined.")
                },
                "Give each theorem its own name.",
            );
        }
        let (name, mut theorem, ctx, statement) = self.theorem_parts(node)?;
        let simp = node.children.iter().any(|child| {
            child.kind == K::Attribute
                && child.child(K::Name).is_some_and(|n| self.text(n) == "simp")
        });
        if simp && !matches!(statement, Prop::Eq(..)) {
            return fail(
                node.span,
                "E_PROOF_SIMP_RULE",
                "A @simp theorem must state an equality.",
                "Write a left-to-right equation, or leave this theorem unmarked.",
            );
        }
        if node.kind == K::Axiom {
            theorem.admitted = true;
            self.warnings.push(Diagnostic::warning(
                "W_PROOF_AXIOM",
                format!("`{name}` is an axiom: accepted without proof and not trusted for function requirements."),
                node.span,
            ));
            self.theorems.insert(name.clone(), theorem.clone());
            self.exported.insert(
                DeclarationId {
                    file: self.parsed.source.name.clone(),
                    span: node.span,
                },
                Exported {
                    theorem,
                    simp: false,
                },
            );
            return Ok(());
        }
        let Some(body) = node.child(K::Proof).and_then(|p| p.children.first()) else {
            return fail(
                node.span,
                "E_PROOF_STATEMENT",
                "This theorem has no proof.",
                "Write the proof in braces after the statement.",
            );
        };
        self.smt_trusted.set(false);
        let mut current = Current {
            name: name.clone(),
            start: node.span.start,
            theorem: theorem.clone(),
            candidates: None,
            admitted: false,
        };
        if body.kind == K::TacticProof {
            self.tactic_proof(body, &statement, &ctx, &mut current)?;
        } else {
            self.prove(body, &statement, &ctx, &mut current)?;
        }
        theorem.trusted_smt = self.smt_trusted.get();
        if current.admitted {
            theorem.admitted = true;
            // A proof that only cites an admitted theorem says so too (REVIEW2 section 2).
            let direct = self.warnings.iter().any(|w| {
                w.code == "W_PROOF_SORRY"
                    && node.span.start <= w.span.start
                    && w.span.end <= node.span.end
            });
            if !direct {
                self.warnings.push(Diagnostic::warning(
                    "W_PROOF_SORRY",
                    format!("`{name}` relies on a theorem whose proof uses `sorry` or is an axiom, so it is not trusted for function requirements."),
                    node.span,
                ));
            }
        }
        if current.candidates.as_ref().is_some_and(BTreeSet::is_empty) {
            return fail(
                node.span,
                "E_PROOF_RECURSION",
                format!(
                    "`{name}` cites itself without a structurally smaller argument at one fixed position."
                ),
                "Cite the theorem on a piece that a match took out of the same parameter each time.",
            );
        }
        if simp && !theorem.admitted {
            self.simp_theorems.push(name.clone());
        }
        // Unnamed examples retain proof audit metadata by identity, without
        // exposing a name that another module can import or cite.
        self.exported.insert(
            DeclarationId {
                file: self.parsed.source.name.clone(),
                span: node.span,
            },
            Exported {
                theorem: theorem.clone(),
                simp,
            },
        );
        self.theorems.insert(name, theorem);
        Ok(())
    }

    /// A theorem parameter type the kernel can reason about.
    fn readable(&self, ty: &Type) -> bool {
        match ty {
            Type::Parameter(name) => !name.starts_with('?'),
            Type::Rigid(_) => true,
            Type::Function(function) => {
                !function.effects
                    && function.requirements.is_empty()
                    && function.bounds.is_empty()
                    && function
                        .parameters
                        .iter()
                        .all(|parameter| self.readable(&parameter.ty))
                    && self.readable(&function.result)
            }
            Type::Named(id, args) => {
                if matches!(id.as_str(), "Nat" | "Int" | "Bool" | "Prop") {
                    return args.is_empty();
                }
                if matches!(id.as_str(), "Set" | "List" | "Option") && args.len() == 1 {
                    return self.readable(&args[0]);
                }
                self.checked
                    .environment
                    .definitions
                    .get(id)
                    .is_some_and(|d| {
                        matches!(d.kind, K::Enum | K::Class) && d.parameters.len() == args.len()
                    })
                    && args.iter().all(|a| self.readable(a))
            }
            _ => false,
        }
    }

    fn class_type(ty: &Type) -> Option<(&str, &[Type])> {
        match ty {
            Type::Borrow(inner) | Type::StoredView(inner) => Self::class_type(inner),
            Type::Named(id, args) => Some((id, args)),
            _ => None,
        }
    }

    /// Fields in the proof representation: parent storage first, then this
    /// class's fields in a stable order. The proof kernel uses names to project
    /// them, so imported definitions need not retain parser nodes.
    fn class_fields(&self, ty: &Type) -> Result<Vec<(String, Type)>, String> {
        let Some((id, args)) = Self::class_type(ty) else {
            return Err(format!("{ty} is not a class value"));
        };
        let Some(definition) = self.checked.environment.definitions.get(id) else {
            return Err(format!("class `{id}` has no checked definition"));
        };
        if definition.kind != K::Class || definition.parameters.len() != args.len() {
            return Err(format!("`{id}` is not a concrete class"));
        }
        let substitutions = definition
            .parameters
            .iter()
            .cloned()
            .zip(args.iter().cloned())
            .collect::<BTreeMap<_, _>>();
        let mut fields = if let Some(parent) = &definition.parent {
            let parent = parent.replace_parameters(&substitutions);
            if parent == Type::named("Object") {
                vec![]
            } else {
                self.class_fields(&parent)?
            }
        } else {
            vec![]
        };
        for (name, field) in &definition.fields {
            if fields.iter().any(|(inherited, _)| inherited == name) {
                return Err(format!("field `{name}` shadows an inherited field"));
            }
            fields.push((name.clone(), field.replace_parameters(&substitutions)));
        }
        Ok(fields)
    }

    fn own_class_fields(&self, ty: &Type) -> Result<Vec<(String, Type)>, String> {
        let Some((id, args)) = Self::class_type(ty) else {
            return Err(format!("{ty} is not a class value"));
        };
        let Some(definition) = self.checked.environment.definitions.get(id) else {
            return Err(format!("class `{id}` has no checked definition"));
        };
        if definition.kind != K::Class || definition.parameters.len() != args.len() {
            return Err(format!("`{id}` is not a concrete class"));
        }
        let substitutions = definition
            .parameters
            .iter()
            .cloned()
            .zip(args.iter().cloned())
            .collect::<BTreeMap<_, _>>();
        let names = own_field_names(definition);
        Ok(names
            .into_iter()
            .filter_map(|name| {
                definition
                    .fields
                    .get(&name)
                    .map(|field| (name, field.replace_parameters(&substitutions)))
            })
            .collect())
    }

    fn class_field_path(&self, ty: &Type, field: &str) -> Result<(Vec<usize>, Type), String> {
        let Some((id, args)) = Self::class_type(ty) else {
            return Err(format!("{ty} is not a class value"));
        };
        let Some(definition) = self.checked.environment.definitions.get(id) else {
            return Err(format!("class `{id}` has no checked definition"));
        };
        if definition.kind != K::Class || definition.parameters.len() != args.len() {
            return Err(format!("`{id}` is not a concrete class"));
        }
        let substitutions = definition
            .parameters
            .iter()
            .cloned()
            .zip(args.iter().cloned())
            .collect::<BTreeMap<_, _>>();
        let parent = definition
            .parent
            .as_ref()
            .map(|parent| parent.replace_parameters(&substitutions))
            .filter(|parent| *parent != Type::named("Object"));
        let own = self.own_class_fields(ty)?;
        if let Some(index) = own.iter().position(|(name, _)| name == field) {
            let (_, field_ty) = &own[index];
            return Ok((
                vec![usize::from(parent.is_some()) + index],
                field_ty.clone(),
            ));
        }
        let Some(parent) = parent else {
            return Err(format!("class `{id}` has no field `{field}`"));
        };
        let (mut path, field_type) = self.class_field_path(&parent, field)?;
        path.insert(0, 0);
        Ok((path, field_type))
    }

    fn class_field_visibility(
        &self,
        ty: &Type,
        field: &str,
    ) -> Option<(String, crate::checker::Visibility)> {
        let (id, args) = Self::class_type(ty)?;
        let definition = self.checked.environment.definitions.get(id)?;
        let substitutions = definition
            .parameters
            .iter()
            .cloned()
            .zip(args.iter().cloned())
            .collect::<BTreeMap<_, _>>();
        if definition.fields.contains_key(field) {
            return Some((
                id.to_owned(),
                definition
                    .member_visibility
                    .get(field)
                    .copied()
                    .unwrap_or(crate::checker::Visibility::Private),
            ));
        }
        let parent = definition
            .parent
            .as_ref()?
            .replace_parameters(&substitutions);
        if parent == Type::named("Object") {
            return None;
        }
        self.class_field_visibility(&parent, field)
    }

    fn proof_can_access(&self, owner: &str, member: &str) -> bool {
        let Some(definition) = self.checked.environment.definitions.get(owner) else {
            return false;
        };
        if definition.kind == K::Trait {
            return true;
        }
        match definition
            .member_visibility
            .get(member)
            .copied()
            .unwrap_or(crate::checker::Visibility::Private)
        {
            crate::checker::Visibility::Public => true,
            crate::checker::Visibility::Package => {
                std::path::Path::new(&definition.file).parent()
                    == std::path::Path::new(&self.parsed.source.name).parent()
            }
            crate::checker::Visibility::Private => self.access_owner.as_deref() == Some(owner),
        }
    }

    fn plain_class(&self, ty: &Type) -> Result<Vec<(String, Type)>, String> {
        self.plain_class_inner(ty, &mut BTreeSet::new())
    }

    fn plain_class_inner(
        &self,
        ty: &Type,
        seen: &mut BTreeSet<String>,
    ) -> Result<Vec<(String, Type)>, String> {
        let Some((id, _)) = Self::class_type(ty) else {
            return Err(format!("{ty} is not a class value"));
        };
        let Some(definition) = self.checked.environment.definitions.get(id) else {
            return Err(format!("class `{id}` has no checked definition"));
        };
        if definition.kind != K::Class {
            return Err(format!("`{id}` is not a class"));
        }
        if !seen.insert(id.to_owned()) {
            return Err(format!("class field `{id}` recursively contains itself"));
        }
        if let Some(field) = definition.mutable_fields.iter().next() {
            return Err(format!("field `{field}` is mutable"));
        }
        let fields = self.class_fields(ty)?;
        for (field_name, field_type) in &fields {
            fn check_field(
                kernel: &Kernel<'_>,
                ty: &Type,
                seen: &mut BTreeSet<String>,
            ) -> Result<(), String> {
                match ty {
                    Type::Parameter(name) if !name.starts_with('?') => Ok(()),
                    Type::Rigid(_) => Ok(()),
                    Type::Named(id, args) if matches!(id.as_str(), "Int" | "Nat" | "Bool") => {
                        if args.is_empty() {
                            Ok(())
                        } else {
                            Err(format!("{ty} has type arguments"))
                        }
                    }
                    Type::Named(id, args) if id == "Set" && args.len() == 1 => {
                        check_field(kernel, &args[0], seen)
                    }
                    Type::Named(id, args) => {
                        let Some(definition) = kernel.checked.environment.definitions.get(id)
                        else {
                            return Err(format!("type `{id}` has no checked definition"));
                        };
                        if definition.parameters.len() != args.len() {
                            return Err(format!("`{id}` has the wrong number of type arguments"));
                        }
                        if definition.kind == K::Enum {
                            return Ok(());
                        }
                        if definition.kind != K::Class {
                            return Err(format!("{id} is not a readable class or enum"));
                        }
                        kernel.plain_class_inner(ty, seen).map(|_| ())
                    }
                    _ => Err(format!("{ty} is not a readable field type")),
                }
            }
            check_field(self, field_type, seen)
                .map_err(|reason| format!("field `{field_name}` {reason}"))?;
        }
        let mut parent = definition.parent.as_ref().map(|parent| {
            let substitutions = definition
                .parameters
                .iter()
                .cloned()
                .zip(
                    Self::class_type(ty)
                        .map(|(_, args)| args.to_vec())
                        .unwrap_or_default(),
                )
                .collect::<BTreeMap<_, _>>();
            parent.replace_parameters(&substitutions)
        });
        if let Some(parent_type) = parent.take()
            && parent_type != Type::named("Object")
        {
            self.plain_class_inner(&parent_type, seen)?;
        }
        for constructor in &definition.constructors {
            match &constructor.callable {
                Some(Callable::Builtin(name)) if name.starts_with("record.") => {}
                Some(Callable::Declaration {
                    id,
                    kind: DeclarationKind::Constructor,
                    ..
                }) => {
                    if !self.functions.contains_key(id) {
                        let reason = self
                            .opaque
                            .get(id)
                            .cloned()
                            .unwrap_or_else(|| "the constructor body is not readable".into());
                        let (location, detail) = reason
                            .split_once(": ")
                            .map(|(location, detail)| (location.to_owned(), detail.to_owned()))
                            .unwrap_or_else(|| (id.file.clone(), reason));
                        return Err(format!("constructor at {location} is unreadable: {detail}"));
                    }
                }
                _ => return Err("a constructor has no readable body".into()),
            }
        }
        seen.remove(id);
        Ok(fields)
    }

    fn symbolic_value(&self, name: &str, ty: &Type) -> Option<Term> {
        fn build(
            kernel: &Kernel<'_>,
            name: &str,
            ty: &Type,
            seen: &mut BTreeSet<String>,
        ) -> Option<Term> {
            let (id, args) = Kernel::class_type(ty)?;
            let definition = kernel.checked.environment.definitions.get(id)?;
            if definition.kind != K::Class
                || definition.parameters.len() != args.len()
                || !definition.mutable_fields.is_empty()
                || !seen.insert(id.to_owned())
            {
                return None;
            }
            let substitutions = definition
                .parameters
                .iter()
                .cloned()
                .zip(args.iter().cloned())
                .collect::<BTreeMap<_, _>>();
            let parent = definition
                .parent
                .as_ref()
                .map(|parent| parent.replace_parameters(&substitutions))
                .filter(|parent| *parent != Type::named("Object"))
                // Parent fields share the subclass's surface projection names;
                // storage nesting is carried by the constructor path only.
                .map(|parent| build(kernel, name, &parent, seen));
            let parent = match parent {
                Some(Some(parent)) => Some(parent),
                Some(None) => return None,
                None => None,
            };
            let fields = kernel.own_class_fields(ty).ok()?;
            let mut values = Vec::with_capacity(fields.len() + usize::from(parent.is_some()));
            if let Some(parent) = parent {
                values.push(parent);
            }
            for (field, field_type) in fields {
                let field_name = format!("{name}.{field}");
                let value = if Kernel::class_type(&field_type).is_some() {
                    build(kernel, &field_name, &field_type, seen)
                        .unwrap_or_else(|| Term::Var(field_name.clone()))
                } else {
                    Term::Var(field_name)
                };
                values.push(value);
            }
            seen.remove(id);
            Some(Term::Ctor {
                ty: id.to_owned(),
                case: "new".into(),
                args: values,
            })
        }
        build(self, name, ty, &mut BTreeSet::new())
    }

    fn projection_specs(&self, ty: &Type, prefix: &str) -> Vec<(String, Type, Vec<usize>)> {
        fn walk(
            kernel: &Kernel<'_>,
            ty: &Type,
            prefix: &str,
            seen: &mut BTreeSet<String>,
            out: &mut Vec<(String, Type, Vec<usize>)>,
        ) {
            let Some((id, _)) = Kernel::class_type(ty) else {
                return;
            };
            if !seen.insert(id.to_owned()) {
                return;
            }
            if let Ok(fields) = kernel.class_fields(ty) {
                for (field, field_ty) in fields {
                    let Ok((path, _)) = kernel.class_field_path(ty, &field) else {
                        continue;
                    };
                    let name = format!("{prefix}.{field}");
                    out.push((name.clone(), field_ty.clone(), path.clone()));
                    if Kernel::class_type(&field_ty).is_some() {
                        let start = out.len();
                        walk(kernel, &field_ty, &name, seen, out);
                        for (_, _, child_path) in &mut out[start..] {
                            let mut full = path.clone();
                            full.extend(child_path.iter().copied());
                            *child_path = full;
                        }
                    }
                }
            }
            seen.remove(id);
        }
        let mut out = vec![];
        walk(self, ty, prefix, &mut BTreeSet::new(), &mut out);
        out
    }

    fn project_value(&self, value: Term, ty: &Type, field: &str) -> Result<(Term, Type), String> {
        let (path, field_ty) = self.class_field_path(ty, field)?;
        Ok((self.project_path_value(value, &path, field), field_ty))
    }

    fn project_path_value(&self, value: Term, path: &[usize], shown_path: &str) -> Term {
        Term::Op(field_marker(shown_path, path), vec![value])
    }

    fn local_class_node(&self, id: &str) -> Option<&Node> {
        self.parsed.root.children.iter().find(|node| {
            if node.kind != K::Class {
                return false;
            }
            let Some(name) = node.child(K::Name).map(|name| self.text(name)) else {
                return false;
            };
            self.checked
                .environment
                .names
                .get(name)
                .is_some_and(|resolved| resolved == id)
                || name == id
        })
    }

    fn class_initializer(&mut self, ty: &Type, field: &str, scope: &Scope) -> Found<Option<Term>> {
        let Some((id, _)) = Self::class_type(ty) else {
            return Ok(None);
        };
        let Some(binding) = self.local_class_node(id).and_then(|class_node| {
            class_node
                .children
                .iter()
                .find(|member| {
                    member.kind == K::Binding
                        && member
                            .child(K::Name)
                            .is_some_and(|name| self.text(name) == field)
                })
                .cloned()
        }) else {
            return Ok(None);
        };
        let Some(field_annotation) = binding.children.iter().find(|child| {
            matches!(
                child.kind,
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
            )
        }) else {
            return Ok(None);
        };
        let Some(initializer) = binding
            .children
            .last()
            .filter(|last| last.span.start >= field_annotation.span.end)
        else {
            return Ok(None);
        };
        let Some((_, field_type)) = self
            .own_class_fields(ty)
            .ok()
            .and_then(|fields| fields.into_iter().find(|(name, _)| name == field))
        else {
            return Ok(None);
        };
        self.term(initializer, Some(&field_type), scope)
            .map(|(value, _)| Some(value))
    }

    fn default_instance(
        &mut self,
        ty: &Type,
        scope: &Scope,
        seen: &mut BTreeSet<String>,
    ) -> Found<Option<Term>> {
        let Some((id, args)) = Self::class_type(ty) else {
            return Ok(None);
        };
        if !seen.insert(id.to_owned()) {
            return Ok(None);
        }
        let Some(definition) = self.checked.environment.definitions.get(id).cloned() else {
            seen.remove(id);
            return Ok(None);
        };
        if definition.kind != K::Class || definition.parameters.len() != args.len() {
            seen.remove(id);
            return Ok(None);
        }
        let substitutions = definition
            .parameters
            .iter()
            .cloned()
            .zip(args.iter().cloned())
            .collect::<BTreeMap<_, _>>();
        let parent_type = definition
            .parent
            .as_ref()
            .map(|parent| parent.replace_parameters(&substitutions))
            .filter(|parent| *parent != Type::named("Object"));
        let parent = if let Some(parent_type) = parent_type {
            match self.default_instance(&parent_type, scope, seen)? {
                Some(parent) => Some(parent),
                None => {
                    seen.remove(id);
                    return Ok(None);
                }
            }
        } else {
            None
        };
        let fields = self.own_class_fields(ty).unwrap_or_default();
        let mut values = vec![];
        if let Some(parent) = parent {
            values.push(parent);
        }
        for (field, field_type) in fields {
            let Some(value) = self.class_initializer(ty, &field, scope)? else {
                seen.remove(id);
                return Ok(None);
            };
            if !self.readable(&field_type) {
                seen.remove(id);
                return Ok(None);
            }
            values.push(value);
        }
        seen.remove(id);
        Ok(Some(self.ctor(id, "new", values)))
    }

    fn hidden_arguments(
        &self,
        function: &DeclarationId,
        visible: &[(String, Type, Term)],
        scope: &Scope,
    ) -> Vec<Term> {
        let Some(definition) = self.functions.get(function) else {
            return vec![];
        };
        let visible_count = visible.len();
        definition
            .params
            .iter()
            .skip(visible_count)
            .map(|hidden| {
                for (root, ty, value) in visible {
                    let Some(suffix) = hidden.strip_prefix(&format!("{root}.")) else {
                        continue;
                    };
                    if let Some((_, _, path)) = self
                        .projection_specs(ty, root)
                        .into_iter()
                        .find(|(name, _, _)| name == hidden)
                    {
                        return self.project_path_value(value.clone(), &path, suffix);
                    }
                }
                // A scope alias is only a fallback for a hidden value with no
                // visible root. If a visible argument or receiver matches the
                // root, always derive the field from that actual term above;
                // the caller may have a same-spelled field for a different
                // value.
                if let Some((_, term, _)) = scope.get(hidden) {
                    return term.clone();
                }
                Term::Var(hidden.clone())
            })
            .collect()
    }

    /// Adds substitutions for the symbolic field variables used while a
    /// theorem was checked. This keeps a theorem argument's fields attached
    /// to that exact receiver when cited, imported, or required by a function.
    fn add_class_projection_substitutions(
        &self,
        params: &[(String, Type)],
        arguments: &BTreeMap<String, Term>,
        substitutions: &mut BTreeMap<String, Term>,
    ) {
        for (name, ty) in params {
            let Some(value) = arguments.get(name) else {
                continue;
            };
            for (projected, _, path) in self.projection_specs(ty, name) {
                let suffix = projected
                    .strip_prefix(&format!("{name}."))
                    .unwrap_or(&projected)
                    .to_owned();
                substitutions.insert(
                    projected,
                    self.project_path_value(value.clone(), &path, &suffix),
                );
            }
        }
    }

    fn mismatch<T>(&mut self, span: Span, goal: &Prop, found: &Prop) -> Found<T> {
        let goal = self.normal(goal);
        let found = self.normal(found);
        if let Prop::Holds(term) = &found
            && matches!(term, Term::Nat(_) | Term::Int(_) | Term::Ctor { .. })
        {
            return fail(
                span,
                "E_PROOF_VALUE",
                format!("This is the value `{term}`, not a proof of `{goal}`."),
                "A proof position needs a proposition and a proof of it, such as `h` or `rfl`.",
            );
        }
        fail(
            span,
            "E_PROOF_MISMATCH",
            format!("This proves `{found}`, but the goal is `{goal}`."),
            "Both sides are shown after the kernel unfolds the pure functions it can.",
        )
    }

    /// Checks that `node` proves `goal`.
    fn prove(
        &mut self,
        node: &Node,
        goal: &Prop,
        ctx: &Context,
        current: &mut Current,
    ) -> Found<()> {
        match node.kind {
            K::Group => self.prove(&node.children[0], goal, ctx, current),
            K::Block => {
                let Some((last, lets)) = node.children.split_last() else {
                    return fail(
                        node.span,
                        "E_PROOF_SHAPE",
                        "This proof is empty.",
                        "Write a proof term in the braces, or use `by { ... }` for a tactic proof.",
                    );
                };
                let mut ctx = ctx.clone();
                for statement in lets {
                    if statement.kind != K::Binding {
                        return fail(
                            statement.span,
                            "E_PROOF_SHAPE",
                            "Only `let` may come before the final proof term.",
                            "Name intermediate facts with let.",
                        );
                    }
                    let pattern = &statement.children[0];
                    let prop = self.infer(statement.children.last().unwrap(), &ctx, current)?;
                    match pattern.kind {
                        K::PatternName => ctx.hyps.push((self.text(pattern).to_owned(), prop)),
                        K::PatternTuple if pattern.children.len() == 2 => {
                            let Prop::And(a, b) = self.normal(&prop) else {
                                return fail(
                                    statement.span,
                                    "E_PROOF_SHAPE",
                                    format!(
                                        "Only a proof of `P and Q` splits into two names; this proves `{prop}`."
                                    ),
                                    "Split an `and` proof, or bind one name.",
                                );
                            };
                            for (child, part) in pattern.children.iter().zip([*a, *b]) {
                                if child.kind == K::PatternName {
                                    ctx.hyps.push((self.text(child).to_owned(), part));
                                }
                            }
                        }
                        _ => {
                            return fail(
                                pattern.span,
                                "E_PROOF_SHAPE",
                                "A proof `let` binds one name or a pair.",
                                "Write `let h := ...` or `let (p, q) := ...`.",
                            );
                        }
                    }
                }
                self.prove(last, goal, &ctx, current)
            }
            K::Name if self.text(node) == "True" => {
                if matches!(goal, Prop::True) {
                    self.defined_prop(goal, ctx, node.span)
                } else {
                    fail(
                        node.span,
                        "E_PROOF_SHAPE",
                        format!("`True` proves true, but the goal is `{goal}`."),
                        "Use True only for a goal written true.",
                    )
                }
            }
            K::Name if self.text(node) == "sorry" => {
                current.admitted = true;
                self.warnings.push(Diagnostic::warning(
                    "W_PROOF_SORRY",
                    "This proof uses `sorry`; the theorem is not trusted for function requirements.",
                    node.span,
                ));
                Ok(())
            }
            K::Name if self.text(node) == "rfl" => {
                if matches!(
                    self.normal(goal),
                    Prop::Holds(Term::Bool(true)) | Prop::True
                ) {
                    return self.defined_prop(goal, ctx, node.span);
                }
                let Prop::Eq(a, b) = goal else {
                    return fail(
                        node.span,
                        "E_PROOF_SHAPE",
                        format!("`rfl` proves an equality, but the goal is `{goal}`."),
                        "Use rfl for a goal written with `=`.",
                    );
                };
                if self.same(
                    &Prop::Eq(a.clone(), a.clone()),
                    &Prop::Eq(a.clone(), b.clone()),
                ) {
                    return self.defined_prop(goal, ctx, node.span);
                }
                let Prop::Eq(left, right) = self.normal(goal) else {
                    unreachable!()
                };
                fail(
                    node.span,
                    "E_PROOF_MISMATCH",
                    format!(
                        "`rfl` needs both sides to reduce to the same term, but they reduce to `{left}` and `{right}`."
                    ),
                    "Split a variable with match or induction so the functions can unfold.",
                )
            }
            K::Name if self.text(node) == "decide" => match self.decide(goal) {
                Some(true) => self.defined_prop(goal, ctx, node.span),
                Some(false) => fail(
                    node.span,
                    "E_PROOF_FALSE",
                    format!("`decide` computed `{goal}` and found it false."),
                    "Check the statement.",
                ),
                None => fail(
                    node.span,
                    "E_PROOF_SHAPE",
                    "`decide` proves a statement with no variables, such as `3 > 2`.",
                    "Split the variables first, or use another proof.",
                ),
            },
            K::Simp => self.simplify(node, goal, ctx, current),
            K::Smt => self.solve_arithmetic(node, goal, ctx),
            K::Tuple => self.prove_tuple(&node.children, node.span, goal, ctx, current),
            K::Lambda => {
                let name = node
                    .child(K::LambdaParameters)
                    .and_then(|p| p.children.first())
                    .and_then(|p| p.child(K::Name))
                    .map(|n| self.text(n).to_owned());
                let Some(name) = name else {
                    return fail(
                        node.span,
                        "E_PROOF_SHAPE",
                        "A proof lambda takes one parameter.",
                        "Write `h => ...`.",
                    );
                };
                let body = node.children.last().unwrap();
                match goal {
                    Prop::Not(inner) => {
                        let mut inner_ctx = ctx.clone();
                        inner_ctx.hyps.push((name, *inner.clone()));
                        self.prove(body, &Prop::False, &inner_ctx, current)?;
                        self.defined_prop(goal, ctx, node.span)
                    }
                    Prop::Implies(premise, conclusion) => {
                        let mut inner_ctx = ctx.clone();
                        inner_ctx.hyps.push((name, *premise.clone()));
                        self.prove(body, conclusion, &inner_ctx, current)?;
                        self.defined_prop(goal, ctx, node.span)
                    }
                    Prop::Forall {
                        name: bound,
                        ty,
                        body: inner,
                    } => {
                        let mut inner_ctx = ctx.clone();
                        inner_ctx.scope =
                            inner_ctx
                                .scope
                                .with(&name, Term::Var(name.clone()), ty.clone());
                        let mut map = BTreeMap::new();
                        map.insert(bound.clone(), Term::Var(name.clone()));
                        let mut fresh = self.fresh;
                        let goal = inner.subst_prop(&map, &mut fresh);
                        self.fresh = fresh;
                        self.prove(body, &goal, &inner_ctx, current)?;
                        self.defined_prop(
                            &Prop::Forall {
                                name: bound.clone(),
                                ty: ty.clone(),
                                body: Box::new(inner.as_ref().clone()),
                            },
                            ctx,
                            node.span,
                        )
                    }
                    _ => fail(
                        node.span,
                        "E_PROOF_SHAPE",
                        format!(
                            "A lambda proves `not P`, `P -> Q` or `forall x, P`, but the goal is `{goal}`."
                        ),
                        "Use a lambda only for those goals.",
                    ),
                }
            }
            K::Call => {
                let callee = crate::lexer::path_name(self.text(&node.children[0]));
                let args = self.arguments(node);
                match (callee.as_str(), args.as_slice()) {
                    ("exists" | "Exists" | "Witness", [witness, proof]) => {
                        let Prop::Exists { name, ty, body } = goal else {
                            return fail(
                                node.span,
                                "E_PROOF_SHAPE",
                                format!("`exists` proves `exists x, P`, but the goal is `{goal}`."),
                                "Use exists(witness, proof) only for an existential goal.",
                            );
                        };
                        let (witness, found) = self.term(witness, Some(ty), &ctx.scope)?;
                        let mut map = BTreeMap::new();
                        map.insert(name.clone(), witness);
                        let mut fresh = self.fresh;
                        let body = body.subst_prop(&map, &mut fresh);
                        self.fresh = fresh;
                        if !compatible(ty, &found) {
                            return fail(
                                node.children[1].span,
                                "E_PROOF_TYPE",
                                format!("This witness is {found}, but the goal needs {ty}."),
                                "Give a value of the quantified type.",
                            );
                        }
                        self.prove(proof, &body, ctx, current)?;
                        self.defined_prop(goal, ctx, node.span)
                    }
                    ("And", [left, right]) => {
                        let target = self.unfold_head(goal);
                        let Prop::And(a, b) = &target else {
                            return fail(
                                node.span,
                                "E_PROOF_SHAPE",
                                format!("`And` proves `P and Q`, but the goal is `{goal}`."),
                                "Use And only for a goal written with and.",
                            );
                        };
                        self.prove(left, a.as_ref(), ctx, current)?;
                        self.prove(right, b.as_ref(), ctx, current)?;
                        self.defined_prop(goal, ctx, node.span)
                    }
                    ("OrLeft" | "OrRight" | "Left" | "Right", [arg]) => {
                        let target = self.unfold_head(goal);
                        let Prop::Or(a, b) = &target else {
                            return fail(
                                node.span,
                                "E_PROOF_SHAPE",
                                format!("`{callee}` proves `P or Q`, but the goal is `{goal}`."),
                                "Use OrLeft or OrRight for a goal written with or.",
                            );
                        };
                        let side = if callee == "OrLeft" || callee == "Left" {
                            a.as_ref()
                        } else {
                            b.as_ref()
                        };
                        self.prove(arg, side, ctx, current)?;
                        self.defined_prop(goal, ctx, node.span)
                    }
                    ("Iff", [forward, backward]) => {
                        let target = self.unfold_head(goal);
                        let Prop::Iff(a, b) = &target else {
                            return fail(
                                node.span,
                                "E_PROOF_SHAPE",
                                format!("`Iff` proves `P ↔ Q`, but the goal is `{goal}`."),
                                "Use Iff only for a goal written with ↔.",
                            );
                        };
                        self.prove(forward, &Prop::Implies(a.clone(), b.clone()), ctx, current)?;
                        self.prove(backward, &Prop::Implies(b.clone(), a.clone()), ctx, current)?;
                        self.defined_prop(goal, ctx, node.span)
                    }
                    ("And" | "OrLeft" | "OrRight" | "Iff" | "exists" | "Exists" | "Witness", _) => {
                        let expected = match callee.as_str() {
                            "And" => "two proofs",
                            "Iff" => "two implications",
                            "exists" | "Exists" | "Witness" => "a witness and a proof",
                            _ => "one proof",
                        };
                        fail(
                            node.span,
                            "E_PROOF_SHAPE",
                            format!("`{callee}` takes {expected}."),
                            "Pass the constructor's arguments in order.",
                        )
                    }
                    ("absurd", [arg]) => {
                        let found = self.infer(arg, ctx, current)?;
                        match self.normal(&found) {
                            Prop::False | Prop::Holds(Term::Bool(false)) => {
                                self.defined_prop(goal, ctx, node.span)
                            }
                            Prop::Eq(a, b)
                                if is_value(&a) && is_value(&b) && head(&a) != head(&b) =>
                            {
                                self.defined_prop(goal, ctx, node.span)
                            }
                            other => fail(
                                node.span,
                                "E_PROOF_SHAPE",
                                format!(
                                    "`absurd` needs a contradiction or an equation between different cases, but its argument proves `{other}`."
                                ),
                                "Pass a proof such as `Zero = Succ(k)`, or apply a `not` proof to its claim.",
                            ),
                        }
                    }
                    ("symm", [arg]) => {
                        let Prop::Eq(a, b) = goal else {
                            return fail(
                                node.span,
                                "E_PROOF_SHAPE",
                                format!("`symm` proves an equality, but the goal is `{goal}`."),
                                "Use symm for a goal written with `=`.",
                            );
                        };
                        self.prove(arg, &Prop::Eq(b.clone(), a.clone()), ctx, current)
                    }
                    // With the goal known, one side of trans may be rfl or another goal-checked proof.
                    ("trans", [p, q]) if matches!(goal, Prop::Eq(..)) => {
                        let Prop::Eq(a, c) = goal else { unreachable!() };
                        if let Ok(Prop::Eq(start, middle)) = self.infer(p, ctx, current) {
                            if !self.same(
                                &Prop::Eq(a.clone(), c.clone()),
                                &Prop::Eq(start.clone(), c.clone()),
                            ) {
                                return self.mismatch(
                                    p.span,
                                    &Prop::Eq(a.clone(), middle.clone()),
                                    &Prop::Eq(start, middle),
                                );
                            }
                            self.prove(q, &Prop::Eq(middle, c.clone()), ctx, current)?;
                            return self.defined_from(
                                goal,
                                &Prop::Eq(start, c.clone()),
                                ctx,
                                node.span,
                            );
                        }
                        let Prop::Eq(middle, end) = self.infer(q, ctx, current)? else {
                            return fail(
                                node.span,
                                "E_PROOF_SHAPE",
                                "`trans` chains two equalities.",
                                "Pass proofs of `a = b` and `b = c`.",
                            );
                        };
                        if !self.same(
                            &Prop::Eq(c.clone(), c.clone()),
                            &Prop::Eq(c.clone(), end.clone()),
                        ) {
                            return self.mismatch(
                                q.span,
                                &Prop::Eq(middle.clone(), c.clone()),
                                &Prop::Eq(middle, end),
                            );
                        }
                        self.prove(p, &Prop::Eq(a.clone(), middle), ctx, current)?;
                        self.defined_from(goal, &Prop::Eq(a.clone(), end), ctx, node.span)
                    }
                    _ => {
                        let found = self.infer(node, ctx, current)?;
                        if self.same(&found, goal) {
                            self.defined_from(goal, &found, ctx, node.span)
                        } else {
                            self.mismatch(node.span, goal, &found)
                        }
                    }
                }
            }
            K::Match => self.split(node, goal, ctx, current, false),
            K::Induction => self.split(node, goal, ctx, current, true),
            _ => {
                let found = self.infer(node, ctx, current)?;
                if self.same(&found, goal) {
                    self.defined_from(goal, &found, ctx, node.span)
                } else {
                    self.mismatch(node.span, goal, &found)
                }
            }
        }
    }

    fn prove_tuple(
        &mut self,
        items: &[Node],
        span: Span,
        goal: &Prop,
        ctx: &Context,
        current: &mut Current,
    ) -> Found<()> {
        match items {
            [only] => self.prove(only, goal, ctx, current),
            [first, rest @ ..] => {
                let target = self.unfold_head(goal);
                let Prop::And(a, b) = &target else {
                    return fail(
                        span,
                        "E_PROOF_SHAPE",
                        format!("A tuple proves `P and Q`, but the goal is `{goal}`."),
                        "Use a tuple only for a goal written with and.",
                    );
                };
                self.prove(first, a, ctx, current)?;
                self.prove_tuple(rest, span, b, ctx, current)
            }
            [] => fail(
                span,
                "E_PROOF_SHAPE",
                "An empty tuple proves nothing.",
                "Write a proof term.",
            ),
        }
    }

    /// Works out what `node` proves.
    fn infer(&mut self, node: &Node, ctx: &Context, current: &mut Current) -> Found<Prop> {
        match node.kind {
            K::Group => self.infer(&node.children[0], ctx, current),
            K::Name if self.text(node) == "True" => Ok(Prop::True),
            K::Name => {
                let name = crate::lexer::path_name(self.text(node));
                self.fact(&name, node.span, ctx, current)
            }
            K::Member if self.projects_fact(&node.children[0], ctx, current) => {
                self.project(node, ctx, current)
            }
            K::Member => {
                let name = crate::lexer::path_name(self.text(node));
                self.fact(&name, node.span, ctx, current)
            }
            K::Call => {
                let callee = crate::lexer::path_name(self.text(&node.children[0]));
                let args = self.arguments(node);
                // A projected fact can take arguments too: `h.left(x)` applies
                // the left side of a conjunction.
                let projected = if node.children[0].kind == K::Member
                    && self.projects_fact(&node.children[0], ctx, current)
                {
                    Some(self.infer(&node.children[0], ctx, current)?)
                } else {
                    ctx.hyps
                        .iter()
                        .rev()
                        .find(|(n, _)| *n == callee)
                        .map(|(_, prop)| prop.clone())
                };
                if let Some(mut hypothesis) = projected {
                    for arg in &args {
                        hypothesis = self.unfold_head(&hypothesis);
                        match hypothesis {
                            Prop::Forall { name, ty, body } => {
                                // Lean writes `h1 h2` where h1 : A ⊆ B and
                                // h2 : x ∈ A. Infer the quantifier witness from
                                // the proof argument before trying a data argument.
                                if let Prop::Implies(premise, conclusion) = body.as_ref()
                                    && let Ok(actual) = self.infer(arg, ctx, current)
                                {
                                    let holes = BTreeSet::from([name.clone()]);
                                    let mut binding = BTreeMap::new();
                                    // The premise and the argument may use different
                                    // surface forms of the same set proposition.
                                    let premise_key = expand_prop(premise);
                                    let actual_key = expand_prop(&actual);
                                    if unify_prop(&premise_key, &actual_key, &holes, &mut binding)
                                        && let Some(witness) = binding.get(&name).cloned()
                                    {
                                        let map = BTreeMap::from([(name, witness)]);
                                        let mut fresh = self.fresh;
                                        let premise = premise.subst_prop(&map, &mut fresh);
                                        hypothesis = conclusion.subst_prop(&map, &mut fresh);
                                        self.fresh = fresh;
                                        self.prove(arg, &premise, ctx, current)?;
                                        continue;
                                    }
                                }
                                let (value, found) = self.term(arg, Some(&ty), &ctx.scope)?;
                                if !compatible(&ty, &found) {
                                    return fail(
                                        arg.span,
                                        "E_PROOF_TYPE",
                                        format!(
                                            "This argument is {found}, but `{callee}` needs {ty}."
                                        ),
                                        "Apply the quantified hypothesis at a value of its type.",
                                    );
                                }
                                let mut map = BTreeMap::new();
                                map.insert(name, value);
                                let mut fresh = self.fresh;
                                hypothesis = body.subst_prop(&map, &mut fresh);
                                self.fresh = fresh;
                            }
                            Prop::Implies(premise, conclusion) => {
                                self.prove(arg, &premise, ctx, current)?;
                                hypothesis = *conclusion;
                            }
                            Prop::Not(inner) => {
                                self.prove(arg, &inner, ctx, current)?;
                                hypothesis = Prop::False;
                            }
                            other => {
                                return fail(
                                    arg.span,
                                    "E_PROOF_SHAPE",
                                    format!(
                                        "`{callee}` proves `{other}`, which takes no argument."
                                    ),
                                    "Apply only a forall, implication or not hypothesis.",
                                );
                            }
                        }
                    }
                    return Ok(hypothesis);
                }
                match (callee.as_str(), args.as_slice()) {
                    ("And", [left, right]) => {
                        let a = self.infer(left, ctx, current)?;
                        let b = self.infer(right, ctx, current)?;
                        Ok(Prop::And(Box::new(a), Box::new(b)))
                    }
                    ("Iff", [forward, backward]) => {
                        if forward.kind == K::Lambda || backward.kind == K::Lambda {
                            return fail(
                                node.span,
                                "E_PROOF_SHAPE",
                                "`Iff` cannot infer an implication from a lambda.",
                                "Write the statement explicitly, as in have h : P ↔ Q := Iff(f, g).",
                            );
                        }
                        let (Prop::Implies(a, b), Prop::Implies(c, d)) = (
                            self.infer(forward, ctx, current)?,
                            self.infer(backward, ctx, current)?,
                        ) else {
                            return fail(
                                node.span,
                                "E_PROOF_SHAPE",
                                "`Iff` takes a proof of `P -> Q` and a proof of `Q -> P`.",
                                "Pass two named implications, or write have h : P ↔ Q := Iff(...).",
                            );
                        };
                        if !self.same(&a, &d) || !self.same(&b, &c) {
                            return fail(
                                node.span,
                                "E_PROOF_MISMATCH",
                                "The two directions of `Iff` do not go between the same propositions.",
                                "Write Iff(P -> Q, Q -> P).",
                            );
                        }
                        Ok(Prop::Iff(a, b))
                    }
                    ("OrLeft" | "OrRight", [_]) => fail(
                        node.span,
                        "E_PROOF_SHAPE",
                        format!(
                            "`{callee}` needs the other side of the disjunction, which a `have` without a statement cannot supply."
                        ),
                        "Write the statement explicitly, as in have h : P or Q := OrLeft(p).",
                    ),
                    ("exists" | "Exists" | "Witness", [_, _]) => fail(
                        node.span,
                        "E_PROOF_SHAPE",
                        "`exists` needs the quantified statement, which a `have` without a statement cannot supply.",
                        "Write the statement explicitly, as in have h : exists x : T, P := exists(witness, proof).",
                    ),
                    ("symm", [arg]) => match self.infer(arg, ctx, current)? {
                        Prop::Eq(a, b) => Ok(Prop::Eq(b, a)),
                        other => fail(
                            arg.span,
                            "E_PROOF_SHAPE",
                            format!("`symm` needs an equality, but this proves `{other}`."),
                            "Pass a proof written with `=`.",
                        ),
                    },
                    ("trans", [p, q]) => {
                        let (Prop::Eq(a, b), Prop::Eq(c, d)) =
                            (self.infer(p, ctx, current)?, self.infer(q, ctx, current)?)
                        else {
                            return fail(
                                node.span,
                                "E_PROOF_SHAPE",
                                "`trans` chains two equalities.",
                                "Pass proofs of `a = b` and `b = c`.",
                            );
                        };
                        if !self.same(
                            &Prop::Eq(b.clone(), b.clone()),
                            &Prop::Eq(b.clone(), c.clone()),
                        ) {
                            return fail(
                                node.span,
                                "E_PROOF_MISMATCH",
                                format!(
                                    "`trans` needs the middle terms to match, but they are `{b}` and `{c}`."
                                ),
                                "The first proof must end where the second begins.",
                            );
                        }
                        Ok(Prop::Eq(a, d))
                    }
                    ("congrArg", [f, p]) => {
                        let premise = self.infer(p, ctx, current)?;
                        let Prop::Eq(a, b) = &premise else {
                            return fail(
                                p.span,
                                "E_PROOF_SHAPE",
                                "`congrArg` needs a proof of an equality as its second argument.",
                                "Pass a proof written with `=`.",
                            );
                        };
                        let result = Prop::Eq(self.apply(f, a, ctx)?, self.apply(f, b, ctx)?);
                        self.transparent(&result, node.span)?;
                        self.defined_from(&result, &premise, ctx, node.span)?;
                        Ok(result)
                    }
                    _ if self.theorems.contains_key(&callee) || callee == current.name => {
                        self.cite(&callee, &args, node.span, ctx, current)
                    }
                    _ if let Some(later) = self.declared(&callee, current.start) => {
                        if later {
                            fail(
                                node.span,
                                "E_PROOF_ORDER",
                                format!(
                                    "`{callee}` is declared further down; a theorem can cite only the theorems above it."
                                ),
                                "Move the cited theorem above this one, so no proof depends on itself in a circle.",
                            )
                        } else {
                            fail(
                                node.span,
                                "E_PROOF_UNCHECKED",
                                format!("`{callee}` did not check, so it cannot be cited."),
                                "Fix the error reported for that theorem first.",
                            )
                        }
                    }
                    _ => fail(
                        node.span,
                        "E_PROOF_UNKNOWN",
                        format!("`{callee}` is not a theorem, a proof rule or a `not` assumption."),
                        "Cite an earlier theorem or use rfl, symm, trans, congrArg, absurd or decide.",
                    ),
                }
            }
            K::Literal => fail(
                node.span,
                "E_PROOF_VALUE",
                format!("`{}` is a value, not a proof.", self.text(node)),
                "A proof position needs an assumption, a theorem or a proof constructor such as rfl.",
            ),
            _ => fail(
                node.span,
                "E_PROOF_SHAPE",
                "The kernel cannot tell what this proves.",
                "Cite a theorem or an assumption, or write the proof against a stated goal.",
            ),
        }
    }

    /// A conditional induction hypothesis, once its assumptions at the smaller
    /// value follow from known facts (Q81).
    fn conditional(&mut self, name: &str, ctx: &Context, span: Span) -> Found<Option<Prop>> {
        let Some((_, needs, statement)) = ctx
            .conditional
            .iter()
            .rev()
            .find(|(n, _, _)| n == name)
            .cloned()
        else {
            return Ok(None);
        };
        for needed in &needs {
            if !self.known_fact(needed, ctx, span) {
                return fail(
                    span,
                    "E_PROOF_ASSUMPTION",
                    format!(
                        "Using `{name}` needs `{needed}`, the assumption at the smaller value, and nothing here establishes it."
                    ),
                    "Add an assumption or split cases so the smaller value satisfies it.",
                );
            }
        }
        Ok(Some(statement))
    }

    /// The proposition a bare proof name stands for: an assumption, a
    /// conditional induction hypothesis or a theorem with no parameters.
    fn fact(
        &mut self,
        name: &str,
        span: Span,
        ctx: &Context,
        current: &mut Current,
    ) -> Found<Prop> {
        if let Some((_, prop)) = ctx.hyps.iter().rev().find(|(n, _)| n == name) {
            return Ok(prop.clone());
        }
        if let Some(prop) = self.conditional(name, ctx, span)? {
            return Ok(prop);
        }
        if self.theorems.get(name).is_some_and(|t| t.params.is_empty())
            || name == current.name && current.theorem.params.is_empty()
        {
            return self.cite(name, &[], span, ctx, current);
        }
        fail(
            span,
            "E_PROOF_UNKNOWN",
            format!("`{name}` is not an assumption or a theorem here."),
            "Cite an earlier theorem, an assumption from requires, or a name bound by let or match.",
        )
    }

    /// Whether a member chain starts at a local proof fact, so `h.left` is a
    /// conjunction projection while `std.addZeroR` stays a dotted name.
    fn projects_fact(&self, node: &Node, ctx: &Context, current: &Current) -> bool {
        match node.kind {
            K::Name => {
                let name = crate::lexer::path_name(self.text(node));
                ctx.hyps.iter().any(|(n, _)| *n == name)
                    || ctx.conditional.iter().any(|(n, _, _)| *n == name)
                    || self
                        .theorems
                        .get(&name)
                        .is_some_and(|t| t.params.is_empty())
                    || name == current.name && current.theorem.params.is_empty()
            }
            K::Member => self.projects_fact(&node.children[0], ctx, current),
            // A call in proof position yields a proposition, and a dotted path
            // never contains a call, so `pair(n).left` is a projection.
            K::Call => true,
            K::Group if node.children.len() == 1 => {
                self.projects_fact(&node.children[0], ctx, current)
            }
            _ => false,
        }
    }

    /// `h.left` and `h.right`: conjunction elimination as a proof term, so one
    /// side is reachable without splitting the whole hypothesis into cases.
    /// The base unfolds first, which takes `x ∈ A ∩ B` to `x ∈ A and x ∈ B`.
    fn project(&mut self, node: &Node, ctx: &Context, current: &mut Current) -> Found<Prop> {
        let base = self.infer(&node.children[0], ctx, current)?;
        let prop = self.unfold_head(&base);
        let member = self.text(&node.children[1]).to_owned();
        let shown = self.text(node).to_owned();
        match (member.as_str(), prop) {
            ("left", Prop::And(a, _)) => Ok(*a),
            ("right", Prop::And(_, b)) => Ok(*b),
            ("left" | "right", Prop::Or(..)) => fail(
                node.span,
                "E_PROOF_SHAPE",
                format!("`{shown}` is a disjunction, which does not say which side holds."),
                "Split the disjunction with `cases`, then prove each branch.",
            ),
            ("left" | "right", Prop::Iff(..)) => fail(
                node.span,
                "E_PROOF_SHAPE",
                format!("`{shown}` is an iff, which has two directions, not two sides."),
                "Split it with `cases`, then use the forward and backward hypotheses.",
            ),
            ("left" | "right", Prop::Exists { .. }) => fail(
                node.span,
                "E_PROOF_SHAPE",
                format!("`{shown}` is an existential, which has a witness, not a side."),
                "Split it with `cases` to introduce the witness.",
            ),
            ("left" | "right", other) => fail(
                node.span,
                "E_PROOF_SHAPE",
                format!(
                    "`{shown}` takes a side of a conjunction, but the base proves `{}`.",
                    pretty_prop(&other)
                ),
                "Use `left` or `right` on an `and` fact, or split this fact with `cases`.",
            ),
            _ => fail(
                node.span,
                "E_PROOF_SHAPE",
                format!("`{shown}` is not a conjunction projection."),
                "A conjunction has `left` and `right`.",
            ),
        }
    }

    /// Whether a theorem of this name exists, and if so whether it comes after `start`.
    fn declared(&self, name: &str, start: usize) -> Option<bool> {
        // An imported name without a checked theorem failed in its own module.
        if self.checked.environment.theorems.contains_key(name) {
            return Some(false);
        }
        self.parsed
            .root
            .children
            .iter()
            .find(|n| {
                n.kind == K::Theorem && n.child(K::Name).is_some_and(|c| self.text(c) == name)
            })
            .map(|n| n.span.start > start)
    }

    /// The function in `congrArg(f, p)` applied to one side of the equality.
    fn apply(&mut self, f: &Node, value: &Term, ctx: &Context) -> Found<Term> {
        match f.kind {
            K::Name | K::Member => {
                let name = crate::lexer::path_name(self.text(f));
                let shadowed = name
                    .split('.')
                    .next()
                    .is_some_and(|root| ctx.scope.get(root).is_some());
                if shadowed {
                    return fail(
                        f.span,
                        "E_PROOF_OPAQUE",
                        "congrArg needs a named pure function, not a value parameter.",
                        "Use a visible free function, enum constructor or lambda.",
                    );
                }
                if let Some((ty, fields, _)) = self.case_owner(&name, None)
                    && fields.len() == 1
                {
                    if !compatible(&fields[0], &self.term_type(value, ctx)) {
                        return fail(
                            f.span,
                            "E_PROOF_TYPE",
                            format!(
                                "Case `{name}` needs {}, not {}.",
                                fields[0],
                                self.term_type(value, ctx)
                            ),
                            "Apply the constructor to an equality of its field type.",
                        );
                    }
                    return Ok(self.ctor(
                        &ty,
                        name.rsplit('.').next().unwrap(),
                        vec![value.clone()],
                    ));
                }
                let found = self.visible_functions(&name).into_iter().find(|id| {
                    self.signatures.get(id).is_some_and(|(_, params, _)| {
                        params.len() == 1 && self.fits(&params[0].1, &self.term_type(value, ctx))
                    })
                });
                match found {
                    Some(span) => Ok(Term::Call {
                        function: span,
                        name: name.clone(),
                        args: vec![value.clone()],
                        dispatch: None,
                    }),
                    None => fail(
                        f.span,
                        "E_PROOF_UNKNOWN",
                        format!("`{name}` is not a one-argument function or enum case."),
                        "Pass a function or case with one parameter, or a lambda.",
                    ),
                }
            }
            K::Lambda => {
                let parameter = f
                    .child(K::LambdaParameters)
                    .and_then(|p| p.children.first());
                let Some(parameter) = parameter else {
                    return fail(
                        f.span,
                        "E_PROOF_SHAPE",
                        "The lambda in congrArg takes one parameter.",
                        "Write `x => ...`.",
                    );
                };
                let name = parameter
                    .child(K::Name)
                    .map(|n| self.text(n).to_owned())
                    .unwrap_or_default();
                let ty = parameter
                    .children
                    .iter()
                    .find(|n| matches!(n.kind, K::TypeName))
                    .map(|_| Type::Invalid);
                let ty = match ty {
                    Some(_) | None => self.term_type(value, ctx),
                };
                let scope = ctx.scope.with(&name, value.clone(), ty);
                Ok(self.term(f.children.last().unwrap(), None, &scope)?.0)
            }
            _ => fail(
                f.span,
                "E_PROOF_SHAPE",
                "congrArg's first argument is a function name, a case or a lambda.",
                "Write congrArg(Succ, h) or congrArg(x => add(x, n), h).",
            ),
        }
    }

    fn term_type(&self, t: &Term, ctx: &Context) -> Type {
        match t {
            Term::Nat(_) => nat(),
            Term::Int(_) => Type::named("Int"),
            Term::Bool(_) => Type::named("Bool"),
            Term::Ctor { ty, case, args } => {
                let parameters = self.enum_parameters(ty);
                if parameters.is_empty() {
                    return Type::named(ty);
                }
                let schema = Type::applied(
                    ty,
                    parameters
                        .iter()
                        .map(|p| Type::Parameter(p.clone()))
                        .collect(),
                );
                let fields = self.case_fields(&schema, case).unwrap_or_default();
                let variables: Holes = parameters
                    .iter()
                    .map(|p| (p.clone(), Type::Parameter(format!("?{p}"))))
                    .collect();
                let mut holes = Holes::new();
                for (field, arg) in fields.iter().zip(args) {
                    unify(
                        &instantiate(field, &variables),
                        &self.term_type(arg, ctx),
                        &mut holes,
                    );
                }
                let result = Type::Named(
                    ty.clone(),
                    parameters.iter().map(|p| variables[p].clone()).collect(),
                );
                settle(&result, &holes)
            }
            Term::Call { function, args, .. } => {
                let Some((_, params, result)) = self.signatures.get(function) else {
                    return Type::Invalid;
                };
                let mut names = vec![];
                for (_, t) in params {
                    generic::variables(t, &mut names);
                }
                generic::variables(result, &mut names);
                if names.is_empty() {
                    return result.clone();
                }
                let variables: Holes = names
                    .iter()
                    .map(|p| (p.clone(), Type::Parameter(format!("?{p}"))))
                    .collect();
                let mut holes = Holes::new();
                for ((_, param), arg) in params.iter().zip(args) {
                    unify(
                        &instantiate(param, &variables),
                        &self.term_type(arg, ctx),
                        &mut holes,
                    );
                }
                settle(&instantiate(result, &variables), &holes)
            }
            // Applying a function parameter: its result type is the function
            // type bound in scope. `congrArg` over such a call needs this to
            // type the lambda's binder.
            Term::App(function, _) => match function.as_ref() {
                Term::Var(name) => ctx
                    .scope
                    .get(name)
                    .and_then(|(_, _, ty)| match ty {
                        Type::Function(signature) => Some((*signature.result).clone()),
                        _ => None,
                    })
                    .unwrap_or(Type::Invalid),
                Term::Lam(params, body) => params
                    .last()
                    .map(|_| self.term_type(body, ctx))
                    .unwrap_or(Type::Invalid),
                _ => Type::Invalid,
            },
            Term::Var(v) => ctx
                .scope
                .terms
                .iter()
                .find(|(_, term, _)| matches!(term, Term::Var(x) if x == v))
                .map(|(_, _, ty)| ty.clone())
                .unwrap_or(Type::Invalid),
            Term::Op(op, _)
                if matches!(
                    op.as_str(),
                    "<" | "<=" | ">" | ">=" | "==" | "!=" | "&&" | "||" | "!"
                ) =>
            {
                Type::named("Bool")
            }
            Term::Op(op, args) if op == "seq" => self.term_type(&args[1], ctx),
            // A union or intersection carries its universe in whichever side
            // is not the empty literal.
            Term::Op(op, args) if matches!(op.as_str(), "∪" | "∩") => args
                .iter()
                .map(|arg| self.term_type(arg, ctx))
                .find(|ty| set_element_type(ty).is_some())
                .unwrap_or(Type::Invalid),
            Term::Op(op, args) if op == "ᶜ" => args
                .first()
                .map(|arg| self.term_type(arg, ctx))
                .unwrap_or(Type::Invalid),
            Term::Op(op, _) if op == "$list.length" => Type::named("Int"),
            Term::Op(op, _)
                if matches!(
                    op.as_str(),
                    "$list.isEmpty" | "$option.isSome" | "$option.isNone"
                ) =>
            {
                Type::named("Bool")
            }
            Term::Op(op, args) if matches!(op.as_str(), "$list.first" | "$list.last") => {
                sequences::list_item(&self.term_type(&args[0], ctx))
                    .map(|item| Type::applied("Option", vec![item.clone()]))
                    .unwrap_or(Type::Invalid)
            }
            Term::Op(_, args) => args
                .first()
                .map(|t| self.term_type(t, ctx))
                .unwrap_or(Type::Invalid),
            Term::If(_, a, _) => self.term_type(a, ctx),
            Term::FamilyUnion { element, .. } | Term::FamilyInter { element, .. } => {
                Type::applied("Set", vec![element.clone()])
            }
            Term::Builder { element, .. } => Type::applied("Set", vec![element.clone()]),
            Term::Finite(elements) => elements
                .first()
                .map(|element| Type::applied("Set", vec![self.term_type(element, ctx)]))
                .unwrap_or(Type::Invalid),
            _ => Type::Invalid,
        }
    }

    /// Instantiates a theorem with arguments and checks its assumptions (Q70).
    fn cite(
        &mut self,
        name: &str,
        args: &[&Node],
        span: Span,
        ctx: &Context,
        current: &mut Current,
    ) -> Found<Prop> {
        let own = name == current.name;
        let theorem = if own {
            current.theorem.clone()
        } else {
            self.theorems[name].clone()
        };
        if theorem.trusted_smt {
            self.smt_trusted.set(true);
        }
        if theorem.admitted {
            current.admitted = true;
        }
        if args.len() != theorem.params.len() {
            return fail(
                span,
                "E_PROOF_SHAPE",
                format!("`{name}` takes {} arguments.", theorem.params.len()),
                "Pass one term for each theorem parameter.",
            );
        }
        let mut map = BTreeMap::new();
        let mut terms = vec![];
        let constraints = Self::theorem_constraints(&theorem);
        let mut types = theorem.params.iter().map(|(_, t)| t).collect::<Vec<_>>();
        for (ty, bound) in &constraints {
            types.extend([ty, bound]);
        }
        let variables = if own {
            Holes::new()
        } else {
            self.holes_for(&types)
        };
        let mut holes = Holes::new();
        for (arg, (pname, ty)) in args.iter().zip(&theorem.params) {
            let ty = &generic::resolve(&instantiate(ty, &variables), &holes);
            let (term, found) = self.term(arg, Some(ty), &ctx.scope)?;
            self.defined_term(&term, ctx, arg.span)?;
            if !unify(ty, &found, &mut holes) {
                return fail(
                    arg.span,
                    "E_PROOF_TYPE",
                    format!("`{name}` expects {ty} for `{pname}`, but this is {found}."),
                    "Pass a term of the parameter's type.",
                );
            }
            map.insert(pname.clone(), term.clone());
            terms.push(term);
        }
        self.check_bounds(&constraints, &variables, &holes, &ctx.scope, span)?;
        let root_arguments = theorem
            .params
            .iter()
            .filter_map(|(name, _)| map.get(name).cloned().map(|term| (name.clone(), term)))
            .collect::<BTreeMap<_, _>>();
        let projection_params = theorem
            .params
            .iter()
            .map(|(name, ty)| {
                (
                    name.clone(),
                    generic::resolve(&instantiate(ty, &variables), &holes),
                )
            })
            .collect::<Vec<_>>();
        self.add_class_projection_substitutions(&projection_params, &root_arguments, &mut map);
        if own {
            let here = terms
                .iter()
                .enumerate()
                .filter(|(i, t)| matches!(t, Term::Var(v) if ctx.smaller.get(v) == Some(i)))
                .map(|(i, _)| i)
                .collect::<BTreeSet<_>>();
            current.candidates = Some(match current.candidates.take() {
                Some(previous) => previous.intersection(&here).copied().collect(),
                None => here,
            });
            if current.candidates.as_ref().is_some_and(BTreeSet::is_empty) {
                return fail(
                    span,
                    "E_PROOF_RECURSION",
                    format!("`{name}` cites itself here without a smaller argument."),
                    "Cite the theorem on a piece that a match took out of the same parameter.",
                );
            }
        }
        let mut fresh = self.fresh;
        // Capture-avoiding: a theorem parameter named like a law's binder must
        // not rewrite that binder, or `hAssoc` at `a := inv(a)` changes the
        // quantified law instead of instantiating it.
        let mut substitute = |p: &Prop| p.subst_prop(&map, &mut fresh);
        let requires = theorem
            .requires
            .iter()
            .map(|(n, p)| {
                (
                    n.clone(),
                    Prop::specialize_types(&substitute(p), &variables, &holes),
                )
            })
            .collect::<Vec<_>>();
        let statement = Prop::specialize_types(&substitute(&theorem.statement), &variables, &holes);
        self.fresh = fresh;
        for (hname, needed) in requires {
            let known = ctx.hyps.clone().iter().any(|(_, h)| self.same(h, &needed));
            if !known && self.decide(&needed) != Some(true) && !self.known_fact(&needed, ctx, span)
            {
                return fail(
                    span,
                    "E_PROOF_ASSUMPTION",
                    format!(
                        "Citing `{name}` needs `{needed}` (its assumption `{hname}`), and nothing here establishes it."
                    ),
                    "Add it as a requires assumption, or split cases so it holds.",
                );
            }
        }
        Ok(statement)
    }

    /// `match` and `induction` on a variable split the goal into one goal per case.
    fn split(
        &mut self,
        node: &Node,
        goal: &Prop,
        ctx: &Context,
        current: &mut Current,
        induction: bool,
    ) -> Found<()> {
        let scrutinee = &node.children[0];
        let name = self.text(scrutinee).to_owned();
        if !induction
            && let Some((_, Prop::Or(a, b))) =
                ctx.hyps.iter().rev().find(|(n, _)| *n == name).cloned()
        {
            return self.split_or(node, (*a, *b), goal, ctx, current);
        }
        let Some((_, Term::Var(var), ty)) = ctx.scope.get(&name).cloned() else {
            return fail(
                scrutinee.span,
                "E_PROOF_SHAPE",
                format!(
                    "A proof can match on a variable or an `or` assumption; `{name}` is neither."
                ),
                "Match on a theorem parameter or a name a match bound.",
            );
        };
        let Type::Named(_, _) = &ty else {
            return fail(
                scrutinee.span,
                "E_PROOF_SHAPE",
                format!("`{name}` is {ty}, which has no cases."),
                "Match on a Nat or an enum.",
            );
        };
        let Some(variants) = self.value_cases(&ty) else {
            return fail(
                scrutinee.span,
                "E_PROOF_SHAPE",
                format!("`{name}` is {ty}, which has no cases."),
                "Split a Nat, Bool, Option, List or enum.",
            );
        };
        // Assumptions about the induction variable hold for ih only at the smaller
        // value, so each use of ih must establish them there (Q81).
        let mentioning = if induction {
            ctx.assumptions
                .iter()
                .filter(|(_, h)| h.mentions(&var))
                .map(|(_, h)| h.clone())
                .collect::<Vec<_>>()
        } else {
            vec![]
        };
        let origin = ctx
            .smaller
            .get(&var)
            .copied()
            .or_else(|| current.theorem.params.iter().position(|(p, _)| *p == var));
        let mut covered = BTreeSet::new();
        let mut wildcard = false;
        for arm in node.children.iter().skip(1) {
            if arm.child(K::Guard).is_some() {
                return fail(
                    arm.span,
                    "E_PROOF_SHAPE",
                    "A proof cannot use a guarded arm.",
                    "Split cases with patterns only.",
                );
            }
            let pattern = &arm.children[0];
            let mut scope = ctx.scope.clone();
            let pat = self.pattern(pattern, &ty, &mut scope)?;
            let case_term = match &pat {
                Pat::Any => {
                    wildcard = true;
                    Term::Var(var.clone())
                }
                Pat::Bind(b) => {
                    wildcard = true;
                    Term::Var(b.clone())
                }
                Pat::Ctor(case, _) => {
                    if Self::covers_case(&pat) {
                        covered.insert(case.clone());
                    }
                    self.pat_term(&pat, &ty)
                }
                Pat::Lit(_) | Pat::Alt(_) | Pat::Range { .. } => {
                    return fail(
                        pattern.span,
                        "E_PROOF_SHAPE",
                        "A proof splits cases with case patterns.",
                        "Write Zero and Succ(k), or the enum's cases.",
                    );
                }
            };
            let map = BTreeMap::from([(var.clone(), case_term.clone())]);
            let mut fresh = self.fresh;
            let mut inner = Context {
                scope: Scope {
                    bounds: scope.bounds.clone(),
                    terms: scope
                        .terms
                        .iter()
                        .map(|(n, t, ty)| (n.clone(), t.subst(&map, &mut fresh), ty.clone()))
                        .collect(),
                },
                hyps: ctx
                    .hyps
                    .iter()
                    .map(|(n, h)| (n.clone(), h.map(&mut |t| t.subst(&map, &mut fresh))))
                    .collect(),
                assumptions: ctx.assumptions.clone(),
                smaller: ctx.smaller.clone(),
                conditional: ctx
                    .conditional
                    .iter()
                    .map(|(n, needs, statement)| {
                        (
                            n.clone(),
                            needs
                                .iter()
                                .map(|p| p.map(&mut |t| t.subst(&map, &mut fresh)))
                                .collect(),
                            statement.map(&mut |t| t.subst(&map, &mut fresh)),
                        )
                    })
                    .collect(),
                induction_hyps: ctx.induction_hyps.clone(),
            };
            let arm_goal = goal.map(&mut |t| t.subst(&map, &mut fresh));
            self.fresh = fresh;
            let mut bound = BTreeSet::new();
            pat.binders(&mut bound);
            if let (Some(index), Pat::Ctor(..)) = (origin, &pat) {
                for b in &bound {
                    inner.smaller.insert(b.clone(), index);
                }
            }
            let body_index = arm.children.len() - 1;
            if induction {
                let recursive = self.recursive_binders(&pat, &ty, arm.span)?;
                let names = &arm.children[1..body_index];
                self.induction_names(arm.span, names, recursive.len())?;
                for (name, small) in names.iter().zip(recursive) {
                    let ih_name = self.text(name).to_owned();
                    let mut fresh = self.fresh;
                    let smaller = BTreeMap::from([(var.clone(), Term::Var(small))]);
                    let ih = goal.map(&mut |t| t.subst(&smaller, &mut fresh));
                    let needs = mentioning
                        .iter()
                        .map(|p| p.map(&mut |t| t.subst(&smaller, &mut fresh)))
                        .collect::<Vec<_>>();
                    self.fresh = fresh;
                    if needs.is_empty() {
                        inner.hyps.push((ih_name, ih));
                    } else {
                        inner.conditional.push((ih_name, needs, ih));
                    }
                }
            }
            self.prove(&arm.children[body_index], &arm_goal, &inner, current)?;
        }
        if !wildcard {
            let missing = variants
                .iter()
                .filter(|(c, _)| !covered.contains(c))
                .map(|(c, _)| c.clone())
                .collect::<Vec<_>>();
            if !missing.is_empty() {
                let fixes = crate::actions::proof_case_fixes(
                    &self.parsed.source,
                    node,
                    &ty,
                    &variants,
                    &missing,
                    induction,
                );
                return fail_with(
                    node.span,
                    "E_PROOF_CASES",
                    format!(
                        "The proof does not cover {}.",
                        missing
                            .iter()
                            .map(|case| sequences::case_label(case))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    "Add an arm for every case.",
                    fixes,
                );
            }
        }
        Ok(())
    }

    fn split_or(
        &mut self,
        node: &Node,
        sides: (Prop, Prop),
        goal: &Prop,
        ctx: &Context,
        current: &mut Current,
    ) -> Found<()> {
        let mut seen = BTreeSet::new();
        for arm in node.children.iter().skip(1) {
            let pattern = &arm.children[0];
            let case = pattern
                .children
                .first()
                .map(|c| self.text(c).to_owned())
                .unwrap_or_default();
            let side = match case.as_str() {
                "Left" => sides.0.clone(),
                "Right" => sides.1.clone(),
                _ => {
                    return fail(
                        pattern.span,
                        "E_PROOF_SHAPE",
                        "An `or` assumption splits into Left(p) and Right(q).",
                        "Write the arms Left(p) and Right(q).",
                    );
                }
            };
            let Some(binder) = pattern.children.get(1).filter(|c| c.kind == K::PatternName) else {
                return fail(
                    pattern.span,
                    "E_PROOF_SHAPE",
                    "Name the proof inside Left or Right.",
                    "Write Left(p) =>.",
                );
            };
            seen.insert(case);
            let mut inner = ctx.clone();
            inner.hyps.push((self.text(binder).to_owned(), side));
            self.prove(arm.children.last().unwrap(), goal, &inner, current)?;
        }
        if seen.len() < 2 {
            let missing = ["Left", "Right"]
                .into_iter()
                .filter(|side| !seen.contains(*side))
                .collect::<Vec<_>>();
            let fixes = crate::actions::proof_or_fixes(&self.parsed.source, node, &missing);
            return fail_with(
                node.span,
                "E_PROOF_CASES",
                "Splitting an `or` needs both Left and Right.",
                "Add the missing arm.",
                fixes,
            );
        }
        Ok(())
    }

    fn pat_term(&mut self, pat: &Pat, ty: &Type) -> Term {
        match pat {
            Pat::Bind(b) => Term::Var(b.clone()),
            Pat::Any => {
                let name = self.fresh("_");
                Term::Var(name)
            }
            Pat::Lit(lit) => lit.clone(),
            Pat::Alt(_) | Pat::Range { .. } => {
                let name = self.fresh("_");
                Term::Var(name)
            }
            Pat::Ctor(case, pats) => {
                let fields = self.case_fields(ty, case).unwrap_or_default();
                let args = pats
                    .iter()
                    .zip(&fields)
                    .map(|(p, field)| self.pat_term(p, field))
                    .collect();
                let Type::Named(id, _) = ty else {
                    unreachable!()
                };
                self.ctor(id, case, args)
            }
        }
    }

    fn induction_names(&self, span: Span, names: &[Node], count: usize) -> Found<()> {
        if names.len() != count {
            return fail(
                span,
                "E_PROOF_CASES",
                format!(
                    "This case has {count} recursive fields, but {} induction hypothesis names.",
                    names.len()
                ),
                "Write one hypothesis name per recursive field, in field order; a nonrecursive case takes none.",
            );
        }
        let mut seen = BTreeSet::new();
        for name in names {
            if !seen.insert(self.text(name)) {
                return fail(
                    name.span,
                    "E_PROOF_CASES",
                    "Induction hypothesis names must be distinct.",
                    "Give each recursive field its own hypothesis name.",
                );
            }
        }
        Ok(())
    }

    fn recursive_binders(&mut self, pat: &Pat, ty: &Type, span: Span) -> Found<Vec<String>> {
        let Pat::Ctor(case, pats) = pat else {
            return Ok(vec![]);
        };
        let Some(fields) = self.case_fields(ty, case) else {
            return Ok(vec![]);
        };
        let mut names = vec![];
        for (pattern, field) in pats.iter().zip(fields) {
            if &field != ty {
                continue;
            }
            let Pat::Bind(name) = pattern else {
                return fail(
                    span,
                    "E_PROOF_CASES",
                    "Each recursive field in an induction case needs a value name.",
                    "Bind the entire recursive field to a name, then write its induction hypothesis after the pattern.",
                );
            };
            names.push(name.clone());
        }
        Ok(names)
    }
}

/// Two values that can never be equal: different cases or literals, or the same case with
/// a pair of arguments that clash, since a case's fields determine it (injectivity).
fn values_clash(a: &Term, b: &Term) -> bool {
    if !is_value(a) || !is_value(b) {
        return false;
    }
    if head(a) != head(b) {
        return true;
    }
    match (a, b) {
        (Term::Ctor { args: left, .. }, Term::Ctor { args: right, .. }) => {
            left.len() == right.len() && left.iter().zip(right).any(|(x, y)| values_clash(x, y))
        }
        (Term::Nat(x), Term::Nat(y)) => x != y,
        _ => false,
    }
}

fn head(t: &Term) -> (String, Option<String>) {
    match t {
        Term::Nat(0) => ("Nat".into(), Some("Zero".into())),
        Term::Nat(_) => ("Nat".into(), Some("Succ".into())),
        Term::Ctor { ty, case, .. } => (ty.clone(), Some(case.clone())),
        Term::App(function, _) => head(function),
        Term::Lam(..) => ("<lambda>".into(), None),
        Term::Prop(..) => ("<prop>".into(), None),
        other => (other.to_string(), None),
    }
}

/// Checks every theorem in a well-typed file, in source order, so a theorem can
/// cite only those before it, and itself on smaller values.
pub(crate) fn check(
    parsed: &Parsed,
    checked: &Checked,
) -> (
    Vec<Diagnostic>,
    BTreeMap<Span, crate::contracts::Requirement>,
    Definitions,
) {
    let theorems = parsed
        .root
        .children
        .iter()
        .filter(|n| matches!(n.kind, K::Theorem | K::Example | K::Axiom))
        .collect::<Vec<_>>();
    let mut kernel = Kernel::new(parsed, checked);
    let mut errors = theorems
        .into_iter()
        .filter_map(|node| kernel.theorem(node).err())
        .collect::<Vec<_>>();
    let (found, requirements) = kernel.function_requirements();
    errors.extend(found);
    errors.extend(kernel.warnings);
    let definitions = Definitions {
        signatures: kernel.signatures,
        constraints: kernel.constraints,
        functions: kernel.functions,
        opaque: kernel.opaque,
        theorems: kernel.exported,
    };
    (errors, requirements, definitions)
}

/// A theorem whose checked proof still relies on a solver, axiom or admission.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrustedTheorem {
    pub name: String,
    pub span: Span,
    pub reasons: Vec<&'static str>,
}

pub fn trusted_theorems(parsed: &Parsed, checked: &Checked) -> Vec<TrustedTheorem> {
    parsed
        .root
        .children
        .iter()
        .filter_map(|node| {
            if !matches!(node.kind, K::Theorem | K::Example | K::Axiom) {
                return None;
            }
            let id = DeclarationId {
                file: parsed.source.name.clone(),
                span: node.span,
            };
            let theorem = &checked.environment.proofs.theorems.get(&id)?.theorem;
            let mut reasons = vec![];
            if theorem.trusted_smt {
                reasons.push("smt (unchecked solver result)");
            }
            if theorem.admitted {
                reasons.push(if node.kind == K::Axiom {
                    "axiom"
                } else {
                    "sorry or axiom dependency"
                });
            }
            if reasons.is_empty() {
                return None;
            }
            Some(TrustedTheorem {
                name: node
                    .child(K::Name)
                    .map(|n| parsed.source.slice(n.span).to_owned())
                    .unwrap_or_else(|| "example".into()),
                span: node.span,
                reasons,
            })
        })
        .collect()
}
