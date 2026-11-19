//! Class theorem dispatch stays valid across the entire loaded program.
use kestrel::{
    modules::{ProjectCheck, check_path, check_session, check_text},
    native::TemporaryDirectory,
};
use std::{fs, path::Path};

const LIBRARY: &str = "public class Base { public new() {}\npublic function value() : Int = 0\n}\npublic theorem alwaysZero(x : Base) : x.value() = 0 {rfl}\npublic function use(x : Base) : Int requires alwaysZero(x) = x.value()\n";
fn fixture(root: &Path, child: &str, app: &str) {
    fs::write(root.join("library.kes"), LIBRARY).unwrap();
    fs::write(root.join("child.kes"), child).unwrap();
    fs::write(root.join("app.kes"), app).unwrap();
}
fn override_refused(project: &ProjectCheck) {
    assert!(
        project.modules.iter().any(|m| m
            .checked
            .diagnostics
            .iter()
            .any(|d| d.code == "E_PROOF_OVERRIDDEN" && d.message.contains("Hidden"))),
        "{}",
        project.render_diagnostics()
    );
}

#[test]
fn private_downstream_overrides_invalidate_an_imported_theorem_in_all_entry_points() {
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    let child = "import library.{Base}\nclass Hidden extends Base { new() {super()}\n@override public function value() : Int = 1\n}\npublic function marker() : Int = 1\n";
    let app =
        "import library.{Base,use}\nimport child.{marker}\nprint(use(Base()))\nprint(marker())\n";
    fixture(dir.path(), child, app);
    override_refused(&check_path(dir.path().join("app.kes"), dir.path()).unwrap());
    override_refused(&check_text(dir.path().join("app.kes"), dir.path(), app.into()).unwrap());
    override_refused(&check_session(app.into(), dir.path()).unwrap());
}

#[test]
fn imported_function_requirements_cannot_hide_an_invalid_class_theorem() {
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    fixture(
        dir.path(),
        "",
        "import library.{Base,use}\nclass Hidden extends Base { new() {super()}\n@override public function value() : Int = 1\n}\nprint(use(Hidden()))\n",
    );
    override_refused(&check_path(dir.path().join("app.kes"), dir.path()).unwrap());
}

#[test]
fn an_overload_does_not_invalidate_a_parent_method_theorem() {
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    let child = "import library.{Base}\nclass Hidden extends Base { new() {super()}\npublic function value(n : Int) : Int = n\n}\npublic function marker() : Int = 1\n";
    fixture(
        dir.path(),
        child,
        "import library.{Base,use}\nimport child.{marker}\nprint(use(Base()))\nprint(marker())\n",
    );
    let project = check_path(dir.path().join("app.kes"), dir.path()).unwrap();
    assert!(project.is_ok(), "{}", project.render_diagnostics());
}

fn check(source: &str) -> kestrel::checker::Checked {
    let parsed = kestrel::parse_file("program.kes", source);
    assert!(parsed.is_ok(), "{:?}", parsed.diagnostics);
    kestrel::checker::check(&parsed)
}

#[test]
fn inherited_projections_follow_the_actual_receiver_layout() {
    let base = "class Grand {public final g:Int; new(g:Int){this.g=g}}\nclass Parent extends Grand {public final p:Int; new(g:Int,p:Int){super(g);this.p=p}\npublic function getP():Int=this.p}\nclass Child extends Parent {public final c:Int;new(g:Int,p:Int,c:Int){super(g,p);this.c=c}}\n";
    let result = check(&format!("{base}theorem right(x:Child):x.getP()=x.p{{rfl}}"));
    assert!(result.is_ok(), "{:?}", result.diagnostics);
    let result = check(&format!("{base}theorem wrong(x:Child):x.getP()=x.c{{rfl}}"));
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "E_PROOF_MISMATCH"),
        "{:?}",
        result.diagnostics
    );
}

#[test]
fn concrete_descendants_are_considered_for_generic_theorem_receivers() {
    let result = check(
        "class Base[T] {new(){}\npublic function value():Int=0}\nclass Child extends Base[Int] {new(){}\n@override public function value():Int=1}\ntheorem allZero(x:Base[T]):x.value()=0{rfl}",
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "E_PROOF_OVERRIDDEN"),
        "{:?}",
        result.diagnostics
    );
}

#[test]
fn default_proofs_specialize_separately_for_each_implementing_class() {
    let base = "trait Measure {public function value():Int\npublic function measure():Int=this.value()}\nclass A {has Measure;new(){}\npublic function value():Int=1}\nclass B {has Measure;new(){}\npublic function value():Int=2}\n";
    let result = check(&format!(
        "{base}theorem first(a:A):a.measure()=1{{rfl}}\ntheorem second(b:B):b.measure()=2{{rfl}}"
    ));
    assert!(result.is_ok(), "{:?}", result.diagnostics);
    let result = check(&format!(
        "{base}theorem wrong(a:A,b:B):a.measure()=b.measure(){{rfl}}"
    ));
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "E_PROOF_MISMATCH"),
        "{:?}",
        result.diagnostics
    );
}

#[test]
fn class_proofs_respect_member_privacy_but_can_unfold_public_implementations() {
    let base = "class ProofBox {final hidden:Int;public new(n:Int){this.hidden=n}\nfunction internal():Int=this.hidden\npublic function width():Int=this.internal()}\n";
    let result = check(&format!(
        "{base}theorem publicRead():ProofBox(3).width()=3{{rfl}}"
    ));
    assert!(result.is_ok(), "{:?}", result.diagnostics);
    for expression in ["x.hidden", "x.internal()"] {
        let result = check(&format!(
            "{base}theorem privateRead(x:ProofBox):{expression}={expression}{{rfl}}"
        ));
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| d.code == "E_MEMBER_PRIVATE"),
            "{:?}",
            result.diagnostics
        );
    }
}

#[test]
fn defaults_supply_projections_for_readable_explicit_arguments() {
    let result = check(
        "record Point(value:Int)\ntrait Read {public function read(p:Point):Int\npublic function value(p:Point):Int=this.read(p)}\nclass PointReader {has Read;new(){}\npublic function read(p:Point):Int=p.value}\ntheorem reads(r:PointReader,p:Point):r.value(p)=p.value{rfl}",
    );
    assert!(result.is_ok(), "{:?}", result.diagnostics);
    let result = check(
        "record Point(value:Int)\ntrait Read {public function read(p:Point):Int\npublic function relay(p:Point):Int=this.read(p)\npublic function value(p:Point):Int=this.relay(p)}\nclass PointReader {has Read;new(){}\npublic function read(p:Point):Int=p.value}\ntheorem reads(r:PointReader,p:Point):r.value(p)=p.value{rfl}",
    );
    assert!(result.is_ok(), "{:?}", result.diagnostics);
}
