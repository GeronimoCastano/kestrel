use kestrel::{
    core,
    modules::{ProjectCheck, check_path, check_source},
    native::{self, TemporaryDirectory},
};
use std::{env, fs, process::Command};

const JOBS: &str = r#"
record Job(name : String)
class Jobs {
    private var items : Array[Job]
    new() { this.items = Array.empty() }
    public function size() : Int = this.items.length()
    public mut function add(job : Job) : proof this.size() > 0 {
        this.items.append(job)
        proof this.size() > 0
    }
    public mut function next(proof ready : this.size() > 0) : Job {
        let job := this.items[this.size() - 1]
        this.items.pop()
        job
    }
}
"#;
const DOG: &str = r#"
class Dog {
    private var shots : Int
    new() { this.shots = 0 }
    public function shotCount() : Int = this.shots
    public mut function vaccinate() : void { this.shots = this.shots + 1 }
    public mut function lapse() : void { this.shots = 0 }
}
function admit(d : Dog, proof safe : d.shotCount() >= 2) : Int = d.shotCount()
"#;

fn project(source: &str) -> ProjectCheck {
    ProjectCheck {
        modules: vec![check_source("state.kes", source)],
    }
}
fn good(source: &str) {
    let p = project(source);
    assert!(p.is_ok(), "{}\n{source}", p.render_diagnostics());
}
fn bad(source: &str, code: &str) {
    let p = project(source);
    assert!(
        p.modules
            .iter()
            .any(|m| m.checked.diagnostics.iter().any(|d| d.code == code)),
        "Expected {code}:\n{}\n{source}",
        p.render_diagnostics()
    );
}
fn run(source: &str) -> String {
    let p = project(source);
    assert!(p.is_ok(), "{}", p.render_diagnostics());
    let program = core::lower(&p).unwrap_or_else(|e| panic!("{e:#?}"));
    let temp = TemporaryDirectory::new(&env::temp_dir()).unwrap();
    let binary = temp.path().join("state-proof");
    native::build(&program, &binary).unwrap();
    let output = Command::new(binary).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn insertion_returns_evidence_and_runs_even_inside_an_erased_argument() {
    let source = format!(
        "{JOBS}\nlet jobs:=Jobs()\nlet ready:=jobs.add(Job(\"compile\"))\nlet saved:=ready\nprint(jobs.next(saved).name)\nprint(jobs.next(jobs.add(Job(\"test\"))).name)\nprint(jobs.size())"
    );
    assert_eq!(run(&source), "compile\ntest\n0\n");
}

#[test]
fn two_insertions_do_not_stack_evidence() {
    let source = format!(
        "{JOBS}\nlet jobs:=Jobs()\nlet ready1:=jobs.add(Job(\"one\"))\nlet ready2:=jobs.add(Job(\"two\"))\njobs.next(ready1)\njobs.next(ready2)"
    );
    bad(&source, "E_PROOF_ASSUMPTION");
}

#[test]
fn a_second_insertion_supports_a_direct_proof_and_removal() {
    let source = format!(
        "{JOBS}\nlet jobs:=Jobs()\nlet ready1:=jobs.add(Job(\"compile\"))\nprint(jobs.next(ready1).name)\njobs.add(Job(\"test\"))\nlet ready2:=proof jobs.size()>0\nprint(jobs.next(ready2).name)"
    );
    assert_eq!(run(&source), "compile\ntest\n");
}

#[test]
fn two_state_changes_establish_a_named_proof_without_a_branch() {
    let source = format!(
        "{DOG}\nlet rex:=Dog()\nrex.vaccinate()\nrex.vaccinate()\nlet safe:=proof rex.shotCount()>=2\nprint(admit(rex,safe))"
    );
    assert_eq!(run(&source), "2\n");
    bad(
        &source.replacen("rex.vaccinate()\n", "", 1),
        "E_PROOF_UNESTABLISHED",
    );
}

#[test]
fn old_evidence_stays_invalid_after_state_becomes_valid_again() {
    let source = format!(
        "{JOBS}\nlet jobs:=Jobs()\nlet ready:=jobs.add(Job(\"one\"))\nlet saved:=ready\njobs.next(ready)\njobs.add(Job(\"two\"))\njobs.next(saved)"
    );
    bad(&source, "E_PROOF_ASSUMPTION");
    bad(
        &format!(
            "{DOG}\nlet rex:=Dog()\nrex.vaccinate()\nrex.vaccinate()\nlet safe:=proof rex.shotCount()>=2\nrex.lapse()\nrex.vaccinate()\nrex.vaccinate()\nadmit(rex,safe)"
        ),
        "E_PROOF_ASSUMPTION",
    );
}

#[test]
fn witnesses_are_tied_to_their_object_but_unrelated_changes_are_allowed() {
    bad(
        &format!(
            "{JOBS}\nlet a:=Jobs()\nlet b:=Jobs()\nlet ready:=a.add(Job(\"one\"))\nb.next(ready)"
        ),
        "E_PROOF_ASSUMPTION",
    );
    good(&format!(
        "{JOBS}\nlet a:=Jobs()\nlet b:=Jobs()\nlet ready:=a.add(Job(\"one\"))\nb.add(Job(\"two\"))\na.next(ready)"
    ));
}

#[test]
fn false_producers_and_wrong_returned_claims_are_rejected() {
    bad(
        "function lie() : proof false = proof false",
        "E_PROOF_UNESTABLISHED",
    );
    bad(
        "function lie() : proof false { let h:=proof true\nreturn h }",
        "E_PROOF_RESULT",
    );
    bad(
        &JOBS.replace("this.items.append(job)", ""),
        "E_PROOF_UNESTABLISHED",
    );
    bad(
        &format!(
            "{DOG}\nfunction wrong(d : Dog, proof h : d.shotCount()>=1) : proof d.shotCount()>=2 = h"
        ),
        "E_PROOF_RESULT",
    );
}

#[test]
fn trait_methods_cannot_declare_proof_results() {
    bad(
        "trait Queue {\n    function size() : Int\n    function take() : proof this.size() > 0\n}",
        "E_PROOF_RESULT_UNSUPPORTED",
    );
}

#[test]
fn early_returns_do_not_certify_a_fallthrough_value() {
    bad(
        "function f(n : Int) : Int {\n    if n > 0 { return 1 }\n    2\n}\nlet v := f(5)\nlet h := proof v == 2",
        "E_PROOF_UNESTABLISHED",
    );
}

#[test]
fn straight_line_results_still_supply_their_value() {
    let source = "function plusOne(n : Int) : Int = n + 1\nlet v := plusOne(4)\nlet h := proof v == 5\nprint(v)";
    assert_eq!(run(source), "5\n");
}

#[test]
fn mutation_through_a_parameter_invalidates_evidence() {
    bad(
        &format!(
            "{DOG}\nfunction reset(mut d : Dog) : void = d.lapse()\nlet rex:=Dog()\nrex.vaccinate()\nrex.vaccinate()\nlet safe:=proof rex.shotCount()>=2\nreset(rex)\nadmit(rex,safe)"
        ),
        "E_PROOF_ASSUMPTION",
    );
}

#[test]
fn control_flow_does_not_leak_a_branch_or_an_optional_iteration() {
    let prefix = format!("{DOG}\nfunction demo(flag : Bool) : void {{\nlet rex:=Dog()\n");
    bad(
        &format!(
            "{prefix}if flag {{ rex.vaccinate()\nrex.vaccinate() }}\nlet safe:=proof rex.shotCount()>=2\n}}"
        ),
        "E_PROOF_UNESTABLISHED",
    );
    bad(
        &format!(
            "{prefix}while flag {{ rex.vaccinate()\nrex.vaccinate() }}\nlet safe:=proof rex.shotCount()>=2\n}}"
        ),
        "E_PROOF_UNESTABLISHED",
    );
    good(&format!(
        "{prefix}if flag {{ rex.vaccinate()\nrex.vaccinate() }} else {{ rex.vaccinate()\nrex.vaccinate() }}\nlet safe:=proof rex.shotCount()>=2\n}}"
    ));
    bad(
        &format!(
            "{prefix}rex.vaccinate()\nrex.vaccinate()\nlet safe:=proof rex.shotCount()>=2\nif flag {{ rex.lapse() }}\nlet ignored:=admit(rex,safe)\n}}"
        ),
        "E_PROOF_ASSUMPTION",
    );
}

#[test]
fn scalar_snapshots_and_shadowing_keep_their_original_meaning() {
    good(&format!(
        "{DOG}\nlet rex:=Dog()\nrex.vaccinate()\nlet old:=rex.shotCount()\nrex.lapse()\nlet h:=proof old==1"
    ));
    bad(
        "function positive(n : Int, proof h : n>0) : Int = n\nlet mut n:=1\nlet h:=proof n>0\nn=0\npositive(n,h)",
        "E_PROOF_ASSUMPTION",
    );
    good("let n:=1\nlet h:=proof n>0\n{ let n:=0\nlet h:=proof n==0 }\nlet same:=proof n==1");
}

#[test]
fn proof_claims_are_pure_and_do_not_run_effectful_code() {
    bad(
        "function noisy() : Bool with Effects { print(\"bad\")\ntrue }\nlet h:=proof noisy()",
        "E_EFFECT_REQUIRED",
    );
    good("function pass(n : Int, proof h : n>0) : proof n>0 = h");
    good("theorem same(n : Int) : n=n {rfl}\nfunction evidence(n : Int) : proof n==n = same(n)");
}

#[test]
fn append_then_replacement_does_not_keep_nonempty_facts() {
    let source = JOBS.replace(
        "this.items.append(job)\n        proof",
        "this.items.append(job)\n        this.items = Array.empty()\n        proof",
    );
    bad(&source, "E_PROOF_UNESTABLISHED");
}

#[test]
fn opaque_calls_and_captured_mutation_discard_state_evidence() {
    bad(
        &format!(
            "{DOG}\nfunction reset(mut d : Dog, flag : Bool) : void {{if flag {{d.lapse()}}}}\nlet rex:=Dog()\nrex.vaccinate()\nrex.vaccinate()\nlet safe:=proof rex.shotCount()>=2\nreset(rex,true)\nadmit(rex,safe)"
        ),
        "E_PROOF_ASSUMPTION",
    );
}

#[test]
fn scalar_arguments_keep_source_evaluation_order() {
    let source = format!(
        "{DOG}\nfunction first(a b : Int) : Int = a\nlet rex:=Dog()\nlet saved:=first(rex.shotCount(),{{rex.vaccinate()\nrex.shotCount()}})\nlet h:=proof saved==0"
    );
    good(&source);
    bad(
        &source.replace("saved==0", "saved==1"),
        "E_PROOF_UNESTABLISHED",
    );
}

#[test]
fn proof_result_claims_do_not_disappear_into_function_values() {
    bad(
        &format!("{JOBS}\nlet jobs:=Jobs()\nlet add:=jobs.add"),
        "E_PROOF_FUNCTION_VALUE",
    );
}

#[test]
fn proof_bindings_can_receive_fresh_evidence_after_mutation() {
    let source = format!(
        "{JOBS}\nlet jobs:=Jobs()\nlet mut ready:=jobs.add(Job(\"one\"))\nprint(jobs.next(ready).name)\nready=jobs.add(Job(\"two\"))\nprint(jobs.next(ready).name)"
    );
    assert_eq!(run(&source), "one\ntwo\n");
}

#[test]
fn rebinding_an_owner_does_not_detach_new_evidence_from_mutations() {
    bad(
        &format!(
            "{DOG}\nlet mut rex:=Dog()\nrex=Dog()\nrex.vaccinate()\nrex.vaccinate()\nlet safe:=proof rex.shotCount()>=2\nrex.lapse()\nadmit(rex,safe)"
        ),
        "E_PROOF_ASSUMPTION",
    );
}

#[test]
fn erased_observers_cannot_hide_failing_arguments_or_discarded_computations() {
    bad(
        "function ignore(n : Int) : Int = 1\nlet h:=proof ignore(1 / 0)>0",
        "E_PROOF_UNESTABLISHED",
    );
    bad(
        "function misleading() : Int { let unused:=1 / 0\n1 }\nlet h:=proof misleading()>0",
        "E_PROOF_UNESTABLISHED",
    );
}

#[test]
fn overrides_cannot_change_a_method_used_to_establish_evidence() {
    let source = format!(
        "{DOG}\nclass Fake extends Dog {{ new() {{ super() }}\n@override public function shotCount() : Int = 0 }}\nlet rex:=Fake()\nrex.vaccinate()\nrex.vaccinate()\nlet safe:=proof rex.shotCount()>=2"
    );
    let p = project(&source);
    assert!(
        !p.is_ok(),
        "An overridden observer must not establish false evidence"
    );
}

#[test]
fn verified_producer_summaries_cross_module_boundaries() {
    let temp = TemporaryDirectory::new(&env::temp_dir()).unwrap();
    fs::write(
        temp.path().join("jobs.kes"),
        JOBS.replace("class Jobs", "public class Jobs")
            .replace("record Job", "public record Job"),
    )
    .unwrap();
    let main = temp.path().join("main.kes");
    fs::write(&main,"import jobs.{Jobs, Job}\nlet jobs:=Jobs()\nlet ready:=jobs.add(Job(\"compile\"))\nprint(jobs.next(ready).name)").unwrap();
    let p = check_path(&main, temp.path()).unwrap();
    assert!(p.is_ok(), "{}", p.render_diagnostics());
    let program = core::lower(&p).unwrap_or_else(|e| panic!("{e:#?}"));
    native::build(&program, &temp.path().join("imported")).unwrap();
    let output = Command::new(temp.path().join("imported")).output().unwrap();
    assert_eq!(output.stdout, b"compile\n");
}
