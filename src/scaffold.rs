//! `kestrel new`: writes a package folder that checks, runs and tests on the first try.
use crate::modules::PROJECT_FILE;
use crate::package_lock::clean_git_command;
use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

/// The longest package name `kestrel new` writes.
const MAX_NAME: usize = 64;

/// Windows cannot create these folders, whatever their contents.
const WINDOWS_DEVICES: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// The shape of package `kestrel new` writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Template {
    /// An entry point, a module and a test.
    Bin,
    /// A module and a test, with no entry point.
    Lib,
}

/// A package folder `kestrel new` created.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Created {
    /// The package name, which is the folder name.
    pub name: String,
    /// The folder that was created, as it was named on the command line.
    pub folder: PathBuf,
    /// The template that was written.
    pub template: Template,
}

/// Writes a new package at `folder`; refuses an existing folder, an invalid name,
/// and a failing `git init`. On any failure the folder is removed again.
pub fn create(folder: &Path, template: Template, git: bool) -> Result<Created, String> {
    let Some(name) = folder.file_name().and_then(|name| name.to_str()) else {
        return Err("`kestrel new` needs a folder name, such as `hello`.".into());
    };
    check_name(name)?;
    fs::create_dir(folder).map_err(|error| match error.kind() {
        ErrorKind::AlreadyExists => format!(
            "`{name}` already exists. `kestrel new` writes a new folder; pick another name."
        ),
        ErrorKind::NotFound => format!(
            "Cannot create {}: its parent folder does not exist.",
            folder.display()
        ),
        _ => format!("Cannot create {}: {error}", folder.display()),
    })?;
    if let Err(error) = populate(folder, name, template, git) {
        let _ = fs::remove_dir_all(folder);
        return Err(error);
    }
    Ok(Created {
        name: name.to_owned(),
        folder: folder.to_owned(),
        template,
    })
}

/// Why `name` cannot name a package, or `None` when it can.
fn name_error(name: &str) -> Option<String> {
    let mut chars = name.chars();
    let starts_right = chars.next().is_some_and(|c| c.is_ascii_lowercase());
    if !starts_right || !chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
        return Some(format!(
            "`{name}` is not a package name. Use lowercase letters, digits and underscores, starting with a letter."
        ));
    }
    if name.len() > MAX_NAME {
        return Some(format!(
            "`{name}` is {} characters long; a package name may have at most {MAX_NAME}.",
            name.len()
        ));
    }
    if crate::parser::reserved(name) {
        return Some(format!(
            "`{name}` is a Kestrel keyword, so it cannot name a package."
        ));
    }
    if WINDOWS_DEVICES.contains(&name) {
        return Some(format!(
            "`{name}` is a reserved device name on Windows, so it cannot name a package."
        ));
    }
    None
}

fn check_name(name: &str) -> Result<(), String> {
    name_error(name).map_or(Ok(()), Err)
}

fn populate(folder: &Path, name: &str, template: Template, git: bool) -> Result<(), String> {
    write(&folder.join(PROJECT_FILE), &format!("package {name}\n"))?;
    if template == Template::Bin {
        write(&folder.join("main.kes"), MAIN)?;
    }
    write(&folder.join("greet.kes"), GREETING)?;
    fs::create_dir(folder.join("tests"))
        .map_err(|error| format!("Cannot create {}: {error}", folder.join("tests").display()))?;
    write(&folder.join("tests/greet.kes"), TEST)?;
    write(&folder.join(".gitignore"), GITIGNORE)?;
    if git {
        initialize_git(folder)?;
    }
    Ok(())
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    fs::write(path, text).map_err(|error| format!("Cannot write {}: {error}", path.display()))
}

/// Starts a repository the way `git init` does, without inheriting a parent's
/// `GIT_DIR` or hooks.
fn initialize_git(folder: &Path) -> Result<(), String> {
    let output = clean_git_command()
        .args(["init", "--quiet"])
        .arg(folder)
        .output()
        .map_err(|error| format!("Cannot start Git to initialize the repository: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let detail = stderr.trim();
    Err(if detail.is_empty() {
        "Git could not initialize the repository.".into()
    } else {
        format!("Git could not initialize the repository: {detail}")
    })
}

/// The entry point of a runnable package.
const MAIN: &str = r#"import greet.{greeting}

print(greeting("world"))
"#;

/// The module the entry point and the test both import.
const GREETING: &str = r#"public function greeting(who : String) : String = f"Hello, {who}!"
"#;

/// The test `kestrel test` runs.
const TEST: &str = r#"import greet.{greeting}
import std.test.{assertEq}

@test
function greetsTheWorld() : void {
    assertEq(greeting("world"), "Hello, world!")
}
"#;

/// What `kestrel build` leaves beside its source.
const GITIGNORE: &str = r#"# `kestrel build` writes the executable beside its source.
/main
*.exe
"#;
