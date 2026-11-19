//! The selected designs work together across a fetched package boundary.
use kestrel::native::TemporaryDirectory;
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}
fn kestrel(root: &Path, cache: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_kestrel"))
        .current_dir(root)
        .env("XDG_CACHE_HOME", cache)
        .args(args)
        .output()
        .unwrap()
}
fn success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn locked_package_defaults_class_proofs_induction_and_trust_reporting_work_together() {
    let directory = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    let upstream = directory.path().join("geometry");
    let app = directory.path().join("app");
    let cache = directory.path().join("cache");
    fs::create_dir(&upstream).unwrap();
    fs::create_dir(&app).unwrap();
    fs::write(upstream.join("kestrel.project"), "package geometry\n").unwrap();
    let mut shapes = r#"
public trait Measured {
    public function side() : Int
    public function measure() : Int = this.side()
}
public class Square {
    has Measured
    public final width : Int
    public new(width : Int) { this.width = width }
    public function side() : Int = this.width
}
public theorem measurement(s : Square) : s.measure() = s.width { rfl }

public enum Tree { Leaf, Node(Tree, Tree) }
public function mirror(t : Tree) : Tree = match t {
    Leaf => Leaf,
    Node(a, b) => Node(mirror(b), mirror(a)),
}
public theorem mirrorTwice(t : Tree) : mirror(mirror(t)) = t by {
    induction t {
        Leaf => { rfl }
        Node(a, b) iha ihb => { simp [iha, ihb] }
    }
}
"#
    .to_owned();
    let with_solver = Command::new("z3").arg("--version").output().is_ok();
    if with_solver {
        shapes.push_str("public theorem linear(n : Int) : n + 1 > n { smt }\npublic theorem nonlinear(n : Int) : n * n >= 0 { smt }\n");
    }
    fs::write(upstream.join("shapes.kes"), shapes).unwrap();
    git(&upstream, &["init", "--quiet"]);
    git(&upstream, &["add", "."]);
    git(
        &upstream,
        &[
            "-c",
            "user.name=Package test",
            "-c",
            "user.email=package@example.invalid",
            "commit",
            "--quiet",
            "-m",
            "Package fixture",
        ],
    );
    let commit = git(&upstream, &["rev-parse", "HEAD"]);
    fs::write(
        app.join("kestrel.project"),
        format!(
            "package app\ndepends geometry from \"file://{}\" at \"{commit}\"\n",
            upstream.display()
        ),
    )
    .unwrap();
    fs::write(app.join("main.kes"), "import geometry.shapes.{Square, Measured, measurement}\nfunction known(s : Square) : Int requires measurement(s) = s.measure()\nlet square := Square(3)\nprint(known(square))\nlet view : &dyn Measured = square\nprint(view.measure())\n").unwrap();
    success(&kestrel(&app, &cache, &["fetch"]));
    assert!(app.join("kestrel.lock").is_file());
    fs::remove_dir_all(&upstream).unwrap();
    let output = kestrel(&app, &cache, &["run", "main.kes"]);
    success(&output);
    assert_eq!(output.stdout, b"3\n3\n");
    let output = kestrel(&app, &cache, &["check", "--trusted", "main.kes"]);
    success(&output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    if with_solver {
        assert!(stdout.contains("trusted nonlinear: smt"), "{stdout}");
        assert!(!stdout.contains("trusted linear:"), "{stdout}");
    }
    assert!(!stdout.contains("trusted measurement:"), "{stdout}");
}
