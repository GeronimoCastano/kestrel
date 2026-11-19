//! Trait requirements follow the same rules in programs and erased proofs.
use super::*;

impl Kernel<'_> {
    pub(super) fn check_bounds(
        &self,
        constraints: &[(Type, Type)],
        variables: &Holes,
        holes: &Holes,
        scope: &Scope,
        span: Span,
    ) -> Found<()> {
        for (actual, bound) in constraints {
            let actual = generic::resolve(&instantiate(actual, variables), holes);
            let bound = generic::resolve(&instantiate(bound, variables), holes);
            if !crate::checker::proof_implements(
                self.parsed,
                self.checked,
                &scope.bounds,
                &actual,
                &bound,
            ) {
                return fail(
                    span,
                    "E_PROOF_BOUND",
                    format!(
                        "This application cannot establish the required trait {} for {}.",
                        settle(&bound, holes),
                        settle(&actual, holes)
                    ),
                    "Declare the bound on the theorem's type parameter, or pass a type that provides the trait.",
                );
            }
        }
        Ok(())
    }

    pub(super) fn theorem_constraints(theorem: &Theorem) -> Vec<(Type, Type)> {
        Self::bounds_constraints(&theorem.bounds)
    }

    pub(super) fn bounds_constraints(bounds: &BTreeMap<String, Vec<Type>>) -> Vec<(Type, Type)> {
        bounds
            .iter()
            .flat_map(|(name, bounds)| {
                bounds
                    .iter()
                    .map(move |bound| (Type::Parameter(name.clone()), bound.clone()))
            })
            .collect()
    }

    /// A rewrite's value substitution must also satisfy its type requirements.
    pub(super) fn rewrite_bounds(
        &self,
        theorem: &Theorem,
        map: &BTreeMap<String, Term>,
        ctx: &Context,
    ) -> bool {
        if theorem.bounds.is_empty() {
            return true;
        }
        let constraints = Self::theorem_constraints(theorem);
        let mut names = vec![];
        for (_, ty) in &theorem.params {
            generic::variables(ty, &mut names);
        }
        for (ty, bound) in &constraints {
            generic::variables(ty, &mut names);
            generic::variables(bound, &mut names);
        }
        let variables = names
            .into_iter()
            .map(|name| {
                let hole = Type::Parameter(format!("?bound:{name}"));
                (name, hole)
            })
            .collect();
        let mut holes = Holes::new();
        for (name, ty) in &theorem.params {
            let Some(term) = map.get(name) else {
                return false;
            };
            if !unify(
                &instantiate(ty, &variables),
                &self.term_type(term, ctx),
                &mut holes,
            ) {
                return false;
            }
        }
        self.check_bounds(
            &constraints,
            &variables,
            &holes,
            &ctx.scope,
            Span::default(),
        )
        .is_ok()
    }
}
