use kestrel::{
    modules::{check_path, load_packages},
    native::TemporaryDirectory,
    testing::{self, Options},
};
use std::{fs, path::Path, process::Command};

/// Writes `files` under a fresh folder; each entry is a relative path and its text.
fn workspace(files: &[(&str, &str)]) -> TemporaryDirectory {
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    for (path, text) in files {
        let path = dir.path().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    dir
}

fn report(package: &Path, filter: Option<&str>) -> (bool, String) {
    let mut out = vec![];
    let passed = testing::run(
        package,
        &Options {
            timeout: None,
            filter: filter.map(str::to_owned),
            jobs: 4,
            libraries: vec![],
        },
        &mut out,
    )
    .unwrap();
    (passed, String::from_utf8(out).unwrap())
}

const MATRIX: (&str, &str) = (
    "matrix/ops.kes",
    "public function scale(x : Int, by : Int) : Int = x * by\n",
);

#[test]
fn failed_postconditions_fail_only_the_current_test() {
    let dir = workspace(&[
        ("kestrel.project", "package promises\n"),
        (
            "tests/promises.kes",
            "@test\nfunction broken() : void ensures false {}\n@test\nfunction fine() : void ensures true {}\n",
        ),
    ]);
    let (passed, text) = report(dir.path(), None);
    assert!(!passed, "{text}");
    assert!(text.contains("FAIL  tests/promises.kes  broken"), "{text}");
    assert!(text.contains("ok    tests/promises.kes  fine"), "{text}");
    assert!(text.contains("Postcondition failed: false"), "{text}");
    assert!(text.contains("1 passed, 1 failed"), "{text}");
}
const SHAPES: &str = r#"import matrix.ops.{scale}
public class Square { has Equals, Text
    public final side : Int
    public new(side : Int) { this.side = side }
    public function equals(other : Square) : Bool = this.side == other.side
    public function text() : String = "Square(" + this.side.text() + ")"
}
public function area(s : Square) : Int = scale(s.side, s.side)
package function corners() : Int = 4
private function hidden() : Int = 1
"#;

fn geometry(tests: &str) -> TemporaryDirectory {
    workspace(&[
        ("matrix/kestrel.project", "package matrix\n"),
        MATRIX,
        (
            "geometry/kestrel.project",
            "package geometry\ndepends matrix\n",
        ),
        ("geometry/shapes.kes", SHAPES),
        ("geometry/tests/shapes.kes", tests),
    ])
}

#[test]
fn tests_run_in_their_own_processes_and_report_every_failure() {
    let dir = geometry(
        r#"import shapes.{Square, area, corners}
import std.test.{assertEq}
@test
function areaOfTwo() : void { assertEq(area(Square(2)), 4) }
@test
function fourCorners() : void { assertEq(corners(), 4) }
@test
function squaresDiffer() : void { assertEq(Square(2), Square(3)) }
@test
function parses() : void or Error {
    let n := parseInt("nope")?
    assertEq(n, 1)
}
@test
function overflows() : void {
    let big := 9223372036854775807
    assertEq(big + 1, 0)
}
@test
function listsCompare() : void { assertEq([1, 2], [1, 2]) }
"#,
    );
    let (passed, text) = report(&dir.path().join("geometry"), None);
    assert!(!passed);
    for line in [
        "running 6 tests from 1 file",
        "ok    tests/shapes.kes  areaOfTwo",
        "ok    tests/shapes.kes  fourCorners",
        "FAIL  tests/shapes.kes  squaresDiffer",
        "FAIL  tests/shapes.kes  parses",
        "FAIL  tests/shapes.kes  overflows",
        "ok    tests/shapes.kes  listsCompare",
        "  left: Square(2)\n  right: Square(3)",
        "test returned an Error: RuntimeError.Parse(\"Cannot parse Int: nope\")",
        "runtime error: Int addition overflow",
        "3 passed, 3 failed",
    ] {
        assert!(text.contains(line), "missing {line:?} in\n{text}");
    }
    let (passed, text) = report(&dir.path().join("geometry"), Some("Corners"));
    assert!(passed, "{text}");
    assert!(text.contains("1 passed, 0 failed"), "{text}");
}

#[test]
fn the_cli_exits_with_one_when_a_test_fails() {
    let dir = geometry(
        "import std.test.{assertEq}\n@test\nfunction good() : void { assertEq(1, 1) }\n@test\nfunction bad() : void { assertEq(1, 2) }\n",
    );
    let run = |filter: &str| {
        Command::new(env!("CARGO_BIN_EXE_kestrel"))
            .args(["test", "--filter", filter, "--jobs", "2"])
            .current_dir(dir.path().join("geometry"))
            .output()
            .unwrap()
    };
    assert_eq!(run("good").status.code(), Some(0));
    let failed = run("bad");
    assert_eq!(failed.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&failed.stdout).contains("FAIL  tests/shapes.kes  bad"));
}

#[test]
fn tests_reach_package_members_of_the_folder_they_mirror_but_not_private_ones() {
    let dir = geometry("import shapes.{hidden}\n");
    let project = check_path(
        dir.path().join("geometry/tests/shapes.kes"),
        dir.path().join("geometry"),
    )
    .unwrap();
    assert!(project.render_diagnostics().contains("E_PRIVATE_IMPORT"));
    // A file outside the mirrored folder gets no package access.
    let dir = workspace(&[
        ("app/kestrel.project", "package app\n"),
        (
            "app/geometry/shapes.kes",
            "package function corners() : Int = 4\n",
        ),
        ("app/other/use.kes", "import geometry.shapes.{corners}\n"),
        (
            "app/tests/geometry/shapes.kes",
            "import geometry.shapes.{corners}\n",
        ),
    ]);
    let root = dir.path().join("app");
    let outside = check_path(root.join("other/use.kes"), &root).unwrap();
    assert!(outside.render_diagnostics().contains("E_PRIVATE_IMPORT"));
    let mirrored = check_path(root.join("tests/geometry/shapes.kes"), &root).unwrap();
    assert!(mirrored.is_ok(), "{}", mirrored.render_diagnostics());
}

#[test]
fn dependencies_are_sibling_folders_named_in_the_project_file() {
    let shapes = |project: &str| {
        let dir = workspace(&[
            ("matrix/kestrel.project", "package matrix\n"),
            MATRIX,
            ("geometry/kestrel.project", project),
            ("geometry/shapes.kes", SHAPES),
        ]);
        let result = check_path(
            dir.path().join("geometry/shapes.kes"),
            dir.path().join("geometry"),
        );
        (dir, result)
    };
    let (_dir, declared) = shapes("package geometry\ndepends matrix\n");
    assert!(declared.unwrap().is_ok());
    let (_dir, undeclared) = shapes("package geometry\n");
    assert!(
        undeclared
            .unwrap()
            .render_diagnostics()
            .contains("E_UNDECLARED_DEPENDENCY")
    );
    let (_dir, missing) = shapes("package geometry\ndepends matrix\ndepends linear\n");
    assert!(
        missing
            .unwrap_err()
            .to_string()
            .contains("has no kestrel.project")
    );

    let renamed = workspace(&[
        ("matrix/kestrel.project", "package grid\n"),
        (
            "geometry/kestrel.project",
            "package geometry\ndepends matrix\n",
        ),
    ]);
    let error = load_packages(&renamed.path().join("geometry")).unwrap_err();
    assert!(error.contains("must match"), "{error}");
    let cycle = workspace(&[
        (
            "matrix/kestrel.project",
            "package matrix\ndepends geometry\n",
        ),
        (
            "geometry/kestrel.project",
            "package geometry\ndepends matrix\n",
        ),
    ]);
    let error = load_packages(&cycle.path().join("geometry")).unwrap_err();
    assert!(error.contains("geometry -> matrix -> geometry"), "{error}");
    let shadow = workspace(&[
        ("matrix/kestrel.project", "package matrix\n"),
        MATRIX,
        (
            "geometry/kestrel.project",
            "package geometry\ndepends matrix\n",
        ),
        (
            "geometry/matrix/ops.kes",
            "public function scale(x : Int, by : Int) : Int = x\n",
        ),
        ("geometry/shapes.kes", SHAPES),
    ]);
    let project = check_path(
        shadow.path().join("geometry/shapes.kes"),
        shadow.path().join("geometry"),
    )
    .unwrap();
    assert!(project.render_diagnostics().contains("E_DEPENDENCY_SHADOW"));
}

#[test]
fn tests_have_a_fixed_shape_and_a_fixed_place() {
    let dir =
        geometry("@test\nfunction takes(x : Int) : void {}\n@test\nfunction gives() : Int = 1\n");
    let project = check_path(
        dir.path().join("geometry/tests/shapes.kes"),
        dir.path().join("geometry"),
    )
    .unwrap();
    assert_eq!(
        project
            .render_diagnostics()
            .matches("E_TEST_SIGNATURE")
            .count(),
        2
    );
    let dir = workspace(&[
        ("app/kestrel.project", "package app\n"),
        ("app/code.kes", "@test\nfunction misplaced() : void {}\n"),
        ("app/tests/code.kes", "@test\nfunction fine() : void {}\n"),
        ("app/vendor/kestrel.project", "package vendor\n"),
        (
            "app/vendor/own.kes",
            "@test\nfunction separate() : void {}\n",
        ),
    ]);
    let (passed, text) = report(&dir.path().join("app"), None);
    assert!(!passed);
    assert!(text.contains("FAIL  code.kes  (misplaced @test)"), "{text}");
    assert!(text.contains("E_TEST_LOCATION"), "{text}");
    assert!(text.contains("ok    tests/code.kes  fine"), "{text}");
    assert!(!text.contains("vendor"), "{text}");
}

#[test]
fn the_repository_runs_its_own_example_test() {
    let (passed, text) = report(Path::new(env!("CARGO_MANIFEST_DIR")), None);
    assert!(passed, "{text}");
    assert!(
        text.contains("ok    tests/examples/rules.kes  biggestIsSymmetric"),
        "{text}"
    );
}

/// With KESTREL_SANITIZE set, a test program is rebuilt with AddressSanitizer and
/// UndefinedBehaviorSanitizer, and each of its tests must fail or pass cleanly.
#[test]
fn test_programs_are_clean_under_the_sanitizers() {
    if std::env::var_os("KESTREL_SANITIZE").is_none() {
        return;
    }
    let dir = geometry(
        r#"import shapes.{Square, area}
import std.test.{assertEq}
@test
function passes() : void { assertEq([Square(1)], [Square(1)]) }
@test
function differs() : void { assertEq(Map("a" => Square(1)), Map("a" => Square(2))) }
@test
function fails() : void or Error {
    let n := parseInt("x")?
    assertEq(n, area(Square(n)))
}
"#,
    );
    let root = dir.path().join("geometry");
    let project = check_path(root.join("tests/shapes.kes"), &root).unwrap();
    assert!(project.is_ok(), "{}", project.render_diagnostics());
    let module = project.modules.last().unwrap();
    let spans = module
        .parsed
        .root
        .children
        .iter()
        .filter(|node| kestrel::modules::is_test(node, &module.parsed))
        .map(|node| node.span)
        .collect::<Vec<_>>();
    let program = kestrel::core::lower_tests(&project, &spans).unwrap();
    let source = dir.path().join("tests.c");
    let binary = kestrel::native::executable_path(&dir.path().join("tests"));
    fs::write(&source, kestrel::native::emit_c(&program).unwrap()).unwrap();
    let built = Command::new("clang")
        .args(["-std=c11", "-O1", "-g", "-fsanitize=address,undefined"])
        .args(kestrel::native::thread_flags())
        .args(["-fno-sanitize-recover=undefined", "-Wno-unused-function"])
        .arg(&source)
        .args(kestrel::native::system_libraries())
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    for (index, expected) in [(0, true), (1, false), (2, false)] {
        let run = Command::new(&binary)
            .arg(index.to_string())
            .env("ASAN_OPTIONS", "detect_leaks=0")
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&run.stderr);
        assert_eq!(run.status.success(), expected, "test {index}: {stderr}");
        assert!(!stderr.contains("Sanitizer"), "test {index}: {stderr}");
        assert!(
            !stderr.contains("runtime error: load"),
            "test {index}: {stderr}"
        );
    }
}

#[cfg(unix)]
#[test]
fn timeouts_stop_a_case_and_its_group_without_blocking_verbose_or_later_tests() {
    use std::time::Duration;
    let dir = workspace(&[
        ("kestrel.project", "package timeout\n"),
        ("writer.h", "void writer_child(const char *path);\n"),
        (
            "writer.c",
            r#"
#define _POSIX_C_SOURCE 200809L
#include <unistd.h>
#include <fcntl.h>
#include <time.h>
void writer_child(const char *path) {
    int ready[2];
    if (pipe(ready) != 0) return;
    if (fork() == 0) {
        close(ready[0]);
        int fd = open(path,O_CREAT|O_WRONLY|O_APPEND,0600);
        if (fd>=0) write(fd,"x",1);
        write(ready[1],"x",1); close(ready[1]);
        struct timespec pause = {0,10000000};
        for (int i=0;i<200;++i) { if (fd>=0) write(fd,"x",1); nanosleep(&pause,0); }
        if (fd>=0) close(fd);
        _exit(0);
    }
    close(ready[1]); char byte; read(ready[0],&byte,1); close(ready[0]);
}
"#,
        ),
    ]);
    let log = dir.path().join("child.log");
    let early_log = dir.path().join("early-child.log");
    let source = format!(
        "import c \"writer.h\"\nimport std.test.{{assertEq}}\n@test function hangs() : void with Effects {{unsafe {{writer_child({:?})}}\nwhile true {{}}}}\n@test function verbose() : void with Effects {{for (let i : Int = 0; i < 1000; i = i + 1) {{print(\"{}\")}}}}\n@test function stillRuns() : void {{assertEq(1,1)}}\n@test function returnsBeforeChild() : void with Effects {{unsafe {{writer_child({:?})}}}}\n",
        log.to_str().unwrap(),
        "x".repeat(2000),
        early_log.to_str().unwrap()
    );
    fs::create_dir_all(dir.path().join("tests")).unwrap();
    fs::write(dir.path().join("tests/timeouts.kes"), source).unwrap();
    let mut output = vec![];
    let passed = testing::run(
        dir.path(),
        &Options {
            filter: None,
            jobs: 2,
            libraries: vec![],
            timeout: Some(Duration::from_millis(500)),
        },
        &mut output,
    )
    .unwrap();
    let report = String::from_utf8(output).unwrap();
    assert!(!passed, "{report}");
    assert!(report.contains("0.5 second timeout"), "{report}");
    assert!(report.contains("3 passed, 1 failed"), "{report}");
    assert!(
        report.contains("ok    tests/timeouts.kes  verbose"),
        "{report}"
    );
    assert!(
        report.contains("ok    tests/timeouts.kes  stillRuns"),
        "{report}"
    );
    std::thread::sleep(Duration::from_millis(50));
    let before = fs::metadata(&log).unwrap().len();
    let early_before = fs::metadata(&early_log).unwrap().len();
    assert!(before > 0);
    assert!(early_before > 0);
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(
        fs::metadata(log).unwrap().len(),
        before,
        "A timed-out test left its writer child running."
    );
    assert_eq!(
        fs::metadata(&early_log).unwrap().len(),
        early_before,
        "A completed test left its writer child running."
    );
    let cli = Command::new(env!("CARGO_BIN_EXE_kestrel"))
        .args(["test", "--filter", "stillRuns", "--timeout", "0.5"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(cli.status.success(), "{cli:?}");
    assert!(String::from_utf8_lossy(&cli.stdout).contains("1 passed, 0 failed"));
    // Without a deadline the runner still ends the case when the test returns. Left
    // alone, the writer would add 201 bytes over two seconds.
    let (passed, untimed) = crate::report(dir.path(), Some("returnsBeforeChild"));
    assert!(passed, "{untimed}");
    std::thread::sleep(Duration::from_millis(50));
    let grown = fs::metadata(&early_log).unwrap().len() - early_before;
    assert!(grown > 0 && grown < 201, "The writer added {grown} bytes.");
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(
        fs::metadata(&early_log).unwrap().len() - early_before,
        grown,
        "An untimed test left its writer child running."
    );
}

#[test]
fn returned_error_reports_include_bigint_payloads() {
    let dir = workspace(&[
        ("kestrel.project", "package numbers\n"),
        (
            "tests/numbers.kes",
            "enum NumberError extends Error {Large(BigInt)}\n@test\nfunction broken() : void or Error=NumberError.Large(1000000000000000000000)\n",
        ),
    ]);
    let (passed, text) = report(dir.path(), None);
    assert!(!passed, "{text}");
    assert!(
        text.contains("NumberError.Large(1000000000000000000000)"),
        "{text}"
    );
    assert!(text.contains("0 passed, 1 failed"), "{text}");
}
