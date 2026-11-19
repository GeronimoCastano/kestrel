//! Keep the fields of a proof parameter tied to the value supplied for it.
use super::*;

impl Kernel<'_> {
    pub(super) fn bind_class_parameter(&self, scope: &mut Scope, name: &str, ty: &Type) {
        let readable = self.plain_class(ty).is_ok();
        let value = readable
            .then(|| self.symbolic_value(name, ty))
            .flatten()
            .unwrap_or_else(|| Term::Var(name.to_owned()));
        *scope = scope.with(name, value, ty.clone());
        if readable {
            for (projected, field_type, _) in self.projection_specs(ty, name) {
                let value = self
                    .plain_class(&field_type)
                    .is_ok()
                    .then(|| self.symbolic_value(&projected, &field_type))
                    .flatten()
                    .unwrap_or_else(|| Term::Var(projected.clone()));
                *scope = scope.with(&projected, value, field_type);
            }
        }
    }

    /// A readable object is determined by its leaf fields. These are the holes
    /// inferred by apply and simp; the surface theorem still takes one object.
    pub(super) fn theorem_matching_params(&self, theorem: &Theorem) -> Vec<(String, Type)> {
        let mut result = vec![];
        for (name, ty) in &theorem.params {
            if self.plain_class(ty).is_ok()
                && let Some(value) = self.symbolic_value(name, ty)
            {
                let mut leaves = BTreeSet::new();
                value.free(&mut leaves);
                if !leaves.is_empty() {
                    result.extend(
                        self.projection_specs(ty, name)
                            .into_iter()
                            .filter(|(name, _, _)| leaves.contains(name))
                            .map(|(name, ty, _)| (name, ty)),
                    );
                    continue;
                }
            }
            result.push((name.clone(), ty.clone()));
        }
        result
    }

    /// Rebuild inferred objects only after every leaf field has a witness.
    pub(super) fn complete_class_arguments(
        &self,
        theorem: &Theorem,
        map: &mut BTreeMap<String, Term>,
    ) -> bool {
        for (name, ty) in &theorem.params {
            if map.contains_key(name) {
                continue;
            }
            if self.plain_class(ty).is_err() {
                continue;
            }
            let Some(value) = self.symbolic_value(name, ty) else {
                continue;
            };
            let mut leaves = BTreeSet::new();
            value.free(&mut leaves);
            if leaves.is_empty() || leaves.iter().any(|field| !map.contains_key(field)) {
                continue;
            }
            let value = value.subst(map, &mut 0);
            map.insert(name.clone(), value);
        }
        if theorem
            .params
            .iter()
            .any(|(name, _)| !map.contains_key(name))
        {
            return false;
        }
        let arguments = map.clone();
        self.add_class_projection_substitutions(&theorem.params, &arguments, map);
        true
    }
}
