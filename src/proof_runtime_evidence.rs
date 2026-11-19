//! Checked theorem calls used as erased arguments in executable code.
use super::*;
use crate::contracts::Term as Contract;

pub(crate) fn runtime_theorem<'a>(
    environment: &'a crate::checker::Environment,
    name: &str,
) -> Option<&'a Theorem> {
    let id = environment.theorems.get(name)?;
    Some(&environment.proofs.theorems.get(id)?.theorem)
}

pub(crate) fn check_runtime_evidence(
    parsed: &Parsed,
    checked: &Checked,
    call: &Node,
    argument: &Node,
    expected: Option<&Contract>,
    facts: &[Contract],
) -> Found<()> {
    let mut kernel = Kernel::new(parsed, checked);
    // Only previously checked exports may serve as executable evidence. Local
    // declarations are not ordinary values and must not shadow imported names.
    for node in &parsed.root.children {
        if node.kind != K::Theorem {
            continue;
        }
        let Some(name) = node.child(K::Name) else {
            continue;
        };
        let id = DeclarationId {
            file: parsed.source.name.clone(),
            span: node.span,
        };
        if let Some(export) = checked.environment.proofs.theorems.get(&id) {
            kernel.theorems.insert(
                parsed.source.slice(name.span).to_owned(),
                export.theorem.clone(),
            );
        }
    }
    let enclosing = parsed
        .root
        .descendants()
        .filter(|node| {
            matches!(node.kind, K::Function | K::Constructor | K::Lambda)
                && node.span.start <= call.span.start
                && node.span.end >= call.span.end
        })
        .min_by_key(|node| node.span.end - node.span.start);
    let mut ctx = Context::default();
    if let Some(function) = enclosing {
        if let Some((id, _)) = kernel
            .signatures
            .iter()
            .find(|(id, _)| id.file == parsed.source.name && id.span == function.span)
        {
            if let Some(info) = kernel.method_infos.get(id) {
                kernel.access_owner = Some(info.owner.clone());
                kernel.bind_class_parameter(&mut ctx.scope, "this", &info.receiver);
            }
            if let Some(bounds) = kernel.constraints.get(id) {
                for (subject, bound) in bounds {
                    if let Type::Parameter(name) | Type::Rigid(name) = subject {
                        ctx.scope
                            .bounds
                            .entry(name.clone())
                            .or_default()
                            .push(bound.clone());
                    }
                }
            }
        }
        for node in function.descendants() {
            if node.span.start > call.span.start {
                continue;
            }
            if let Some(ty) = checked.binding_types.get(&node.span)
                && let Some(name) = node.child(K::Name)
            {
                kernel.bind_class_parameter(&mut ctx.scope, parsed.source.slice(name.span), ty);
            }
        }
    }
    // Expression types also cover top-level values and pattern bindings. The
    // evidence expression itself is checked against the same lexical bindings.
    let root = enclosing.unwrap_or(&parsed.root);
    let types = checked
        .expression_types
        .iter()
        .cloned()
        .collect::<BTreeMap<_, _>>();
    for node in root
        .descendants()
        .filter(|node| node.kind == K::Name && node.span.start <= argument.span.end)
    {
        let name = parsed.source.slice(node.span);
        if let Some(ty) = types.get(&node.span)
            && !matches!(ty, Type::Function(_) | Type::Overloaded(_))
        {
            kernel.bind_class_parameter(&mut ctx.scope, name, ty);
        }
    }
    for fact in facts {
        if let Some(prop) = kernel.runtime_prop(fact, &ctx.scope) {
            ctx.hyps.push((String::new(), prop));
        }
    }
    let mut current = Current {
        name: "#runtime-evidence".into(),
        start: usize::MAX,
        candidates: None,
        admitted: false,
        theorem: Theorem {
            params: vec![],
            bounds: ctx.scope.bounds.clone(),
            requires: vec![],
            statement: Prop::True,
            dispatches: vec![],
            presupposes: false,
            admitted: false,
            trusted_smt: false,
        },
    };
    kernel.runtime_bounds.set(true);
    // A theorem may have simplified its statement past a method call, so its
    // recorded dispatch obligations and assumption-definedness also matter.
    for node in argument.descendants().filter(|node| node.kind == K::Call) {
        let name = crate::lexer::path_name(kernel.text(&node.children[0]));
        if let Some(theorem) = kernel.theorems.get(&name) {
            if let Some(error) = dispatch_override_error(
                &theorem.dispatches,
                &checked.environment.definitions,
                node.span,
            ) {
                return Err(error);
            }
            if theorem.presupposes {
                return fail(
                    node.span,
                    "E_PROOF_UNDEFINED",
                    "This theorem's assumptions rely on their own arithmetic succeeding.",
                    "Establish arithmetic bounds before using this theorem as executable evidence.",
                );
            }
        }
    }
    let inferred = kernel.infer(argument, &ctx, &mut current)?;
    if current.admitted {
        return fail(
            argument.span,
            "E_PROOF_SORRY",
            "An admitted theorem or axiom cannot discharge an executable proof argument.",
            "Prove the theorem before using it as an executable requirement.",
        );
    }
    if let Some(error) = dispatch_override_error(
        &kernel.dispatches_in_prop(&inferred),
        &checked.environment.definitions,
        argument.span,
    ) {
        return Err(error);
    }
    kernel.transparent(&inferred, argument.span)?;
    kernel.defined_prop(&inferred, &ctx, argument.span)?;
    if let Some(expected) = expected {
        let Some(goal) = kernel.runtime_prop(expected, &ctx.scope) else {
            return fail(
                argument.span,
                "E_PROOF_UNSUPPORTED",
                "The kernel cannot read this executable proof claim.",
                "Use a claim about readable values and pure functions.",
            );
        };
        if let Some(error) = dispatch_override_error(
            &kernel.dispatches_in_prop(&goal),
            &checked.environment.definitions,
            argument.span,
        ) {
            return Err(error);
        }
        kernel.transparent(&goal, argument.span)?;
        kernel.defined_prop(&goal, &ctx, argument.span)?;
        if !kernel.same(&inferred, &goal) {
            return fail(
                argument.span,
                "E_PROOF_MISMATCH",
                format!("This argument proves `{inferred}`, but the parameter needs `{goal}`."),
                "Pass a theorem proving this parameter's claim for the supplied values.",
            );
        }
    }
    Ok(())
}

impl Kernel<'_> {
    fn runtime_prop(&mut self, value: &Contract, scope: &Scope) -> Option<Prop> {
        Some(match value {
            Contract::Bool(true) => Prop::True,
            Contract::Bool(false) => Prop::False,
            Contract::Binary(op, a, b) if matches!(op.as_str(), "&&" | "||") => {
                let (a, b) = (
                    Box::new(self.runtime_prop(a, scope)?),
                    Box::new(self.runtime_prop(b, scope)?),
                );
                if op == "&&" {
                    Prop::And(a, b)
                } else {
                    Prop::Or(a, b)
                }
            }
            Contract::Unary(op, value) if op == "!" => {
                Prop::Not(Box::new(self.runtime_prop(value, scope)?))
            }
            Contract::Binary(op, a, b) if matches!(op.as_str(), "=" | "==") => {
                let (a, aty) = self.runtime_term(a, scope)?;
                let (b, bty) = self.runtime_term(b, scope)?;
                // User-defined equality is not propositional equality.
                if op == "=="
                    && !matches!(&aty, Type::Named(id,args) if args.is_empty() && matches!(id.as_str(),"Nat"|"Int"|"Bool"))
                {
                    return None;
                }
                if !compatible(&aty, &bty) {
                    return None;
                }
                Prop::Eq(a, b)
            }
            _ => {
                let (term, ty) = self.runtime_term(value, scope)?;
                if ty != Type::named("Bool") {
                    return None;
                }
                Prop::Holds(term)
            }
        })
    }

    fn runtime_term(&mut self, value: &Contract, scope: &Scope) -> Option<(Term, Type)> {
        Some(match value {
            Contract::Int(n) => (Term::Int(*n), Type::named("Int")),
            Contract::Nat(n) => (Term::Nat(*n), nat()),
            Contract::Bool(b) => (Term::Bool(*b), Type::named("Bool")),
            Contract::Name(name) => {
                let (_, term, ty) = scope.get(name)?;
                (term.clone(), ty.clone())
            }
            Contract::This => {
                let (_, term, ty) = scope.get("this")?;
                (term.clone(), ty.clone())
            }
            Contract::Field(base, name) => {
                let (base, ty) = self.runtime_term(base, scope)?;
                self.plain_class(&ty).ok()?;
                self.project_value(base, &ty, name).ok()?
            }
            Contract::Unary(op, value) => {
                let (value, ty) = self.runtime_term(value, scope)?;
                if !(op == "-" && ty == Type::named("Int")
                    || op == "!" && ty == Type::named("Bool"))
                {
                    return None;
                }
                (Term::Op(op.clone(), vec![value]), ty)
            }
            Contract::Binary(op, a, b) => {
                let (a, aty) = self.runtime_term(a, scope)?;
                let (b, bty) = self.runtime_term(b, scope)?;
                if !compatible(&aty, &bty) {
                    return None;
                }
                let ty = if matches!(
                    op.as_str(),
                    "==" | "!=" | "<" | "<=" | ">" | ">=" | "&&" | "||"
                ) {
                    Type::named("Bool")
                } else {
                    aty.clone()
                };
                if !matches!(&aty, Type::Named(id,args) if args.is_empty() && matches!(id.as_str(),"Nat"|"Int"|"Bool"))
                {
                    return None;
                }
                (Term::Op(op.clone(), vec![a, b]), ty)
            }
            Contract::Call(id, args) => {
                let (name, params, result) = self.signatures.get(id)?.clone();
                if params.len() != args.len() {
                    return None;
                }
                let mut terms = vec![];
                let mut visible = vec![];
                let mut holes = Holes::new();
                let types = params
                    .iter()
                    .map(|(_, ty)| ty)
                    .chain(std::iter::once(&result))
                    .collect::<Vec<_>>();
                let variables = self.holes_for(&types);
                for ((name, expected), arg) in params.iter().zip(args) {
                    let (term, ty) = self.runtime_term(arg, scope)?;
                    let expected = instantiate(expected, &variables);
                    let comparable = Self::class_type(&expected)
                        .and_then(|(owner, _)| {
                            class_as_supertype(
                                ty.clone(),
                                owner,
                                &self.checked.environment.definitions,
                            )
                        })
                        .unwrap_or_else(|| ty.clone());
                    if !unify(&expected, &comparable, &mut holes) {
                        return None;
                    }
                    visible.push((name.clone(), ty, term.clone()));
                    terms.push(term);
                }
                let dispatch = self
                    .method_infos
                    .get(id)
                    .filter(|info| !info.is_static)
                    .and_then(|_| {
                        params
                            .first()
                            .map(|(_, ty)| Box::new(settle(&instantiate(ty, &variables), &holes)))
                    });
                terms.extend(self.hidden_arguments(id, &visible, scope));
                (
                    Term::Call {
                        function: id.clone(),
                        name,
                        args: terms,
                        dispatch,
                    },
                    settle(&instantiate(&result, &variables), &holes),
                )
            }
            Contract::Case(ty, case, args) => {
                let definition = self.checked.environment.definitions.get(ty)?;
                let (fields, result) = if case == "new" && definition.record {
                    let result = Type::Named(
                        ty.clone(),
                        definition
                            .parameters
                            .iter()
                            .cloned()
                            .map(Type::Parameter)
                            .collect(),
                    );
                    let fields = self
                        .own_class_fields(&result)
                        .ok()?
                        .into_iter()
                        .map(|(_, ty)| ty)
                        .collect::<Vec<_>>();
                    (fields, result)
                } else {
                    let variant = definition
                        .variants
                        .iter()
                        .find(|variant| &variant.name == case)?;
                    (variant.fields.clone(), variant.result.clone())
                };
                if fields.len() != args.len() {
                    return None;
                }
                let mut terms = vec![];
                let mut holes = Holes::new();
                let types = fields
                    .iter()
                    .chain(std::iter::once(&result))
                    .collect::<Vec<_>>();
                let variables = self.holes_for(&types);
                for (arg, ty) in args.iter().zip(&fields) {
                    let (term, found) = self.runtime_term(arg, scope)?;
                    if !unify(&instantiate(ty, &variables), &found, &mut holes) {
                        return None;
                    }
                    terms.push(term);
                }
                let result = settle(&instantiate(&result, &variables), &holes);
                if case == "new" {
                    self.plain_class(&result).ok()?;
                }
                (self.ctor(ty, case, terms), result)
            }
            Contract::Double(_) | Contract::Text(_) | Contract::Opaque(_) => return None,
        })
    }
}
