//! Logical views of built-in values. These terms exist only in the proof
//! checker; native List, Bool and Option layouts and operations are unchanged.
use super::*;

pub(super) const NIL: &str = "list:nil";
pub(super) const CONS: &str = "list:cons";

pub(super) fn case_label(case: &str) -> &str {
    match case {
        NIL => "[]",
        CONS => "[x, ..rest]",
        other => other,
    }
}

pub(super) fn list_item(ty: &Type) -> Option<&Type> {
    match ty {
        Type::Named(id, args) if id == "List" && args.len() == 1 => args.first(),
        _ => None,
    }
}

pub(super) fn nil() -> Term {
    Term::Ctor {
        ty: "List".into(),
        case: NIL.into(),
        args: vec![],
    }
}

pub(super) fn cons(value: Term, rest: Term) -> Term {
    Term::Ctor {
        ty: "List".into(),
        case: CONS.into(),
        args: vec![value, rest],
    }
}

/// One defining equation, exposed only when its scrutinee has a known shape.
pub(super) fn step(op: &str, args: &[Term]) -> Option<Term> {
    if (op.starts_with("$list.") || op.starts_with("$option."))
        && let Some(Term::If(condition, yes, no)) = args.first()
    {
        let mut yes_args = args.to_vec();
        let mut no_args = args.to_vec();
        yes_args[0] = *yes.clone();
        no_args[0] = *no.clone();
        return Some(Term::If(
            condition.clone(),
            Box::new(Term::Op(op.into(), yes_args)),
            Box::new(Term::Op(op.into(), no_args)),
        ));
    }
    let Term::Ctor {
        ty,
        case,
        args: fields,
    } = args.first()?
    else {
        return None;
    };
    if ty == "Option" && args.len() == 1 {
        return match (op, case.as_str()) {
            ("$option.isSome", "Some") | ("$option.isNone", "None") => Some(Term::Bool(true)),
            ("$option.isSome", "None") | ("$option.isNone", "Some") => Some(Term::Bool(false)),
            _ => None,
        };
    }
    if ty != "List" {
        return None;
    }
    let empty = case == NIL && fields.is_empty();
    let pair = (case == CONS && fields.len() == 2).then_some(fields.as_slice());
    let recurse = |name: &str, rest: &Term| Term::Op(name.into(), vec![rest.clone()]);
    match (op, args) {
        ("$list.concat", [_, right]) if empty => Some(right.clone()),
        ("$list.concat", [_, right]) => pair.map(|p| {
            cons(
                p[0].clone(),
                Term::Op(op.into(), vec![p[1].clone(), right.clone()]),
            )
        }),
        ("$list.length", [_]) if empty => Some(Term::Int(0)),
        ("$list.length", [_]) => {
            pair.map(|p| Term::Op("+".into(), vec![Term::Int(1), recurse(op, &p[1])]))
        }
        ("$list.isEmpty", [_]) if empty => Some(Term::Bool(true)),
        ("$list.isEmpty", [_]) if pair.is_some() => Some(Term::Bool(false)),
        ("$list.reversed", [_]) if empty => Some(nil()),
        ("$list.reversed", [_]) => pair.map(|p| {
            Term::Op(
                "$list.concat".into(),
                vec![recurse(op, &p[1]), cons(p[0].clone(), nil())],
            )
        }),
        ("$list.first" | "$list.last", [_]) if empty => Some(Term::Ctor {
            ty: "Option".into(),
            case: "None".into(),
            args: vec![],
        }),
        ("$list.first", [_]) => pair.map(|p| Term::Ctor {
            ty: "Option".into(),
            case: "Some".into(),
            args: vec![p[0].clone()],
        }),
        ("$list.last", [_]) => pair.map(|p| {
            Term::If(
                Box::new(recurse("$list.isEmpty", &p[1])),
                Box::new(Term::Ctor {
                    ty: "Option".into(),
                    case: "Some".into(),
                    args: vec![p[0].clone()],
                }),
                Box::new(recurse(op, &p[1])),
            )
        }),
        _ => None,
    }
}

impl Kernel<'_> {
    /// The complete cases of a logical type, including built-ins whose runtime
    /// representation is not an enum declaration.
    pub(super) fn value_cases(&self, ty: &Type) -> Option<Vec<(String, Vec<Type>)>> {
        let Type::Named(id, arguments) = ty else {
            return None;
        };
        match (id.as_str(), arguments.as_slice()) {
            ("List", [item]) => {
                return Some(vec![
                    (NIL.into(), vec![]),
                    (CONS.into(), vec![item.clone(), ty.clone()]),
                ]);
            }
            ("Option", [item]) => {
                return Some(vec![
                    ("Some".into(), vec![item.clone()]),
                    ("None".into(), vec![]),
                ]);
            }
            ("Bool", []) => return Some(vec![("true".into(), vec![]), ("false".into(), vec![])]),
            _ => {}
        }
        let definition = self.checked.environment.definitions.get(id)?;
        if definition.kind != K::Enum || definition.parameters.len() != arguments.len() {
            return None;
        }
        let substitutions = definition
            .parameters
            .iter()
            .cloned()
            .zip(arguments.iter().cloned())
            .collect();
        Some(
            definition
                .variants
                .iter()
                .map(|v| {
                    (
                        v.name.clone(),
                        v.fields
                            .iter()
                            .map(|field| instantiate(field, &substitutions))
                            .collect(),
                    )
                })
                .collect(),
        )
    }

    pub(super) fn case_fields(&self, ty: &Type, case: &str) -> Option<Vec<Type>> {
        self.value_cases(ty)?
            .into_iter()
            .find(|(name, _)| name == case)
            .map(|(_, fields)| fields)
    }

    /// A restricted constructor pattern covers its whole case only when every
    /// field is unconstrained. In particular [x, y] does not cover all nonempty
    /// lists, even though its outer logical constructor is cons.
    pub(super) fn covers_case(pat: &Pat) -> bool {
        matches!(pat, Pat::Ctor(_, fields) if fields.iter().all(|field| matches!(field, Pat::Bind(_) | Pat::Any)))
    }

    pub(super) fn list_literal(
        &mut self,
        node: &Node,
        expected: Option<&Type>,
        scope: &Scope,
    ) -> Found<(Term, Type)> {
        let mut item = expected
            .and_then(list_item)
            .cloned()
            .unwrap_or(Type::Parameter("?".into()));
        let mut values = vec![];
        let mut holes = Holes::new();
        for child in &node.children {
            let (value, found) = self.term(child, Some(&generic::resolve(&item, &holes)), scope)?;
            if !unify(&item, &found, &mut holes) {
                return fail(
                    child.span,
                    "E_PROOF_TYPE",
                    "List elements must have the same type.",
                    "Use one element type throughout the list.",
                );
            }
            if item == Type::Parameter("?".into()) {
                item = found;
            }
            values.push(value);
        }
        let value = values
            .into_iter()
            .rev()
            .fold(nil(), |rest, value| cons(value, rest));
        Ok((value, Type::applied("List", vec![settle(&item, &holes)])))
    }

    pub(super) fn list_pattern(&mut self, node: &Node, ty: &Type, scope: &mut Scope) -> Found<Pat> {
        let Some(item) = list_item(ty) else {
            return fail(
                node.span,
                "E_PROOF_TYPE",
                "A list pattern needs a List value.",
                "Match this type with its own cases.",
            );
        };
        let mut parts = node.children.as_slice();
        let mut tail = Pat::Ctor(NIL.into(), vec![]);
        if let Some(rest) = parts.last().filter(|part| part.kind == K::PatternRest) {
            tail = match rest.children.first() {
                Some(name) => self.pattern(&Node::leaf(K::PatternName, name.span), ty, scope)?,
                None => Pat::Any,
            };
            parts = &parts[..parts.len() - 1];
        }
        let mut prefix = vec![];
        for part in parts {
            if part.kind == K::PatternRest {
                return fail(
                    part.span,
                    "E_PROOF_UNSUPPORTED",
                    "A proof list pattern puts its rest at the end.",
                    "Use [] or [x, ..rest], or match the remaining suffix separately.",
                );
            }
            prefix.push(self.pattern(part, item, scope)?);
        }
        Ok(prefix
            .into_iter()
            .rev()
            .fold(tail, |tail, head| Pat::Ctor(CONS.into(), vec![head, tail])))
    }

    /// Only operations with explicit logical equations belong here. Unknown
    /// built-ins remain opaque rather than becoming unverified assumptions.
    pub(super) fn sequence_method(
        &mut self,
        callee: &Node,
        args: &[&Node],
        scope: &Scope,
    ) -> Option<Found<(Term, Type)>> {
        if callee.kind != K::Member || callee.children.len() != 2 {
            return None;
        }
        let (receiver, ty) = self.term(&callee.children[0], None, scope).ok()?;
        let method = self.text(&callee.children[1]);
        if matches!(method, "first" | "last")
            && let Some(item) = list_item(&ty)
            && !crate::types::copies(item)
            && !crate::checker::proof_implements(
                self.parsed,
                self.checked,
                &scope.bounds,
                item,
                &Type::named("Copy"),
            )
        {
            return Some(fail(
                callee.span,
                "E_PROOF_UNSUPPORTED",
                "This list accessor returns a borrowed view, which the kernel cannot represent yet.",
                "Use a copyable element type, or reason about the list with patterns.",
            ));
        }
        let (op, result, count) = if let Some(item) = list_item(&ty) {
            match method {
                "length" => ("$list.length", Type::named("Int"), 0),
                "isEmpty" => ("$list.isEmpty", Type::named("Bool"), 0),
                "reversed" => ("$list.reversed", ty.clone(), 0),
                "first" => (
                    "$list.first",
                    Type::applied("Option", vec![item.clone()]),
                    0,
                ),
                "last" => ("$list.last", Type::applied("Option", vec![item.clone()]), 0),
                _ => return None,
            }
        } else if matches!(&ty, Type::Named(id, a) if id == "Option" && a.len() == 1) {
            match method {
                "isSome" => ("$option.isSome", Type::named("Bool"), 0),
                "isNone" => ("$option.isNone", Type::named("Bool"), 0),
                _ => return None,
            }
        } else {
            return None;
        };
        if args.len() != count {
            return Some(fail(
                callee.span,
                "E_PROOF_TYPE",
                format!("`{method}` takes {count} arguments."),
                "Supply the method's arguments in order.",
            ));
        }
        let mut terms = vec![receiver];
        for arg in args {
            match self.term(arg, Some(&Type::named("Int")), scope) {
                Ok((term, _)) => terms.push(term),
                Err(error) => return Some(Err(error)),
            }
        }
        Some(Ok((Term::Op(op.into(), terms), result)))
    }
}
