use kestrel::modules::{check_path, check_source};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

struct Project(PathBuf);
static NEXT: AtomicU64 = AtomicU64::new(0);
impl Project {
    fn new(files: &[(&str, &str)]) -> Self {
        let root = std::env::temp_dir().join(format!(
            "kestrel-check-{}-{}",
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

#[test]
fn selective_imports_resolve_without_running_a_module() {
    let project = Project::new(&[
        ("math.kes", "public function twice(x : Int) : Int = x * 2"),
        (
            "main.kes",
            "import math.{twice}\nlet result : Int = twice(3)",
        ),
    ]);
    let result = project.check("main.kes");
    assert!(result.is_ok(), "{}", result.render_diagnostics());
    assert_eq!(
        result.modules.len(),
        3,
        "the program, its import and the prelude"
    );
}

#[test]
fn default_bodies_check_foreign_callback_ownership_even_when_imported_or_unused() {
    let header = "import python \"builtins\" as py\npublic class Point {public final label : String\nnew(label : String) {this.label=label}\npublic function read(n : Int) : String=this.label}\n";
    for declaration in [
        "public function install(n : Int = {let p:=Point(\"local\"+\" object\")\nlet ignored:=py.sorted([1,2],key=p.read)\n0}) : Int with Effects=n",
        "public function install(p : Point,n : Int = {let ignored:=py.sorted([1,2],key=p.read)\n0}) : Int with Effects=n",
        "public trait Installer {function install(p : Point,n : Int = {let ignored:=py.sorted([1,2],key=p.read)\n0}) : Int with Effects}",
    ] {
        let source = format!("{header}{declaration}");
        let local = check_source("defaults.kes", &source);
        assert!(
            local
                .checked
                .diagnostics
                .iter()
                .any(|error| error.code == "E_LAMBDA_ESCAPE"),
            "{:?}",
            local.checked.diagnostics
        );
        let project = Project::new(&[
            ("defaults.kes", &source),
            ("app.kes", "import defaults as d\nlet n:=1"),
        ]);
        assert!(
            project
                .check("app.kes")
                .render_diagnostics()
                .contains("E_LAMBDA_ESCAPE")
        );
    }
    let project = Project::new(&[
        (
            "defaults.kes",
            "public function recursive(n : Int = recursive()) : Int=n",
        ),
        (
            "app.kes",
            "import defaults.{recursive}\nlet n:=recursive(1)",
        ),
    ]);
    let checked = project.check("app.kes");
    assert!(checked.is_ok(), "{}", checked.render_diagnostics());
}

#[test]
fn named_arrow_inputs_export_labels_and_argument_permissions() {
    let project = Project::new(&[
        (
            "actions.kes",
            "public class Counter {public var n : Int\nnew() {this.n=0}}\npublic alias Update = (mut value : Counter) -> Int\npublic function increment(mut target : Counter) : Int {target.n=target.n+1\ntarget.n}",
        ),
        (
            "app.kes",
            "import actions.{Counter,Update,increment}\nlet f : Update=increment\nlet c:=Counter()\nf(value=c)",
        ),
        (
            "bad.kes",
            "import actions.{Counter,Update}\nfunction bad(f : Update,c : Counter) : Int=f(value=c)",
        ),
    ]);
    let checked = project.check("app.kes");
    assert!(checked.is_ok(), "{}", checked.render_diagnostics());
    assert!(
        project
            .check("bad.kes")
            .render_diagnostics()
            .contains("E_MUT_PARAMETER")
    );
}

#[test]
fn mutable_callback_aliases_and_results_keep_permissions_across_modules() {
    let project = Project::new(&[
        (
            "counters.kes",
            "public alias Counter = mut (() -> Int)\npublic function make() : Counter {let mut n:=0\nmove () => {n=n+1\nn}}\npublic function next(mut f : Counter) : Int=f()",
        ),
        (
            "app.kes",
            "import counters.{Counter,make,next}\nlet f : Counter=make()\nnext(f)\nnext(f)",
        ),
        (
            "bad.kes",
            "import counters.{Counter}\nfunction bad(f : Counter) : Int=f()",
        ),
    ]);
    let result = project.check("app.kes");
    assert!(result.is_ok(), "{}", result.render_diagnostics());
    let result = project.check("bad.kes");
    assert!(
        result.render_diagnostics().contains("E_MUT_PARAMETER"),
        "{}",
        result.render_diagnostics()
    );
}

#[test]
fn postconditions_check_in_imported_bodies_and_header_cycles() {
    let project = Project::new(&[
        (
            "lib.kes",
            "public function checked(n : Int) : Int ensures result == n = n",
        ),
        ("main.kes", "import lib.{checked}\nchecked(3)"),
        (
            "a.kes",
            "import b.{other}\npublic function value() : Int ensures result > 0 = 1",
        ),
        (
            "b.kes",
            "import a.{value}\npublic function other() : Int = value()",
        ),
    ]);
    let valid = project.check("main.kes");
    assert!(valid.is_ok(), "{}", valid.render_diagnostics());
    for entry in ["a.kes", "b.kes"] {
        let valid = project.check(entry);
        assert!(valid.is_ok(), "{}", valid.render_diagnostics());
    }
}

#[test]
fn imports_keep_borrows_from_long_function_call_chains() {
    let mut library =
        "public class Point {public new() {}}\nfunction id0(p : Point) : Point = p\n".to_owned();
    for i in 1..24 {
        library.push_str(&format!(
            "public function id{i}(p : Point) : Point = id{}(p)\n",
            i - 1
        ));
    }
    let project = Project::new(&[
        ("lib.kes", &library),
        (
            "main.kes",
            "import lib.{Point,id23}\nfunction bad() : Point {let p := Point()\nid23(p)}",
        ),
        (
            "safe.kes",
            "import lib.{Point,id23}\nfunction good(p : Point) : Point = id23(p)",
        ),
    ]);
    assert!(
        project
            .check("main.kes")
            .render_diagnostics()
            .contains("E_BORROW_ESCAPE")
    );
    let safe = project.check("safe.kes");
    assert!(safe.is_ok(), "{}", safe.render_diagnostics());
}

#[test]
fn imported_functions_keep_their_theorem_assumptions() {
    let library = "theorem atZero(n : Nat) : n = Zero requires h : n = Zero {h}\npublic function onlyZero(n : Nat) : Nat requires atZero(n) {n}";
    for (source, code) in [
        ("import math.{onlyZero}\nlet n := onlyZero(Zero)", None),
        (
            "import math.{onlyZero}\nlet n := onlyZero(1)",
            Some("E_PROOF_ASSUMPTION"),
        ),
        (
            "import math.{onlyZero}\nlet saved := onlyZero\nlet n := saved(1)",
            Some("E_PROOF_FUNCTION_VALUE"),
        ),
    ] {
        let project = Project::new(&[("math.kes", library), ("main.kes", source)]);
        let result = project.check("main.kes");
        match code {
            None => assert!(result.is_ok(), "{}", result.render_diagnostics()),
            Some(code) => assert!(
                result.render_diagnostics().contains(code),
                "{}",
                result.render_diagnostics()
            ),
        }
    }
}

#[test]
fn importing_a_script_is_refused() {
    let project = Project::new(&[
        ("script.kes", "print(\"should never run\")"),
        ("main.kes", "import script"),
    ]);
    assert!(
        project
            .check("main.kes")
            .render_diagnostics()
            .contains("E_IMPORT_SCRIPT")
    );
}

#[test]
fn imports_must_come_first() {
    let result = check_source("test", "let a := 1\nimport std.process.{args}\nargs");
    // The isolated entry point must enforce the same import-order rule as the loader.
    assert!(
        result
            .checked
            .diagnostics
            .iter()
            .any(|d| d.code == "E_IMPORT_ORDER")
    );
}

#[test]
fn private_declarations_do_not_leak_through_imports() {
    let project = Project::new(&[
        ("lib.kes", "function hidden() : Int = 1"),
        ("main.kes", "import lib.{hidden}\nhidden()"),
    ]);
    assert!(
        project
            .check("main.kes")
            .render_diagnostics()
            .contains("E_PRIVATE_IMPORT")
    );
}

#[test]
fn package_visibility_is_limited_to_the_same_folder() {
    let project = Project::new(&[
        ("lib.kes", "package function shared() : Int = 1"),
        ("main.kes", "import lib.{shared}\nshared()"),
        ("other/main.kes", "import lib.{shared}\nshared()"),
    ]);
    let result = project.check("main.kes");
    assert!(result.is_ok(), "{}", result.render_diagnostics());
    assert!(
        project
            .check("other/main.kes")
            .render_diagnostics()
            .contains("E_PRIVATE_IMPORT")
    );
}

#[test]
fn exported_classes_keep_their_method_types() {
    let project = Project::new(&[
        (
            "shapes.kes",
            "public class Square { final side : double\nnew(side : double) { this.side = side }\npublic function area() : double = this.side * this.side }",
        ),
        (
            "main.kes",
            "import shapes.{Square}\nlet sq := Square(2.0)\nlet area : double = sq.area()",
        ),
    ]);
    let result = project.check("main.kes");
    assert!(result.is_ok(), "{}", result.render_diagnostics());
}

#[test]
fn primitive_function_import_cycles_check_from_either_entry() {
    let project = Project::new(&[
        (
            "a.kes",
            "import b.{other}\npublic function value() : Int = 1",
        ),
        (
            "b.kes",
            "import a.{value}\npublic function other() : Int = 2",
        ),
        ("missing.kes", "import absent.{value}"),
    ]);
    for entry in ["a.kes", "b.kes"] {
        let result = project.check(entry);
        assert!(result.is_ok(), "{}", result.render_diagnostics());
        assert_eq!(
            result.modules.len(),
            3,
            "each entry, the module it imports and the prelude"
        );
    }
    assert!(
        project
            .check("missing.kes")
            .render_diagnostics()
            .contains("E_MODULE_NOT_FOUND")
    );
}

#[test]
fn cyclic_interfaces_preserve_effects_mut_parameters_and_visibility() {
    for (body, expected) in [
        ("other(n)", Some("E_EFFECT")),
        ("change(n)", Some("E_MUT")),
        ("hidden()", Some("E_PRIVATE_IMPORT")),
        ("false", Some("Expected Int, found Bool")),
        ("n", None),
    ] {
        let selected = if body == "hidden()" {
            "hidden"
        } else {
            "other,change"
        };
        let a = format!("import b.{{{selected}}}\npublic function value(n : Int) : Int = {body}");
        let project = Project::new(&[
            ("a.kes", &a),
            (
                "b.kes",
                "import a.{value}\npublic function other(n : Int) : Int with Effects = n\npublic function change(mut n : Int) : Int = n\nfunction hidden() : Int = 0",
            ),
        ]);
        for entry in ["a.kes", "b.kes"] {
            let result = project.check(entry);
            if let Some(code) = expected {
                assert!(
                    result.render_diagnostics().contains(code),
                    "{body}: {}",
                    result.render_diagnostics()
                );
            } else {
                assert!(result.is_ok(), "{}", result.render_diagnostics());
            }
        }
    }
}

#[test]
fn an_entire_import_component_must_have_certifiable_interfaces() {
    for extra in [
        "const LIMIT := 1",
        "public function inferred() = 1",
        "public function limited(n : Int) : Int or Error requires predicate(n) {n}\nfunction predicate(n : Int) : Bool = n > 0",
        "class Hidden {new() {}\npublic function size() = 1}",
        "theorem same(n : Nat) : n = n {rfl}",
        "print(1)",
    ] {
        let d = format!("import b.{{other}}\npublic function third() : Int = other()\n{extra}");
        let project = Project::new(&[
            (
                "a.kes",
                "import b.{other}\nimport d.{third}\npublic function value() : Int = other()",
            ),
            (
                "b.kes",
                "import a.{value}\npublic function other() : Int = 2",
            ),
            ("d.kes", &d),
        ]);
        for entry in ["a.kes", "b.kes", "d.kes"] {
            let result = project.check(entry);
            assert!(
                result
                    .render_diagnostics()
                    .contains("E_IMPORT_CYCLE_UNSUPPORTED"),
                "{entry}, {extra}: {}",
                result.render_diagnostics()
            );
        }
    }
}

const SHAPES: &str = "import units.{Unit, Measured, total, larger}\npublic trait Shape {\nfunction area() : double\n}\npublic class Square {\nhas Shape\npublic final side : double\npublic final unit : Unit\nnew(side : double, move unit : Unit) {this.side = side\nthis.unit = unit}\npublic function area() : double = this.side * this.side\npublic function measured() : Measured = Measured(this.area())\n}\npublic function both(a : Square, b : Square) : double = total(a.measured(), b.measured())\npublic function bigger(a : Square, b : Square) : Square = larger(a, b)";
const UNITS: &str = "import shapes.{Shape, Square}\npublic enum Unit {\nMeters\nFeet\n}\npublic class Measured {\npublic final value : double\nnew(value : double) {this.value = value}\n}\npublic alias Area = double\npublic function total(a : Measured, b : Measured) : Area = a.value + b.value\npublic function larger[S : Shape](a : S, b : S) : S = if a.area() >= b.area() then a else b\npublic function unitName(s : Square) : String = match s.unit {\nMeters => \"m\",\nFeet => \"ft\",\n}";

#[test]
fn classes_traits_enums_aliases_and_generics_form_cycles_from_every_entry() {
    let project = Project::new(&[
        ("shapes.kes", SHAPES),
        ("units.kes", UNITS),
        (
            "main.kes",
            "import shapes.{Square, both, bigger}\nimport units.{Unit, unitName}\nlet small := Square(2.0, Unit.Meters)\nlet large := Square(3.0, Unit.Feet)\nprint(both(small, large))\nprint(unitName(bigger(small, large)))",
        ),
    ]);
    for entry in ["shapes.kes", "units.kes", "main.kes"] {
        let result = project.check(entry);
        assert!(result.is_ok(), "{entry}: {}", result.render_diagnostics());
    }
    let errors = Project::new(&[
        ("shapes.kes", SHAPES),
        (
            "units.kes",
            &UNITS.replace("public class Measured", "class Measured"),
        ),
    ])
    .check("units.kes")
    .render_diagnostics();
    assert!(errors.contains("E_PRIVATE_IMPORT"), "{errors}");
}

#[test]
fn cycle_headers_keep_every_returned_borrow_from_every_entry() {
    let left = "import right.{Holder}\npublic class Label {\npublic final value : String\nnew(value : String) {this.value = value}\n}\npublic function pick(h : Holder) : Label = h.label";
    let right = "import left.{Label, pick}\npublic class Holder {\npublic final label : Label\nnew(move label : Label) {this.label = label}\n}\npublic function take(move h : Holder) : void {}\n";
    let bad = format!(
        "{right}public function bad() : String {{let h := Holder(Label(\"x\"))\nlet l := pick(h)\ntake(h)\nl.value}}"
    );
    let good = format!(
        "{right}public function good() : String {{let h := Holder(Label(\"x\"))\nlet n := pick(h).value\ntake(h)\nn}}"
    );
    let outside = "import right.{Holder, take}\nimport left.{Label, pick}\nlet h := Holder(Label(\"x\"))\nlet l := pick(h)\ntake(h)\nprint(l.value)";
    let project = Project::new(&[
        ("left.kes", left),
        ("right.kes", &bad),
        ("outside.kes", outside),
    ]);
    for entry in ["left.kes", "right.kes", "outside.kes"] {
        let errors = project.check(entry).render_diagnostics();
        assert!(errors.contains("E_BORROW_CONFLICT"), "{entry}: {errors}");
    }
    let project = Project::new(&[("left.kes", left), ("right.kes", &good)]);
    for entry in ["left.kes", "right.kes"] {
        let result = project.check(entry);
        assert!(result.is_ok(), "{entry}: {}", result.render_diagnostics());
    }
}

#[test]
fn alias_chains_settle_across_a_cycle_and_alias_loops_are_refused() {
    let project = Project::new(&[
        (
            "a.kes",
            "import b.{B1, B3, useAll}\npublic alias A1 = B1\npublic alias A2 = List[B3]\npublic alias A4 = Int\npublic function first(xs : A2) : Int = match xs {\n[x, ..] => x,\n_ => 0,\n}",
        ),
        (
            "b.kes",
            "import a.{A1, A2, A4, first}\npublic alias B1 = A2\npublic alias B3 = A4\npublic function useAll(xs : A1) : Int = first(xs) + 1",
        ),
        (
            "main.kes",
            "import b.{useAll}\nlet n : Int = useAll([41, 2])",
        ),
    ]);
    for entry in ["a.kes", "b.kes", "main.kes"] {
        let result = project.check(entry);
        assert!(result.is_ok(), "{entry}: {}", result.render_diagnostics());
    }
    let project = Project::new(&[
        (
            "a.kes",
            "import b.{B}\npublic alias A = B\npublic function f(x : A) : Int = 1",
        ),
        ("b.kes", "import a.{A}\npublic alias B = A"),
        ("main.kes", "import a.{f}\nlet n : Int = f(3)"),
    ]);
    for entry in ["a.kes", "b.kes", "main.kes"] {
        let errors = project.check(entry).render_diagnostics();
        assert!(
            errors.matches("E_TYPE_CYCLE").count() == 2,
            "{entry}: {errors}"
        );
    }
}

#[test]
fn recursive_enums_in_two_modules_check_from_every_entry() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for entry in [
        "testdata/language/cycleexpr.kes",
        "testdata/language/cycleblock.kes",
        "testdata/language/datacycles.kes",
    ] {
        let result = check_path(root.join(entry), root).unwrap();
        assert!(result.is_ok(), "{entry}: {}", result.render_diagnostics());
        assert_eq!(
            result.modules.len(),
            if entry.ends_with("datacycles.kes") {
                4
            } else {
                3
            }
        );
    }
}

#[test]
fn self_imports_and_overlapping_function_cycles_keep_closed_interfaces() {
    let project = Project::new(&[
        (
            "a.kes",
            "import b.{other}\nimport d.{third}\npublic function value() : Int = 1",
        ),
        (
            "b.kes",
            "import a.{value}\npublic function other() : Int = value()",
        ),
        (
            "d.kes",
            "import b.{other}\npublic function third() : Int = other()",
        ),
        (
            "selfcycle.kes",
            "import selfcycle as same\npublic function value(n : Int) : Int = if n == 0 then 0 else same.value(n - 1)",
        ),
    ]);
    for entry in ["a.kes", "b.kes", "d.kes", "selfcycle.kes"] {
        let result = project.check(entry);
        assert!(result.is_ok(), "{}", result.render_diagnostics());
    }
}

#[test]
fn import_cycle_functions_stay_opaque_to_proofs_in_both_import_orders() {
    for imports in [
        "import a.{value}\nimport b.{other}",
        "import b.{other}\nimport a.{value}",
    ] {
        let main = format!("{imports}\ntheorem same(n : Nat) : value(n) = n {{rfl}}");
        let project = Project::new(&[
            (
                "a.kes",
                "import b.{other}\npublic function value(n : Nat) : Nat = n",
            ),
            (
                "b.kes",
                "import a.{value}\npublic function other(n : Nat) : Nat = value(n)",
            ),
            ("main.kes", &main),
        ]);
        let result = project.check("main.kes");
        assert!(
            result.render_diagnostics().contains("E_PROOF_OPAQUE"),
            "{}",
            result.render_diagnostics()
        );
    }
}

#[test]
fn ambiguous_folder_and_dotted_file_paths_are_not_guessed() {
    let project = Project::new(&[
        ("a/b.kes", "public function value() : Int = 1"),
        ("a.b.kes", "public function value() : Int = 2"),
        ("main.kes", "import a.b.{value}"),
    ]);
    assert!(
        project
            .check("main.kes")
            .render_diagnostics()
            .contains("E_MODULE_AMBIGUOUS")
    );
}

#[test]
fn standard_test_equality_keeps_the_same_type_rule() {
    assert!(
        check_source("test.kes", "import std.test.{assertEq}\nassertEq(1, 2)")
            .checked
            .is_ok()
    );
    let result = check_source("test.kes", "import std.test.{assertEq}\nassertEq(1, 1.0)");
    assert!(
        result
            .checked
            .diagnostics
            .iter()
            .any(|d| d.code == "E_CALL_ARGUMENTS")
    );
}

#[test]
fn failed_dependencies_prevent_a_successful_project_check() {
    let project = Project::new(&[
        ("lib.kes", "public function f() : Int = false"),
        ("main.kes", "import lib.{f}\nf()"),
    ]);
    let result = project.check("main.kes");
    assert!(!result.is_ok());
    assert!(
        result
            .render_diagnostics()
            .contains("Expected Int, found Bool")
    );
}

#[test]
fn module_names_follow_the_lowercase_rule() {
    // An imported module needs a lowercase name; the file a command names does not,
    // because nothing imports it (REVIEW2 1.4).
    let project = Project::new(&[
        ("Bad.kes", "public function f() : Int = 1"),
        ("main.kes", "import Bad.{f}\nprint(f())"),
    ]);
    assert!(
        project
            .check("main.kes")
            .render_diagnostics()
            .contains("E_MODULE_NAMING")
    );
    assert!(
        !project
            .check("Bad.kes")
            .render_diagnostics()
            .contains("E_MODULE_NAMING")
    );
}

#[test]
fn existing_example_test_checks_without_executing() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let result = check_path(root.join("tests/examples/rules.kes"), root).unwrap();
    assert!(result.is_ok(), "{}", result.render_diagnostics());
}

#[test]
fn sealed_traits_accept_implementations_only_in_their_own_module() {
    let project = Project::new(&[
        (
            "shapes.kes",
            "public sealed trait Shape { function area() : double }\npublic class Square { has Shape\npublic function area() : double = 1.0 }",
        ),
        (
            "main.kes",
            "import shapes.{Shape}\nclass Circle { has Shape\npublic function area() : double = 1.0 }",
        ),
    ]);
    let local = project.check("shapes.kes");
    assert!(local.is_ok(), "{}", local.render_diagnostics());
    assert!(
        project
            .check("main.kes")
            .render_diagnostics()
            .contains("E_SEALED_IMPLEMENTATION")
    );
}

#[test]
fn undetermined_exports_cannot_reuse_another_modules_inference_variables() {
    let project = Project::new(&[
        ("lib.kes", "public const EMPTY := []"),
        ("main.kes", "import lib.{EMPTY}\nlet xs : List[Int] = EMPTY"),
    ]);
    let result = project.check("main.kes");
    assert!(!result.is_ok());
    assert!(result.render_diagnostics().contains("E_EXPORT_INFERENCE"));
}

#[test]
fn imported_methods_preserve_independent_mutation_and_io_permissions() {
    let project = Project::new(&[
        (
            "counter.kes",
            include_str!("../testdata/language/mutation.kes"),
        ),
        (
            "good.kes",
            "import counter.{Counter}\nfunction change(mut a : Counter, mut b : Counter) : void = a.incrementBoth(b)\nfunction report(c : Counter) : void with Effects = c.printSomething()",
        ),
        (
            "readonly.kes",
            "import counter.{Counter}\nfunction change(c : Counter) : void with Effects = c.increment()",
        ),
        (
            "io.kes",
            "import counter.{Counter}\nfunction report(mut c : Counter) : void = c.printSomething()",
        ),
        (
            "overlap.kes",
            "import counter.{Counter}\nlet c := Counter()\nc.incrementBoth(c)",
        ),
    ]);
    let good = project.check("good.kes");
    assert!(good.is_ok(), "{}", good.render_diagnostics());
    assert!(
        project
            .check("readonly.kes")
            .render_diagnostics()
            .contains("E_MUT_PARAMETER")
    );
    assert!(
        project
            .check("io.kes")
            .render_diagnostics()
            .contains("E_EFFECT_REQUIRED")
    );
    assert!(
        project
            .check("overlap.kes")
            .render_diagnostics()
            .contains("E_BORROW_CONFLICT")
    );
}

#[test]
fn imported_functions_keep_their_borrows_and_contracts() {
    let shapes = "public class Square {\npublic final side : double\nnew(side : double) { this.side = side }\npublic function area() : double = this.side * this.side\n}\npublic class Panel {\npublic final square : &Square\nnew(square : Square) { this.square = square }\n}\npublic function biggest(a : Square, b : Square) : Square = if a.area() > b.area() then a else b\npublic function half(n : Int) : Int or Error requires n > 0 = n / 2\n";
    let project = Project::new(&[
        ("shapes.kes", shapes),
        (
            "good.kes",
            "import shapes.{Square, Panel, biggest, half}\nfunction pick(a : Square, b : Square) : Square = biggest(a, b)\nfunction frame(s : Square) : Panel = Panel(s)\nfunction four() : Int or Error = half(8)",
        ),
        (
            "escape.kes",
            "import shapes.{Square, biggest}\nfunction make() : Square { let s := Square(1.0)\nlet t := Square(2.0)\nbiggest(s, t) }",
        ),
        (
            "panel.kes",
            "import shapes.{Square, Panel}\nfunction make() : Panel { let s := Square(1.0)\nPanel(s) }",
        ),
        (
            "contract.kes",
            "import shapes.{half}\nfunction zero() : Int or Error = half(0)",
        ),
    ]);
    let good = project.check("good.kes");
    assert!(good.is_ok(), "{}", good.render_diagnostics());
    assert!(
        project
            .check("escape.kes")
            .render_diagnostics()
            .contains("E_BORROW_ESCAPE")
    );
    assert!(
        project
            .check("panel.kes")
            .render_diagnostics()
            .contains("E_BORROW_ESCAPE")
    );
    assert!(
        project
            .check("contract.kes")
            .render_diagnostics()
            .contains("E_CONTRACT_VIOLATED")
    );
}

#[test]
fn associated_type_declarations_survive_module_exports() {
    let project = Project::new(&[
        (
            "source.kes",
            "public trait Source {type Item\nfunction get() : Item}\npublic class Value {has Source\nnew() {}\npublic function get() : Int = 7}",
        ),
        (
            "main.kes",
            "import source.{Source,Value}\nfunction read[T : Source(Item = Int)](x : T) : Int = x.get()\nread(Value())",
        ),
    ]);
    let result = project.check("main.kes");
    assert!(result.is_ok(), "{}", result.render_diagnostics());
}

#[test]
fn whole_module_imports_bind_the_last_component_or_an_alias() {
    let project = Project::new(&[
        (
            "geometry/shape.kes",
            "public const LIMIT : Int = 10\npublic class Point {public final x : Int\nnew(x : Int) {this.x = x}}\npublic function area(p : Point) : Int = p.x\nfunction hidden() : Int = 0",
        ),
        (
            "main.kes",
            "import geometry.shape\nimport geometry.shape as g\nlet p : shape.Point = shape.Point(3)\nlet q : g.Point = g.Point(4)\nshape.area(p) + g.area(q) + g.LIMIT",
        ),
    ]);
    let result = project.check("main.kes");
    assert!(result.is_ok(), "{}", result.render_diagnostics());
    for source in [
        "import geometry.shape as g\ng.hidden()",
        "import geometry.shape as g\ng.missing()",
    ] {
        fs::write(project.0.join("main.kes"), source).unwrap();
        assert!(
            project
                .check("main.kes")
                .render_diagnostics()
                .contains("E_NAMESPACE_MEMBER")
        );
    }
}

#[test]
fn whole_module_imports_bind_their_types_unqualified() {
    let source = "import std.kesdas as kd\nlet df : DataFrame = kd.DataFrame([kd.Series([1, 2], name = \"v\")])\nlet series : Series = kd.Series([1, 2], name = \"v\")\nlet made := DataFrame([Series([1], name = \"v\")])";
    let result = check_source("types.kes", source);
    assert!(result.checked.is_ok(), "{:?}", result.checked.diagnostics);

    // The same rule applies without an alias, and qualified names still work.
    let source = "import std.kesdas\nlet df : DataFrame = kesdas.DataFrame([kesdas.Series([1], name = \"v\")])";
    let result = check_source("types.kes", source);
    assert!(result.checked.is_ok(), "{:?}", result.checked.diagnostics);

    // A type from a numerical module is imported the same way.
    let result = check_source(
        "types.kes",
        "import std.complex as c\nfunction f(z : Complex) : void {}",
    );
    assert!(result.checked.is_ok(), "{:?}", result.checked.diagnostics);

    // Values and functions still need the qualifier.
    for source in [
        "import std.linalg as la\nsolve(1)",
        "import std.kesdas as kd\nreadCsv(\"x\")",
    ] {
        let result = check_source("types.kes", source);
        assert!(
            result
                .checked
                .diagnostics
                .iter()
                .any(|d| d.code == "E_UNKNOWN_NAME"),
            "{}: {:?}",
            source,
            result.checked.diagnostics
        );
    }

    // Two modules that export one type name cannot both bind it unqualified.
    let project = Project::new(&[
        ("a.kes", "public class Shared { new() {} }"),
        ("b.kes", "public class Shared { new() {} }"),
        (
            "main.kes",
            "import a as x\nimport b as y\nlet value : Shared = x.Shared()",
        ),
    ]);
    let result = project.check("main.kes");
    assert!(
        result.render_diagnostics().contains("E_AMBIGUOUS_TYPE"),
        "{}",
        result.render_diagnostics()
    );

    // Importing the same module twice binds the same type ID; that is not ambiguous.
    let project = Project::new(&[
        ("a.kes", "public class Shared { new() {} }"),
        (
            "main.kes",
            "import a as x\nimport a as y\nlet value : Shared = x.Shared()",
        ),
    ]);
    let result = project.check("main.kes");
    assert!(result.is_ok(), "{}", result.render_diagnostics());
}

#[test]
fn module_namespaces_are_not_values_and_functions_stay_qualified() {
    for (source, code) in [
        ("import std.math as m\nm", "E_NAMESPACE_VALUE"),
        ("import std.math as m\nsin(1.0)", "E_UNKNOWN_NAME"),
        (
            "import std.math as m\nfunction f(x : m) : void {}",
            "E_NAMESPACE_TYPE",
        ),
        (
            "import std.math as m\nimport std.str as m",
            "E_DUPLICATE_IMPORT",
        ),
        ("import std.math as this", "E_RESERVED_NAME"),
    ] {
        let result = check_source("namespaces.kes", source);
        assert!(
            result.checked.diagnostics.iter().any(|d| d.code == code),
            "{source}: {:?}",
            result.checked.diagnostics
        );
    }
    let result = check_source(
        "namespaces.kes",
        "import std . math as m\nimport std . collections as c\nlet xs : c . List[Int] = [1,2]\nm.sqrt(4.0)",
    );
    assert!(result.checked.is_ok(), "{:?}", result.checked.diagnostics);
}

#[test]
fn namespace_calls_keep_borrow_summaries_for_same_named_exports() {
    let project = Project::new(&[
        (
            "model.kes",
            "public class Point {public final x : Int\nnew(x : Int) {this.x = x}}",
        ),
        (
            "left.kes",
            "import model.{Point}\npublic function pick(p : Point) : Point = p",
        ),
        (
            "right.kes",
            "import model.{Point}\npublic function pick(p : Point) : Point = Point(9)",
        ),
        (
            "main.kes",
            "import model.{Point}\nimport left as a\nimport right as b\nfunction bad() : Point {let p := Point(1)\na.pick(p)}",
        ),
    ]);
    assert!(
        project
            .check("main.kes")
            .render_diagnostics()
            .contains("E_BORROW_ESCAPE")
    );
    fs::write(project.0.join("main.kes"),"import model.{Point}\nimport left as a\nimport right as b\nfunction good() : Point {let p := Point(1)\nb.pick(p)}").unwrap();
    let result = project.check("main.kes");
    assert!(result.is_ok(), "{}", result.render_diagnostics());
    fs::write(project.0.join("left.kes"),"import model.{Point}\npublic enum Choice {Hold(Point),Empty}\npublic alias Lock = Mutex[Point]\npublic function add(a : Int,b : Int) : Int = a+b").unwrap();
    fs::write(project.0.join("main.kes"),"import model.{Point}\nimport left as a\nlet p := Point(1)\nlet choice := a.Choice.Hold(p)\nprint(p.x)").unwrap();
    assert!(
        project
            .check("main.kes")
            .render_diagnostics()
            .contains("E_USE_AFTER_MOVE")
    );
    fs::write(
        project.0.join("main.kes"),
        "import left as a\nfunction make() : Int -> Int = a.add(2,_)\nlet f := make()\nf(3)",
    )
    .unwrap();
    let result = project.check("main.kes");
    assert!(result.is_ok(), "{}", result.render_diagnostics());
    fs::write(project.0.join("main.kes"),"import model.{Point}\nimport left as a\nlet p := Point(1)\nlet lock := a.Lock(p)\nprint(p.x)").unwrap();
    assert!(
        project
            .check("main.kes")
            .render_diagnostics()
            .contains("E_USE_AFTER_MOVE")
    );
}

#[test]
fn namespace_calls_cannot_erase_theorem_requirements() {
    let library = "theorem zero(n : Nat) : n = Zero requires h : n = Zero {h}\npublic function guarded(n : Nat) : Nat requires zero(n) {n}\npublic function positive(n : Int) : Int or Error requires n > 0 {n}\npublic function checked(n : Nat) : Nat requires zero(n) {n}\npublic function checked(n : Bool) : Bool = n\npublic class Machine {new(n : Nat) requires zero(n) {}\npublic static function guarded(n : Nat) : Nat requires zero(n) {n}}\n";
    for (source, code) in [
        ("import math as m\nm.guarded(Zero)", None),
        ("import math as m\nm.guarded(1)", Some("E_PROOF_ASSUMPTION")),
        (
            "import math as m\nlet f := m.guarded",
            Some("E_PROOF_FUNCTION_VALUE"),
        ),
        (
            "import math as m\nm.positive(0)?",
            Some("E_CONTRACT_VIOLATED"),
        ),
        ("import math as m\nm.Machine(Zero)", None),
        ("import math as m\nm.Machine(1)", Some("E_PROOF_ASSUMPTION")),
        ("import math as m\nm.Machine.guarded(Zero)", None),
        (
            "import math as m\nm.Machine.guarded(1)",
            Some("E_PROOF_ASSUMPTION"),
        ),
        (
            "import math as m\nlet n : Nat = 1\nm.checked(n)",
            Some("E_PROOF_ASSUMPTION"),
        ),
        ("import math as m\nm.checked(true)", None),
    ] {
        let project = Project::new(&[("math.kes", library), ("main.kes", source)]);
        let result = project.check("main.kes");
        if let Some(code) = code {
            assert!(
                result.render_diagnostics().contains(code),
                "{source}\n{}",
                result.render_diagnostics()
            );
        } else {
            assert!(result.is_ok(), "{}", result.render_diagnostics());
        }
    }
}

#[test]
fn qualified_proof_calls_cannot_resolve_to_a_local_function_with_the_same_tail() {
    let project = Project::new(&[
        (
            "remote.kes",
            "public function identity(n : Nat) : Nat = Succ(n)",
        ),
        (
            "main.kes",
            "import remote as r\nfunction identity(n : Nat) : Nat = n\ntheorem bad() : r.identity(Zero) = Zero {rfl}",
        ),
    ]);
    let result = project.check("main.kes");
    assert!(
        result.render_diagnostics().contains("E_PROOF_MISMATCH"),
        "{}",
        result.render_diagnostics()
    );
}

#[test]
fn imported_trait_defaults_keep_concrete_receiver_field_projections() {
    let geometry = "public trait HasWidth { public function value(k : Int) : Int\npublic function measure(k : Int) : Int = this.value(k) }\npublic class WidthSquare { has HasWidth\npublic final width : Int\nnew(width : Int) { this.width = width }\npublic function value(k : Int) : Int = this.width + k }";
    let main = "import geometry.{WidthSquare}\ntheorem measure(s : WidthSquare, k : Int) : s.measure(k) = s.width + k { rfl }\ntheorem constructed() : WidthSquare(3).measure(4) = 7 { rfl }";
    let project = Project::new(&[("geometry.kes", geometry), ("main.kes", main)]);
    let result = project.check("main.kes");
    assert!(result.is_ok(), "{}", result.render_diagnostics());
}

#[test]
fn proofs_unfold_imported_functions_and_their_private_transitive_helpers() {
    let project = Project::new(&[
        (
            "base.kes",
            "private function hidden(n : Nat) : Nat = n\npublic function identity(n : Nat) : Nat = hidden(n)\npublic function add(a : Nat,b : Nat) : Nat = match a {Zero => b,Succ(k) => Succ(add(k,b)),}",
        ),
        (
            "middle.kes",
            "import base as b\npublic function keep(n : Nat) : Nat = b.identity(n)",
        ),
        (
            "main.kes",
            "import base.{identity}\nimport base as b\nimport middle\ntheorem a(n : Nat) : identity(n) = n {rfl}\ntheorem c(n : Nat) : middle.keep(n) = n {rfl}\ntheorem zeroRight(n : Nat) : b.add(n,Zero) = n {induction n {Zero => rfl,Succ(k) ih => congrArg(Succ,ih),}}\ntheorem lifted(n : Nat) : middle.keep(n) = middle.keep(b.add(n,Zero)) {congrArg(middle.keep,symm(zeroRight(n)))}",
        ),
    ]);
    let result = project.check("main.kes");
    assert!(result.is_ok(), "{}", result.render_diagnostics());
}

#[test]
fn imported_proof_functions_keep_file_identity_and_visibility() {
    let left = "public function value(n : Nat) : Nat = n\n";
    let right = "public function value(n : Nat) : Nat = Zero\nprivate function hidden(n : Nat) : Nat = n\nprivate enum Secret { HiddenCase(Nat) }\n";
    for (body, code) in [
        (
            "theorem t(n : Nat) : l.value(n) = n {rfl}\ntheorem s(n : Nat) : r.value(n) = Zero {rfl}",
            None,
        ),
        (
            "theorem t(n : Nat) : r.value(n) = n {rfl}",
            Some("E_PROOF_MISMATCH"),
        ),
        (
            "theorem t(n : Nat) : n = n {let h := congrArg(hidden,t(n))\nrfl}",
            Some("E_PROOF_RECURSION"),
        ),
        (
            "theorem identity(n : Nat) : n = n {rfl}\ntheorem t(n : Nat) : n = n {let h := congrArg(hidden,identity(n))\nrfl}",
            Some("E_PROOF_UNKNOWN"),
        ),
        (
            "theorem identity(n : Nat) : n = n {rfl}\ntheorem t(n : Nat) : n = n {let h := congrArg(r.hidden,identity(n))\nrfl}",
            Some("E_PROOF_UNKNOWN"),
        ),
        (
            "theorem t(n : Nat) : r.hidden(n) = n {rfl}",
            Some("E_PROOF_OPAQUE"),
        ),
        (
            "theorem t() : HiddenCase(Zero) = HiddenCase(Zero) {rfl}",
            Some("E_PROOF_OPAQUE"),
        ),
    ] {
        let main = format!("import left as l\nimport right as r\n{body}");
        let project = Project::new(&[
            ("left.kes", left),
            ("right.kes", right),
            ("main.kes", &main),
        ]);
        let result = project.check("main.kes");
        if let Some(code) = code {
            assert!(
                result.render_diagnostics().contains(code),
                "{body}\n{}",
                result.render_diagnostics()
            );
        } else {
            assert!(result.is_ok(), "{}", result.render_diagnostics());
        }
    }
}

#[test]
fn imported_functions_preserve_opacity_and_checked_evaluations_in_proofs() {
    let library = "public function loop(n : Nat) : Nat = loop(n)\npublic function noisy(n : Nat) : Nat with Effects {print(n)\nn}\npublic function discarded(n : Int) : Int {let unused := n + 1\n0}\npublic function guarded(n : Nat) : Nat or Error requires n > Zero {n}\n";
    for (statement, code) in [
        (
            "theorem t(n : Nat) : lib.loop(n) = lib.loop(n) {rfl}",
            Some("E_PROOF_OPAQUE"),
        ),
        (
            "theorem t(n : Nat) : lib.noisy(n) = lib.noisy(n) {rfl}",
            Some("E_PROOF_OPAQUE"),
        ),
        (
            "theorem t(n : Nat) : lib.guarded(n) = lib.guarded(n) {rfl}",
            Some("E_PROOF_PRECONDITION"),
        ),
        // Over the integers the discarded `n + 1` cannot fail (REVIEW 2.8).
        ("theorem t(n : Int) : lib.discarded(n) = 0 {rfl}", None),
        (
            "theorem t(n : Int) : lib.discarded(n) = 0 requires room : n < 9223372036854775807 {rfl}",
            None,
        ),
    ] {
        let source = format!("import library as lib\n{statement}");
        let project = Project::new(&[("library.kes", library), ("main.kes", &source)]);
        let result = project.check("main.kes");
        if let Some(code) = code {
            assert!(
                result.render_diagnostics().contains(code),
                "{statement}\n{}",
                result.render_diagnostics()
            );
        } else {
            assert!(result.is_ok(), "{}", result.render_diagnostics());
        }
    }
}

#[test]
fn public_theorems_are_cited_through_selective_and_namespace_imports() {
    let facts = "public function add(a : Nat,b : Nat) : Nat = match a {Zero => b,Succ(k) => Succ(add(k,b)),}\n@simp public theorem addZeroR(n : Nat) : add(n,Zero) = n {induction n {Zero => rfl,Succ(k) ih => congrArg(Succ,ih),}}\ntheorem secret(n : Nat) : n = n {rfl}\npackage theorem nearby(n : Nat) : n = n {rfl}\npublic theorem onlyZero(n : Nat) : n = Zero requires h : n = Zero {h}\n";
    let broken = "public theorem wrong(n : Nat) : n = Zero {rfl}\n";
    for (main, code) in [
        (
            "import facts.{add, addZeroR}\ntheorem t(n : Nat) : add(n,Zero) = n {addZeroR(n)}",
            None,
        ),
        (
            "import facts as f\ntheorem t(n : Nat) : f.add(n,Zero) = n {f.addZeroR(n)}\ntheorem u(n : Nat) : n = f.add(n,Zero) {symm(f.addZeroR(n))}",
            None,
        ),
        (
            "import facts.{add, addZeroR}\ntheorem t(a : Nat) : add(add(a,Zero),Zero) = a {simp [addZeroR]}",
            None,
        ),
        (
            "import facts as f\ntheorem t(a : Nat) : f.add(f.add(a,Zero),Zero) = a {simp [f.addZeroR]}",
            None,
        ),
        (
            "import facts.{nearby}\ntheorem t(n : Nat) : n = n {nearby(n)}",
            None,
        ),
        (
            "import facts.{onlyZero}\ntheorem t(n : Nat) : n = Zero requires h : n = Zero {onlyZero(n)}",
            None,
        ),
        (
            "import facts as f\nfunction g(n : Nat) : Nat requires f.onlyZero(n) {n}\nfunction h() : Nat = g(Zero)",
            None,
        ),
        (
            "import facts.{add, addZeroR}\npublic function g(n : Nat) : Nat requires addZeroR(n) {add(n,Zero)}",
            None,
        ),
        ("import facts.{secret}", Some("E_PRIVATE_IMPORT")),
        (
            "import facts as f\ntheorem t(n : Nat) : n = n {f.secret(n)}",
            Some("E_PROOF_UNKNOWN"),
        ),
        (
            "import facts.{onlyZero}\ntheorem t(n : Nat) : n = Zero {onlyZero(n)}",
            Some("E_PROOF_ASSUMPTION"),
        ),
        (
            "import facts as f\nfunction g(n : Nat) : Nat requires f.onlyZero(n) {n}\nfunction h() : Nat = g(Succ(Zero))",
            Some("E_PROOF_ASSUMPTION"),
        ),
        (
            "import facts.{addZeroR}\ntheorem addZeroR(n : Nat) : n = n {rfl}",
            Some("E_DUPLICATE_NAME"),
        ),
        (
            "import facts.{addZeroR}\nimport facts.{addZeroR}",
            Some("E_DUPLICATE_IMPORT"),
        ),
        (
            "import broken.{wrong}\ntheorem t(n : Nat) : n = Zero {wrong(n)}",
            Some("E_PROOF_UNCHECKED"),
        ),
    ] {
        let project = Project::new(&[
            ("facts.kes", facts),
            ("broken.kes", broken),
            ("main.kes", main),
        ]);
        let result = project.check("main.kes");
        match code {
            Some(code) => assert!(
                result.render_diagnostics().contains(code),
                "{main}\n{}",
                result.render_diagnostics()
            ),
            None => assert!(result.is_ok(), "{main}\n{}", result.render_diagnostics()),
        }
    }
}

#[test]
fn imported_proof_bounds_survive_aliases_citations_and_requirements() {
    let library = "public function identity[T : Equals](x : T) : T = x\npublic theorem same[T : Equals](x : T) : identity(x) = x {rfl}\npublic function guarded[T : Equals](x : T) : T requires same(x) = x\n";
    let project = Project::new(&[
        ("lib.kes", library),
        (
            "good.kes",
            "import lib.{identity,same,guarded}\nimport lib as l\ntheorem a[U : Equals](x : U) : identity(x) = x {same(x)}\ntheorem b[V : Equals](x : V) : l.guarded(x) = x {l.same(x)}\nfunction value[W : Equals](x : W) : W requires l.same(x) = x",
        ),
        (
            "badcall.kes",
            "import lib as l\ntheorem t(x : T) : l.identity(x) = x {rfl}",
        ),
        (
            "badcite.kes",
            "import lib.{same}\ntheorem t(x : T) : x = x {same(x)}",
        ),
        (
            "badrequire.kes",
            "import lib as l\nfunction value(x : T) : T requires l.same(x) = x",
        ),
    ]);
    let result = project.check("good.kes");
    assert!(result.is_ok(), "{}", result.render_diagnostics());
    for file in ["badcall.kes", "badcite.kes", "badrequire.kes"] {
        let result = project.check(file);
        assert!(
            result.render_diagnostics().contains("E_PROOF_BOUND"),
            "{file}: {}",
            result.render_diagnostics()
        );
    }
}

#[test]
fn proof_signatures_distinguish_identical_spans_in_different_files() {
    let imports = "import lib.{f}\n";
    let library = format!(
        "{}public function f(n : Int) : Int = n",
        " ".repeat(imports.len())
    );
    let main = format!(
        "{imports}public function f(n : Nat) : Nat = n\ntheorem same(n : Nat) : f(n) = n {{rfl}}"
    );
    let project = Project::new(&[("lib.kes", &library), ("main.kes", &main)]);
    let result = project.check("main.kes");
    assert!(result.is_ok(), "{}", result.render_diagnostics());
}

#[test]
fn imported_functions_refine_multiterm_lengths() {
    let project = Project::new(&[
        (
            "lib.kes",
            "public enum Vec[T](n : Nat) {Nil : Vec[T](0)\nCons(x : T,xs : Vec[T](n)) : Vec[T](n + 1)}\npublic function rebuild(move xs : Vec[T](n + m),left : Vec[T](n),right : Vec[T](m)) : Vec[T](n + m) = match xs {Nil => Nil,Cons(x,rest) => Cons(x,rest),}",
        ),
        (
            "main.kes",
            "import lib.{Vec,rebuild}\nlet left : Vec[Int](1) = Cons(1,Nil)\nlet right : Vec[Int](1) = Cons(2,Nil)\nlet result : Vec[Int](2) = rebuild(Cons(3,Cons(4,Nil)),left,right)",
        ),
    ]);
    let result = project.check("main.kes");
    assert!(result.is_ok(), "{}", result.render_diagnostics());
}

#[test]
fn imported_borrowed_superclass_results_keep_concrete_types() {
    let project = Project::new(&[
        (
            "lib.kes",
            "public class Figure {new() {}\npublic function area() : double = 0.0}\npublic class Square extends Figure {public final side : double\nnew(side : double) {this.side=side}\n@override public function area() : double = this.side * this.side}\npublic function choose(a : Figure,b : Figure,flag : Bool) : Figure = if flag then a else b",
        ),
        (
            "main.kes",
            "import lib as shapes\nfunction relay(a : shapes.Figure,b : shapes.Figure) : shapes.Figure = shapes.choose(a,b,true)\nlet s:=shapes.Square(3.0)\nlet f:=shapes.Figure()\nprint(relay(s,f).area())",
        ),
    ]);
    let checked = project.check("main.kes");
    assert!(checked.is_ok(), "{}", checked.render_diagnostics());
    kestrel::core::lower(&checked).unwrap();
}

#[test]
fn imported_defaults_retain_their_lifetime_and_definition_scope() {
    let project = Project::new(&[
        (
            "lib.kes",
            "public class Point {public final label : String\nnew(label : String) {this.label=label}}\npublic function identity(p : Point = Point(\"fresh\")) : Point = p\npublic const SEED : Int = 3\npublic function read(n : Int = SEED) : Int = n",
        ),
        (
            "good.kes",
            "import lib as l\nfunction text() : String = l.identity().label\nfunction caller(seed : Int) : Int = l.read()\nprint(text())",
        ),
        (
            "bad.kes",
            "import lib.{Point,identity}\nfunction make() : Point = identity()",
        ),
        (
            "recursive.kes",
            "public function recursive(n : Int = recursive()) : Int = n",
        ),
        ("cycle.kes", "import recursive.{recursive}\nrecursive()"),
    ]);
    let good = project.check("good.kes");
    assert!(good.is_ok(), "{}", good.render_diagnostics());
    kestrel::core::lower(&good).unwrap();
    assert!(
        project
            .check("bad.kes")
            .render_diagnostics()
            .contains("E_BORROW_TEMPORARY")
    );
    let recursive = project.check("cycle.kes");
    assert!(recursive.is_ok(), "{}", recursive.render_diagnostics());
    let errors = kestrel::core::lower(&recursive).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.diagnostic.code == "E_DEFAULT_RECURSION"),
        "{errors:?}"
    );
}

#[test]
fn theorems_in_another_package_need_public_visibility() {
    let project = Project::new(&[
        (
            "lib/facts.kes",
            "package theorem near(n : Nat) : n = n {rfl}\npublic theorem far(n : Nat) : n = n {rfl}\n",
        ),
        (
            "main.kes",
            "import lib.facts.{far}\ntheorem t(n : Nat) : n = n {far(n)}",
        ),
        ("other.kes", "import lib.facts.{near}"),
    ]);
    let result = project.check("main.kes");
    assert!(result.is_ok(), "{}", result.render_diagnostics());
    let result = project.check("other.kes");
    assert!(
        result.render_diagnostics().contains("E_PRIVATE_IMPORT"),
        "{}",
        result.render_diagnostics()
    );
}

#[test]
fn imported_functions_carry_the_identity_of_the_theorems_they_require() {
    let lib = "theorem zero(n : Nat) : n = Zero requires h : n = Zero {h}\npublic function onlyZero(n : Nat) : Nat requires zero(n) {n}\n";
    for (main, code) in [
        (
            "import lib.{onlyZero}\ntheorem t() : onlyZero(Zero) = Zero {rfl}",
            None,
        ),
        (
            "import lib as l\ntheorem t(n : Nat) : l.onlyZero(n) = n requires h : n = Zero {rfl}",
            None,
        ),
        (
            "import lib.{onlyZero}\ntheorem t(n : Nat) : onlyZero(n) = n {rfl}",
            Some("E_PROOF_ASSUMPTION"),
        ),
        (
            "import lib.{onlyZero}\ntheorem zero(n : Nat) : n = n {rfl}\ntheorem t(n : Nat) : onlyZero(n) = n {rfl}",
            Some("E_PROOF_ASSUMPTION"),
        ),
    ] {
        let project = Project::new(&[("lib.kes", lib), ("main.kes", main)]);
        let result = project.check("main.kes");
        match code {
            Some(code) => assert!(
                result.render_diagnostics().contains(code),
                "{main}\n{}",
                result.render_diagnostics()
            ),
            None => assert!(result.is_ok(), "{main}\n{}", result.render_diagnostics()),
        }
    }
}

#[test]
fn a_plain_simp_uses_imported_rules_only_when_imported_by_name() {
    let facts = "public function add(a : Nat,b : Nat) : Nat = match a {Zero => b,Succ(k) => Succ(add(k,b)),}\n@simp public theorem addZeroR(n : Nat) : add(n,Zero) = n {induction n {Zero => rfl,Succ(k) ih => congrArg(Succ,ih),}}\npublic theorem plain(n : Nat) : add(n,Zero) = n {addZeroR(n)}\n";
    for (main, code) in [
        (
            "import facts.{add, addZeroR}\ntheorem t(a : Nat) : add(add(a,Zero),Zero) = a {simp}",
            None,
        ),
        (
            "import facts as f\ntheorem t(a : Nat) : f.add(f.add(a,Zero),Zero) = a {simp [f.addZeroR]}",
            None,
        ),
        (
            "import facts as f\ntheorem t(a : Nat) : f.add(f.add(a,Zero),Zero) = a {simp}",
            Some("E_PROOF_MISMATCH"),
        ),
        // An unmarked theorem never joins a plain simp, even when imported by name.
        (
            "import facts.{add, plain}\ntheorem t(a : Nat) : add(add(a,Zero),Zero) = a {simp}",
            Some("E_PROOF_MISMATCH"),
        ),
    ] {
        let project = Project::new(&[("facts.kes", facts), ("main.kes", main)]);
        let result = project.check("main.kes");
        match code {
            Some(code) => assert!(
                result.render_diagnostics().contains(code),
                "{main}\n{}",
                result.render_diagnostics()
            ),
            None => assert!(result.is_ok(), "{main}\n{}", result.render_diagnostics()),
        }
    }
}

#[test]
fn unicode_exports_keep_their_spelling_through_namespaces_and_aliases() {
    let project = Project::new(&[
        (
            "names.kes",
            "public class ǅata[T] {public final n : T\nnew(move n : T) {this.n=n}}\npublic function sommé(n : Int) : Int = n + 1",
        ),
        (
            "main.kes",
            "import names as g\nimport names as ajouté\nlet 数 := g.ǅata(2)\nprint(ajouté.sommé(数.n))",
        ),
    ]);
    let checked = project.check("main.kes");
    assert!(checked.is_ok(), "{}", checked.render_diagnostics());
    kestrel::core::lower(&checked).unwrap();
}

#[test]
fn typed_default_arguments_work_in_import_cycles_from_every_entry() {
    let recursive = Project::new(&[
        (
            "a.kes",
            "import b.{other}\npublic function value(n : Int = other()) : Int = n",
        ),
        (
            "b.kes",
            "import a.{value}\npublic function other(n : Int = value()) : Int = n",
        ),
        ("main.kes", "import a.{value}\nvalue()"),
    ]);
    let checked = recursive.check("main.kes");
    assert!(checked.is_ok(), "{}", checked.render_diagnostics());
    let errors = kestrel::core::lower(&checked).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| e.diagnostic.code == "E_DEFAULT_RECURSION")
    );
    let borrowing = Project::new(&[
        (
            "a.kes",
            "import b.{other}\npublic class Point {new() {}}\npublic function identity(p : Point = Point()) : Point = p",
        ),
        (
            "b.kes",
            "import a.{Point, identity}\npublic function other() : Int = 1",
        ),
        (
            "main.kes",
            "import a.{Point, identity}\nfunction bad() : Point = identity()",
        ),
    ]);
    assert!(
        borrowing
            .check("main.kes")
            .render_diagnostics()
            .contains("E_BORROW_TEMPORARY")
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for entry in [
        "testdata/language/cycledefaulta.kes",
        "testdata/language/cycledefaultb.kes",
        "testdata/language/cycledefaults.kes",
    ] {
        let checked = check_path(root.join(entry), root).unwrap();
        assert!(checked.is_ok(), "{entry}: {}", checked.render_diagnostics());
        kestrel::core::lower(&checked).unwrap();
    }
    for (default, expected) in [
        ("missing", "E_UNKNOWN_NAME"),
        ("true", "E_TYPE"),
        ("effect()", "E_EFFECT_REQUIRED"),
        ("parseInt(\"bad\")?", "E_DEFAULT_CONTROL"),
    ] {
        let source = format!(
            "import b.{{other}}\nfunction effect() : Int with Effects = 1\npublic function bad(n : Int = {default}) : Int = n"
        );
        let project = Project::new(&[
            ("a.kes", &source),
            (
                "b.kes",
                "import a.{bad}\npublic function other() : Int = bad(1)",
            ),
            ("main.kes", "import a.{bad}\nbad()"),
        ]);
        for entry in ["a.kes", "b.kes", "main.kes"] {
            let checked = project.check(entry);
            assert!(
                checked.render_diagnostics().contains(expected),
                "{entry}, {default}: {}",
                checked.render_diagnostics()
            );
        }
    }
}

#[test]
fn runtime_contracts_in_import_cycles_keep_guards_without_header_proofs() {
    let project = Project::new(&[
        (
            "a.kes",
            "import b.{other}\npublic function checked(n : Int) : Int or Error requires n > 0 = n",
        ),
        (
            "b.kes",
            "import a.{checked}\npublic function other(n : Int) : Int or Error = checked(n)\npublic function zero() : Int or Error = checked(0)",
        ),
        ("main.kes", "import b.{other, zero}\nother(3)?\nzero()?"),
    ]);
    for entry in ["a.kes", "b.kes", "main.kes"] {
        let checked = project.check(entry);
        assert!(checked.is_ok(), "{entry}: {}", checked.render_diagnostics());
        kestrel::core::lower(&checked).unwrap();
        let b = checked
            .modules
            .iter()
            .find(|m| m.parsed.source.name.ends_with("/b.kes"))
            .unwrap();
        assert!(
            b.checked.contract_checks.is_empty(),
            "Header calls must not acquire a proved status: {:?}",
            b.checked.contract_checks
        );
    }
    fs::write(
        project.0.join("main.kes"),
        "import a.{checked}\nchecked(0)?",
    )
    .unwrap();
    assert!(
        project
            .check("main.kes")
            .render_diagnostics()
            .contains("E_CONTRACT_VIOLATED")
    );
    for (condition, result, expected) in [
        ("n > 0", "Int", "E_CONTRACT_ERROR_TYPE"),
        ("n + 1", "Int or Error", "E_TYPE"),
    ] {
        let a = format!(
            "import b.{{other}}\npublic function checked(n : Int) : {result} requires {condition} = n"
        );
        let project = Project::new(&[
            ("a.kes", &a),
            (
                "b.kes",
                "import a.{checked}\npublic function other() : Int = 1",
            ),
        ]);
        for entry in ["a.kes", "b.kes"] {
            assert!(project.check(entry).render_diagnostics().contains(expected));
        }
    }
}

#[test]
fn dependent_defaults_use_imported_parameter_names_and_monomorphic_values() {
    let project = Project::new(&[
        (
            "lib.kes",
            "public function area(width : Int,height : Int=width) : Int=width*height\npublic function label[T : Text](value : T,text : String=value.text()) : String=text",
        ),
        (
            "main.kes",
            "import lib as l\nfunction call(width : Int) : Int=l.area(width)\nprint(call(3))\nprint(l.label(4))\nlet later:=l.area(_)\nprint(later(5))",
        ),
    ]);
    let checked = project.check("main.kes");
    assert!(checked.is_ok(), "{}", checked.render_diagnostics());
    kestrel::core::lower(&checked).unwrap();
}

#[test]
fn imported_function_helpers_preserve_callback_capture_dependencies() {
    let project = Project::new(&[
        (
            "lib.kes",
            "public function keep(f : Int -> String) : Int -> String=f\npublic function relay(f : Int -> String) : Int -> String=keep(f)",
        ),
        (
            "bad.kes",
            "import lib.{relay}\nclass Point {public final label : String\nnew(label : String) {this.label=label}}\nfunction bad() : Int -> String {let p:=Point(\"heap\"+\" label\")\nrelay((n : Int) => p.label)}",
        ),
        (
            "good.kes",
            "import lib.{relay}\nclass Point {public final label : String\nnew(label : String) {this.label=label}}\nfunction good() : Int -> String {let p:=Point(\"heap\"+\" label\")\nrelay(move (n : Int) => p.label)}",
        ),
    ]);
    assert!(
        project
            .check("bad.kes")
            .render_diagnostics()
            .contains("E_LAMBDA_ESCAPE")
    );
    let checked = project.check("good.kes");
    assert!(checked.is_ok(), "{}", checked.render_diagnostics());
}

#[test]
fn imported_mutable_trait_views_cannot_hide_script_state_changes() {
    let project = Project::new(&[
        (
            "values.kes",
            "public trait Mutable {mut function bump() : void}\npublic alias View=Box[dyn Mutable]\npublic class Tally {has Mutable\nvar n : Int\npublic new() {this.n=0}\npublic mut function bump() : void {this.n=this.n+1}}",
        ),
        (
            "bad.kes",
            "import values.{View,Tally}\nlet value : View=box(Tally())\nfunction change() : void=value.bump()",
        ),
        (
            "good.kes",
            "import values.{View,Tally}\nlet value : View=box(Tally())\nfunction change(mut value : View) : void=value.bump()\nchange(value)",
        ),
    ]);
    assert!(
        project
            .check("bad.kes")
            .render_diagnostics()
            .contains("E_SCRIPT_MUTABLE_CAPTURE")
    );
    let checked = project.check("good.kes");
    assert!(checked.is_ok(), "{}", checked.render_diagnostics());
}

#[test]
fn imported_array_factories_preserve_borrows_in_the_element_fields() {
    let project = Project::new(&[
        (
            "values.kes",
            "public class Point {public final label : String\npublic new(label : String) {this.label=label}}\npublic class Panel {public final point : &Point\nnew(point : Point) {this.point=point}}\npublic function panels(p : Point) : Array[Panel]=Array.generate(2,i => Panel(p))",
        ),
        (
            "bad.kes",
            "import values.{Point,Panel,panels}\nfunction bad() : Array[Panel] {let p:=Point(\"heap\")\npanels(p)}",
        ),
        (
            "good.kes",
            "import values.{Point,panels}\nlet p:=Point(\"kept\")\nlet values:=panels(p)\nvalues[0].point.label",
        ),
    ]);
    assert!(
        project
            .check("bad.kes")
            .render_diagnostics()
            .contains("E_BORROW_ESCAPE")
    );
    let checked = project.check("good.kes");
    assert!(checked.is_ok(), "{}", checked.render_diagnostics());
}

#[test]
fn imported_constructors_check_early_this_exposure_without_being_called() {
    let project = Project::new(&[
        (
            "points.kes",
            "public class Point {public final label : String\nnew() with Effects {print(this.read())\nthis.label=\"ready\"}\npublic function read() : String=this.label}",
        ),
        ("app.kes", "import points as p\nlet n:=1"),
    ]);
    assert!(
        project
            .check("app.kes")
            .render_diagnostics()
            .contains("E_THIS_UNINITIALIZED")
    );
}

#[test]
fn static_final_fields_cross_modules_with_their_visibility() {
    let project = Project::new(&[
        (
            "limits.kes",
            "public class Limits {\n    public static final size : Int = 3\n    private static final secret : Int = 4\n    new() {}\n}",
        ),
        ("main.kes", "import limits.{Limits}\nprint(Limits.size)"),
    ]);
    let result = project.check("main.kes");
    assert!(result.is_ok(), "{}", result.render_diagnostics());
    let project = Project::new(&[
        (
            "limits.kes",
            "public class Limits {\n    private static final secret : Int = 4\n    new() {}\n}",
        ),
        ("main.kes", "import limits.{Limits}\nLimits.secret"),
    ]);
    let result = project.check("main.kes");
    let rendered = result.render_diagnostics();
    assert!(rendered.contains("E_MEMBER_PRIVATE"), "{rendered}");
}

#[test]
fn constants_and_their_uses_work_inside_import_cycles() {
    let project = Project::new(&[
        (
            "alpha.kes",
            "import beta.{B_VALUE}\npublic const A_VALUE : Int = B_VALUE + 1\npublic function withDefault(n : Int = A_VALUE) : Int = n",
        ),
        (
            "beta.kes",
            "import alpha.{A_VALUE, withDefault}\npublic const B_VALUE : Int = 1\npublic function check() : Int = withDefault() + A_VALUE",
        ),
        (
            "main.kes",
            "import alpha.{A_VALUE, withDefault}\nimport beta.{B_VALUE, check}\nprint(A_VALUE)\nprint(B_VALUE)\nprint(withDefault())\nprint(check())",
        ),
    ]);
    let result = project.check("main.kes");
    assert!(result.is_ok(), "{}", result.render_diagnostics());
}

#[test]
fn text_checks_see_the_prelude_and_math_names() {
    let result = check_source(
        "buffer.kes",
        "let s := Stack[Int]()\nlet q := Queue[Int]()\nprint(max(1, 2))\nprint(floor(2.5))\n",
    );
    assert!(result.checked.is_ok(), "{:#?}", result.checked.diagnostics);
    let own = check_source(
        "buffer.kes",
        "function max(a : Int, b : Int) : Int = a\nprint(max(1, 2))\n",
    );
    assert!(own.checked.is_ok(), "{:#?}", own.checked.diagnostics);
    let locals = check_source(
        "buffer.kes",
        "function local() : Int {\n    let max := 3\n    return max\n}\nlet min : Int = 4\nprint(local() + min)\n",
    );
    assert!(locals.checked.is_ok(), "{:#?}", locals.checked.diagnostics);
}
