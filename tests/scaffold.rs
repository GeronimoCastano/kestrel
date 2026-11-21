use kestrel::native::TemporaryDirectory;
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn workspace() -> TemporaryDirectory {
    TemporaryDirectory::new(&std::env::temp_dir()).unwrap()
}

fn kestrel(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_kestrel"))
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn a_new_package_checks_runs_and_tests_without_edits() {
    let dir = workspace();
    let created = kestrel(dir.path(), &["new", "hello"]);
    assert!(created.status.success(), "{}", text(&created.stderr));
    let printed = text(&created.stdout);
    assert!(printed.contains("Created package `hello`."), "{printed}");
    assert!(printed.contains("kestrel run main.kes"), "{printed}");
    let package = dir.path().join("hello");
    for file in [
        "kestrel.project",
        "main.kes",
        "greet.kes",
        "tests/greet.kes",
        ".gitignore",
    ] {
        assert!(package.join(file).is_file(), "missing {file}");
    }
    assert_eq!(
        fs::read_to_string(package.join("kestrel.project")).unwrap(),
        "package hello\n"
    );
    let checked = kestrel(&package, &["check", "main.kes"]);
    assert!(checked.status.success(), "{}", text(&checked.stderr));
    let ran = kestrel(&package, &["run", "main.kes"]);
    assert!(ran.status.success(), "{}", text(&ran.stderr));
    assert_eq!(text(&ran.stdout), "Hello, world!\n");
    let tested = kestrel(&package, &["test"]);
    assert!(tested.status.success(), "{}", text(&tested.stdout));
    assert!(
        text(&tested.stdout).contains("1 passed, 0 failed"),
        "{}",
        text(&tested.stdout)
    );
}

#[test]
fn the_library_template_leaves_out_the_entry_point() {
    let dir = workspace();
    let created = kestrel(dir.path(), &["new", "--lib", "geometry"]);
    assert!(created.status.success(), "{}", text(&created.stderr));
    let printed = text(&created.stdout);
    assert!(!printed.contains("kestrel run main.kes"), "{printed}");
    let package = dir.path().join("geometry");
    assert!(!package.join("main.kes").exists());
    assert!(package.join("greet.kes").is_file());
    assert!(package.join("tests/greet.kes").is_file());
    let checked = kestrel(&package, &["check", "greet.kes"]);
    assert!(checked.status.success(), "{}", text(&checked.stderr));
    let tested = kestrel(&package, &["test"]);
    assert!(tested.status.success(), "{}", text(&tested.stdout));
    assert!(
        text(&tested.stdout).contains("1 passed, 0 failed"),
        "{}",
        text(&tested.stdout)
    );
}

#[test]
fn the_bin_flag_is_accepted() {
    let dir = workspace();
    let created = kestrel(dir.path(), &["new", "--bin", "hello"]);
    assert!(created.status.success(), "{}", text(&created.stderr));
    assert!(dir.path().join("hello/main.kes").is_file());
}

#[test]
fn an_existing_folder_is_refused_and_left_untouched() {
    let dir = workspace();
    let package = dir.path().join("hello");
    fs::create_dir(&package).unwrap();
    fs::write(package.join("keep.txt"), "keep\n").unwrap();
    let refused = kestrel(dir.path(), &["new", "hello"]);
    assert_eq!(refused.status.code(), Some(2));
    let stderr = text(&refused.stderr);
    assert!(stderr.contains("already exists"), "{stderr}");
    assert_eq!(
        fs::read_to_string(package.join("keep.txt")).unwrap(),
        "keep\n"
    );
    assert!(!package.join("kestrel.project").exists());
}

#[test]
fn names_that_cannot_be_packages_are_refused() {
    let dir = workspace();
    for (name, expected) in [
        ("Hello", "not a package name"),
        ("hello-world", "not a package name"),
        ("hello.world", "not a package name"),
        ("2fast", "not a package name"),
        ("_hidden", "not a package name"),
        ("new", "is a Kestrel keyword"),
        ("con", "reserved device name"),
    ] {
        let refused = kestrel(dir.path(), &["new", name]);
        assert_eq!(refused.status.code(), Some(2), "{name}");
        let stderr = text(&refused.stderr);
        assert!(stderr.contains(expected), "{name}: {stderr}");
        assert!(!dir.path().join(name).exists(), "{name} was created");
    }
    let long = "a".repeat(65);
    let refused = kestrel(dir.path(), &["new", &long]);
    assert_eq!(refused.status.code(), Some(2));
    assert!(
        text(&refused.stderr).contains("at most 64"),
        "{}",
        text(&refused.stderr)
    );
    assert!(!dir.path().join(&long).exists());
}

#[test]
fn the_command_reports_its_usage() {
    let dir = workspace();
    for (args, expected) in [
        (&["new"][..], "new takes one package name"),
        (&["new", "--lib"][..], "new takes one package name"),
        (&["new", "one", "two"][..], "new takes one package name"),
        (
            &["new", "--lib", "--bin", "hello"][..],
            "exclude each other",
        ),
        (
            &["new", "hello", "--root", "."][..],
            "new takes one package name and the flags",
        ),
    ] {
        let refused = kestrel(dir.path(), args);
        assert_eq!(refused.status.code(), Some(2), "{args:?}");
        let stderr = text(&refused.stderr);
        assert!(stderr.contains(expected), "{args:?}: {stderr}");
    }
    assert!(fs::read_dir(dir.path()).unwrap().next().is_none());
}

#[test]
fn git_initializes_a_repository_when_asked() {
    if !Command::new("git")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
    {
        return;
    }
    let dir = workspace();
    let created = kestrel(dir.path(), &["new", "--git", "hello"]);
    assert!(created.status.success(), "{}", text(&created.stderr));
    assert!(dir.path().join("hello/.git").is_dir());
}

#[cfg(unix)]
#[test]
fn a_failed_git_init_removes_the_folder_it_started() {
    let dir = workspace();
    let failed = Command::new(env!("CARGO_BIN_EXE_kestrel"))
        .args(["new", "--git", "hello"])
        .current_dir(dir.path())
        .env("PATH", "")
        .output()
        .unwrap();
    assert_eq!(failed.status.code(), Some(2));
    assert!(
        text(&failed.stderr).contains("Cannot start Git"),
        "{}",
        text(&failed.stderr)
    );
    assert!(!dir.path().join("hello").exists());
}
