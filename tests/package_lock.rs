use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Workspace(PathBuf);

impl Workspace {
    fn new() -> Self {
        let serial = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "kestrel-package-lock-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn repository(root: &Path, package: &str, project_body: &str, source: &str) -> PathBuf {
    let repo = root.join(package);
    fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.name", "Kestrel tests"]);
    git(
        &repo,
        &["config", "user.email", "kestrel-tests@example.invalid"],
    );
    git(&repo, &["symbolic-ref", "HEAD", "refs/heads/main"]);
    write(&repo, "kestrel.project", project_body);
    write(&repo, "ops.kes", source);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "initial"]);
    repo
}

/// A file URL Git accepts on every platform: file:///C:/... on Windows.
fn file_url(path: &Path) -> String {
    let path = path.display().to_string().replace('\\', "/");
    if path.starts_with('/') {
        format!("file://{path}")
    } else {
        format!("file:///{path}")
    }
}

fn project(root: &Path, body: &str) -> PathBuf {
    let path = root.join("app");
    fs::create_dir_all(&path).unwrap();
    write(&path, "kestrel.project", &format!("package app\n{body}"));
    write(
        &path,
        "main.kes",
        "import matrix.ops.{answer}\nprint(answer())\n",
    );
    path
}

fn run_cli(root: &Path, cache_home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_kestrel"))
        .args(args)
        .current_dir(root)
        .env("HOME", root.join("home"))
        .env("XDG_CACHE_HOME", cache_home)
        .output()
        .unwrap()
}

fn assert_success(output: Output) {
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn lock(root: &Path) -> serde_json::Value {
    serde_json::from_slice(&fs::read(root.join("kestrel.lock")).unwrap()).unwrap()
}

fn commit_for(lock: &serde_json::Value, name: &str) -> String {
    lock["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["name"] == name)
        .unwrap()["commit"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn cache_source(cache_home: &Path, name: &str, url: &str, commit: &str) -> PathBuf {
    let url_hash = format!("{:x}", Sha256::digest(url.as_bytes()));
    cache_home
        .join("kestrel/packages")
        .join(name)
        .join(url_hash)
        .join(commit)
        .join("source")
}

#[test]
fn fetch_locks_git_dependency_and_build_works_offline_from_verified_cache() {
    let work = Workspace::new();
    let sources = work.path().join("sources");
    let matrix = repository(
        &sources,
        "matrix",
        "package matrix\n",
        "public function answer() : Int = 42\n",
    );
    git(&matrix, &["branch", "feature"]);
    let url = file_url(&matrix);
    let app = project(
        work.path(),
        &format!("depends matrix from \"{url}\" at \"feature\"\n"),
    );
    let cache_home = work.path().join("cache");

    assert_success(run_cli(&app, &cache_home, &["fetch"]));
    let pin = lock(&app);
    assert_eq!(pin["packages"][0]["name"], "matrix");
    assert_eq!(pin["packages"][0]["url"], url);
    assert_eq!(pin["packages"][0]["reference"], "feature");
    assert!(
        pin["packages"][0]["tree_hash"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );

    // The original source is unavailable to prove that the build uses the pin and cache.
    fs::remove_dir_all(&matrix).unwrap();
    let executable = app.join("built-app");
    let executable_arg = executable.to_string_lossy().into_owned();
    let output = run_cli(
        &app,
        &cache_home,
        &[
            "build",
            "--root",
            app.to_str().unwrap(),
            "-o",
            &executable_arg,
            "main.kes",
        ],
    );
    assert_success(output);
    assert!(kestrel::native::executable_path(&executable).is_file());
}

#[test]
fn builds_reject_tampered_cache_and_fetch_restores_the_pinned_tree() {
    let work = Workspace::new();
    let sources = work.path().join("sources");
    let matrix = repository(
        &sources,
        "matrix",
        "package matrix\n",
        "public function answer() : Int = 42\n",
    );
    let url = file_url(&matrix);
    let app = project(
        work.path(),
        &format!("depends matrix from \"{url}\" at \"main\"\n"),
    );
    let cache_home = work.path().join("cache");
    assert_success(run_cli(&app, &cache_home, &["fetch"]));
    let pin = lock(&app);
    let commit = commit_for(&pin, "matrix");
    let source = cache_source(&cache_home, "matrix", &url, &commit);
    write(&source, "ops.kes", "public function answer() : Int = 99\n");

    let check = run_cli(
        &app,
        &cache_home,
        &["check", "--root", app.to_str().unwrap(), "main.kes"],
    );
    assert!(!check.status.success());
    assert!(String::from_utf8_lossy(&check.stderr).contains("E_PACKAGE_HASH"));

    // fetch preserves the existing commit and can repair an edited cache entry.
    assert_success(run_cli(&app, &cache_home, &["fetch"]));
    assert_eq!(commit_for(&lock(&app), "matrix"), commit);
    assert_success(run_cli(
        &app,
        &cache_home,
        &["check", "--root", app.to_str().unwrap(), "main.kes"],
    ));

    #[cfg(unix)]
    {
        fs::remove_dir_all(&source).unwrap();
        std::os::unix::fs::symlink(&matrix, &source).unwrap();
        let check = run_cli(
            &app,
            &cache_home,
            &["check", "--root", app.to_str().unwrap(), "main.kes"],
        );
        assert!(!check.status.success());
        assert!(String::from_utf8_lossy(&check.stderr).contains("symlink"));
        assert_success(run_cli(&app, &cache_home, &["fetch"]));
    }
}

#[test]
fn transitive_git_pin_conflicts_report_both_dependency_paths() {
    let work = Workspace::new();
    let sources = work.path().join("sources");
    let common = repository(
        &sources,
        "common",
        "package common\n",
        "public function answer() : Int = 1\n",
    );
    git(&common, &["tag", "v1"]);
    write(&common, "ops.kes", "public function answer() : Int = 2\n");
    git(&common, &["add", "-A"]);
    git(&common, &["commit", "-q", "-m", "second version"]);
    git(&common, &["tag", "v2"]);
    let common_url = file_url(&common);

    let a = repository(
        &sources,
        "a",
        &format!("package a\ndepends common from \"{common_url}\" at \"v1\"\n"),
        "public function marker() : Int = 1\n",
    );
    let b = repository(
        &sources,
        "b",
        &format!("package b\ndepends common from \"{common_url}\" at \"v2\"\n"),
        "public function marker() : Int = 2\n",
    );
    let app = project(
        work.path(),
        &format!(
            "depends a from \"{}\" at \"main\"\ndepends b from \"{}\" at \"main\"\n",
            file_url(&a),
            file_url(&b)
        ),
    );
    let output = run_cli(&app, &work.path().join("cache"), &["fetch"]);
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("E_PACKAGE_VERSION"), "{error}");
    assert!(error.contains("app -> a -> common"), "{error}");
    assert!(error.contains("app -> b -> common"), "{error}");
}

#[test]
fn fetch_keeps_pins_and_update_moves_only_the_requested_branch_pin() {
    let work = Workspace::new();
    let sources = work.path().join("sources");
    let matrix = repository(
        &sources,
        "matrix",
        "package matrix\n",
        "public function answer() : Int = 1\n",
    );
    let other = repository(
        &sources,
        "other",
        "package other\n",
        "public function marker() : Int = 0\n",
    );
    let url = file_url(&matrix);
    let other_url = file_url(&other);
    let app = project(
        work.path(),
        &format!(
            "depends matrix from \"{url}\" at \"main\"\ndepends other from \"{other_url}\" at \"main\"\n"
        ),
    );
    let cache_home = work.path().join("cache");
    assert_success(run_cli(&app, &cache_home, &["fetch"]));
    let before = lock(&app);
    let old_matrix = commit_for(&before, "matrix");
    let old_other = commit_for(&before, "other");

    write(&matrix, "ops.kes", "public function answer() : Int = 2\n");
    git(&matrix, &["add", "-A"]);
    git(&matrix, &["commit", "-q", "-m", "second matrix version"]);
    let new_matrix = git(&matrix, &["rev-parse", "HEAD"]);

    // fetch is repeatable; update NAME deliberately advances one dependency.
    assert_success(run_cli(&app, &cache_home, &["fetch"]));
    assert_eq!(commit_for(&lock(&app), "matrix"), old_matrix);
    assert_success(run_cli(&app, &cache_home, &["update", "matrix"]));
    let after = lock(&app);
    assert_eq!(commit_for(&after, "matrix"), new_matrix);
    assert_eq!(commit_for(&after, "other"), old_other);
}

#[test]
fn fetch_does_not_run_git_smudge_filters_from_a_package() {
    let work = Workspace::new();
    let sources = work.path().join("sources");
    let matrix = repository(
        &sources,
        "matrix",
        "package matrix\n",
        "public function answer() : Int = 42\n",
    );
    write(&matrix, ".gitattributes", "*.kes filter=package-code\n");
    git(&matrix, &["add", "-A"]);
    git(&matrix, &["commit", "-q", "-m", "add filter attribute"]);
    let url = file_url(&matrix);
    let app = project(
        work.path(),
        &format!("depends matrix from \"{url}\" at \"main\"\n"),
    );
    let marker = work.path().join("filter-ran");
    let smudge = format!("sh -c 'touch {}; cat'", marker.display());
    let output = Command::new(env!("CARGO_BIN_EXE_kestrel"))
        .arg("fetch")
        .current_dir(&app)
        .env("HOME", work.path().join("home"))
        .env("XDG_CACHE_HOME", work.path().join("cache"))
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "filter.package-code.smudge")
        .env("GIT_CONFIG_VALUE_0", smudge)
        .env("GIT_DIR", "/")
        .env("GIT_WORK_TREE", "/")
        .output()
        .unwrap();
    assert_success(output);
    assert!(
        !marker.exists(),
        "Git ran a smudge filter while fetching source code"
    );
}
