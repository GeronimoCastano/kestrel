//! Infer a user trait's associated types from the whole implementation.
use super::*;

impl Checker<'_> {
    pub(super) fn infer_associated(
        &self,
        concrete: &Type,
        definition: &Definition,
        arguments: &[Type],
        explicit: Option<&BTreeMap<String, Type>>,
    ) -> Result<BTreeMap<String, Type>, &'static str> {
        let mut inference = self.inference.clone();
        let mut substitutions = definition
            .parameters
            .iter()
            .cloned()
            .zip(arguments.iter().cloned())
            .collect::<BTreeMap<_, _>>();
        let mut slots = BTreeMap::new();
        for name in &definition.associated_types {
            let variable = inference.fresh();
            if let Some(value) = explicit.and_then(|bindings| bindings.get(name))
                && !inference.unify(&variable, value)
            {
                return Err("conflicting bindings");
            }
            slots.insert(name.clone(), variable.clone());
            substitutions.insert(name.clone(), variable);
        }
        // A `Self` result or parameter is the implementing class here.
        substitutions.insert("Self".to_owned(), concrete.clone());
        let mut states = vec![inference];
        for (name, contracts) in &definition.methods {
            let methods = self.methods_with_access(concrete, name, &mut BTreeSet::new(), false);
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
                            if next.len() > 128 {
                                return Err("too many possible overload combinations");
                            }
                        }
                    }
                }
                if next.is_empty() {
                    let has_default = matches!(
                        &contract.callable,
                        Some(crate::resolution::Callable::Declaration { id, .. })
                            if id.file == definition.file
                                && definition.default_methods.contains(&id.span)
                    );
                    let has_matching_parameters = methods.iter().any(|method| {
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
                    if has_default && !has_matching_parameters {
                        continue;
                    }
                    return Err("missing methods or conflicting associated types");
                }
                states = next;
            }
        }
        let mut allowed = BTreeSet::new();
        concrete.map(&mut |part| {
            if let Type::Parameter(name) | Type::Rigid(name) = part {
                allowed.insert(name.clone());
            }
            None
        });
        let mut solutions = vec![];
        for state in states {
            let bindings = slots
                .iter()
                .map(|(name, variable)| (name.clone(), state.resolve(variable)))
                .collect::<BTreeMap<_, _>>();
            let complete = bindings.values().all(|ty| {
                let mut valid = ty.variables().is_empty();
                ty.map(&mut |part| {
                    if let Type::Parameter(name) | Type::Rigid(name) = part {
                        valid &= allowed.contains(name);
                    }
                    None
                });
                valid
            });
            if !complete {
                return Err("an associated type is not determined by the methods");
            }
            if !solutions.contains(&bindings) {
                solutions.push(bindings);
            }
        }
        match solutions.as_slice() {
            [one] => Ok(one.clone()),
            _ => Err("several different associated type bindings match"),
        }
    }
}
