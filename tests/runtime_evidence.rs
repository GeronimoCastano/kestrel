//! Direct theorem calls used as compile-time-only proof arguments.
use kestrel::{
    core,
    modules::{ProjectCheck, check_source},
    native::{self, TemporaryDirectory},
};
use std::{env, process::Command};

fn project(source: &str) -> ProjectCheck {
    ProjectCheck {
        modules: vec![check_source("runtime_evidence.kes", source)],
    }
}

fn refuses(source: &str, code: &str) {
    let project = project(source);
    assert!(
        project.modules.iter().any(|module| module
            .checked
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == code)),
        "Expected {code} for:\n{source}\n{}",
        project.render_diagnostics()
    );
}

#[test]
fn direct_theorem_evidence_is_type_checked_and_erased_from_native_calls() {
    let source = r#"
theorem reflexive(n : Int) : n = n {rfl}
function keep(n : Int, proof equal : n == n) : Int = n
print(keep(7, reflexive(7)))
class Width {
    public final value : Int
    new(value : Int) { this.value = value }
    public function width() : Int = this.value
}
theorem width(w : Width) : w.width() = w.value { rfl }
function accept(w : Width, proof h : w.width() == w.value) : Int = w.width()
let w := Width(3)
print(accept(w, width(w)))
print(w.width())
print(accept(Width(4), width(Width(4))))
"#;
    let project = project(source);
    assert!(project.is_ok(), "{}", project.render_diagnostics());
    let program = core::lower(&project).unwrap_or_else(|errors| panic!("{errors:#?}"));
    let temp = TemporaryDirectory::new(&env::temp_dir()).unwrap();
    let binary = temp.path().join("runtime-evidence");
    native::build(&program, &binary).unwrap();
    let output = Command::new(binary).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"7\n3\n3\n4\n");
}

#[test]
fn theorem_evidence_must_prove_the_required_claim() {
    refuses(
        "theorem reflexive(n : Int) : n = n {rfl}\nfunction nonnegative(n : Int, proof claim : n >= 0) : Int = n\nnonnegative(3, reflexive(3))",
        "E_PROOF_MISMATCH",
    );
}

#[test]
fn admitted_evidence_is_rejected_even_when_the_parameter_is_unused() {
    refuses(
        "theorem admitted(n : Int) : n = n {sorry}\nfunction ignore(n : Int, proof h : n == n) : Int = 0\nignore(3, admitted(3))",
        "E_PROOF_SORRY",
    );
}

#[test]
fn direct_theorem_evidence_rejects_named_and_partial_arguments() {
    refuses(
        "theorem reflexive(n : Int) : n = n {rfl}\nfunction keep(n : Int, proof equal : n == n) : Int = n\nkeep(3, reflexive(n = 3))",
        "E_PROOF_ARGUMENTS",
    );
    refuses(
        "theorem reflexive(n : Int) : n = n {rfl}\nfunction keep(n : Int, proof equal : n == n) : Int = n\nkeep(3, reflexive(_))",
        "E_PROOF_ARGUMENTS",
    );
}
