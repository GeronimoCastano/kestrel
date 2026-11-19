use kestrel::{checker::check, parse_file, proof::trusted_theorems};
use std::process::Command;

fn has_z3() -> bool {
    Command::new("z3").arg("--version").output().is_ok()
}

fn trusted(source: &str) -> Vec<String> {
    let parsed = parse_file("certificates.kes", source);
    let checked = check(&parsed);
    assert!(checked.is_ok(), "{:?}", checked.diagnostics);
    trusted_theorems(&parsed, &checked)
        .into_iter()
        .map(|t| t.name)
        .collect()
}

#[test]
fn example_arithmetic_and_linear_combinations_are_checked() {
    if !has_z3() {
        return;
    }
    assert!(trusted(include_str!("../testdata/language/arithmetic.kes")).is_empty());
    assert!(trusted("theorem combine(x : Int,y : Int) : x < 3 requires h0 : x <= y requires h1 : y <= 2 {smt}\ntheorem bools(a : Bool,b : Bool) : (a && b) = (b && a) {smt}\ntheorem multiples(x : Int) : (2 * x) + 1 > 2 * x {smt}").is_empty());
}

#[test]
fn nonlinear_results_and_their_citations_remain_visible() {
    if !has_z3() {
        return;
    }
    assert_eq!(
        trusted(
            "theorem square(n : Int) : n * n >= 0 {smt}\ntheorem cited(n : Int) : n * n >= 0 { square(n) }\ntheorem applied(n : Int) : n * n >= 0 by { apply square }"
        ),
        ["square", "cited", "applied"]
    );
    assert_eq!(
        trusted(
            "@simp theorem square(n : Int) : n * n = n * n {smt}\ntheorem cited(n : Int) : n * n = n * n {simp [square]}"
        ),
        ["square", "cited"]
    );
}

#[test]
fn axioms_and_admitted_dependencies_are_listed_without_a_solver() {
    assert_eq!(
        trusted(
            "axiom assumed(n : Int) : n = 0\ntheorem missing(n : Int) : n = 0 {sorry}\ntheorem cited(n : Int) : n = 0 {assumed(n)}"
        ),
        ["assumed", "missing", "cited"]
    );
}

#[test]
fn trusted_cli_reports_theorem_locations_and_omits_checked_goals() {
    if !has_z3() {
        return;
    }
    let dir = kestrel::native::TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    let path = dir.path().join("proofs.kes");
    std::fs::write(&path, "theorem linear(n : Int) : n + 1 > n {smt}\ntheorem square(n : Int) : n * n >= 0 {smt}\naxiom assumed() : 1 = 2\ntheorem missing() : 1 = 2 {sorry}\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_kestrel"))
        .args(["check", "--trusted", "--root"])
        .arg(dir.path())
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("proofs.kes:2:1: trusted square: smt"),
        "{stdout}"
    );
    assert!(
        stdout.contains("proofs.kes:3:1: trusted assumed: axiom"),
        "{stdout}"
    );
    assert!(
        stdout.contains("proofs.kes:4:1: trusted missing: sorry"),
        "{stdout}"
    );
    assert!(!stdout.contains("trusted linear"), "{stdout}");
}

#[cfg(unix)]
#[test]
fn a_bad_solver_answer_cannot_be_marked_certificate_checked() {
    use std::os::unix::fs::PermissionsExt;
    let dir = kestrel::native::TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    let solver = dir.path().join("solver");
    std::fs::write(&solver, "#!/bin/sh\nprintf 'unsat\\n'\n").unwrap();
    std::fs::set_permissions(&solver, std::fs::Permissions::from_mode(0o700)).unwrap();
    let proof = dir.path().join("false.kes");
    std::fs::write(&proof, "theorem falseClaim(n : Int) : n > 0 {smt}").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_kestrel"))
        .args(["check", "--trusted", "--root"])
        .arg(dir.path())
        .arg(&proof)
        .env("KESTREL_Z3", solver)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("trusted falseClaim: smt"));
}

#[test]
fn unnamed_admitted_examples_are_audited_too() {
    assert_eq!(trusted("example() : 1 = 2 {sorry}"), ["example"]);
}

#[test]
fn unused_simp_rules_do_not_add_solver_trust() {
    if !has_z3() {
        return;
    }
    assert_eq!(
        trusted(
            "@simp theorem nonlinear(n : Int) : n * n = n * n {smt}\ntheorem trivial() : 1 = 1 {simp}\ntheorem named() : 2 = 2 {simp [nonlinear]}"
        ),
        ["nonlinear"]
    );
}
