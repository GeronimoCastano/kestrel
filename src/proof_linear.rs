//! Independent certificates for linear integer arithmetic. The search builds
//! nonnegative integer multipliers; the checker recomputes their sum.
use super::*;

const BRANCH_LIMIT: usize = 128;
const ROW_LIMIT: usize = 512;

/// `constant + sum(coefficients[name] * name) <= 0`.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
struct Linear {
    coefficients: BTreeMap<String, i128>,
    constant: i128,
}
impl Linear {
    fn number(constant: i128) -> Self {
        Self {
            constant,
            ..Self::default()
        }
    }
    fn variable(name: &str) -> Self {
        Self {
            coefficients: BTreeMap::from([(name.to_owned(), 1)]),
            constant: 0,
        }
    }
    fn scale(&self, factor: i128) -> Option<Self> {
        let mut result = Self::number(self.constant.checked_mul(factor)?);
        for (name, coefficient) in &self.coefficients {
            let value = coefficient.checked_mul(factor)?;
            if value != 0 {
                result.coefficients.insert(name.clone(), value);
            }
        }
        Some(result)
    }
    fn add(&self, other: &Self) -> Option<Self> {
        let mut result = self.clone();
        result.constant = result.constant.checked_add(other.constant)?;
        for (name, coefficient) in &other.coefficients {
            let value = result
                .coefficients
                .get(name)
                .copied()
                .unwrap_or(0)
                .checked_add(*coefficient)?;
            if value == 0 {
                result.coefficients.remove(name);
            } else {
                result.coefficients.insert(name.clone(), value);
            }
        }
        Some(result)
    }
    fn difference(&self, other: &Self) -> Option<Self> {
        self.add(&other.scale(-1)?)
    }
    fn strict(&self) -> Option<Self> {
        self.add(&Self::number(1))
    }
}

#[derive(Clone)]
enum Formula {
    Constant(bool),
    Le(Linear),
    And(Vec<Self>),
    Or(Vec<Self>),
    Not(Box<Self>),
}
impl Formula {
    fn and(parts: Vec<Self>) -> Self {
        Self::And(parts)
    }
    fn or(parts: Vec<Self>) -> Self {
        Self::Or(parts)
    }
    fn not(self) -> Self {
        Self::Not(Box::new(self))
    }
    fn implies(self, other: Self) -> Self {
        Self::or(vec![self.not(), other])
    }
    /// Enumerates conjunctions, with negation pushed to integer inequalities.
    fn branches(&self, positive: bool, fuel: &mut usize) -> Option<Vec<Vec<Linear>>> {
        *fuel = fuel.checked_sub(1)?;
        match self {
            Self::Constant(value) => Some(if *value == positive {
                vec![vec![]]
            } else {
                vec![]
            }),
            Self::Le(row) => Some(vec![vec![if positive {
                row.clone()
            } else {
                row.scale(-1)?.strict()?
            }]]),
            Self::Not(inner) => inner.branches(!positive, fuel),
            Self::And(parts) | Self::Or(parts) => {
                let conjunction = matches!(self, Self::And(_)) == positive;
                let mut result = if conjunction { vec![vec![]] } else { vec![] };
                for part in parts {
                    let next = part.branches(positive, fuel)?;
                    if conjunction {
                        if result.len().checked_mul(next.len())? > BRANCH_LIMIT {
                            return None;
                        }
                        let mut product = vec![];
                        for left in &result {
                            for right in &next {
                                if left.len() + right.len() > ROW_LIMIT {
                                    return None;
                                }
                                let mut joined = left.clone();
                                joined.extend(right.clone());
                                product.push(joined);
                            }
                        }
                        result = product;
                    } else {
                        result.extend(next);
                        if result.len() > BRANCH_LIMIT {
                            return None;
                        }
                    }
                }
                Some(result)
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Number {
    Int,
    Nat,
}
#[derive(Clone)]
struct Piece {
    when: Formula,
    value: Linear,
}
#[derive(Clone)]
enum Value {
    Numeric(Number, Vec<Piece>),
    Boolean(Formula),
}
struct Expression {
    value: Value,
    defined: Formula,
}
struct Reader<'a> {
    ctx: &'a Context,
    math: bool,
    fuel: usize,
}
impl Reader<'_> {
    fn numeric(
        &self,
        kind: Number,
        pieces: Vec<Piece>,
        checks: Vec<Formula>,
    ) -> Option<Expression> {
        if pieces.len() > BRANCH_LIMIT {
            return None;
        }
        let mut checks = checks;
        for piece in &pieces {
            let mut bounds = vec![];
            if kind == Number::Nat {
                bounds.push(Formula::Le(piece.value.scale(-1)?));
            }
            if !self.math {
                let (low, high) = if kind == Number::Nat {
                    (0, i128::from(u64::MAX))
                } else {
                    (i128::from(i64::MIN), i128::from(i64::MAX))
                };
                bounds.push(Formula::Le(Linear::number(low).difference(&piece.value)?));
                bounds.push(Formula::Le(piece.value.difference(&Linear::number(high))?));
            }
            checks.push(piece.when.clone().implies(Formula::and(bounds)));
        }
        Some(Expression {
            value: Value::Numeric(kind, pieces),
            defined: Formula::and(checks),
        })
    }
    fn literal(&self, kind: Number, n: i128) -> Option<Expression> {
        self.numeric(
            kind,
            vec![Piece {
                when: Formula::Constant(true),
                value: Linear::number(n),
            }],
            vec![],
        )
    }
    fn variable_type(&self, name: &str) -> Option<&Type> {
        self.ctx
            .scope
            .terms
            .iter()
            .rev()
            .find_map(|(_, term, ty)| matches!(term, Term::Var(n) if n == name).then_some(ty))
    }
    fn term(&mut self, term: &Term, depth: usize) -> Option<Expression> {
        self.fuel = self.fuel.checked_sub(1)?;
        if depth >= 96 {
            return None;
        }
        match term {
            Term::Int(n) => self.literal(Number::Int, i128::from(*n)),
            Term::Nat(n) => self.literal(Number::Nat, i128::from(*n)),
            Term::Bool(b) => Some(Expression {
                value: Value::Boolean(Formula::Constant(*b)),
                defined: Formula::Constant(true),
            }),
            Term::Var(name) => {
                let Type::Named(ty, args) = self.variable_type(name)? else {
                    return None;
                };
                if !args.is_empty() {
                    return None;
                }
                if ty == "Bool" {
                    let variable = Linear::variable(name);
                    Some(Expression {
                        value: Value::Boolean(Formula::Le(
                            Linear::number(1).difference(&variable)?,
                        )),
                        defined: Formula::and(vec![
                            Formula::Le(variable.scale(-1)?),
                            Formula::Le(variable.difference(&Linear::number(1))?),
                        ]),
                    })
                } else {
                    let kind = match ty.as_str() {
                        "Int" => Number::Int,
                        "Nat" => Number::Nat,
                        _ => return None,
                    };
                    self.numeric(
                        kind,
                        vec![Piece {
                            when: Formula::Constant(true),
                            value: Linear::variable(name),
                        }],
                        vec![],
                    )
                }
            }
            Term::Ctor { ty, case, args } if ty == "Nat" && case == "Succ" && args.len() == 1 => {
                let child = self.term(&args[0], depth + 1)?;
                let Value::Numeric(Number::Nat, pieces) = child.value else {
                    return None;
                };
                let pieces = pieces
                    .into_iter()
                    .map(|p| {
                        Some(Piece {
                            when: p.when,
                            value: p.value.add(&Linear::number(1))?,
                        })
                    })
                    .collect::<Option<Vec<_>>>()?;
                self.numeric(Number::Nat, pieces, vec![child.defined])
            }
            Term::If(condition, left, right) => {
                let condition = self.term(condition, depth + 1)?;
                let Value::Boolean(condition_value) = condition.value else {
                    return None;
                };
                let left = self.term(left, depth + 1)?;
                let right = self.term(right, depth + 1)?;
                let defined = Formula::and(vec![
                    condition.defined,
                    condition_value.clone().implies(left.defined),
                    condition_value.clone().not().implies(right.defined),
                ]);
                let value = match (left.value, right.value) {
                    (Value::Numeric(a, mut left), Value::Numeric(b, mut right)) if a == b => {
                        for p in &mut left {
                            p.when = Formula::and(vec![condition_value.clone(), p.when.clone()]);
                        }
                        for p in &mut right {
                            p.when =
                                Formula::and(vec![condition_value.clone().not(), p.when.clone()]);
                        }
                        left.extend(right);
                        if left.len() > BRANCH_LIMIT {
                            return None;
                        }
                        Value::Numeric(a, left)
                    }
                    (Value::Boolean(left), Value::Boolean(right)) => {
                        Value::Boolean(Formula::or(vec![
                            Formula::and(vec![condition_value.clone(), left]),
                            Formula::and(vec![condition_value.not(), right]),
                        ]))
                    }
                    _ => return None,
                };
                Some(Expression { value, defined })
            }
            Term::Op(op, args) if args.len() == 1 => {
                let child = self.term(&args[0], depth + 1)?;
                match (op.as_str(), child.value) {
                    ("!", Value::Boolean(value)) => Some(Expression {
                        value: Value::Boolean(value.not()),
                        defined: child.defined,
                    }),
                    ("-", Value::Numeric(Number::Int, pieces)) => {
                        let pieces = pieces
                            .into_iter()
                            .map(|p| {
                                Some(Piece {
                                    when: p.when,
                                    value: p.value.scale(-1)?,
                                })
                            })
                            .collect::<Option<Vec<_>>>()?;
                        self.numeric(Number::Int, pieces, vec![child.defined])
                    }
                    _ => None,
                }
            }
            Term::Op(op, args) if args.len() == 2 => self.binary(op, &args[0], &args[1], depth + 1),
            _ => None,
        }
    }
    fn binary(&mut self, op: &str, left: &Term, right: &Term, depth: usize) -> Option<Expression> {
        let left = self.term(left, depth)?;
        let right = self.term(right, depth)?;
        match (left.value, right.value) {
            (Value::Boolean(a), Value::Boolean(b)) => {
                let checks = match op {
                    "&&" => a.clone().implies(right.defined),
                    "||" => a.clone().not().implies(right.defined),
                    "==" | "!=" => right.defined,
                    _ => return None,
                };
                let value = match op {
                    "&&" => Formula::and(vec![a, b]),
                    "||" => Formula::or(vec![a, b]),
                    "==" | "!=" => {
                        let equal = Formula::or(vec![
                            Formula::and(vec![a.clone(), b.clone()]),
                            Formula::and(vec![a.not(), b.not()]),
                        ]);
                        if op == "!=" { equal.not() } else { equal }
                    }
                    _ => return None,
                };
                Some(Expression {
                    value: Value::Boolean(value),
                    defined: Formula::and(vec![left.defined, checks]),
                })
            }
            (Value::Numeric(a, lefts), Value::Numeric(b, rights)) if a == b => {
                if lefts.len().checked_mul(rights.len())? > BRANCH_LIMIT {
                    return None;
                }
                let mut pieces = vec![];
                let mut alternatives = vec![];
                let arithmetic = matches!(op, "+" | "-" | "*");
                for left in &lefts {
                    for right in &rights {
                        let when = Formula::and(vec![left.when.clone(), right.when.clone()]);
                        if arithmetic {
                            let value = match op {
                                "+" => left.value.add(&right.value)?,
                                "-" => left.value.difference(&right.value)?,
                                "*" if left.value.coefficients.is_empty() => {
                                    right.value.scale(left.value.constant)?
                                }
                                "*" if right.value.coefficients.is_empty() => {
                                    left.value.scale(right.value.constant)?
                                }
                                _ => return None,
                            };
                            pieces.push(Piece { when, value });
                        } else {
                            let difference = left.value.difference(&right.value)?;
                            let comparison = match op {
                                "<=" => Formula::Le(difference),
                                "<" => Formula::Le(difference.strict()?),
                                ">=" => Formula::Le(difference.scale(-1)?),
                                ">" => Formula::Le(difference.scale(-1)?.strict()?),
                                "==" | "!=" => {
                                    let equal = Formula::and(vec![
                                        Formula::Le(difference.clone()),
                                        Formula::Le(difference.scale(-1)?),
                                    ]);
                                    if op == "!=" { equal.not() } else { equal }
                                }
                                _ => return None,
                            };
                            alternatives.push(Formula::and(vec![when, comparison]));
                        }
                    }
                }
                let checks = vec![left.defined, right.defined];
                if arithmetic {
                    self.numeric(a, pieces, checks)
                } else {
                    Some(Expression {
                        value: Value::Boolean(Formula::or(alternatives)),
                        defined: Formula::and(checks),
                    })
                }
            }
            _ => None,
        }
    }
    fn prop(&mut self, prop: &Prop, depth: usize) -> Option<(Formula, Formula)> {
        self.fuel = self.fuel.checked_sub(1)?;
        if depth >= 96 {
            return None;
        }
        match prop {
            Prop::Eq(left, right) => {
                let expression = self.binary("==", left, right, depth + 1)?;
                let Value::Boolean(value) = expression.value else {
                    return None;
                };
                Some((value, expression.defined))
            }
            Prop::Holds(term) => {
                let expression = self.term(term, depth + 1)?;
                let Value::Boolean(value) = expression.value else {
                    return None;
                };
                Some((value, expression.defined))
            }
            Prop::True => Some((Formula::Constant(true), Formula::Constant(true))),
            Prop::False => Some((Formula::Constant(false), Formula::Constant(true))),
            Prop::Not(prop) => {
                let (value, checks) = self.prop(prop, depth + 1)?;
                Some((value.not(), checks))
            }
            Prop::And(a, b) | Prop::Or(a, b) => {
                let (a, ac) = self.prop(a, depth + 1)?;
                let (b, bc) = self.prop(b, depth + 1)?;
                Some((
                    if matches!(prop, Prop::And(..)) {
                        Formula::and(vec![a, b])
                    } else {
                        Formula::or(vec![a, b])
                    },
                    Formula::and(vec![ac, bc]),
                ))
            }
            _ => None,
        }
    }
}

/// Search output is untrusted until `check_certificate` recomputes it.
#[derive(Clone, Debug)]
struct Certificate {
    multipliers: Vec<i128>,
}
fn check_certificate(input: &[Linear], certificate: &Certificate) -> bool {
    if input.len() != certificate.multipliers.len() {
        return false;
    }
    let mut sum = Linear::default();
    for (row, multiplier) in input.iter().zip(&certificate.multipliers) {
        if *multiplier < 0 {
            return false;
        }
        let Some(part) = row.scale(*multiplier) else {
            return false;
        };
        let Some(next) = sum.add(&part) else {
            return false;
        };
        sum = next;
    }
    sum.coefficients.is_empty() && sum.constant > 0
}
fn search(input: &[Linear]) -> Option<Certificate> {
    let mut rows = input
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let mut multipliers = vec![0; input.len()];
            multipliers[i] = 1;
            (row.clone(), Certificate { multipliers })
        })
        .collect::<Vec<_>>();
    for _ in 0..64 {
        if let Some((_, certificate)) = rows
            .iter()
            .find(|(r, _)| r.coefficients.is_empty() && r.constant > 0)
        {
            return Some(certificate.clone());
        }
        let variables = rows
            .iter()
            .flat_map(|(r, _)| r.coefficients.keys().cloned())
            .collect::<BTreeSet<_>>();
        let variable = variables.into_iter().min_by_key(|v| {
            let positive = rows
                .iter()
                .filter(|(r, _)| r.coefficients.get(v).copied().unwrap_or(0) > 0)
                .count();
            let negative = rows
                .iter()
                .filter(|(r, _)| r.coefficients.get(v).copied().unwrap_or(0) < 0)
                .count();
            positive * negative
        })?;
        let positive = rows
            .iter()
            .filter(|(r, _)| r.coefficients.get(&variable).copied().unwrap_or(0) > 0)
            .collect::<Vec<_>>();
        let negative = rows
            .iter()
            .filter(|(r, _)| r.coefficients.get(&variable).copied().unwrap_or(0) < 0)
            .collect::<Vec<_>>();
        if positive.len().checked_mul(negative.len())? > ROW_LIMIT {
            return None;
        }
        let mut next = rows
            .iter()
            .filter(|(r, _)| !r.coefficients.contains_key(&variable))
            .cloned()
            .collect::<Vec<_>>();
        let mut seen = next.iter().map(|(r, _)| r.clone()).collect::<BTreeSet<_>>();
        for (p, pc) in &positive {
            for (n, nc) in &negative {
                let a = n.coefficients[&variable].checked_neg()?;
                let b = p.coefficients[&variable];
                let row = p.scale(a)?.add(&n.scale(b)?)?;
                if row.coefficients.is_empty() && row.constant <= 0 {
                    continue;
                }
                if !seen.insert(row.clone()) {
                    continue;
                }
                let multipliers = pc
                    .multipliers
                    .iter()
                    .zip(&nc.multipliers)
                    .map(|(x, y)| x.checked_mul(a)?.checked_add(y.checked_mul(b)?))
                    .collect::<Option<Vec<_>>>()?;
                next.push((row, Certificate { multipliers }));
                if next.len() > ROW_LIMIT {
                    return None;
                }
            }
        }
        rows = next;
    }
    None
}

/// Reconstructs the obligation from kernel terms rather than from SMT text.
/// Unsupported or oversized goals remain explicitly trusted solver results.
pub(super) fn checked(
    goal: &Prop,
    ctx: &Context,
    math: bool,
    core: Option<&BTreeSet<String>>,
) -> bool {
    let result = || -> Option<bool> {
        let mut reader = Reader {
            ctx,
            math,
            fuel: 8192,
        };
        let (value, defined) = reader.prop(goal, 0)?;
        let mut conclusions = vec![value, defined];
        let mut assumptions = vec![];
        for (index, (_, prop)) in ctx.hyps.iter().enumerate() {
            let (value, defined) = reader.prop(prop, 0)?;
            conclusions.push(defined);
            if core.is_none_or(|names| names.contains(&format!("h{index}"))) {
                assumptions.push(value);
            }
        }
        for (_, term, ty) in &ctx.scope.terms {
            if matches!(ty, Type::Named(n,a) if a.is_empty() && matches!(n.as_str(), "Int" | "Nat" | "Bool"))
                && (matches!(
                    term,
                    Term::Var(_) | Term::Int(_) | Term::Nat(_) | Term::Bool(_)
                ) || matches!(term, Term::Ctor { ty, .. } if ty == "Nat"))
            {
                // A variable's domain, or an already constructed case witness.
                // Existence implies its arithmetic has succeeded.
                assumptions.push(reader.term(term, 0)?.defined);
            }
        }
        assumptions.push(Formula::and(conclusions).not());
        let branches = Formula::and(assumptions).branches(true, &mut 32768)?;
        for branch in branches {
            let certificate = search(&branch)?;
            if !check_certificate(&branch, &certificate) {
                return Some(false);
            }
        }
        Some(true)
    };
    result().unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn certificate_checker_rejects_forged_negative_and_overflowing_weights() {
        let x = Linear::variable("x");
        let rows = [
            x.clone(),
            x.scale(-1).unwrap().add(&Linear::number(1)).unwrap(),
        ];
        assert!(check_certificate(
            &rows,
            &Certificate {
                multipliers: vec![1, 1]
            }
        ));
        for multipliers in [vec![0, 0], vec![1, 0], vec![-1, -1], vec![1]] {
            assert!(
                !check_certificate(&rows, &Certificate { multipliers }),
                "{rows:?}"
            );
        }
        let false_rows = [x.clone(), x.scale(-1).unwrap()];
        assert!(!check_certificate(
            &false_rows,
            &Certificate {
                multipliers: vec![1, 1]
            }
        ));
        let large = [
            x.scale(2).unwrap(),
            x.scale(-2).unwrap().add(&Linear::number(1)).unwrap(),
        ];
        assert!(!check_certificate(
            &large,
            &Certificate {
                multipliers: vec![i128::MAX, i128::MAX]
            }
        ));
    }
    fn op(name: &str, a: Term, b: Term) -> Term {
        Term::Op(name.into(), vec![a, b])
    }
    fn context(name: &str, ty: &str) -> Context {
        Context {
            scope: Scope::default().with(name, Term::Var(name.into()), Type::named(ty)),
            ..Context::default()
        }
    }
    #[test]
    fn linear_proofs_preserve_natural_underflow_and_boolean_paths() {
        let ctx = context("n", "Nat");
        let n = Term::Var("n".into());
        let subtraction = op("-", n.clone(), Term::Nat(1));
        let goal = Prop::Eq(subtraction.clone(), subtraction.clone());
        assert!(!checked(&goal, &ctx, true, None));
        let mut bounded = ctx.clone();
        bounded.hyps.push((
            "positive".into(),
            Prop::Holds(op(">", n.clone(), Term::Nat(0))),
        ));
        assert!(checked(&goal, &bounded, true, None));
        let safe = Prop::Holds(op(
            "||",
            op("==", n.clone(), Term::Nat(0)),
            op(">=", subtraction, Term::Nat(0)),
        ));
        assert!(checked(&safe, &ctx, true, None));
        let ctx = context("b", "Bool");
        let b = Term::Var("b".into());
        let negative = Term::Op("!".into(), vec![b.clone()]);
        assert!(checked(
            &Prop::Holds(op("||", b.clone(), negative.clone())),
            &ctx,
            true,
            None
        ));
        assert!(!checked(
            &Prop::Holds(op("&&", b, negative)),
            &ctx,
            true,
            None
        ));
    }
    #[test]
    fn certificate_search_does_not_close_invalid_linear_implications() {
        let x = Term::Var("x".into());
        let y = Term::Var("y".into());
        for assumption in -3..=3 {
            for conclusion in -3..=3 {
                let mut ctx = context("x", "Int");
                ctx.scope = ctx.scope.with("y", y.clone(), Type::named("Int"));
                ctx.hyps.push((
                    "bound".into(),
                    Prop::Holds(op(
                        "<=",
                        x.clone(),
                        op("+", y.clone(), Term::Int(assumption)),
                    )),
                ));
                let goal = Prop::Holds(op(
                    "<=",
                    x.clone(),
                    op("+", y.clone(), Term::Int(conclusion)),
                ));
                assert_eq!(checked(&goal, &ctx, true, None), assumption <= conclusion);
            }
        }
    }
    #[test]
    fn elimination_produces_a_certificate_for_a_combination_of_three_hypotheses() {
        let x = Linear::variable("x");
        let y = Linear::variable("y");
        let rows = [
            x.difference(&y).unwrap(),
            y.clone(),
            x.scale(-1).unwrap().add(&Linear::number(1)).unwrap(),
        ];
        let certificate = search(&rows).unwrap();
        assert!(check_certificate(&rows, &certificate));
    }
}
