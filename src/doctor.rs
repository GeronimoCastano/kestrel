//! `kestrel doctor`: a read-only diagnosis of an installation.
//!
//! Nothing here fetches or installs. The cache check is the only writer: it drops
//! one temporary file inside the package cache to prove the folder is writable,
//! and removes it before returning.

use crate::{
    modules::read_package,
    package_lock,
    paths::{self, Platform},
};
use std::{
    env,
    ffi::OsString,
    fs,
    io::{self, Write as _},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const COMMIT: &str = env!("KESTREL_COMMIT");
const ACCELERATE: &str = "/System/Library/Frameworks/Accelerate.framework/Accelerate";
const LOCK_FILE: &str = "kestrel.lock";

/// How a check ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Ok,
    Warn,
    Fail,
}

impl Status {
    fn word(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Warn => "warn",
            Self::Fail => "fail",
        }
    }
}

/// Whether a finding makes the installation unusable or only less useful.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Required,
    Warning,
}

impl Level {
    fn word(self) -> &'static str {
        match self {
            Self::Required => "required",
            Self::Warning => "warning",
        }
    }
}

/// One line of the report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Check {
    pub name: &'static str,
    pub level: Level,
    pub status: Status,
    pub detail: String,
    pub fix: Option<String>,
}

/// The folder shape the running binary was installed into.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutKind {
    Windows,
    Homebrew,
    Source,
    Standalone,
}

impl LayoutKind {
    fn word(self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::Homebrew => "homebrew",
            Self::Source => "source",
            Self::Standalone => "standalone",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    pub kind: LayoutKind,
    pub root: PathBuf,
}

/// Everything `kestrel doctor` found.
#[derive(Clone, Debug)]
pub struct Report {
    pub version: &'static str,
    pub commit: String,
    pub platform: Platform,
    pub target: String,
    pub layout: Layout,
    pub project: PathBuf,
    pub cache: Option<PathBuf>,
    pub home: Option<PathBuf>,
    pub local_app_data: Option<PathBuf>,
    pub checks: Vec<Check>,
}

impl Report {
    pub fn failures(&self) -> usize {
        self.checks
            .iter()
            .filter(|check| check.status == Status::Fail)
            .count()
    }

    pub fn warnings(&self) -> usize {
        self.checks
            .iter()
            .filter(|check| check.status == Status::Warn)
            .count()
    }

    /// A required check failed. Warnings leave the installation usable.
    pub fn healthy(&self) -> bool {
        self.failures() == 0
    }

    /// The human report, with absolute paths shortened to `~` and `%LOCALAPPDATA%`.
    pub fn render(&self) -> String {
        let mut text = format!(
            "kestrel doctor {} on {} ({})\n\n",
            self.version,
            self.platform.name(),
            self.target
        );
        for check in &self.checks {
            let detail = self.shorten(&check.detail);
            text.push_str(&format!(
                "{:<6}{:<12}{}\n",
                check.status.word(),
                check.name,
                detail
            ));
            if let Some(fix) = &check.fix {
                text.push_str(&format!("      fix: {}\n", self.shorten(fix)));
            }
        }
        text.push('\n');
        text.push_str(&self.summary());
        text
    }

    /// Replaces the user's folders with `~` and `%LOCALAPPDATA%` in the human report.
    /// The JSON keeps the absolute paths.
    fn shorten(&self, text: &str) -> String {
        let mut text = text.to_owned();
        if let Some(local) = &self.local_app_data {
            text = replace_prefix(&text, &local.display().to_string(), "%LOCALAPPDATA%");
        }
        if let Some(home) = &self.home {
            text = replace_prefix(&text, &home.display().to_string(), "~");
        }
        text
    }

    fn summary(&self) -> String {
        let failures = self.failures();
        let warnings = self.warnings();
        let required = if failures == 0 {
            "all required checks passed".to_owned()
        } else {
            format!(
                "{failures} required check{} failed",
                if failures == 1 { "" } else { "s" }
            )
        };
        match warnings {
            0 => format!("{required}."),
            1 => format!("{required}, 1 warning."),
            _ => format!("{required}, {warnings} warnings."),
        }
    }

    /// One JSON object for scripts and bug reports. Paths are absolute here.
    pub fn json(&self) -> serde_json::Value {
        let checks = self
            .checks
            .iter()
            .map(|check| {
                serde_json::json!({
                    "name": check.name,
                    "level": check.level.word(),
                    "status": check.status.word(),
                    "detail": check.detail,
                    "fix": check.fix,
                })
            })
            .collect::<Vec<_>>();
        serde_json::json!({
            "version": self.version,
            "commit": self.commit,
            "platform": self.platform.name(),
            "target": self.target,
            "layout": self.layout.kind.word(),
            "installRoot": self.layout.root.display().to_string(),
            "project": self.project.display().to_string(),
            "cache": self.cache.as_ref().map(|path| path.display().to_string()),
            "checks": checks,
            "warnings": self.warnings(),
            "failures": self.failures(),
            "healthy": self.healthy(),
        })
    }
}

/// Diagnoses this installation. `root` is the project folder whose lock is read.
pub fn run(root: &Path) -> Result<Report, String> {
    let environment = Environment::current(root)?;
    Ok(diagnose(&environment, &System))
}

/// The facts `diagnose` reads. Tests build this directly, so no test needs a real
/// compiler, solver or library.
struct Environment {
    platform: Platform,
    executable: PathBuf,
    path: Option<OsString>,
    cc: Option<OsString>,
    lapack: Option<OsString>,
    z3: Option<OsString>,
    home: Option<PathBuf>,
    user_profile: Option<PathBuf>,
    local_app_data: Option<PathBuf>,
    xdg_cache_home: Option<PathBuf>,
    commit: String,
    root: PathBuf,
}

impl Environment {
    fn current(root: &Path) -> Result<Self, String> {
        let executable = env::current_exe()
            .map_err(|error| format!("Cannot find this binary's path: {error}"))?;
        let path = |name: &str| env::var_os(name).filter(|value| !value.is_empty());
        Ok(Self {
            platform: Platform::current(),
            executable,
            path: path("PATH"),
            cc: path("KESTREL_CC"),
            lapack: path("KESTREL_LAPACK"),
            z3: path("KESTREL_Z3"),
            home: path("HOME").map(PathBuf::from),
            user_profile: path("USERPROFILE").map(PathBuf::from),
            local_app_data: path("LOCALAPPDATA").map(PathBuf::from),
            xdg_cache_home: path("XDG_CACHE_HOME").map(PathBuf::from),
            commit: COMMIT.to_owned(),
            root: root.to_owned(),
        })
    }

    fn home(&self) -> Option<&Path> {
        self.home.as_deref().or(self.user_profile.as_deref())
    }

    fn install_root(&self) -> Option<&Path> {
        self.executable.parent()?.parent()
    }

    /// A path a shell expands from a variable, so a fix never carries the user's name.
    fn shell(&self, path: &Path) -> String {
        let text = path.display().to_string();
        if self.platform == Platform::Windows
            && let Some(local) = &self.local_app_data
            && let Some(rest) = under(&text, &local.display().to_string())
        {
            return format!("\"$env:LOCALAPPDATA{rest}\"");
        }
        if let Some(home) = &self.home
            && let Some(rest) = under(&text, &home.display().to_string())
        {
            return format!("\"$HOME{rest}\"");
        }
        if let Some(profile) = &self.user_profile
            && let Some(rest) = under(&text, &profile.display().to_string())
        {
            return format!("\"$env:USERPROFILE{rest}\"");
        }
        format!("\"{text}\"")
    }

    fn search_path(&self, program: &str) -> Option<PathBuf> {
        let path = self.path.as_ref()?;
        let extensions: &[&str] = if self.platform == Platform::Windows {
            &[".exe", ".cmd", ".bat", ""]
        } else {
            &[""]
        };
        for folder in env::split_paths(path) {
            if folder.as_os_str().is_empty() {
                continue;
            }
            for extension in extensions {
                let candidate = folder.join(format!("{program}{extension}"));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
        None
    }

    /// A variable that names a program: a path when it looks like one, otherwise a
    /// name on `PATH`, the way `Command::new` would run it.
    fn resolve_program(&self, value: &OsString) -> Option<PathBuf> {
        let path = PathBuf::from(value);
        if path.is_file() {
            return Some(path);
        }
        let name = path.to_str()?;
        if name.contains('/') || name.contains('\\') {
            return None;
        }
        self.search_path(name)
    }

    fn bundled(&self, tool: &str) -> Option<PathBuf> {
        let root = self.install_root()?;
        paths::bundled_at_with(root, tool, self.platform.executable_suffix())
    }
}

fn under(text: &str, prefix: &str) -> Option<String> {
    let rest = text.strip_prefix(prefix)?;
    if rest.is_empty() || rest.starts_with('/') || rest.starts_with('\\') {
        return Some(rest.to_owned());
    }
    None
}

/// Replaces `prefix` where it ends a path, never in the middle of a longer name.
fn replace_prefix(text: &str, prefix: &str, replacement: &str) -> String {
    if prefix.is_empty() {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(index) = rest.find(prefix) {
        let (before, tail) = rest.split_at(index);
        out.push_str(before);
        let after = &tail[prefix.len()..];
        let ends_a_path = after.is_empty()
            || after.starts_with('/')
            || after.starts_with('\\')
            || after.starts_with('"')
            || after.starts_with('\'')
            || after.starts_with(':')
            || after.starts_with(',');
        if ends_a_path {
            out.push_str(replacement);
        } else {
            out.push_str(prefix);
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

fn detect_layout(environment: &Environment) -> Layout {
    let executable = &environment.executable;
    let folder = executable.parent().unwrap_or(Path::new(""));
    let root = folder
        .parent()
        .map(Path::to_owned)
        .unwrap_or_else(|| folder.to_owned());
    let text = normalized(executable);
    let canonical = fs::canonicalize(executable).unwrap_or_else(|_| executable.clone());
    let canonical_text = normalized(&canonical);
    if environment.platform == Platform::Windows
        && (text.contains("/programs/kestrel/bin/")
            || (folder
                .file_name()
                .is_some_and(|name| name.eq_ignore_ascii_case("bin"))
                && (root.join("toolchain").is_dir() || root.join("z3").is_dir())))
    {
        return Layout {
            kind: LayoutKind::Windows,
            root,
        };
    }
    if [text.as_str(), canonical_text.as_str()].iter().any(|text| {
        text.contains("/cellar/") || text.contains("/homebrew/") || text.contains("/linuxbrew/")
    }) {
        return Layout {
            kind: LayoutKind::Homebrew,
            root,
        };
    }
    let folder_name = folder.file_name().and_then(|name| name.to_str());
    let parent_name = folder
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str());
    if matches!(folder_name, Some("debug" | "release")) && parent_name == Some("target") {
        return Layout {
            kind: LayoutKind::Source,
            root: folder.to_owned(),
        };
    }
    Layout {
        kind: LayoutKind::Standalone,
        root: folder.to_owned(),
    }
}

fn normalized(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase()
}

fn diagnose(environment: &Environment, probe: &dyn Probe) -> Report {
    let layout = detect_layout(environment);
    let (cache, cache_root) = cache_check(environment);
    let checks = vec![
        commit_check(environment),
        layout_check(&layout),
        clang_check(environment, probe),
        lapack_check(environment, probe),
        z3_check(environment, probe),
        path_check(environment),
        cache,
        lock_check(environment, cache_root.as_deref()),
        editor_check(environment),
    ];
    Report {
        version: VERSION,
        commit: environment.commit.clone(),
        platform: environment.platform,
        target: environment.platform.target(),
        layout,
        project: environment.root.clone(),
        cache: cache_root,
        home: environment.home().map(Path::to_owned),
        local_app_data: environment.local_app_data.clone(),
        checks,
    }
}

fn passing(name: &'static str, level: Level, detail: String) -> Check {
    Check {
        name,
        level,
        status: Status::Ok,
        detail,
        fix: None,
    }
}

fn warning(name: &'static str, detail: String, fix: String) -> Check {
    Check {
        name,
        level: Level::Warning,
        status: Status::Warn,
        detail,
        fix: Some(fix),
    }
}

fn commit_check(environment: &Environment) -> Check {
    if environment.commit == "unknown" {
        warning(
            "commit",
            "unknown; this binary was built outside a Git checkout".into(),
            "build from a Git checkout, or install a release archive".into(),
        )
    } else {
        passing("commit", Level::Warning, environment.commit.clone())
    }
}

fn layout_check(layout: &Layout) -> Check {
    let location = layout.root.display().to_string();
    let detail = match layout.kind {
        LayoutKind::Windows => format!("Windows install at {location}"),
        LayoutKind::Homebrew => format!("Homebrew install at {location}"),
        LayoutKind::Source => format!("source build at {location}"),
        LayoutKind::Standalone => format!("standalone binary at {location}"),
    };
    passing("layout", Level::Warning, detail)
}

fn clang_check(environment: &Environment, probe: &dyn Probe) -> Check {
    let (candidate, overridden) = if let Some(value) = &environment.cc {
        match environment.resolve_program(value) {
            Some(path) => (path, true),
            None => {
                return Check {
                    name: "clang",
                    level: Level::Required,
                    status: Status::Fail,
                    detail: format!(
                        "KESTREL_CC names {}, which is not a file and not on PATH",
                        Path::new(value).display()
                    ),
                    fix: Some(cc_fix(environment)),
                };
            }
        }
    } else if let Some(path) = environment.bundled("clang") {
        (path, false)
    } else if let Some(path) = environment.search_path("clang") {
        (path, false)
    } else {
        return Check {
            name: "clang",
            level: Level::Required,
            status: Status::Fail,
            detail: "not found; `kestrel build` and `kestrel run` cannot work without it".into(),
            fix: Some(clang_install(environment)),
        };
    };
    match probe.clang(&candidate) {
        Ok(version) => passing(
            "clang",
            Level::Required,
            format!("{version} at {}", candidate.display()),
        ),
        Err(message) => Check {
            name: "clang",
            level: Level::Required,
            status: Status::Fail,
            detail: format!("{}: {message}", candidate.display()),
            fix: Some(if overridden {
                cc_fix(environment)
            } else {
                clang_install(environment)
            }),
        },
    }
}

fn cc_fix(environment: &Environment) -> String {
    if environment.platform == Platform::Windows {
        "remove KESTREL_CC, or set it to a Clang such as `$env:KESTREL_CC = 'C:\\Program Files\\LLVM\\bin\\clang.exe'`"
            .into()
    } else {
        "unset KESTREL_CC, or set it to a Clang such as `export KESTREL_CC=/usr/bin/clang`".into()
    }
}

fn clang_install(environment: &Environment) -> String {
    match environment.platform {
        Platform::MacOs => {
            "install the Command Line Tools with `xcode-select --install`, or set KESTREL_CC".into()
        }
        Platform::Linux => {
            "install Clang with your package manager, such as `sudo apt install clang`, or set KESTREL_CC"
                .into()
        }
        Platform::Windows => {
            "run install.ps1 again to restore the bundled Clang, or set KESTREL_CC".into()
        }
    }
}

fn lapack_check(environment: &Environment, probe: &dyn Probe) -> Check {
    let override_value = environment
        .lapack
        .as_ref()
        .map(|value| value.to_string_lossy().into_owned());
    // On Windows the compiler points KESTREL_LAPACK at the OpenBLAS it ships, so a
    // failure there is a broken install rather than a setting the user chose.
    let beside = environment
        .executable
        .parent()
        .map(|folder| folder.join("libopenblas.dll"))
        .is_some_and(|sibling| Path::new(override_value.as_deref().unwrap_or_default()) == sibling);
    let mut candidates = vec![];
    if let Some(value) = &override_value {
        candidates.push(value.clone());
    } else if environment.platform == Platform::Windows
        && let Some(folder) = environment.executable.parent()
    {
        let sibling = folder.join("libopenblas.dll");
        if sibling.is_file() {
            candidates.push(sibling.display().to_string());
        }
    }
    candidates.push(ACCELERATE.into());
    for name in [
        "libopenblas.so.0",
        "libblas.so.3",
        "libblas.so",
        "libopenblas.dll",
        "openblas.dll",
    ] {
        candidates.push(name.into());
    }
    let mut override_error = None;
    for candidate in &candidates {
        match probe.lapack(candidate) {
            Ok(()) => {
                if let Some(error) = override_error {
                    let named = override_value.as_deref().unwrap_or_default();
                    let detail = if beside {
                        format!(
                            "the OpenBLAS beside the compiler ({}) does not load ({error}); using {}",
                            Path::new(named).display(),
                            describe_library(candidate)
                        )
                    } else {
                        format!(
                            "KESTREL_LAPACK names {}, which does not load ({error}); using {}",
                            Path::new(named).display(),
                            describe_library(candidate)
                        )
                    };
                    return Check {
                        name: "lapack",
                        level: Level::Warning,
                        status: Status::Warn,
                        detail,
                        fix: Some(if beside {
                            lapack_install(environment)
                        } else {
                            lapack_override_fix(environment)
                        }),
                    };
                }
                return passing("lapack", Level::Warning, describe_library(candidate));
            }
            Err(error) => {
                if override_value.as_deref() == Some(candidate.as_str()) {
                    override_error = Some(error);
                }
            }
        }
    }
    let detail = match (&override_value, &override_error) {
        (Some(value), Some(error)) if beside => format!(
            "the OpenBLAS beside the compiler ({}) does not load ({error}); no BLAS library found, so std.linalg will use plain loops (correct, slower)",
            Path::new(value).display()
        ),
        (Some(value), Some(error)) => format!(
            "KESTREL_LAPACK names {}, which does not load ({error}); no BLAS library found, so std.linalg will use plain loops (correct, slower)",
            Path::new(value).display()
        ),
        _ => "no BLAS library found, so std.linalg will use plain loops (correct, slower)".into(),
    };
    warning("lapack", detail, lapack_install(environment))
}

fn describe_library(library: &str) -> String {
    if library == ACCELERATE {
        return "Accelerate framework".into();
    }
    let path = Path::new(library);
    if path.components().count() > 1 {
        path.display().to_string()
    } else {
        library.to_owned()
    }
}

fn lapack_override_fix(environment: &Environment) -> String {
    if environment.platform == Platform::Windows {
        "remove KESTREL_LAPACK, or set it to the DLL that ships with the install".into()
    } else {
        "unset KESTREL_LAPACK, or set it to the OpenBLAS library such as `export KESTREL_LAPACK=/opt/homebrew/lib/libopenblas.dylib`"
            .into()
    }
}

fn lapack_install(environment: &Environment) -> String {
    match environment.platform {
        Platform::MacOs => {
            "Accelerate ships with macOS; if it cannot be loaded, run `xcode-select --install`".into()
        }
        Platform::Linux => {
            "install OpenBLAS, such as `sudo apt install libopenblas0`, or set KESTREL_LAPACK to the library"
                .into()
        }
        Platform::Windows => {
            "run install.ps1 again to restore the bundled OpenBLAS, or set KESTREL_LAPACK to the DLL"
                .into()
        }
    }
}

fn z3_check(environment: &Environment, probe: &dyn Probe) -> Check {
    let (candidate, overridden) = if let Some(value) = &environment.z3 {
        match environment.resolve_program(value) {
            Some(path) => (path, true),
            None => {
                return warning(
                    "z3",
                    format!(
                        "KESTREL_Z3 names {}, which is not a file and not on PATH, so `smt` proofs will fail",
                        Path::new(value).display()
                    ),
                    z3_fix(environment),
                );
            }
        }
    } else if let Some(path) = environment.bundled("z3") {
        (path, false)
    } else if let Some(path) = environment.search_path("z3") {
        (path, false)
    } else {
        return warning(
            "z3",
            "not found, so `smt` proofs will fail".into(),
            z3_install(environment),
        );
    };
    match probe.z3(&candidate) {
        Ok(version) => passing(
            "z3",
            Level::Warning,
            format!("{version} at {}", candidate.display()),
        ),
        Err(message) => warning(
            "z3",
            format!("{}: {message}", candidate.display()),
            if overridden {
                z3_fix(environment)
            } else {
                z3_install(environment)
            },
        ),
    }
}

fn z3_fix(environment: &Environment) -> String {
    if environment.platform == Platform::Windows {
        "remove KESTREL_Z3, or set it to the z3.exe that ships with the install".into()
    } else {
        "unset KESTREL_Z3, or set it to the z3 executable such as `export KESTREL_Z3=/opt/homebrew/bin/z3`"
            .into()
    }
}

fn z3_install(environment: &Environment) -> String {
    match environment.platform {
        Platform::MacOs => "install it with `brew install z3`, or set KESTREL_Z3".into(),
        Platform::Linux => {
            "install it with `sudo apt install z3`, or set KESTREL_Z3 to the executable".into()
        }
        Platform::Windows => {
            "run install.ps1 again to restore the bundled Z3, or set KESTREL_Z3".into()
        }
    }
}

fn path_check(environment: &Environment) -> Check {
    let Some(found) = environment.search_path("kestrel") else {
        let folder = environment
            .executable
            .parent()
            .map(Path::to_owned)
            .unwrap_or_else(|| environment.executable.clone());
        return warning(
            "path",
            "`kestrel` is not on PATH, so the editor and scripts will not find this binary".into(),
            path_fix(environment, &folder),
        );
    };
    let here = fs::canonicalize(&environment.executable)
        .unwrap_or_else(|_| environment.executable.clone());
    let there = fs::canonicalize(&found).unwrap_or_else(|_| found.clone());
    if here == there {
        return passing(
            "path",
            Level::Warning,
            format!("{} is this binary", found.display()),
        );
    }
    warning(
        "path",
        format!(
            "PATH finds {}, but this binary is {}",
            found.display(),
            environment.executable.display()
        ),
        path_fix(
            environment,
            environment.executable.parent().unwrap_or(Path::new("")),
        ),
    )
}

fn path_fix(environment: &Environment, folder: &Path) -> String {
    if environment.platform == Platform::Windows {
        let quoted = environment.shell(folder);
        let expanded = quoted.trim_matches('"');
        format!("$env:Path = \"{expanded};$env:Path\"")
    } else {
        format!("export PATH={}:$PATH", environment.shell(folder))
    }
}

fn cache_check(environment: &Environment) -> (Check, Option<PathBuf>) {
    let resolved = paths::cache_root(
        environment.platform,
        environment.xdg_cache_home.as_deref(),
        environment.home.as_deref(),
        environment.local_app_data.as_deref(),
    );
    let root = match resolved {
        Ok(root) => root,
        Err(message) => {
            return (
                warning(
                    "cache",
                    message,
                    "set XDG_CACHE_HOME or HOME to a folder you can write to".into(),
                ),
                None,
            );
        }
    };
    let display = root.display().to_string();
    match writable(&root) {
        Ok(()) => (
            passing("cache", Level::Warning, format!("{display} is writable")),
            Some(root),
        ),
        Err(error) => {
            let fix = if environment.platform == Platform::Windows {
                "check the folder's permissions, or set XDG_CACHE_HOME to a writable folder"
                    .to_owned()
            } else {
                format!("chmod u+w {}", environment.shell(&root))
            };
            (
                warning("cache", format!("{display} is not writable: {error}"), fix),
                Some(root),
            )
        }
    }
}

/// Writes and removes one file inside the cache folder. Folders this check creates
/// are removed again, so a missing cache stays missing.
fn writable(root: &Path) -> Result<(), String> {
    let mut created = vec![];
    let mut next = root.to_owned();
    while !next.exists() {
        created.push(next.clone());
        match next.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => next = parent.to_owned(),
            _ => break,
        }
    }
    if let Err(error) = fs::create_dir_all(root) {
        cleanup(&created);
        return Err(error.to_string());
    }
    let probe = root.join(format!(".kestrel-doctor-{}", std::process::id()));
    let result = (|| {
        let mut file = fs::File::create(&probe)?;
        file.write_all(b"kestrel doctor\n")?;
        let text = fs::read_to_string(&probe)?;
        if text != "kestrel doctor\n" {
            return Err(io::Error::other("the file read back differently"));
        }
        Ok(())
    })();
    let _ = fs::remove_file(&probe);
    cleanup(&created);
    result.map_err(|error: io::Error| error.to_string())
}

fn cleanup(created: &[PathBuf]) {
    for folder in created {
        let _ = fs::remove_dir(folder);
    }
}

fn lock_check(environment: &Environment, cache_root: Option<&Path>) -> Check {
    let display = environment.root.display().to_string();
    let package = match read_package(&environment.root) {
        Ok(None) => {
            return passing(
                "lock",
                Level::Required,
                format!("no kestrel.project in {display}"),
            );
        }
        Ok(Some(package)) => package,
        Err(message) => {
            return warning(
                "lock",
                message,
                "fix kestrel.project; SYNTAX.md documents the form".into(),
            );
        }
    };
    let dependencies = package
        .depends
        .iter()
        .filter(|dependency| dependency.git.is_some())
        .collect::<Vec<_>>();
    let pins = match package_lock::inspect_lock(&environment.root, cache_root) {
        Err(message) => {
            return Check {
                name: "lock",
                level: Level::Required,
                status: if package.depends.is_empty() {
                    Status::Warn
                } else {
                    Status::Fail
                },
                detail: message,
                fix: Some(format!(
                    "fix or delete {LOCK_FILE}, then run `kestrel fetch` while online"
                )),
            };
        }
        Ok(None) => {
            if dependencies.is_empty() {
                return passing(
                    "lock",
                    Level::Required,
                    format!("no {LOCK_FILE} and no package pins needed"),
                );
            }
            return Check {
                name: "lock",
                level: Level::Required,
                status: Status::Warn,
                detail: format!(
                    "{LOCK_FILE} is missing; run `kestrel fetch` to pin the dependencies"
                ),
                fix: Some("run `kestrel fetch` while online".into()),
            };
        }
        Ok(Some(pins)) => pins,
    };
    let mut problems = vec![];
    for dependency in &dependencies {
        let source = dependency.git.as_ref().expect("filtered on git");
        match pins.iter().find(|pin| pin.name == dependency.name) {
            None => problems.push(format!("`{}` has no pin", dependency.name)),
            Some(pin) if pin.url != source.url || pin.reference != source.reference => {
                problems.push(format!("the source for `{}` changed", dependency.name));
            }
            Some(_) => {}
        }
    }
    if cache_root.is_some() {
        for pin in pins.iter().filter(|pin| !pin.cached) {
            problems.push(format!("`{}` is pinned but not cached", pin.name));
        }
    }
    if problems.is_empty() {
        let detail = match (pins.len(), cache_root.is_some()) {
            (0, _) => format!("no {LOCK_FILE} and no package pins needed"),
            (1, true) => "1 package pin matches and is cached".to_owned(),
            (1, false) => "1 package pin matches".to_owned(),
            (count, true) => format!("{count} package pins match and are cached"),
            (count, false) => format!("{count} package pins match"),
        };
        return passing("lock", Level::Required, detail);
    }
    Check {
        name: "lock",
        level: Level::Required,
        status: Status::Warn,
        detail: problems.join("; "),
        fix: Some("run `kestrel fetch` while online".into()),
    }
}

fn editor_check(environment: &Environment) -> Check {
    let Some(home) = environment.home() else {
        return passing("vscode", Level::Warning, "no home folder to look in".into());
    };
    let folders = [
        home.join(".vscode/extensions"),
        home.join(".vscode-insiders/extensions"),
        home.join(".vscode-server/extensions"),
    ];
    let mut installed: Vec<String> = vec![];
    for folder in &folders {
        let Ok(entries) = fs::read_dir(folder) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            // The publisher renamed the extension from `kestrel-lang.kestrel` to
            // `kestrel-lang.kestrel-language`, and old copies still work.
            let version = name
                .strip_prefix("kestrel-lang.kestrel-language-")
                .or_else(|| name.strip_prefix("kestrel-lang.kestrel-"));
            if let Some(version) = version {
                let version = version.split('-').next().unwrap_or(version);
                installed.push(version.to_owned());
            }
        }
    }
    installed.sort_by_key(|version| version_key(version));
    if let Some(version) = installed.last() {
        return passing(
            "vscode",
            Level::Warning,
            format!("extension {version} installed"),
        );
    }
    let present = home.join(".vscode").is_dir()
        || home.join(".vscode-insiders").is_dir()
        || home.join(".vscode-server").is_dir()
        || environment.search_path("code").is_some();
    if !present {
        return passing("vscode", Level::Warning, "VS Code not detected".into());
    }
    warning(
        "vscode",
        "VS Code is installed but the Kestrel extension is not".into(),
        "install it with `code --install-extension kestrel-lang.kestrel-language`".into(),
    )
}

fn version_key(version: &str) -> Vec<u32> {
    version
        .split('.')
        .map(|part| part.parse().unwrap_or(0))
        .collect()
}

/// The outside world. Tests substitute a fake, so none of them needs a compiler.
trait Probe {
    fn clang(&self, executable: &Path) -> Result<String, String>;
    fn z3(&self, executable: &Path) -> Result<String, String>;
    fn lapack(&self, library: &str) -> Result<(), String>;
}

struct System;

impl Probe for System {
    fn clang(&self, executable: &Path) -> Result<String, String> {
        let version = run_version(executable, &["--version"])?;
        let mut child = Command::new(executable)
            .args(["-fsyntax-only", "-x", "c", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("cannot run: {error}"))?;
        child
            .stdin
            .take()
            .expect("stdin is piped")
            .write_all(b"int main(void) { return 0; }\n")
            .map_err(|error| format!("cannot send a C file to it: {error}"))?;
        let output = child
            .wait_with_output()
            .map_err(|error| format!("cannot wait for it: {error}"))?;
        if !output.status.success() {
            let message = last_line(&String::from_utf8_lossy(&output.stderr));
            return Err(if message.is_empty() {
                format!("cannot compile C ({})", output.status)
            } else {
                format!("cannot compile C: {message}")
            });
        }
        Ok(version)
    }

    fn z3(&self, executable: &Path) -> Result<String, String> {
        let version = run_version(executable, &["--version"])
            .or_else(|_| run_version(executable, &["-version"]))?;
        let mut child = Command::new(executable)
            .arg("-in")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("cannot run: {error}"))?;
        child
            .stdin
            .take()
            .expect("stdin is piped")
            .write_all(b"(set-option :timeout 2000)\n(check-sat)\n")
            .map_err(|error| format!("cannot send a query to it: {error}"))?;
        let output = child
            .wait_with_output()
            .map_err(|error| format!("cannot wait for it: {error}"))?;
        let answer = first_line(&String::from_utf8_lossy(&output.stdout));
        if !output.status.success() {
            let message = last_line(&String::from_utf8_lossy(&output.stderr));
            return Err(if message.is_empty() {
                format!("cannot answer a query ({})", output.status)
            } else {
                format!("cannot answer a query: {message}")
            });
        }
        if !matches!(answer.as_str(), "sat" | "unsat" | "unknown") {
            return Err(format!("answered a test query with `{answer}`"));
        }
        Ok(version)
    }

    fn lapack(&self, library: &str) -> Result<(), String> {
        blas::loads(library).map_err(|error| clean_loader_error(&error))
    }
}

/// The system loader explains a failure with a page of search paths. Keep the reason.
fn clean_loader_error(message: &str) -> String {
    let message = message.trim();
    if let Some((_, rest)) = message.split_once("tried: ")
        && let Some((_, rest)) = rest.split_once("' (")
        && let Some((reason, _)) = rest.split_once(')')
    {
        return reason.trim().to_owned();
    }
    let message = message
        .strip_prefix("dlopen(")
        .and_then(|rest| rest.split_once("): "))
        .map_or(message, |(_, tail)| tail);
    let length = message.chars().count();
    let mut shortened = message.chars().take(120).collect::<String>();
    if shortened.chars().count() < length {
        shortened.push_str("...");
    }
    shortened
}

fn run_version(executable: &Path, arguments: &[&str]) -> Result<String, String> {
    let output = Command::new(executable)
        .args(arguments)
        .output()
        .map_err(|error| format!("cannot run: {error}"))?;
    if !output.status.success() {
        let message = last_line(&String::from_utf8_lossy(&output.stderr));
        return Err(if message.is_empty() {
            format!("`{}` failed ({})", arguments.join(" "), output.status)
        } else {
            format!("`{}` failed: {message}", arguments.join(" "))
        });
    }
    let text = if output.stdout.is_empty() {
        String::from_utf8_lossy(&output.stderr).into_owned()
    } else {
        String::from_utf8_lossy(&output.stdout).into_owned()
    };
    let line = first_line(&text);
    if line.is_empty() {
        return Err(format!("`{}` printed no version", arguments.join(" ")));
    }
    Ok(line)
}

fn first_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("")
        .to_owned()
}

fn last_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .rfind(|line| !line.is_empty())
        .unwrap_or("")
        .to_owned()
}

/// The two system loaders, called directly so the check does not need a C compiler.
mod blas {
    use std::ffi::CString;

    #[cfg(unix)]
    pub fn loads(library: &str) -> Result<(), String> {
        use std::ffi::{CStr, c_char, c_int, c_void};

        unsafe extern "C" {
            fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
            fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
            fn dlerror() -> *mut c_char;
            fn dlclose(handle: *mut c_void) -> c_int;
        }
        const RTLD_LAZY: c_int = 1;

        let name = CString::new(library).map_err(|_| "the name contains a NUL byte".to_owned())?;
        // SAFETY: `name` is a NUL-terminated C string that lives through the call.
        let handle = unsafe { dlopen(name.as_ptr(), RTLD_LAZY) };
        if handle.is_null() {
            // SAFETY: `dlerror` returns a NUL-terminated message or null.
            let message = unsafe { dlerror() };
            let message = if message.is_null() {
                "it does not load".to_owned()
            } else {
                // SAFETY: the pointer above is a valid C string owned by the loader.
                unsafe { CStr::from_ptr(message) }
                    .to_string_lossy()
                    .into_owned()
            };
            return Err(message);
        }
        let symbol = CString::new("cblas_dgemm").expect("the symbol name has no NUL");
        // SAFETY: the handle is open and the symbol name is NUL-terminated.
        let function = unsafe { dlsym(handle, symbol.as_ptr()) };
        // SAFETY: the handle came from `dlopen` and is not used after this call.
        unsafe { dlclose(handle) };
        if function.is_null() {
            return Err("it has no cblas_dgemm".into());
        }
        Ok(())
    }

    #[cfg(windows)]
    pub fn loads(library: &str) -> Result<(), String> {
        use std::ffi::{c_char, c_void};

        unsafe extern "system" {
            fn LoadLibraryA(name: *const c_char) -> *mut c_void;
            fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *mut c_void;
            fn FreeLibrary(module: *mut c_void) -> i32;
        }

        let name = CString::new(library).map_err(|_| "the name contains a NUL byte".to_owned())?;
        // SAFETY: `name` is a NUL-terminated C string that lives through the call.
        let module = unsafe { LoadLibraryA(name.as_ptr()) };
        if module.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let symbol = CString::new("cblas_dgemm").expect("the symbol name has no NUL");
        // SAFETY: the module is loaded and the symbol name is NUL-terminated.
        let function = unsafe { GetProcAddress(module, symbol.as_ptr()) };
        // SAFETY: the module came from `LoadLibraryA` and is not used after this call.
        unsafe { FreeLibrary(module) };
        if function.is_null() {
            return Err("it has no cblas_dgemm".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::TemporaryDirectory;
    use std::cell::RefCell;

    struct Fake {
        clang: Result<String, String>,
        z3: Result<String, String>,
        libraries: Vec<String>,
        asked: RefCell<Vec<String>>,
        clang_path: RefCell<Option<PathBuf>>,
        z3_path: RefCell<Option<PathBuf>>,
    }

    impl Fake {
        fn working() -> Self {
            Self {
                clang: Ok("Fake clang 1.0.0".into()),
                z3: Ok("Fake Z3 4.0.0".into()),
                libraries: vec![],
                asked: RefCell::new(vec![]),
                clang_path: RefCell::new(None),
                z3_path: RefCell::new(None),
            }
        }

        fn with_library(mut self, name: &str) -> Self {
            self.libraries.push(name.to_owned());
            self
        }
    }

    impl Probe for Fake {
        fn clang(&self, executable: &Path) -> Result<String, String> {
            *self.clang_path.borrow_mut() = Some(executable.to_owned());
            self.clang.clone()
        }

        fn z3(&self, executable: &Path) -> Result<String, String> {
            *self.z3_path.borrow_mut() = Some(executable.to_owned());
            self.z3.clone()
        }

        fn lapack(&self, library: &str) -> Result<(), String> {
            self.asked.borrow_mut().push(library.to_owned());
            if self.libraries.iter().any(|name| name == library) {
                Ok(())
            } else {
                Err("no such library".into())
            }
        }
    }

    fn workspace() -> TemporaryDirectory {
        TemporaryDirectory::new(&std::env::temp_dir()).unwrap()
    }

    fn file(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    /// A home with a compiler on PATH and an executable called `kestrel` there too.
    fn environment(root: &Path, home: &Path) -> Environment {
        file(&home.join("bin/kestrel"), "");
        file(&home.join("bin/clang"), "");
        Environment {
            platform: Platform::current(),
            executable: home.join("bin/kestrel"),
            path: Some(home.join("bin").into_os_string()),
            cc: Some(home.join("bin/clang").into_os_string()),
            lapack: None,
            z3: None,
            home: Some(home.to_owned()),
            user_profile: None,
            local_app_data: None,
            xdg_cache_home: Some(home.join("cache")),
            commit: "4f2a9c1d7e30".into(),
            root: root.to_owned(),
        }
    }

    fn status(report: &Report, name: &str) -> Status {
        report
            .checks
            .iter()
            .find(|check| check.name == name)
            .unwrap_or_else(|| panic!("no `{name}` check in {:?}", report.checks))
            .status
    }

    #[test]
    fn a_good_environment_is_healthy() {
        let root = workspace();
        let home = workspace();
        let mut probe = Fake::working();
        probe = probe.with_library("libblas.so.3");
        let report = diagnose(&environment(root.path(), home.path()), &probe);
        assert!(report.healthy(), "{}", report.render());
        assert_eq!(report.failures(), 0);
        assert_eq!(status(&report, "clang"), Status::Ok);
        assert_eq!(status(&report, "path"), Status::Ok);
        assert_eq!(status(&report, "cache"), Status::Ok);
        assert_eq!(status(&report, "lock"), Status::Ok);
        assert!(report.render().contains("all required checks passed"));
    }

    #[test]
    fn a_missing_clang_fails_the_required_check() {
        let root = workspace();
        let home = workspace();
        let mut environment = environment(root.path(), home.path());
        environment.cc = Some("/nonexistent/clang".into());
        let report = diagnose(&environment, &Fake::working());
        assert!(!report.healthy());
        assert_eq!(status(&report, "clang"), Status::Fail);
        assert!(report.render().contains("KESTREL_CC"));
        assert!(report.render().contains("1 required check failed"));
    }

    #[test]
    fn a_missing_z3_only_warns() {
        let root = workspace();
        let home = workspace();
        let mut environment = environment(root.path(), home.path());
        environment.z3 = None;
        let report = diagnose(&environment, &Fake::working());
        assert!(report.healthy());
        assert_eq!(status(&report, "z3"), Status::Warn);
        assert!(report.render().contains("`smt` proofs will fail"));
    }

    #[test]
    fn a_broken_lapack_only_warns() {
        let root = workspace();
        let home = workspace();
        let mut environment = environment(root.path(), home.path());
        environment.lapack = Some(home.path().join("libbroken.so").into_os_string());
        file(&home.path().join("libbroken.so"), "not a library");
        let probe = Fake::working();
        let report = diagnose(&environment, &probe);
        assert!(report.healthy());
        assert_eq!(status(&report, "lapack"), Status::Warn);
        let asked = probe.asked.borrow();
        assert_eq!(
            asked.first().map(String::as_str),
            Some(
                environment
                    .lapack
                    .as_ref()
                    .unwrap()
                    .to_string_lossy()
                    .as_ref()
            )
        );
        assert!(report.render().contains("does not load"));
    }

    #[test]
    fn a_broken_override_falls_back_with_a_warning() {
        let root = workspace();
        let home = workspace();
        let mut environment = environment(root.path(), home.path());
        environment.lapack = Some(home.path().join("libbroken.so").into_os_string());
        file(&home.path().join("libbroken.so"), "not a library");
        let probe = Fake::working().with_library("libopenblas.so.0");
        let report = diagnose(&environment, &probe);
        assert_eq!(status(&report, "lapack"), Status::Warn);
        assert!(report.render().contains("using libopenblas.so.0"));
    }

    #[test]
    fn the_windows_layout_finds_the_bundled_tools() {
        let root = workspace();
        let install = root.path().join("AppData/Local/Programs/Kestrel");
        let executable = install.join("bin/kestrel.exe");
        for path in [
            executable.clone(),
            install.join("toolchain/bin/clang.exe"),
            install.join("z3/z3.exe"),
            install.join("bin/libopenblas.dll"),
        ] {
            file(&path, "");
        }
        let environment = Environment {
            platform: Platform::Windows,
            executable,
            path: Some(OsString::new()),
            cc: None,
            lapack: None,
            z3: None,
            home: None,
            user_profile: None,
            local_app_data: Some(root.path().join("AppData/Local")),
            xdg_cache_home: None,
            commit: "4f2a9c1d7e30".into(),
            root: root.path().to_owned(),
        };
        let probe = Fake::working();
        let report = diagnose(&environment, &probe);
        assert_eq!(report.layout.kind, LayoutKind::Windows);
        assert!(report.healthy(), "{}", report.render());
        assert_eq!(
            probe.clang_path.borrow().as_deref(),
            Some(install.join("toolchain/bin/clang.exe").as_path())
        );
        assert_eq!(
            probe.z3_path.borrow().as_deref(),
            Some(install.join("z3/z3.exe").as_path())
        );
        assert_eq!(
            probe.asked.borrow().first().map(String::as_str),
            Some(
                install
                    .join("bin/libopenblas.dll")
                    .to_string_lossy()
                    .as_ref()
            )
        );
    }

    #[test]
    fn a_broken_bundled_openblas_is_named_as_such() {
        let root = workspace();
        let install = root.path().join("Kestrel");
        let executable = install.join("bin/kestrel.exe");
        for path in [
            executable.clone(),
            install.join("toolchain/bin/clang.exe"),
            install.join("z3/z3.exe"),
            install.join("bin/libopenblas.dll"),
        ] {
            file(&path, "");
        }
        let environment = Environment {
            platform: Platform::Windows,
            executable,
            path: Some(OsString::new()),
            cc: None,
            lapack: Some(install.join("bin/libopenblas.dll").into_os_string()),
            z3: None,
            home: None,
            user_profile: None,
            local_app_data: Some(root.path().join("AppData/Local")),
            xdg_cache_home: None,
            commit: "4f2a9c1d7e30".into(),
            root: root.path().to_owned(),
        };
        let report = diagnose(&environment, &Fake::working());
        assert_eq!(status(&report, "lapack"), Status::Warn);
        let detail = &report
            .checks
            .iter()
            .find(|check| check.name == "lapack")
            .unwrap()
            .detail;
        assert!(
            detail.contains("the OpenBLAS beside the compiler"),
            "{detail}"
        );
    }

    #[test]
    fn a_standalone_binary_is_recognized() {
        let layout = detect_layout(&Environment {
            platform: Platform::current(),
            executable: PathBuf::from("/usr/local/bin/kestrel"),
            path: Some(OsString::new()),
            cc: None,
            lapack: None,
            z3: None,
            home: None,
            user_profile: None,
            local_app_data: None,
            xdg_cache_home: None,
            commit: String::new(),
            root: PathBuf::new(),
        });
        assert!(matches!(
            layout.kind,
            LayoutKind::Standalone | LayoutKind::Homebrew
        ));
    }

    #[test]
    fn the_editor_check_finds_both_extension_names() {
        let root = workspace();
        let home = workspace();
        file(
            &home
                .path()
                .join(".vscode/extensions/kestrel-lang.kestrel-0.4.2/package.json"),
            "{}",
        );
        file(
            &home
                .path()
                .join(".vscode/extensions/kestrel-lang.kestrel-language-0.4.5/package.json"),
            "{}",
        );
        let report = diagnose(
            &environment(root.path(), home.path()),
            &Fake::working().with_library("libblas.so.3"),
        );
        assert_eq!(status(&report, "vscode"), Status::Ok);
        assert!(report.render().contains("extension 0.4.5 installed"));
    }

    #[test]
    fn a_clang_that_cannot_compile_fails() {
        let root = workspace();
        let home = workspace();
        let mut probe = Fake::working();
        probe.clang = Err("cannot compile C: error: unknown target".into());
        let report = diagnose(&environment(root.path(), home.path()), &probe);
        assert!(!report.healthy());
        assert_eq!(status(&report, "clang"), Status::Fail);
        assert!(report.render().contains("cannot compile C"));
    }

    #[test]
    fn a_shadowing_kestrel_on_path_warns() {
        let root = workspace();
        let home = workspace();
        let earlier = workspace();
        file(&earlier.path().join("kestrel"), "");
        let home_bin = home.path().join("bin");
        let mut environment = environment(root.path(), home.path());
        environment.path = Some(env::join_paths([earlier.path(), home_bin.as_path()]).unwrap());
        let report = diagnose(&environment, &Fake::working().with_library("libblas.so.3"));
        assert!(report.healthy());
        assert_eq!(status(&report, "path"), Status::Warn);
        assert!(report.render().contains("PATH finds"));
    }

    #[test]
    fn a_malformed_lock_fails_a_project_with_dependencies() {
        let root = workspace();
        let home = workspace();
        file(
            &root.path().join("kestrel.project"),
            "package shop\ndepends prices from \"https://example.invalid/prices\" at \"v1\"\n",
        );
        file(&root.path().join("kestrel.lock"), "{ not json");
        let report = diagnose(&environment(root.path(), home.path()), &Fake::working());
        assert!(!report.healthy());
        assert_eq!(status(&report, "lock"), Status::Fail);
        assert!(report.render().contains("not valid JSON"));
    }

    #[test]
    fn a_loader_failure_keeps_only_the_reason() {
        let message = "dlopen(/tmp/libfake.dylib, 0x0001): tried: '/tmp/libfake.dylib' (slice is not valid mach-o file), '/System/Volumes/Preboot/libfake.dylib' (no such file)";
        assert_eq!(
            clean_loader_error(message),
            "slice is not valid mach-o file"
        );
        let linux = "libfoo.so: cannot open shared object file: No such file or directory";
        assert_eq!(clean_loader_error(linux), linux);
    }

    #[test]
    fn a_malformed_lock_only_warns_without_dependencies() {
        let root = workspace();
        let home = workspace();
        file(&root.path().join("kestrel.project"), "package shop\n");
        file(&root.path().join("kestrel.lock"), "{ not json");
        let report = diagnose(&environment(root.path(), home.path()), &Fake::working());
        assert!(report.healthy());
        assert_eq!(status(&report, "lock"), Status::Warn);
    }

    #[test]
    fn a_missing_pin_is_reported() {
        let root = workspace();
        let home = workspace();
        file(
            &root.path().join("kestrel.project"),
            "package shop\ndepends prices from \"https://example.invalid/prices\" at \"v1\"\n",
        );
        file(
            &root.path().join("kestrel.lock"),
            "{\"version\":1,\"packages\":[]}",
        );
        let report = diagnose(&environment(root.path(), home.path()), &Fake::working());
        assert!(report.healthy());
        assert_eq!(status(&report, "lock"), Status::Warn);
        assert!(report.render().contains("has no pin"));
    }

    #[test]
    fn an_uncached_pin_is_reported() {
        let root = workspace();
        let home = workspace();
        file(
            &root.path().join("kestrel.project"),
            "package shop\ndepends prices from \"https://example.invalid/prices\" at \"v1\"\n",
        );
        let commit = "0123456789012345678901234567890123456789";
        let hash = "0".repeat(64);
        file(
            &root.path().join("kestrel.lock"),
            &format!(
                "{{\"version\":1,\"packages\":[{{\"name\":\"prices\",\"url\":\"https://example.invalid/prices\",\"reference\":\"v1\",\"commit\":\"{commit}\",\"tree_hash\":\"sha256:{hash}\"}}]}}"
            ),
        );
        let report = diagnose(&environment(root.path(), home.path()), &Fake::working());
        assert!(report.healthy());
        assert_eq!(status(&report, "lock"), Status::Warn);
        assert!(report.render().contains("pinned but not cached"));
    }

    #[test]
    fn the_summary_counts_failures_and_warnings() {
        let root = workspace();
        let home = workspace();
        let mut environment = environment(root.path(), home.path());
        environment.cc = Some("/nonexistent/clang".into());
        environment.z3 = None;
        let probe = Fake::working().with_library("libblas.so.3");
        let report = diagnose(&environment, &probe);
        assert!(
            report
                .render()
                .contains("1 required check failed, 1 warning.")
        );
    }
}
