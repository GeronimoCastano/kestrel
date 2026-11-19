//! Type matching for generic proofs. Type variables are implicit, so a function
//! or theorem names `T` without declaring it. At each call or citation the
//! kernel replaces those variables with holes, written `?n`, which matching
//! fills from the arguments. A theorem's own variables stay fixed inside its
//! proof. `?` alone is a wildcard: it matches anything and records nothing.
use super::*;

/// Holes filled so far during one call or citation.
pub(crate) type Holes = BTreeMap<String, Type>;

fn hole_name(ty: &Type) -> Option<&str> {
    match ty {
        Type::Parameter(name) if name.starts_with('?') && name != "?" => Some(name),
        _ => None,
    }
}

fn wildcard(ty: &Type) -> bool {
    matches!(ty, Type::Parameter(name) if name == "?")
}

/// The name of a type variable that stays fixed: a theorem's or a body's own `T`.
fn fixed(ty: &Type) -> Option<&str> {
    match ty {
        Type::Parameter(name) if !name.starts_with('?') => Some(name),
        Type::Rigid(name) => Some(name),
        _ => None,
    }
}

/// Replaces filled holes by their types.
pub(crate) fn resolve(ty: &Type, holes: &Holes) -> Type {
    resolve_at(ty, holes, 0)
}

fn resolve_at(ty: &Type, holes: &Holes, depth: usize) -> Type {
    if depth > 64 {
        return ty.clone();
    }
    ty.map(&mut |part| match part {
        Type::Parameter(name) => holes
            .get(name)
            .map(|found| resolve_at(found, holes, depth + 1)),
        _ => None,
    })
}

fn occurs(name: &str, ty: &Type) -> bool {
    let mut found = false;
    ty.map(&mut |part| {
        found |= matches!(part,Type::Parameter(other) if other == name);
        None
    });
    found
}

/// Matches two types, filling holes on either side. Fixed variables match only
/// themselves, so a theorem about every `T` cannot assume `T` is Nat.
pub(crate) fn unify(a: &Type, b: &Type, holes: &mut Holes) -> bool {
    let (a, b) = (resolve(a, holes), resolve(b, holes));
    if wildcard(&a) || wildcard(&b) {
        return true;
    }
    if let (Some(x), Some(y)) = (fixed(&a), fixed(&b)) {
        return x == y;
    }
    if let Some(name) = hole_name(&a) {
        if hole_name(&b) == Some(name) {
            return true;
        }
        if occurs(name, &b) {
            return false;
        }
        holes.insert(name.to_owned(), b);
        return true;
    }
    if hole_name(&b).is_some() {
        return unify(&b, &a, holes);
    }
    match (&a, &b) {
        (Type::Named(x, xs), Type::Named(y, ys)) => {
            x == y && xs.len() == ys.len() && xs.iter().zip(ys).all(|(p, q)| unify(p, q, holes))
        }
        // Parameter names, defaults and passing modes are not part of a
        // function type's identity; two proof functions match when they take
        // and return the same types.
        (Type::Function(x), Type::Function(y)) => {
            x.parameters.len() == y.parameters.len()
                && x.parameters
                    .iter()
                    .zip(&y.parameters)
                    .all(|(p, q)| unify(&p.ty, &q.ty, holes))
                && unify(&x.result, &y.result, holes)
        }
        _ => a == b,
    }
}

/// Whether two types can match, without keeping what matching filled.
pub(crate) fn compatible(a: &Type, b: &Type) -> bool {
    unify(a, b, &mut Holes::new())
}

/// The fixed type variables a type names, in order of appearance.
pub(crate) fn variables(ty: &Type, out: &mut Vec<String>) {
    ty.map(&mut |part| {
        if let Some(name) = fixed(part)
            && !out.iter().any(|n| n == name)
        {
            out.push(name.to_owned());
        }
        None
    });
}

/// Substitutes types for fixed type variables.
pub(crate) fn instantiate(ty: &Type, map: &Holes) -> Type {
    ty.map(&mut |part| fixed(part).and_then(|name| map.get(name)).cloned())
}

/// Turns holes nothing filled into wildcards, so a reported type never leaks a
/// hole into another call's matching.
pub(crate) fn settle(ty: &Type, holes: &Holes) -> Type {
    resolve(ty, holes).map(&mut |part| hole_name(part).map(|_| Type::Parameter("?".into())))
}

impl Kernel<'_> {
    /// Fresh holes for the fixed variables of a signature.
    pub(crate) fn holes_for(&mut self, types: &[&Type]) -> Holes {
        let mut names = vec![];
        for ty in types {
            variables(ty, &mut names);
        }
        names
            .into_iter()
            .map(|name| {
                self.fresh += 1;
                (name, Type::Parameter(format!("?{}", self.fresh)))
            })
            .collect()
    }

    /// Whether a value of `actual` can stand where a signature names `pattern`,
    /// with the signature's own variables free to match.
    pub(crate) fn fits(&self, pattern: &Type, actual: &Type) -> bool {
        let mut names = vec![];
        variables(pattern, &mut names);
        let map = names
            .into_iter()
            .map(|name| {
                let hole = Type::Parameter(format!("?{name}"));
                (name, hole)
            })
            .collect();
        compatible(&instantiate(pattern, &map), actual)
    }

    /// The type parameters an enum declares, such as `T` in `Chain[T]`.
    pub(crate) fn enum_parameters(&self, id: &str) -> Vec<String> {
        if matches!(id, "List" | "Option") {
            return vec!["T".into()];
        }
        self.checked
            .environment
            .definitions
            .get(id)
            .map(|d| d.parameters.clone())
            .unwrap_or_default()
    }
}
