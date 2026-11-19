use std::{
    io::Write,
    process::{Command, Output, Stdio},
};

fn stdin(args: &[&str], source: &str) -> Output {
    let mut process = Command::new(env!("CARGO_BIN_EXE_kestrel"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    process
        .stdin
        .take()
        .unwrap()
        .write_all(source.as_bytes())
        .unwrap();
    process.wait_with_output().unwrap()
}

#[test]
fn parse_from_standard_input() {
    let result = stdin(&["parse", "-"], "let total := 3");
    assert!(result.status.success());
    assert_eq!(String::from_utf8(result.stdout).unwrap(), "-: parses\n");
    assert!(result.stderr.is_empty());
}

#[test]
fn smt_without_the_selected_solver_fails_closed() {
    let dir = kestrel::native::TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    let file = dir.path().join("proof.kes");
    std::fs::write(&file, "theorem t(n : Int) : n = n {smt}").unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_kestrel"))
        .arg("check")
        .arg("--root")
        .arg(dir.path())
        .arg(file)
        .env("KESTREL_Z3", dir.path().join("missing-solver"))
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("E_PROOF_SOLVER"));
}

#[test]
fn tree_output_includes_original_offsets() {
    let result = stdin(&["parse", "--tree", "-"], "f\"{3}\"");
    assert!(result.status.success());
    let output = String::from_utf8(result.stdout).unwrap();
    assert!(output.contains("FormatString@0..6") && output.contains("Literal@3..4"));
}

#[test]
fn syntax_errors_fail_with_locations_and_hints() {
    let result = stdin(&["parse", "-"], "scaled(scale = 3.0, 2.0)");
    assert_eq!(result.status.code(), Some(1));
    let output = String::from_utf8(result.stderr).unwrap();
    assert!(output.contains("-:1:21: error[E_ARGUMENT_ORDER]") && output.contains("hint:"));
}

#[test]
fn project_mode_parses_project_contents() {
    let result = stdin(
        &["parse", "--project", "-"],
        "package geometry\ndepends matrix\n",
    );
    assert!(result.status.success());
}

#[test]
fn token_command_does_not_run_the_parser() {
    let result = stdin(&["tokens", "-"], "function ( } // incomplete grammar");
    assert!(result.status.success());
    assert!(
        String::from_utf8(result.stdout)
            .unwrap()
            .contains("Comment")
    );
    assert!(result.stderr.is_empty());
}

#[test]
fn unknown_commands_report_usage_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_kestrel"))
        .arg("unknown")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("Unknown command")
    );
}

#[test]
fn both_original_files_are_accepted_by_the_executable() {
    let output = Command::new(env!("CARGO_BIN_EXE_kestrel"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args([
            "parse",
            "testdata/language/rules.kes",
            "tests/examples/rules.kes",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
}

#[test]
fn notebook_check_allows_top_level_propagation() {
    let source = "let n := parseInt(\"3\")?\n";
    // A script's top-level `?` reports the Error and exits with status 1 (REVIEW 1.2).
    let script = stdin(&["check", "-"], source);
    assert!(
        script.status.success(),
        "{}",
        String::from_utf8_lossy(&script.stderr)
    );
    let cell = stdin(&["check", "--notebook", "-"], source);
    assert!(
        cell.status.success(),
        "{}",
        String::from_utf8_lossy(&cell.stderr)
    );
}

#[test]
fn check_from_standard_input_reports_type_errors() {
    let good = stdin(&["check", "-"], "function twice(n : Int) : Int = n * 2");
    assert!(good.status.success());
    let bad = stdin(&["check", "-"], "let x : Int = true");
    assert_eq!(bad.status.code(), Some(1));
    assert!(String::from_utf8(bad.stderr).unwrap().contains("E_TYPE"));
}

#[test]
fn check_resolves_module_imports_from_the_repository_root() {
    let output = Command::new(env!("CARGO_BIN_EXE_kestrel"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(["check", "--root", ".", "tests/examples/rules.kes"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn native_run_from_standard_input_reports_only_program_output() {
    let result = stdin(&["run", "-"], "print(f\"answer: {6 * 7}\")");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(String::from_utf8(result.stdout).unwrap(), "answer: 42\n");
}
#[test]
fn native_build_saves_a_standalone_executable_and_reviewable_c() {
    let dir = kestrel::native::TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    let binary = dir.path().join("compiled program");
    let emitted = dir.path().join("generated.c");
    let result = stdin(
        &[
            "build",
            "-o",
            binary.to_str().unwrap(),
            "--emit-c",
            emitted.to_str().unwrap(),
            "-",
        ],
        "print(40 + 2)",
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(std::fs::read_to_string(emitted).unwrap().contains("k_fn_0"));
    let output = Command::new(&binary).output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"42\n");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
}
#[test]
fn invalid_native_build_leaves_existing_outputs_intact() {
    let dir = kestrel::native::TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    let binary = dir.path().join("program");
    std::fs::write(&binary, "previous output").unwrap();
    let output = stdin(
        &["build", "-o", binary.to_str().unwrap(), "-"],
        "print(1 + true)",
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8(output.stderr).unwrap().contains("error["));
    assert_eq!(std::fs::read_to_string(binary).unwrap(), "previous output");
}
#[test]
fn native_output_cannot_overwrite_any_input_module() {
    let dir = kestrel::native::TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    let source = dir.path().join("app.kes");
    std::fs::write(&source, "print(42)").unwrap();
    for option in ["-o", "--emit-c"] {
        let output = Command::new(env!("CARGO_BIN_EXE_kestrel"))
            .args(["build", "--root"])
            .arg(dir.path())
            .arg(option)
            .arg(&source)
            .arg(&source)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("overwrite an input")
        );
        assert_eq!(std::fs::read_to_string(&source).unwrap(), "print(42)");
    }
}

#[test]
fn script_arguments_after_the_separator_are_forwarded_verbatim() {
    let output = stdin(
        &["run", "-", "--", "one two", "λ", "", "--root", "--"],
        "import std.process.{args}\nfor (arg in args) { print(\"<\" + arg + \">\") }",
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "<one two>\n<λ>\n<>\n<--root>\n<-->\n"
    );
    assert!(output.stderr.is_empty());
}

#[cfg(unix)]
#[test]
fn invalid_utf8_arguments_report_a_plain_error() {
    use std::os::unix::ffi::OsStringExt;
    let output = Command::new(env!("CARGO_BIN_EXE_kestrel"))
        .args(["run", "-", "--"])
        .arg(std::ffi::OsString::from_vec(vec![0xff]))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("not valid UTF-8")
    );
}

#[test]
fn test_timeouts_require_positive_finite_seconds() {
    for value in ["0", "-1", "NaN", "inf", "1e999", "1e-999", "later"] {
        let output = Command::new(env!("CARGO_BIN_EXE_kestrel"))
            .args(["test", "--timeout", value])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("timeout"),
            "{value}: {:?}",
            output.stderr
        );
    }
    let output = Command::new(env!("CARGO_BIN_EXE_kestrel"))
        .args(["test", "--timeout"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("needs a number of seconds"));
}

#[test]
fn stdin_path_resolves_project_imports_for_unsaved_buffers() {
    let dir = kestrel::native::TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    let root = dir.path().to_string_lossy().into_owned();
    std::fs::write(dir.path().join("kestrel.project"), "package demo\n").unwrap();
    std::fs::write(
        dir.path().join("helpers.kes"),
        "public function one() : Int = 1\n",
    )
    .unwrap();
    let main = dir.path().join("main.kes");
    std::fs::write(&main, "import helpers\nlet n : Int = helpers.one()\n").unwrap();
    let main = main.to_string_lossy().into_owned();
    let good = stdin(
        &["check", "--root", &root, "--stdin-path", &main, "-"],
        "import helpers\nlet n : Int = helpers.one()\n",
    );
    assert!(
        good.status.success(),
        "{}",
        String::from_utf8_lossy(&good.stderr)
    );
    assert_eq!(
        String::from_utf8(good.stdout).unwrap(),
        "-: checked names and supported types\n"
    );
    let bad = stdin(
        &["check", "--root", &root, "--stdin-path", &main, "-"],
        "import helpers\nlet n : Int = \"no\"\n",
    );
    assert!(!bad.status.success());
    let errors = String::from_utf8(bad.stderr).unwrap();
    assert!(errors.contains("-:2:"), "{errors}");
    assert!(errors.contains("E_TYPE"), "{errors}");
    assert!(!errors.contains("E_IMPORT_ROOT_REQUIRED"), "{errors}");
}

#[test]
fn scripts_run_from_folders_with_capitals_and_hyphens() {
    // The file a command names is never imported, so it needs no module-style name (REVIEW2 1.4).
    let dir = kestrel::native::TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    let folder = dir.path().join("My-Scripts");
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(
        folder.join("Hello-World.kes"),
        "record P(x : Int)\nprint(P(1))\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_kestrel"))
        .current_dir(dir.path())
        .args(["run", "My-Scripts/Hello-World.kes"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"P(x = 1)\n");
}
