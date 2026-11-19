//! Checked theorem applications become compile-time obligations and body facts.
use super::*;
use crate::{
    contracts::{Requirement, Term as Contract},
    resolution::DeclarationId,
};

impl Kernel<'_> {
    pub(super) fn function_requirements(
        &mut self,
    ) -> (Vec<Diagnostic>, BTreeMap<Span, Requirement>) {
        let mut errors = vec![];
        let mut requirements = BTreeMap::new();
        for function in self
            .parsed
            .root
            .descendants()
            .filter(|n| matches!(n.kind, K::Function | K::Constructor))
        {
            for requirement in function
                .children
                .iter()
                .filter(|n| self.checked.theorem_requirements.contains(&n.span))
            {
                // A requirement's facts can remove run-time checks, so its arithmetic
                // keeps the 64-bit bounds.
                self.runtime_bounds.set(true);
                let found = self.function_requirement(function, requirement);
                self.runtime_bounds.set(false);
                match found {
                    Ok(proof) => {
                        requirements.insert(requirement.span, proof);
                    }
                    Err(error) => errors.push(error),
                }
            }
        }
        (errors, requirements)
    }

    fn function_requirement(&mut self, function: &Node, requirement: &Node) -> Found<Requirement> {
        let call = &requirement.children[0];
        let name = &crate::lexer::path_name(self.text(&call.children[0]));
        let Some(theorem) = self.theorems.get(name).cloned() else {
            return fail(
                call.span,
                "E_PROOF_UNCHECKED",
                format!("The required theorem `{name}` did not check."),
                "Fix that theorem's proof before requiring it.",
            );
        };
        if theorem.admitted {
            return fail(
                call.span,
                "E_PROOF_SORRY",
                format!("A function cannot require `{name}` because its proof uses `sorry`."),
                "Prove the theorem before using it to remove run-time checks.",
            );
        }
        if theorem.presupposes {
            // A requirement's facts can remove run-time checks, so its assumptions
            // must be defined by the ones before them (Q82).
            return fail(
                call.span,
                "E_PROOF_UNDEFINED",
                format!(
                    "A function cannot require `{name}`: one of its assumptions relies on its own arithmetic succeeding."
                ),
                "Add assumptions to the theorem that bound that arithmetic first.",
            );
        }
        let signature=function.child(K::Name).and_then(|n|self.signature_entry(self.text(n),function.span))
            .or_else(||self.checked.environment.definitions.values().flat_map(|d|d.methods.values().flatten().chain(d.constructors.iter())).find(|f|matches!(&f.callable,Some(Callable::Declaration {id,..}) if id.span==function.span&&id.file==self.parsed.source.name)).cloned());
        let Some(signature) = signature else {
            return fail(
                requirement.span,
                "E_PROOF_UNSUPPORTED",
                "The kernel cannot read this function's parameter types.",
                "Require the theorem on a function with explicit parameter types.",
            );
        };
        let mut scope = Scope {
            bounds: signature.bounds.clone(),
            ..Scope::default()
        };
        for (subject, bound) in &signature.requirements {
            if let Type::Parameter(name) | Type::Rigid(name) = subject {
                scope
                    .bounds
                    .entry(name.clone())
                    .or_default()
                    .push(bound.clone());
            }
        }
        let mut parameters = vec![];
        for parameter in &signature.parameters {
            let name = parameter.name.clone().unwrap_or_default();
            self.bind_class_parameter(&mut scope, &name, &parameter.ty);
            parameters.push(name);
        }
        if let Some(receiver) = &signature.receiver {
            self.bind_class_parameter(&mut scope, "this", receiver);
        }
        let mut ctx = Context {
            scope: scope.clone(),
            ..Context::default()
        };
        for prior in function
            .children
            .iter()
            .filter(|node| node.kind == K::Requires && node.span.start < requirement.span.start)
        {
            if self.checked.theorem_requirements.contains(&prior.span) {
                continue;
            }
            if let Some(expression) = prior.children.first()
                && let Ok(prop) = self.prop(expression, &scope)
                && self.transparent(&prop, prior.span).is_ok()
                && self.defined_prop(&prop, &ctx, prior.span).is_ok()
            {
                ctx.hyps.push((String::new(), prop));
            }
        }
        let mut supplied = BTreeMap::new();
        let mut position = 0;
        let constraints = Self::theorem_constraints(&theorem);
        let mut types = theorem.params.iter().map(|(_, ty)| ty).collect::<Vec<_>>();
        for (ty, bound) in &constraints {
            types.extend([ty, bound]);
        }
        let variables = self.holes_for(&types);
        let mut holes = Holes::new();
        for arg in &call.children[1..] {
            let pname = if arg.kind == K::NamedArgument {
                self.text(&arg.children[0]).to_owned()
            } else {
                let Some((n, _)) = theorem.params.get(position) else {
                    return fail(
                        arg.span,
                        "E_PROOF_SHAPE",
                        format!("`{name}` takes {} arguments.", theorem.params.len()),
                        "Pass one term per theorem parameter.",
                    );
                };
                position += 1;
                n.clone()
            };
            let Some((_, ty)) = theorem.params.iter().find(|(n, _)| n == &pname) else {
                return fail(
                    arg.span,
                    "E_PROOF_SHAPE",
                    format!("`{name}` has no parameter `{pname}`."),
                    "Use a declared theorem parameter name.",
                );
            };
            let ty = generic::resolve(&instantiate(ty, &variables), &holes);
            let (term, found) = self.term(arg.children.last().unwrap(), Some(&ty), &scope)?;
            self.transparent(&Prop::Eq(term.clone(), term.clone()), arg.span)?;
            self.defined_term(&term, &ctx, arg.span)?;
            if !unify(&ty, &found, &mut holes) {
                return fail(
                    arg.span,
                    "E_PROOF_TYPE",
                    format!("`{name}` expects {ty}, found {found}."),
                    "Pass a term of the declared type.",
                );
            }
            if supplied.insert(pname, term).is_some() {
                return fail(
                    arg.span,
                    "E_PROOF_SHAPE",
                    "A theorem argument is supplied twice.",
                    "Pass each argument once.",
                );
            }
        }
        if supplied.len() != theorem.params.len() {
            return fail(
                call.span,
                "E_PROOF_SHAPE",
                format!("`{name}` takes {} arguments.", theorem.params.len()),
                "Pass one term per theorem parameter.",
            );
        }
        self.check_bounds(&constraints, &variables, &holes, &scope, requirement.span)?;
        if let Some(error) = dispatch_override_error(
            &theorem.dispatches,
            &self.checked.environment.definitions,
            requirement.span,
        ) {
            return Err(error);
        }
        let arguments = supplied.clone();
        self.add_class_projection_substitutions(&theorem.params, &arguments, &mut supplied);
        let mut assumptions = Contract::Bool(true);
        for (_, prop) in &theorem.requires {
            let prop = prop.map(&mut |t| t.subst(&supplied, &mut self.fresh));
            let prop = self.normal(&prop);
            let term = self.contract_prop(&prop, &ctx, requirement.span)?;
            assumptions = Contract::Binary("&&".into(), Box::new(assumptions), Box::new(term));
        }
        let statement = theorem
            .statement
            .map(&mut |t| t.subst(&supplied, &mut self.fresh));
        let statement = self.normal(&statement);
        let mut defaults = BTreeMap::new();
        if let Some(nodes) = function.child(K::ParameterList) {
            for (node, parameter) in nodes.children.iter().zip(&signature.parameters) {
                if parameter.default
                    && let Some(value) = node.children.last()
                    && let Ok((term, _)) = self.term(value, Some(&parameter.ty), &Scope::default())
                {
                    let term = self.reducer().nf(&term);
                    if let Ok(term) = self.contract_term(&term, value.span) {
                        defaults.insert(parameter.name.clone().unwrap_or_default(), term);
                    }
                }
            }
        }
        Ok(Requirement {
            parameters,
            term: assumptions,
            text: self.text(call).into(),
            fact: Some(self.contract_prop(&statement, &ctx, requirement.span)?),
            declaration: Some(DeclarationId {
                file: self.parsed.source.name.clone(),
                span: function.span,
            }),
            proof: None,
            defaults,
        })
    }

    fn contract_prop(&self, prop: &Prop, ctx: &Context, span: Span) -> Found<Contract> {
        Ok(match prop {
            Prop::Eq(a, b) if a == b => Contract::Bool(true),
            Prop::Eq(a, b) => Contract::Binary(
                // Propositional equality does not promise anything about a
                // user-written Equals method (or NaN). Keep it distinct unless
                // the runtime comparison has exactly the proof semantics.
                if matches!(self.term_type(a, ctx), Type::Named(ref n, ref args)
                    if args.is_empty() && matches!(n.as_str(), "Nat" | "Int" | "Bool"))
                {
                    "=="
                } else {
                    "="
                }
                .into(),
                Box::new(self.contract_term(a, span)?),
                Box::new(self.contract_term(b, span)?),
            ),
            Prop::Holds(t) => self.contract_term(t, span)?,
            Prop::And(a, b) | Prop::Or(a, b) => Contract::Binary(
                if matches!(prop, Prop::And(..)) {
                    "&&"
                } else {
                    "||"
                }
                .into(),
                Box::new(self.contract_prop(a, ctx, span)?),
                Box::new(self.contract_prop(b, ctx, span)?),
            ),
            Prop::Not(p) => {
                Contract::Unary("!".into(), Box::new(self.contract_prop(p, ctx, span)?))
            }
            Prop::True => Contract::Bool(true),
            Prop::False => Contract::Bool(false),
            Prop::Mem(..)
            | Prop::Subset { .. }
            | Prop::Implies(..)
            | Prop::Iff(..)
            | Prop::Forall { .. }
            | Prop::Exists { .. }
            | Prop::Atom(_) => {
                return fail(
                    span,
                    "E_PROOF_UNSUPPORTED",
                    "A run-time requires condition cannot use implication, forall or exists.",
                    "Keep run-time conditions Bool, equality, and or not; prove quantified facts as theorems.",
                );
            }
        })
    }

    fn contract_term(&self, term: &Term, span: Span) -> Found<Contract> {
        Ok(match term {
            Term::Var(n) => {
                let mut parts = n.split('.');
                let root = parts.next().unwrap_or(n);
                let mut value = if root == "this" {
                    Contract::This
                } else {
                    Contract::Name(root.to_owned())
                };
                for field in parts.filter(|field| *field != "$super") {
                    value = Contract::Field(Box::new(value), field.to_owned());
                }
                value
            }
            Term::Nat(n) => Contract::Nat(*n),
            Term::Int(n) => Contract::Int(*n),
            Term::Bool(b) => Contract::Bool(*b),
            Term::Call { function, args, .. } => Contract::Call(
                function.clone(),
                args.iter()
                    .map(|t| self.contract_term(t, span))
                    .collect::<Found<_>>()?,
            ),
            Term::Ctor { ty, case, args } if ty == "Nat" && case == "Succ" && args.len() == 1 => {
                Contract::Unary(
                    "natSucc".into(),
                    Box::new(self.contract_term(&args[0], span)?),
                )
            }
            Term::Ctor { ty, case, args } => Contract::Case(
                ty.clone(),
                case.clone(),
                args.iter()
                    .map(|t| self.contract_term(t, span))
                    .collect::<Found<_>>()?,
            ),
            Term::Op(op, args) if args.len() == 1 => {
                Contract::Unary(op.clone(), Box::new(self.contract_term(&args[0], span)?))
            }
            Term::Op(op, args) if args.len() == 2 => Contract::Binary(
                op.clone(),
                Box::new(self.contract_term(&args[0], span)?),
                Box::new(self.contract_term(&args[1], span)?),
            ),
            _ => {
                return fail(
                    span,
                    "E_PROOF_UNSUPPORTED",
                    "This required theorem contains a term the contract prover cannot represent yet.",
                    "Use equations, comparisons and pure calls without a stuck if or match.",
                );
            }
        })
    }
}
