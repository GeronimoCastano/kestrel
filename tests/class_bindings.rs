//! Class theorem arguments keep their fields attached to the supplied value.
use kestrel::{checker::check, parse_file};
const SQUARE: &str = "record Square(side:Int)\n";
fn good(source: &str) {
    let parsed = parse_file("bindings.kes", source);
    assert!(parsed.is_ok(), "{:?}", parsed.diagnostics);
    let checked = check(&parsed);
    assert!(checked.is_ok(), "{source}\n{:?}", checked.diagnostics);
}
#[test]
fn class_theorem_fields_are_instantiated_in_citations_and_apply() {
    if std::process::Command::new("z3")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    let theorem = "theorem twice(s:Square):s.side * 2=s.side+s.side{smt}\n";
    for proof in [
        "{twice(other)}",
        "by {apply twice(other)}",
        "by {apply twice}",
    ] {
        good(&format!(
            "{SQUARE}{theorem}theorem cited(other:Square):other.side*2=other.side+other.side {proof}"
        ));
    }
    good(&format!(
        "{SQUARE}{theorem}theorem rewritten(other:Square,k:Int):(other.side*2)+k=(other.side+other.side)+k{{simp [twice]}}"
    ));
}
#[test]
fn class_theorem_requirements_and_explicit_proof_claims_use_runtime_field_paths() {
    let theorem = "theorem positive(s:Square):s.side>0 requires h:s.side>0{h}\n";
    good(&format!(
        "{SQUARE}{theorem}function positiveOnly(p:Square):Int requires positive(p)=p.side\nfunction guarded(p:Square):Int or Error requires p.side>0{{positiveOnly(p)}}"
    ));
    good(&format!(
        "{SQUARE}{theorem}function accept(p:Square,proof h:p.side>0):Int=p.side\nfunction guarded(p:Square):Int or Error requires p.side>0{{accept(p,positive(p))}}"
    ));
    let parsed = parse_file(
        "bindings.kes",
        format!(
            "{SQUARE}{theorem}function positiveOnly(p:Square):Int requires positive(p)=p.side\nfunction unguarded(p:Square):Int=positiveOnly(p)"
        ),
    );
    let checked = check(&parsed);
    assert!(
        checked
            .diagnostics
            .iter()
            .any(|d| d.code == "E_PROOF_ASSUMPTION"),
        "{:?}",
        checked.diagnostics
    );
}

#[test]
fn class_theorems_can_supply_erased_method_claims_for_parameters_and_constructors() {
    for definition in [
        "record Width(value:Int) { public function width():Int=this.value }",
        "class Width {\n public final value:Int\n new(value:Int){this.value=value}\n public function width():Int=this.value\n }",
    ] {
        let source = format!(
            "{definition}\ntheorem width(w:Width):w.width()=w.value{{rfl}}\nfunction accept(w:Width,proof h:w.width()==w.value):Int=w.width()\nfunction use(w:Width):Int=accept(w,width(w))\nlet answer:=accept(Width(3),width(Width(3)))\n"
        );
        good(&source);
    }
}

#[test]
fn evidence_for_another_object_does_not_prove_the_callers_field_claim() {
    let parsed = parse_file(
        "wrong-evidence.kes",
        format!(
            "{SQUARE}\ntheorem itself(s:Square):s.side=s.side{{rfl}}\nfunction accept(s:Square,proof h:s.side==0):Int=s.side\nfunction use(s:Square):Int=accept(s,itself(s))\n"
        ),
    );
    let checked = check(&parsed);
    assert!(
        checked
            .diagnostics
            .iter()
            .any(|d| d.code == "E_PROOF_MISMATCH"),
        "{:?}",
        checked.diagnostics
    );
}

#[test]
fn constant_evidence_cannot_discharge_an_overridden_method_claim() {
    let parsed = parse_file(
        "overridden-evidence.kes",
        r#"
class Base { new() {}
 public function value():Int=0 }
class Child extends Base { new() {}
 @override public function value():Int=1 }
theorem zero():0=0{rfl}
function consume(x:Base,proof h:x.value()==0):Int=x.value()
let answer:=consume(Child(),zero())
"#,
    );
    let checked = check(&parsed);
    assert!(
        checked
            .diagnostics
            .iter()
            .any(|d| d.code == "E_PROOF_OVERRIDDEN"),
        "{:?}",
        checked.diagnostics
    );
}
