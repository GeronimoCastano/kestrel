//! Regressions from audit/RELEASE-REVIEW-2026-11-15.md.
use kestrel::{modules::check_source, native::TemporaryDirectory};
use std::process::{Command, Output};

fn run_in(source: &str, directory: &std::path::Path) -> Output {
    let file = directory.join("main.kes");
    std::fs::write(&file, source).unwrap();
    Command::new(env!("CARGO_BIN_EXE_kestrel"))
        .arg("run")
        .arg(&file)
        .current_dir(directory)
        .output()
        .unwrap()
}

fn stdout(source: &str) -> String {
    let directory = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    let output = run_in(source, directory.path());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn saved_local_functions_keep_their_captured_mutations() {
    for binding in [
        "let g := grow",
        "let f := grow\nlet g := f",
        "let g := (grow)",
        "let g : Int -> void = grow",
        "let g := if true then grow else grow",
        "let g := match true { true => grow, false => grow }",
        "function idle(n : Int) : void {}\nlet mut g := idle\ng = grow",
        "let g := grow(_)",
        "function wrapped(n : Int) : void { let g := grow\ng(n) }\nlet g := wrapped",
        "function wrapped(n : Int) : void { let g := if true then grow else grow\ng(n) }\nlet g := wrapped",
        "let g := n : Int => grow(n)",
    ] {
        for body in [
            "let first := a[0]\ng(1000)\nfirst.n",
            "for (x in a) { g(1000) }\n0",
        ] {
            let source = format!(
                "class C {{ public var n : Int\nnew(n : Int) {{ this.n = n }} }}\n\
                 function test() : Int {{\n\
                 let a := Array.generate(1, i => C(7))\n\
                 function grow(n : Int) : void {{ for (i in range(n)) {{ a.append(C(i)) }} }}\n\
                 {binding}\n{body}\n}}\nprint(test())\n"
            );
            let checked = check_source("aliases.kes", &source);
            assert!(
                checked
                    .checked
                    .diagnostics
                    .iter()
                    .any(|d| d.code == "E_BORROW_CONFLICT"),
                "{source}\n{:#?}",
                checked.checked.diagnostics
            );
        }
    }
}

#[test]
fn passing_a_local_function_cannot_hide_its_mutation() {
    let source = "class C { public var n : Int\nnew(n : Int) { this.n = n } }\n\
        function apply(f : () -> void) : void { f() }\n\
        function test() : Int {\n\
        let a := Array.generate(1, i => C(7))\n\
        function grow() : void { a.append(C(9)) }\n\
        let g := grow\nlet first := a[0]\napply(g)\nfirst.n\n}\n";
    let checked = check_source("callbacks.kes", source);
    assert!(
        checked
            .checked
            .diagnostics
            .iter()
            .any(|d| d.code == "E_BORROW_CONFLICT"),
        "{:#?}",
        checked.checked.diagnostics
    );
}

#[test]
fn a_generator_callback_keeps_its_captured_mutations() {
    let source = "class C { public var n : Int\nnew(n : Int) { this.n = n } }\n\
        function test() : Int {\n\
        let a := Array.generate(1, i => C(7))\n\
        function grow(n : Int) : Int { a.append(C(n))\nn }\n\
        let g := grow\nlet first := a[0]\nlet values := Array.generate(3, g)\nfirst.n\n}\n";
    let checked = check_source("generator.kes", source);
    assert!(
        checked
            .checked
            .diagnostics
            .iter()
            .any(|d| d.code == "E_BORROW_CONFLICT"),
        "{:#?}",
        checked.checked.diagnostics
    );
}

#[test]
fn tasks_cannot_hide_captured_mutations_behind_local_function_aliases() {
    let source = "class C { public var n : Int\nnew(n : Int) { this.n = n } }\n\
        function test() : void with Effects {\n\
        let a := Array.generate(1, i => C(7))\n\
        function grow() : void { a.append(C(9)) }\nlet g := grow\n\
        scope { spawn g()\nprint(a.length()) }\n}\n";
    let checked = check_source("tasks.kes", source);
    assert!(
        checked
            .checked
            .diagnostics
            .iter()
            .any(|d| d.code == "E_TASK_CONFLICT"),
        "{:#?}",
        checked.checked.diagnostics
    );
}

#[test]
fn local_function_aliases_allow_reads_and_mutation_after_a_borrow_ends() {
    assert_eq!(
        stdout(
            "class C { public var n : Int\nnew(n : Int) { this.n = n } }\n\
        function test() : Int with Effects {\n\
        let a := Array.generate(1, i => C(7))\n\
        function size() : Int = a.length()\nlet read := size\n\
        function grow() : void { a.append(C(9)) }\nlet g := grow\n\
        let first := a[0]\nprint(first.n + read())\ng()\ng()\na.length()\n}\nprint(test())\n"
        ),
        "8\n3\n"
    );
    assert_eq!(
        stdout(
            "class C { public var n : Int\nnew(n : Int) { this.n = n } }\n\
         function test() : Int {\nlet a := Array.generate(1, i => C(7))\n\
         function grow(n : Int) : Int { a.append(C(n))\nn }\nlet g := grow\n\
         let values := Array.generate(3, g)\ng(4)\na.length()\n}\nprint(test())\n"
        ),
        "5\n"
    );
}

#[test]
fn nat_formatting_keeps_all_unsigned_bits() {
    let mut source = String::new();
    let mut expected = String::new();
    for (i, n) in [0_u64, 9223372036854775807, 9223372036854775808, u64::MAX]
        .into_iter()
        .enumerate()
    {
        source.push_str(&format!("let n{i} : Nat = {n}\n"));
        for spec in [
            "d", "x", "X", "o", "b", "#x", "#X", "#o", "#b", ">22", "+d", "025d",
        ] {
            source.push_str(&format!("print(f\"{{n{i}:{spec}}}\")\n"));
        }
        expected.push_str(&format!("{n}\n{n:x}\n{n:X}\n{n:o}\n{n:b}\n{n:#x}\n0X{n:X}\n{n:#o}\n{n:#b}\n{n:>22}\n{n:+}\n{n:025}\n"));
    }
    source.push_str("let largest : Nat = 18446744073709551615\nprint(f\"{largest:,d}\")\nprint(f\"{largest:.1e}\")\nprint(f\"{largest:.1f}\")\nprint(\"{:x}\".format(largest))\nprint(f\"{(-9223372036854775808):d}\")\nprint(f\"{(-9223372036854775808):#x}\")\nlet a : Nat = 65\nprint(f\"{a:c}\")\n");
    expected.push_str("18,446,744,073,709,551,615\n1.8e+19\n18446744073709551616.0\nffffffffffffffff\n-9223372036854775808\n-0x8000000000000000\nA\n");
    assert_eq!(stdout(&source), expected);
}

#[test]
fn a_large_nat_character_format_is_rejected_without_wrapping() {
    let directory = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    let output = run_in(
        "let n : Nat = 18446744073709551615\nprint(f\"{n:c}\")\n",
        directory.path(),
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Unicode scalar"));
}

#[cfg(unix)]
#[test]
fn copying_to_the_same_open_file_preserves_it_for_all_path_aliases() {
    let directory = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    let root = directory.path();
    std::fs::write(root.join("source.txt"), "valuable content").unwrap();
    std::fs::hard_link(root.join("source.txt"), root.join("hard.txt")).unwrap();
    std::os::unix::fs::symlink("source.txt", root.join("symbol.txt")).unwrap();
    std::fs::write(
        root.join("destination.txt"),
        "long existing destination content",
    )
    .unwrap();
    let source = format!(
        "import std.fs\n\
         for (target in [\"source.txt\", \"./source.txt\", {:?}, \"hard.txt\", \"symbol.txt\"]) {{\n\
         print(match fs.copy(\"source.txt\", target) {{ RuntimeError.Io(m) => m, _ => \"unexpected success\" }})\n\
         print(fs.readFile(\"source.txt\")?)\n}}\n\
         fs.copy(\"source.txt\", \"destination.txt\")?\n\
         fs.copy(\"source.txt\", \"new.txt\")?\n",
        root.join("source.txt").to_string_lossy()
    );
    let output = run_in(&source, root);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "Cannot copy a file onto itself\nvaluable content\n".repeat(5)
    );
    for name in [
        "source.txt",
        "hard.txt",
        "symbol.txt",
        "destination.txt",
        "new.txt",
    ] {
        assert_eq!(std::fs::read(root.join(name)).unwrap(), b"valuable content");
    }
}
