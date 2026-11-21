//! Optional arithmetic solver with independently checked linear certificates.
use super::*;
use std::{
    io::Read,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const TIMEOUT_MS: u64 = 2000;
const OUTPUT_LIMIT: u64 = 65_536;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Sort {
    Int,
    Nat,
    Bool,
}
impl Sort {
    /// The values a term may take. A proof uses the integers and the naturals; only
    /// facts for run-time code keep the 64-bit limits (REVIEW 2.8).
    fn bounds(self, term: &str, math: bool) -> String {
        if !math {
            return self.range(term);
        }
        match self {
            Self::Int | Self::Bool => "true".into(),
            Self::Nat => format!("(<= 0 {term})"),
        }
    }
    fn range(self, term: &str) -> String {
        match self {
            Self::Int => {
                format!("(and (<= (- 9223372036854775808) {term}) (<= {term} 9223372036854775807))")
            }
            Self::Nat => format!("(and (<= 0 {term}) (<= {term} 18446744073709551615))"),
            Self::Bool => "true".into(),
        }
    }
}
struct Expr {
    text: String,
    sort: Sort,
    checks: Vec<String>,
}
fn and(parts: &[String]) -> String {
    match parts {
        [] => "true".into(),
        [one] => one.clone(),
        _ => format!("(and {})", parts.join(" ")),
    }
}
fn number(n: i128) -> String {
    if n < 0 {
        format!("(- {})", -n)
    } else {
        n.to_string()
    }
}

struct Encoder<'a> {
    ctx: &'a Context,
    math: bool,
    vars: BTreeMap<String, (String, Sort)>,
    fuel: usize,
    span: Span,
    functions: &'a BTreeMap<DeclarationId, Definition>,
    classes: &'a BTreeMap<String, crate::checker::Definition>,
    methods: &'a BTreeMap<DeclarationId, MethodInfo>,
}
impl Encoder<'_> {
    fn unsupported<T>(&self) -> Found<T> {
        fail(
            self.span,
            "E_PROOF_SMT_UNSUPPORTED",
            "smt accepts arithmetic, Bool logic, if expressions and readable class method bodies; it cannot encode this term.",
            "Use explicit proofs or case splitting for unsupported function calls and enum expressions.",
        )
    }

    fn inline_class_call(
        &self,
        function: &DeclarationId,
        args: &[Term],
        dispatch: Option<&Type>,
    ) -> Option<Term> {
        let method = self.methods.get(function);
        let concrete_dispatch = dispatch
            .and_then(|ty| match ty {
                Type::Named(id, _) => self.classes.get(id).filter(|d| d.kind == K::Class),
                _ => None,
            })
            .is_some();
        let constructor = self.classes.values().any(|class| {
            class.constructors.iter().any(|constructor| {
                matches!(
                    &constructor.callable,
                    Some(Callable::Declaration {
                        id,
                        kind: DeclarationKind::Constructor,
                        ..
                    }) if id == function
                )
            })
        });
        if !constructor && !(method.is_some() && (dispatch.is_none() || concrete_dispatch)) {
            return None;
        }
        let definition = self.functions.get(function)?;
        if !definition.requires.is_empty() || !definition.preconditions.is_empty() {
            return None;
        }
        let substitutions = definition
            .params
            .iter()
            .cloned()
            .zip(args.iter().cloned())
            .collect();
        let mut body = definition.body.subst(&substitutions, &mut 0);
        if let Some(receiver) = dispatch
            && trait_method_default(function, self.classes)
            && concrete_dispatch
        {
            body =
                specialize_trait_calls(&body, receiver, args.first(), self.functions, self.classes);
        }
        Some(body)
    }

    /// Returns only the checks required to evaluate a value. Class values have
    /// no SMT sort, but their constructor and method bodies still carry useful
    /// definedness checks when they appear in an eagerly evaluated sequence.
    fn checked(&mut self, term: &Term, depth: usize) -> Found<Vec<String>> {
        if depth >= 96 || self.fuel == 0 {
            return fail(
                self.span,
                "E_PROOF_LIMIT",
                "The arithmetic query exceeds the size or nesting limit.",
                "Split the proof into smaller lemmas.",
            );
        }
        self.fuel -= 1;
        match term {
            Term::Op(op, args) if op == "seq" && args.len() == 2 => {
                let mut checks = self.checked(&args[0], depth + 1)?;
                checks.extend(self.checked(&args[1], depth + 1)?);
                Ok(checks)
            }
            Term::Ctor { ty, args, .. }
                if self
                    .classes
                    .get(ty)
                    .is_some_and(|class| class.kind == K::Class) =>
            {
                let mut checks = vec![];
                for arg in args {
                    checks.extend(self.checked(arg, depth + 1)?);
                }
                Ok(checks)
            }
            Term::Call {
                function,
                args,
                dispatch,
                ..
            } => {
                let Some(body) = self.inline_class_call(function, args, dispatch.as_deref()) else {
                    return self.unsupported();
                };
                self.checked(&body, depth + 1)
            }
            Term::If(condition, yes, no) => {
                let condition = self.term(condition, depth + 1)?;
                if condition.sort != Sort::Bool {
                    return self.unsupported();
                }
                let yes_checks = self.checked(yes, depth + 1)?;
                let no_checks = self.checked(no, depth + 1)?;
                let mut checks = condition.checks;
                checks.push(format!("(=> {} {})", condition.text, and(&yes_checks)));
                checks.push(format!("(=> (not {}) {})", condition.text, and(&no_checks)));
                Ok(checks)
            }
            Term::Op(op, args) if op.starts_with("$field:") && args.len() == 1 => {
                let projected = self.projected_field(term, op, &args[0])?;
                let value = self.term(&projected, depth + 1)?;
                let mut checks = self.checked(&args[0], depth + 1)?;
                checks.extend(value.checks);
                Ok(checks)
            }
            _ => Ok(self.term(term, depth + 1)?.checks),
        }
    }

    fn projected_field(&self, original: &Term, op: &str, base: &Term) -> Found<Term> {
        let Some((_, path)) = field_marker_parts(op) else {
            return self.unsupported();
        };
        // The kernel registers these names and types for readable theorem
        // parameters. Reuse them so repeated reads of one field share one SMT
        // variable and distinct receiver parameters remain independent.
        if let Term::Var(name) = base {
            let field = format!("{name}.{path}");
            if self
                .ctx
                .scope
                .terms
                .iter()
                .any(|(found, _, _)| found == &field)
            {
                return Ok(Term::Var(field));
            }
        }
        let mut reducer = Reducer {
            functions: self.functions,
            definitions: self.classes,
            fuel: 4096,
            fresh: 0,
        };
        let reduced = reducer.whnf(original);
        if &reduced != original {
            return Ok(reduced);
        }
        self.unsupported()
    }

    fn term(&mut self, t: &Term, depth: usize) -> Found<Expr> {
        if self.fuel == 0 || depth >= 96 {
            return fail(
                self.span,
                "E_PROOF_LIMIT",
                "The arithmetic query exceeds the size or nesting limit.",
                "Split the proof into smaller lemmas.",
            );
        }
        self.fuel -= 1;
        let value = |text, sort| Expr {
            text,
            sort,
            checks: vec![],
        };
        match t {
            Term::Var(name) => {
                if let Some((text, sort)) = self.vars.get(name) {
                    return Ok(value(text.clone(), *sort));
                }
                let Some((_, _, ty)) = self
                    .ctx
                    .scope
                    .terms
                    .iter()
                    .rev()
                    .find(|(_, t, _)| matches!(t,Term::Var(n) if n==name))
                else {
                    return self.unsupported();
                };
                let sort = match ty {
                    Type::Named(n, a) if a.is_empty() => match n.as_str() {
                        "Int" => Sort::Int,
                        "Nat" => Sort::Nat,
                        "Bool" => Sort::Bool,
                        _ => return self.unsupported(),
                    },
                    _ => return self.unsupported(),
                };
                let text = format!("v{}", self.vars.len());
                self.vars.insert(name.clone(), (text.clone(), sort));
                Ok(value(text, sort))
            }
            Term::Int(n) => Ok(value(number(i128::from(*n)), Sort::Int)),
            Term::Nat(n) => Ok(value(n.to_string(), Sort::Nat)),
            Term::Bool(b) => Ok(value(b.to_string(), Sort::Bool)),
            Term::Ctor { ty, case, args } if ty == "Nat" && case == "Succ" && args.len() == 1 => {
                let mut a = self.term(&args[0], depth + 1)?;
                if a.sort != Sort::Nat {
                    return self.unsupported();
                }
                a.text = format!("(+ {} 1)", a.text);
                a.checks.push(Sort::Nat.bounds(&a.text, self.math));
                Ok(a)
            }
            Term::Op(op, args) if args.len() == 1 => {
                if op.starts_with("$field:") {
                    let projected = self.projected_field(t, op, &args[0])?;
                    let mut expr = self.term(&projected, depth + 1)?;
                    // Projecting from a constructor or method evaluates the
                    // entire receiver. Preserve checks for discarded fields.
                    let checks = self.checked(&args[0], depth + 1)?;
                    expr.checks.extend(checks);
                    return Ok(expr);
                }
                let mut a = self.term(&args[0], depth + 1)?;
                match (op.as_str(), a.sort) {
                    ("!", Sort::Bool) => a.text = format!("(not {})", a.text),
                    ("-", Sort::Int) => {
                        a.text = format!("(- {})", a.text);
                        a.checks.push(Sort::Int.bounds(&a.text, self.math));
                    }
                    _ => return self.unsupported(),
                }
                Ok(a)
            }
            Term::Op(op, args) if op == "seq" && args.len() == 2 => {
                let mut checks = self.checked(&args[0], depth + 1)?;
                let mut result = self.term(&args[1], depth + 1)?;
                checks.append(&mut result.checks);
                result.checks = checks;
                Ok(result)
            }
            Term::Op(op, args) if args.len() == 2 => self.binary(op, &args[0], &args[1], depth + 1),
            Term::If(c, a, b) => {
                let c = self.term(c, depth + 1)?;
                let a = self.term(a, depth + 1)?;
                let b = self.term(b, depth + 1)?;
                if c.sort != Sort::Bool || a.sort != b.sort {
                    return self.unsupported();
                }
                let mut checks = c.checks;
                checks.push(format!("(=> {} {})", c.text, and(&a.checks)));
                checks.push(format!("(=> (not {}) {})", c.text, and(&b.checks)));
                Ok(Expr {
                    text: format!("(ite {} {} {})", c.text, a.text, b.text),
                    sort: a.sort,
                    checks,
                })
            }
            Term::Call {
                function,
                args,
                dispatch,
                ..
            } => {
                let Some(body) = self.inline_class_call(function, args, dispatch.as_deref()) else {
                    return self.unsupported();
                };
                self.term(&body, depth + 1)
            }
            _ => self.unsupported(),
        }
    }
    fn binary(&mut self, op: &str, a: &Term, b: &Term, depth: usize) -> Found<Expr> {
        // `x ** k` with a small literal k is k multiplications.
        if op == "**"
            && let Term::Int(power) = b
            && (0..=8).contains(power)
        {
            let base = self.term(a, depth)?;
            if base.sort != Sort::Int {
                return self.unsupported();
            }
            let text = if *power == 0 {
                "1".to_owned()
            } else {
                format!("(* {})", vec![base.text.clone(); *power as usize].join(" "))
            };
            let mut checks = base.checks;
            checks.push(Sort::Int.bounds(&text, self.math));
            return Ok(Expr {
                text,
                sort: Sort::Int,
                checks,
            });
        }
        let mut a = self.term(a, depth)?;
        let b = self.term(b, depth)?;
        if a.sort != b.sort {
            return self.unsupported();
        }
        // Division needs a nonzero divisor. Int `/` rounds toward zero and `%` takes the sign of
        // the dividend, as at run time; SMT-LIB's div and mod round down, so negative operands
        // are mirrored. Nat operands are never negative.
        if matches!(op, "/" | "%") && a.sort != Sort::Bool {
            let (x, y) = (a.text.clone(), b.text.clone());
            let quotient = if a.sort == Sort::Nat {
                format!("(div {x} {y})")
            } else {
                format!(
                    "(ite (>= {x} 0) (ite (> {y} 0) (div {x} {y}) (- (div {x} (- {y})))) (ite (> {y} 0) (- (div (- {x}) {y})) (div (- {x}) (- {y}))))"
                )
            };
            let text = if op == "/" {
                quotient
            } else {
                format!("(- {x} (* {y} {quotient}))")
            };
            a.checks.extend(b.checks);
            a.checks.push(format!("(not (= {y} 0))"));
            a.checks.push(a.sort.bounds(&text, self.math));
            return Ok(Expr {
                text,
                sort: a.sort,
                checks: a.checks,
            });
        }
        let operator = match op {
            "+" | "-" if a.sort != Sort::Bool => op,
            "*" if a.sort != Sort::Bool => op,
            "==" | "!=" => "=",
            "<" | "<=" | ">" | ">=" if a.sort != Sort::Bool => op,
            "&&" if a.sort == Sort::Bool => "and",
            "||" if a.sort == Sort::Bool => "or",
            _ => return self.unsupported(),
        };
        if matches!(op, "&&" | "||") {
            let condition = if op == "&&" {
                a.text.clone()
            } else {
                format!("(not {})", a.text)
            };
            a.checks
                .push(format!("(=> {condition} {})", and(&b.checks)));
        } else {
            a.checks.extend(b.checks);
        }
        let text = format!("({operator} {} {})", a.text, b.text);
        let arithmetic = matches!(op, "+" | "-" | "*");
        if arithmetic {
            a.checks.push(a.sort.bounds(&text, self.math));
        }
        Ok(Expr {
            text: if op == "!=" {
                format!("(not {text})")
            } else {
                text
            },
            sort: if arithmetic { a.sort } else { Sort::Bool },
            checks: a.checks,
        })
    }
    fn prop(&mut self, p: &Prop, depth: usize) -> Found<Expr> {
        if depth >= 96 {
            return fail(
                self.span,
                "E_PROOF_LIMIT",
                "The arithmetic proposition is nested too deeply.",
                "Split it into lemmas.",
            );
        }
        match p {
            Prop::Eq(a, b) => self.binary("==", a, b, depth + 1),
            Prop::Holds(t) => {
                let p = self.term(t, depth + 1)?;
                if p.sort != Sort::Bool {
                    return self.unsupported();
                }
                Ok(p)
            }
            Prop::False => Ok(Expr {
                text: "false".into(),
                sort: Sort::Bool,
                checks: vec![],
            }),
            Prop::Atom(_) => self.unsupported(),
            Prop::Not(p) => {
                let mut p = self.prop(p, depth + 1)?;
                p.text = format!("(not {})", p.text);
                Ok(p)
            }
            Prop::Mem(..)
            | Prop::Subset { .. }
            | Prop::Implies(..)
            | Prop::Iff(..)
            | Prop::Forall { .. }
            | Prop::Exists { .. }
            | Prop::True => self.unsupported(),
            Prop::And(a, b) | Prop::Or(a, b) => {
                let mut a = self.prop(a, depth + 1)?;
                let b = self.prop(b, depth + 1)?;
                a.checks.extend(b.checks);
                Ok(Expr {
                    text: format!(
                        "({} {} {})",
                        if matches!(p, Prop::And(..)) {
                            "and"
                        } else {
                            "or"
                        },
                        a.text,
                        b.text
                    ),
                    sort: Sort::Bool,
                    checks: a.checks,
                })
            }
        }
    }
}

impl Kernel<'_> {
    pub(super) fn solve_arithmetic(
        &mut self,
        node: &Node,
        goal: &Prop,
        ctx: &Context,
    ) -> Found<()> {
        // Preserve evaluated arithmetic. Unfolding can discard an unused let
        // whose checked operation would trap; this backend does not establish
        // definedness of function bodies.
        let kernel_goal = goal;
        let hyps = ctx.hyps.iter().map(|(_, p)| p.clone()).collect::<Vec<_>>();
        let math = !self.runtime_bounds.get();
        let mut encoder = Encoder {
            ctx,
            math,
            vars: BTreeMap::new(),
            fuel: 4096,
            span: node.span,
            functions: &self.functions,
            classes: &self.checked.environment.definitions,
            methods: &self.method_infos,
        };
        let goal = encoder.prop(goal, 0)?;
        let mut defined = goal.checks;
        let mut assumptions = vec![];
        // A case split already supplies a machine-representable constructor.
        // In the Succ(k) arm, k cannot be UINT64_MAX.
        for (_, term, ty) in &ctx.scope.terms {
            if !matches!(term, Term::Var(_))
                && matches!(ty,Type::Named(n,_) if matches!(n.as_str(),"Nat"|"Int"|"Bool"))
            {
                let witness = encoder.term(term, 0)?;
                assumptions.extend(witness.checks);
                assumptions.push(witness.sort.bounds(&witness.text, math));
            }
        }
        let witness_count = assumptions.len();
        for hyp in hyps {
            let hyp = encoder.prop(&hyp, 0)?;
            defined.extend(hyp.checks);
            assumptions.push(hyp.text);
        }
        let mut query = format!(
            "(set-logic QF_NIA)\n(set-option :produce-unsat-cores true)\n(set-option :timeout {TIMEOUT_MS})\n"
        );
        for (name, sort) in encoder.vars.values() {
            query.push_str(&format!(
                "(declare-const {name} {})\n(assert (! {} :named bound_{name}))\n",
                if *sort == Sort::Bool { "Bool" } else { "Int" },
                sort.bounds(name, math)
            ));
        }
        for (index, assumption) in assumptions.iter().enumerate() {
            let label = if index < witness_count {
                format!("case{index}")
            } else {
                format!("h{}", index - witness_count)
            };
            query.push_str(&format!("(assert (! {assumption} :named {label}))\n"));
        }
        query.push_str(&format!(
            "(assert (! (not (and {} {})) :named goal))\n(check-sat)\n",
            and(&defined),
            goal.text
        ));
        let executable = std::env::var_os("KESTREL_Z3")
            .or_else(|| crate::paths::bundled("z3").map(Into::into))
            .unwrap_or_else(|| "z3".into());
        match solver(
            Path::new(&executable),
            &query,
            Duration::from_millis(TIMEOUT_MS + 1000),
        ) {
            Ok(true) => {
                let core = unsat_core(Path::new(&executable), &query);
                if !linear::checked(kernel_goal, ctx, math, core.as_ref()) {
                    self.smt_trusted.set(true);
                }
                Ok(())
            }
            Ok(false) if math => fail(
                node.span,
                "E_PROOF_SMT_FAILED",
                "smt found a counterexample, a Nat subtraction that may go below zero, or a division that may divide by zero.",
                "Add the missing assumption or use an explicit proof.",
            ),
            Ok(false) => fail(
                node.span,
                "E_PROOF_SMT_FAILED",
                "smt found a counterexample or an arithmetic operation that may overflow or go below zero.",
                "Add the required bounds or use an explicit proof; arithmetic must remain defined on this path.",
            ),
            Err(message) => fail(
                node.span,
                "E_PROOF_SOLVER",
                message,
                "Install Z3 or set KESTREL_Z3 to its executable; use an explicit proof when the solver cannot decide.",
            ),
        }
    }
}

fn solver(executable: &Path, query: &str, timeout: Duration) -> Result<bool, String> {
    match solver_output(executable, query, timeout)?.trim() {
        "unsat" => Ok(true),
        "sat" => Ok(false),
        "unknown" => Err("Z3 could not decide this arithmetic query.".into()),
        _ => Err("Z3 returned an unexpected response.".into()),
    }
}

fn unsat_core(executable: &Path, query: &str) -> Option<BTreeSet<String>> {
    let query = format!("{query}(get-unsat-core)\n");
    let output =
        solver_output(executable, &query, Duration::from_millis(TIMEOUT_MS + 1000)).ok()?;
    let rest = output.trim().strip_prefix("unsat")?.trim();
    let names = rest.strip_prefix('(')?.strip_suffix(')')?;
    let mut core = BTreeSet::new();
    for name in names.split_whitespace() {
        if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return None;
        }
        core.insert(name.to_owned());
    }
    Some(core)
}

fn solver_output(executable: &Path, query: &str, timeout: Duration) -> Result<String, String> {
    let dir = crate::native::TemporaryDirectory::new(&std::env::temp_dir())
        .map_err(|e| format!("Cannot create the solver's temporary folder: {e}"))?;
    let input = dir.path().join("query.smt2");
    std::fs::write(&input, query).map_err(|e| format!("Cannot write the solver query: {e}"))?;
    let out = dir.path().join("stdout");
    let err = dir.path().join("stderr");
    let output = std::fs::File::create(&out).map_err(|e| e.to_string())?;
    let errors = std::fs::File::create(&err).map_err(|e| e.to_string())?;
    let mut child = Command::new(executable)
        .args(["-smt2", "-T:2"])
        .arg(input)
        .stdin(Stdio::null())
        .stdout(output)
        .stderr(errors)
        .spawn()
        .map_err(|e| format!("Cannot start Z3: {e}"))?;
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if start.elapsed() < timeout => std::thread::sleep(Duration::from_millis(10)),
            result => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(match result {
                    Err(e) => format!("Cannot wait for Z3: {e}"),
                    _ => "Z3 exceeded the solver time limit.".into(),
                });
            }
        }
    };
    fn read(path: &Path) -> Result<String, String> {
        let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        let mut bytes = vec![];
        file.take(OUTPUT_LIMIT + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() as u64 > OUTPUT_LIMIT {
            return Err("Z3 produced too much output.".into());
        }
        String::from_utf8(bytes).map_err(|_| "Z3 produced invalid text.".into())
    }
    let output = read(&out)?;
    let errors = read(&err)?;
    if !status.success() || !errors.trim().is_empty() {
        return Err("Z3 failed while checking this query.".into());
    }
    Ok(output)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    fn executable(dir: &crate::native::TemporaryDirectory, body: &str) -> std::path::PathBuf {
        let path = dir.path().join("solver");
        std::fs::write(&path, format!("#!/usr/bin/env python3\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        path
    }
    #[test]
    fn solver_accepts_only_one_exact_successful_answer() {
        let dir = crate::native::TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
        for (body, expected) in [
            ("print('unsat')", Some(true)),
            ("print('sat')", Some(false)),
            ("print('unknown')", None),
            ("print('unsat\\nsat')", None),
            ("import sys;print('unsat');sys.exit(1)", None),
            (
                "import sys;print('unsat');print('error',file=sys.stderr)",
                None,
            ),
        ] {
            let path = executable(&dir, body);
            assert_eq!(
                solver(&path, "(check-sat)", Duration::from_secs(1)).ok(),
                expected
            );
        }
        assert!(solver(&dir.path().join("missing"), "", Duration::from_millis(25)).is_err());
    }
    #[test]
    fn solver_timeout_kills_and_reaps_the_process() {
        let dir = crate::native::TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
        let path = executable(&dir, "import time;time.sleep(10)");
        let start = Instant::now();
        assert!(solver(&path, "", Duration::from_millis(25)).is_err());
        assert!(start.elapsed() < Duration::from_secs(2));
    }
}
