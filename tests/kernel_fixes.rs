//! Regression tests for the kernel items K0 through K6 and K8's `byCases`.
//! Each test has the reproducer next to the negative probe that must stay
//! unprovable.

use kestrel::modules::{check_path, check_source};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

fn good(source: &str) {
    let checked = check_source("kernel.kes", source);
    assert!(
        checked.checked.is_ok(),
        "{source}\n{:#?}",
        checked.checked.diagnostics
    );
}

fn refuses(source: &str, code: &str) {
    let checked = check_source("kernel.kes", source);
    assert!(
        checked.checked.diagnostics.iter().any(|d| d.code == code),
        "Expected {code}: {source}\n{:#?}",
        checked.checked.diagnostics
    );
}

struct Project(PathBuf);
static NEXT: AtomicU64 = AtomicU64::new(0);
impl Project {
    fn new(files: &[(&str, &str)]) -> Self {
        let root = std::env::temp_dir().join(format!(
            "kestrel-kernel-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        for (path, source) in files {
            let path = root.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, source).unwrap();
        }
        Self(root)
    }
    fn check(&self, file: &str) -> kestrel::modules::ProjectCheck {
        check_path(self.0.join(file), &self.0).unwrap()
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn clean(project: &Project, file: &str) {
    let result = project.check(file);
    assert!(result.is_ok(), "{}", result.render_diagnostics());
}

#[test]
fn k0_package_root_subfolder_sibling_and_tests_imports() {
    let project = Project::new(&[
        ("kestrel.project", "package mathlib\n"),
        (
            "root.kes",
            "public theorem rootThm(n : Nat) : n = n by { rfl }\n",
        ),
        (
            "sub/useRoot.kes",
            "import root.{rootThm}\npublic theorem useRoot(n : Nat) : n = n by { exact rootThm(n) }\n",
        ),
        (
            "sub/sib.kes",
            "public theorem sibThm(n : Nat) : n = n by { rfl }\n",
        ),
        (
            "sub/useSib.kes",
            "import sib.{sibThm}\npublic theorem useSib(n : Nat) : n = n by { exact sibThm(n) }\n",
        ),
        (
            "tests/check.kes",
            "import root.{rootThm}\npublic theorem check(n : Nat) : n = n by { exact rootThm(n) }\n",
        ),
    ]);
    clean(&project, "sub/useRoot.kes");
    clean(&project, "sub/useSib.kes");
    clean(&project, "tests/check.kes");
}

#[test]
fn k0_missing_module_reports_an_error_without_panicking() {
    let project = Project::new(&[
        ("kestrel.project", "package mathlib\n"),
        ("bad.kes", "import missing.{nope}\n"),
    ]);
    let result = project.check("bad.kes");
    assert!(!result.is_ok());
    assert!(
        result
            .modules
            .iter()
            .flat_map(|module| &module.checked.diagnostics)
            .any(|d| d.code == "E_MODULE_NOT_FOUND"),
        "{}",
        result.render_diagnostics()
    );
}

#[test]
fn k1_numerals_reduce_on_the_left_of_plus_and_times() {
    good("theorem oneAdd(k : Nat) : 1 + k = Succ(k) { rfl }");
    good("theorem twoAdd(k : Nat) : 2 + k = Succ(Succ(k)) { rfl }");
    good("theorem twoSucc(k : Nat) : 2 + k = Succ(Succ(k)) by { rfl }");
    good(
        "theorem addZeroR(n : Nat) : n + Zero = n by {\n\
            induction n { Zero => { rfl } Succ(k) ih => { simp [ih] } }\n\
         }\n\
         theorem oneMul(k : Nat) : 1 * k = k by {\n\
            have h : k + Zero = k := addZeroR(k)\n\
            rw [h]\n\
            rfl\n\
         }",
    );
}

#[test]
fn k1_a_huge_nat_numeral_does_not_hang() {
    // 2^64 does not fit a Nat; the checker reports it instead of expanding.
    refuses(
        "theorem big(k : Nat) : 0x10000000000000000 + k = k { rfl }",
        "E_PROOF_UNSUPPORTED",
    );
}

#[test]
fn k2_nested_induction_uses_the_inner_hypothesis() {
    good(
        "@simp\n\
         theorem addZeroR(n : Nat) : n + Zero = n by {\n\
             induction n { Zero => { rfl } Succ(k) ih => { simp [ih] } }\n\
         }\n\
         @simp\n\
         theorem addSuccR(n : Nat, m : Nat) : n + Succ(m) = Succ(n + m) by {\n\
             induction n { Zero => { rfl } Succ(k) ih => { simp [ih] } }\n\
         }\n\
         theorem nested(a : Nat, b : Nat) : a + b = b + a by {\n\
             induction a {\n\
                 Zero => { simp }\n\
                 Succ(k) ih => {\n\
                     induction b {\n\
                         Zero => { simp }\n\
                         Succ(j) ihb => { simp [addSuccR, ih, ihb] }\n\
                     }\n\
                 }\n\
             }\n\
         }",
    );
}

#[test]
fn k2_generalizing_quantifies_the_outer_hypothesis() {
    good(
        "@simp\n\
         theorem addZeroR(n : Nat) : n + Zero = n by {\n\
             induction n { Zero => { rfl } Succ(k) ih => { simp [ih] } }\n\
         }\n\
         @simp\n\
         theorem addSuccR(n : Nat, m : Nat) : n + Succ(m) = Succ(n + m) by {\n\
             induction n { Zero => { rfl } Succ(k) ih => { simp [ih] } }\n\
         }\n\
         theorem addCommG(a : Nat, b : Nat) : a + b = b + a by {\n\
             induction a generalizing b {\n\
                 Zero => { intro b; simp }\n\
                 Succ(k) ih => {\n\
                     intro b\n\
                     induction b {\n\
                         Zero => { simp }\n\
                         Succ(j) ihb => { have h := ih(j); simp [addSuccR, h] }\n\
                     }\n\
                 }\n\
             }\n\
         }",
    );
}

#[test]
fn k2_smaller_value_assumptions_are_still_required() {
    refuses(
        "theorem smaller(n : Nat) : n = Zero requires h : n = Zero by {\n\
             induction n { Zero => { rfl } Succ(k) ih => { exact ih } }\n\
         }",
        "E_PROOF_ASSUMPTION",
    );
}

#[test]
fn k3_constructor_equality_is_injective() {
    good(
        "theorem injSucc(a : Nat, b : Nat) : Succ(a) = Succ(b) -> a = b by {\n\
             intro h\n\
             cases h\n\
             rfl\n\
         }",
    );
    good(
        "theorem different(a : Nat) : Zero = Succ(a) -> a = a by {\n\
             intro h\n\
             cases h\n\
         }",
    );
}

#[test]
fn k4_congr_arg_over_a_function_parameter() {
    good(
        "theorem t[T](op : (T, T) -> T, inv : (T) -> T, a : T, b : T, c : T,\n\
                     h : op(a, b) = op(a, c)) :\n\
                 op(inv(a), op(a, b)) = op(inv(a), op(a, c)) by {\n\
             have both := congrArg(r => op(inv(a), r), h)\n\
             exact both\n\
         }",
    );
}

#[test]
fn k5_nat_order_is_definitional_on_constructors() {
    good("theorem leRefl(a : Nat) : a <= a { rfl }");
    good("theorem zeroLe(a : Nat) : 0 <= a { rfl }");
    good("theorem twoLe(k : Nat) : 2 <= Succ(Succ(k)) { rfl }");
    refuses("theorem wrong() : 1 <= 0 { rfl }", "E_PROOF_SHAPE");
    refuses(
        "theorem wrong(k : Nat) : 2 <= Succ(k) { rfl }",
        "E_PROOF_SHAPE",
    );
}

#[test]
fn k6_a_forall_hypothesis_is_a_rewrite_rule() {
    good(
        "theorem idLaw[T](op : (T, T) -> T, e : T, x : T,\n\
                        hIdL : forall a : T, op(e, a) = a) : op(e, x) = x by {\n\
             simp [hIdL]\n\
         }",
    );
    good(
        "theorem assocLaw[T](op : (T, T) -> T, x : T, y : T, z : T,\n\
                           hAssoc : forall a : T, forall b : T, forall c : T,\n\
                                    op(op(a, b), c) = op(a, op(b, c))) :\n\
                 op(op(x, y), z) = op(x, op(y, z)) by {\n\
             rw [hAssoc(x, y, z)]\n\
             rfl\n\
         }",
    );
}

#[test]
fn k8_by_cases_splits_a_bool_expression() {
    good(
        "theorem casesOf(b : Bool) : (if b { true } else { false }) = b by {\n\
             byCases h : b\n\
             { rw [h]; rw [h]; rfl }\n\
             { rw [h]; rw [h]; rfl }\n\
         }",
    );
}

#[test]
fn negative_probes_stay_unprovable() {
    refuses(
        "theorem subtraction() : 0 - 1 = 5 { rfl }",
        "E_PROOF_MISMATCH",
    );
    refuses(
        "theorem impossible(k : Nat) : Zero = Succ(k) { rfl }",
        "E_PROOF_MISMATCH",
    );
    refuses(
        "theorem wrong(a : Nat, b : Nat) : a + b = b { rfl }",
        "E_PROOF_MISMATCH",
    );
    refuses(
        "theorem wrong(k : Nat) : 1 + k = Zero { rfl }",
        "E_PROOF_MISMATCH",
    );
}

#[test]
fn k0_tests_folder_file_sees_the_package_root() {
    let project = Project::new(&[
        ("kestrel.project", "package pkg\n"),
        (
            "nat.kes",
            "public theorem addZeroR(n : Nat) : n + Zero = n by {\n\
                 induction n { Zero => { rfl } Succ(k) ih => { simp [ih] } }\n\
             }\n",
        ),
        (
            "tests/nat.kes",
            "import nat.{addZeroR}\n\
             public theorem check(n : Nat) : n + Zero = n by { exact addZeroR(n) }\n",
        ),
    ]);
    clean(&project, "tests/nat.kes");
}

#[test]
fn k7_named_predicates_unfold_in_statements_and_proofs() {
    good(
        "function positive(n : Nat) : Prop = n = n\n\
          theorem pos(n : Nat) : positive(n) by { rfl }\n",
    );
    good(
        "function someNat(n : Nat) : Prop = exists k : Nat, n = k\n\
          theorem some(n : Nat) : someNat(n) by { exists n; rfl }\n",
    );
    good(
        "function positive(n : Nat) : Prop = n = n\n\
          function twice(n : Nat) : Prop = positive(n) and positive(n)\n\
          theorem twiceHolds(n : Nat) : twice(n) by { constructor; { rfl }; { rfl } }\n",
    );
    refuses("function bad(d : Nat) : Prop = exists k : Nat, k", "E_TYPE");
}

#[test]
fn k8_strong_induction_uses_the_hypothesis_at_smaller_values() {
    good(
        "theorem addZeroStrong(n : Nat) : n + 0 = n by {\n\
              strongInduction n {\n\
                  Zero => { rfl }\n\
                  Succ(k) ih => {\n\
                      have hk := ih(k, rfl)\n\
                      rw [hk]\n\
                      rfl\n\
                  }\n\
              }\n\
          }",
    );
    refuses(
        "theorem bad(b : Bool) : b = b by { strongInduction b { true => { rfl } false => { rfl } } }",
        "E_PROOF_SHAPE",
    );
}

#[test]
fn k9_projects_conjunctions_and_chains_calc() {
    good(
        "theorem proj(a : Nat, b : Nat, h : a = b and b = a) : a = a by {\
              cases h.left\n\
              rfl\n\
          }",
    );
    good(
        "theorem chain(a : Nat, b : Nat, c : Nat, h1 : a = b, h2 : b = c) : a = c by {\n\
              calc a = b := h1\n\
                   _ = c := h2\n\
          }",
    );
    refuses(
        "theorem broken(a : Nat, b : Nat, c : Nat, h : b = c) : a = c by {\n\
             calc a = b := h\n\
                  _ = c := h\n\
         }",
        "E_PROOF_MISMATCH",
    );
    refuses(
        "theorem badProjection(a : Nat, h : a = a) : a = a by { cases h.left; rfl }",
        "E_PROOF_SHAPE",
    );
}

#[test]
fn mathlib_checks_and_uses_no_trusted_construct() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("mathlib");
    let mut checked = 0;
    for entry in fs::read_dir(&root).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("kes") {
            continue;
        }
        let text = fs::read_to_string(&path).unwrap();
        for line in text.lines() {
            let code = line.split("//").next().unwrap_or("");
            for banned in ["sorry", "axiom", "opaque", "smt"] {
                assert!(
                    !code
                        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                        .any(|word| word == banned),
                    "{} uses `{banned}`: {line}",
                    path.display()
                );
            }
        }
        let result = check_path(&path, &root).unwrap();
        assert!(result.is_ok(), "{}", result.render_diagnostics());
        checked += 1;
    }
    assert_eq!(checked, 11, "the library has eleven checked files");
}
