use crate::paths::CanonicalPath;
use crate::{
    Parsed,
    checker::{self, Checked, Environment, Symbol, Visibility},
    diagnostic::Diagnostic,
    parse_file, parse_project,
    syntax::{Node, SyntaxKind as K},
    types::{FunctionType, Parameter, Passing, Type},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::{Path, PathBuf},
    sync::Mutex,
};

pub use crate::package_lock::{fetch_packages, update_package};

#[derive(Debug)]
pub struct ModuleCheck {
    pub parsed: Parsed,
    pub checked: Checked,
}

#[derive(Debug)]
pub struct ProjectCheck {
    pub modules: Vec<ModuleCheck>,
}

impl ProjectCheck {
    fn finish(mut modules: Vec<ModuleCheck>) -> Self {
        crate::proof::check_project_dispatches(&mut modules);
        Self { modules }
    }

    pub fn is_ok(&self) -> bool {
        self.modules.iter().all(|module| module.checked.is_ok())
    }
    pub fn render_diagnostics(&self) -> String {
        self.modules
            .iter()
            .flat_map(|module| {
                module
                    .checked
                    .diagnostics
                    .iter()
                    .map(|d| d.render(&module.parsed.source))
            })
            .collect()
    }
}

/// The file in a package's folder that names the package and its dependencies (Q07).
pub const PROJECT_FILE: &str = "kestrel.project";

/// A package folder and the sibling packages its project file depends on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Package {
    pub name: String,
    pub root: PathBuf,
    pub depends: Vec<PackageDependency>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageDependency {
    pub name: String,
    pub git: Option<GitSource>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitSource {
    pub url: String,
    pub reference: String,
}

/// Reads `folder/kestrel.project`; a folder without one is not a package.
pub fn read_package(folder: &Path) -> Result<Option<Package>, String> {
    let file = folder.join(PROJECT_FILE);
    let text = match fs::read_to_string(&file) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("Cannot read {}: {error}", file.display())),
    };
    let parsed = parse_project(file.display().to_string(), text);
    if !parsed.is_ok() {
        return Err(parsed
            .diagnostics
            .iter()
            .map(|d| d.render(&parsed.source))
            .collect());
    }
    let mut name = String::new();
    let mut depends = vec![];
    for node in &parsed.root.children {
        let text = parsed.source.slice(node.children[0].span).to_owned();
        if text.contains('.') {
            return Err(format!(
                "{}: package names are one lowercase word, not `{text}`.",
                file.display()
            ));
        }
        match node.kind {
            K::Package => name = text,
            K::Dependency => {
                let git = if node.children.len() == 3 {
                    let decode = |literal: &Node| -> Result<String, String> {
                        let raw = parsed.source.slice(literal.span);
                        let Some(inner) = raw.strip_prefix('"').and_then(|s| s.strip_suffix('"'))
                        else {
                            return Err(format!(
                                "{}: dependency source values must use double-quoted strings.",
                                file.display()
                            ));
                        };
                        crate::strings::decode(inner, literal.span.start + 1, false)
                            .map_err(|error| error.message)
                    };
                    let url = decode(&node.children[1])?;
                    let reference = decode(&node.children[2])?;
                    if url.is_empty()
                        || url.contains('\0')
                        || reference.is_empty()
                        || reference.contains('\0')
                    {
                        return Err(format!(
                            "{}: a Git dependency needs a nonempty URL and tag or commit.",
                            file.display()
                        ));
                    }
                    Some(GitSource { url, reference })
                } else {
                    None
                };
                depends.push(PackageDependency { name: text, git });
            }
            _ => {}
        }
    }
    Ok(Some(Package {
        name,
        root: folder.canonical().unwrap_or_else(|_| folder.to_owned()),
        depends,
    }))
}

/// The package at `root`, then every package it depends on, each found beside the
/// folder that names it (Q07).
pub fn load_packages(root: &Path) -> Result<Vec<Package>, String> {
    crate::package_lock::load_packages(root)
}

fn invalid(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

/// The standard data structures, written in Kestrel and loaded into every program (Q112).
const PRELUDE_SOURCE: &str = concat!(include_str!("prelude.kes"), include_str!("methods.kes"));

/// The virtual file that holds the prelude. It is never read from disk.
fn prelude_path(root: &Path) -> PathBuf {
    root.join("kestrel_prelude.kes")
}

/// Standard modules written in Kestrel and loaded virtually: name, file and source.
const VIRTUAL_MODULES: [(&str, &str, &str); 22] = [
    (
        "std.regex",
        "kestrel_std_regex.kes",
        include_str!("regex.kes"),
    ),
    ("std.toml", "kestrel_std_toml.kes", include_str!("toml.kes")),
    ("std.csv", "kestrel_std_csv.kes", include_str!("csv.kes")),
    (
        "std.statistics",
        "kestrel_std_statistics.kes",
        include_str!("statistics.kes"),
    ),
    (
        "std.encoding",
        "kestrel_std_encoding.kes",
        include_str!("encoding.kes"),
    ),
    ("std.term", "kestrel_std_term.kes", include_str!("term.kes")),
    ("std.json", "kestrel_std_json.kes", include_str!("json.kes")),
    ("std.path", "kestrel_std_path.kes", include_str!("path.kes")),
    (
        "std.datetime",
        "kestrel_std_datetime.kes",
        include_str!("datetime.kes"),
    ),
    ("std.args", "kestrel_std_args.kes", include_str!("args.kes")),
    (
        "std.complex",
        "kestrel_std_complex.kes",
        include_str!("complex.kes"),
    ),
    ("std.ml", "kestrel_std_ml.kes", include_str!("ml.kes")),
    (
        "std.kesdas",
        "kestrel_std_kesdas.kes",
        include_str!("kesdas.kes"),
    ),
    (
        "std.kesnet",
        "kestrel_std_kesnet.kes",
        include_str!("kesnet.kes"),
    ),
    (
        "std.kesnet.optim",
        "kestrel_std_kesnet_optim.kes",
        include_str!("kesnet_optim.kes"),
    ),
    (
        "std.kesnet.data",
        "kestrel_std_kesnet_data.kes",
        include_str!("kesnet_data.kes"),
    ),
    (
        "std.kesplot",
        "kestrel_std_kesplot.kes",
        include_str!("kesplot.kes"),
    ),
    (
        "std.algebra",
        "kestrel_std_algebra.kes",
        include_str!("algebra.kes"),
    ),
    (
        "std.func",
        "kestrel_std_function.kes",
        include_str!("functions.kes"),
    ),
    ("std.nat", "kestrel_std_nat.kes", include_str!("nat.kes")),
    ("std.real", "kestrel_std_real.kes", include_str!("real.kes")),
    ("std.set", "kestrel_std_set.kes", include_str!("set.kes")),
];

/// Checked results of the standard modules, kept for the life of the process.
static VIRTUAL_CHECKS: Mutex<BTreeMap<String, Checked>> = Mutex::new(BTreeMap::new());

/// The virtual file that holds a standard module. It is never read from disk.
fn virtual_path(root: &Path, file: &str) -> PathBuf {
    root.join(file)
}

/// A bare standard-module spelling, such as `kesdas` for `std.kesdas`.
/// `c` and `python` stay qualified because those words also start foreign imports.
fn standard_alias(name: &str) -> Option<String> {
    if matches!(name, "c" | "python") {
        return None;
    }
    let candidate = format!("std.{name}");
    (virtual_module(&candidate).is_some() || standard_module(&candidate).is_some())
        .then_some(candidate)
}

fn standard_virtual_alias(name: &str) -> Option<(&'static str, &'static str)> {
    if matches!(name, "c" | "python") {
        return None;
    }
    virtual_module(&format!("std.{name}"))
}

/// The virtual module imported under `name`, as (file, source).
fn virtual_module(name: &str) -> Option<(&'static str, &'static str)> {
    VIRTUAL_MODULES
        .iter()
        .find(|(module, _, _)| *module == name)
        .map(|(_, file, source)| (*file, *source))
}

pub fn check_path(path: impl AsRef<Path>, root: impl AsRef<Path>) -> io::Result<ProjectCheck> {
    let root = root.as_ref().canonical()?;
    let path = path.as_ref().canonical()?;
    let mut loader = Loader {
        entry: None,
        packages: load_packages(&root).map_err(invalid)?,
        overlay: None,
        session: None,
        sources: BTreeMap::new(),
        root,
        modules: vec![],
        loaded: BTreeMap::new(),
        visiting: BTreeSet::new(),
        pending: BTreeMap::new(),
        interfaces: BTreeMap::new(),
        cyclic: BTreeSet::new(),
        components: BTreeMap::new(),
        members: vec![],
        cycle_errors: BTreeMap::new(),
    };
    loader.entry = Some(path.clone());
    loader.plan(&path)?;
    loader.load(&path)?;
    Ok(ProjectCheck::finish(loader.modules))
}

/// Checks `text` as the file at `path`, resolving project-local imports from
/// `root`. Unsaved editor buffers use this: the path keeps the module identity,
/// while diagnostics name the buffer `-` as the extension expects.
pub fn check_text(
    path: impl AsRef<Path>,
    root: impl AsRef<Path>,
    text: String,
) -> io::Result<ProjectCheck> {
    let root = root.as_ref().canonical()?;
    let path = path.as_ref();
    let path = match path.canonical() {
        Ok(path) => path,
        Err(_) => {
            let parent = match path.parent() {
                Some(parent) if !parent.as_os_str().is_empty() => parent,
                _ => &root,
            };
            let name = path.file_name().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "The path needs a file name.")
            })?;
            parent.canonical()?.join(name)
        }
    };
    let mut sources = BTreeMap::new();
    sources.insert(path.clone(), text);
    let mut loader = Loader {
        entry: None,
        packages: load_packages(&root).map_err(invalid)?,
        sources,
        overlay: Some((path.clone(), "-".into())),
        session: None,
        root,
        modules: vec![],
        loaded: BTreeMap::new(),
        visiting: BTreeSet::new(),
        pending: BTreeMap::new(),
        interfaces: BTreeMap::new(),
        cyclic: BTreeSet::new(),
        components: BTreeMap::new(),
        members: vec![],
        cycle_errors: BTreeMap::new(),
    };
    loader.entry = Some(path.clone());
    loader.plan(&path)?;
    loader.load(&path)?;
    Ok(ProjectCheck::finish(loader.modules))
}

/// A session rechecks its accepted source history. Its synthetic root stays at index
/// zero, so retained native lambdas keep stable module identities as imports are added.
pub fn check_session(text: String, root: &Path) -> io::Result<ProjectCheck> {
    check_session_cached(text, root, &mut BTreeMap::new())
}

pub(crate) fn check_session_cached(
    text: String,
    root: &Path,
    sources: &mut BTreeMap<PathBuf, String>,
) -> io::Result<ProjectCheck> {
    let root = root.canonical()?;
    let path = root.join("session.kes");
    let mut loader = Loader {
        entry: None,
        packages: load_packages(&root).map_err(invalid)?,
        overlay: None,
        session: Some((path.clone(), text)),
        sources: sources.clone(),
        root,
        modules: vec![],
        loaded: BTreeMap::new(),
        visiting: BTreeSet::new(),
        pending: BTreeMap::new(),
        interfaces: BTreeMap::new(),
        cyclic: BTreeSet::new(),
        components: BTreeMap::new(),
        members: vec![],
        cycle_errors: BTreeMap::new(),
    };
    loader.plan(&path)?;
    let entry = loader.load(&path)?;
    let module = loader.modules.remove(entry);
    loader.modules.insert(0, module);
    *sources = loader.sources;
    Ok(ProjectCheck::finish(loader.modules))
}

struct Loader {
    /// The file a command names. Nothing imports it, so it needs no module-style name
    /// (REVIEW2 1.4): `kestrel run Documents/Hello-World.kes` works.
    entry: Option<PathBuf>,
    /// The project's own package first, then its dependencies; empty without a project file.
    packages: Vec<Package>,
    sources: BTreeMap<PathBuf, String>,
    /// The text of one file supplied by an editor, with the name used in diagnostics.
    overlay: Option<(PathBuf, String)>,
    session: Option<(PathBuf, String)>,
    root: PathBuf,
    modules: Vec<ModuleCheck>,
    loaded: BTreeMap<PathBuf, usize>,
    visiting: BTreeSet<PathBuf>,
    pending: BTreeMap<PathBuf, Parsed>,
    /// Header interfaces of certified cyclic modules, built when the first
    /// member of their component loads.
    interfaces: BTreeMap<PathBuf, Environment>,
    cyclic: BTreeSet<String>,
    /// The certified component each cyclic module belongs to.
    components: BTreeMap<PathBuf, usize>,
    members: Vec<Vec<PathBuf>>,
    /// Alias loops found while a cycle's interfaces settled, by the file that
    /// declares the alias.
    cycle_errors: BTreeMap<PathBuf, Vec<Diagnostic>>,
}

impl Loader {
    fn parse_source(&mut self, path: &Path) -> io::Result<Parsed> {
        if path == prelude_path(&self.root) {
            return Ok(parse_file(path.display().to_string(), PRELUDE_SOURCE));
        }
        for (module, file, source) in VIRTUAL_MODULES {
            if path == virtual_path(&self.root, file) {
                return Ok(parse_file(module, source));
            }
        }
        let session = self.session.as_ref().filter(|(root, _)| root == path);
        let source = match session {
            Some((_, text)) => text.clone(),
            None => match self.sources.get(path) {
                Some(text) => text.clone(),
                None => {
                    let text = fs::read_to_string(path)?;
                    self.sources.insert(path.to_owned(), text.clone());
                    text
                }
            },
        };
        // The editor supplies one file's text and still names it `-`, so the
        // checker's output matches the line prefix the extension reads.
        let overlay = self
            .overlay
            .as_ref()
            .filter(|(overlay, _)| overlay == path)
            .map(|(_, name)| name.clone());
        Ok(parse_file(
            if let Some(name) = overlay {
                name
            } else if session.is_some() {
                "<session>".into()
            } else {
                path.display().to_string()
            },
            source,
        ))
    }

    /// Discover components before checking bodies, so certification does not
    /// depend on which member was imported first. Each source is parsed once.
    fn plan(&mut self, path: &Path) -> io::Result<()> {
        let mut queue = vec![path.to_owned()];
        let mut graph = BTreeMap::<PathBuf, BTreeSet<PathBuf>>::new();
        while let Some(path) = queue.pop() {
            if self.pending.contains_key(&path) {
                continue;
            }
            let parsed = self.parse_source(&path)?;
            let mut edges = BTreeSet::new();
            if parsed.is_ok() {
                for import in parsed.root.children.iter().filter(|n| n.kind == K::Import) {
                    let Some(head) = import.children.first() else {
                        continue;
                    };
                    let name = crate::lexer::path_name(parsed.source.slice(head.span));
                    // An explicit standard module wins. A bare name resolves
                    // locally first, then as the short standard alias: the same
                    // order the import binder uses, so every file it loads was
                    // planned under the name the binder will ask for.
                    if let Some((file, _)) = virtual_module(&name) {
                        let target = virtual_path(&self.root, file);
                        edges.insert(target.clone());
                        queue.push(target);
                        continue;
                    }
                    if matches!(name.as_str(), "c" | "python") {
                        continue;
                    }
                    if let Ok(Some((_, _, files))) = self.local_import(&path, &name) {
                        if let [target] = files.as_slice() {
                            edges.insert(target.clone());
                            queue.push(target.clone());
                        }
                        continue;
                    }
                    if let Some((file, _)) = standard_virtual_alias(&name) {
                        let target = virtual_path(&self.root, file);
                        edges.insert(target.clone());
                        queue.push(target);
                        continue;
                    }
                    if standard_module(&name).is_some() {
                        continue;
                    }
                }
            }
            // Every other file depends on the prelude, so the prelude is planned and loaded with it.
            if path != prelude_path(&self.root) {
                edges.insert(prelude_path(&self.root));
                queue.push(prelude_path(&self.root));
            }
            graph.insert(path.clone(), edges);
            self.pending.insert(path, parsed);
        }
        // Two iterative graph walks find strongly connected components without
        // adding compiler stack depth for a wide, already visited import graph.
        let mut seen = BTreeSet::new();
        let mut order = vec![];
        for start in graph.keys() {
            let mut stack = vec![(start.clone(), false)];
            while let Some((node, finished)) = stack.pop() {
                if finished {
                    order.push(node);
                } else if seen.insert(node.clone()) {
                    stack.push((node.clone(), true));
                    stack.extend(graph[&node].iter().cloned().map(|next| (next, false)));
                }
            }
        }
        let mut reverse = BTreeMap::<PathBuf, Vec<PathBuf>>::new();
        for (node, edges) in &graph {
            for target in edges {
                reverse
                    .entry(target.clone())
                    .or_default()
                    .push(node.clone());
            }
        }
        seen.clear();
        for start in order.into_iter().rev() {
            if seen.contains(&start) {
                continue;
            }
            let mut members = vec![];
            let mut stack = vec![start];
            while let Some(node) = stack.pop() {
                if !seen.insert(node.clone()) {
                    continue;
                }
                members.push(node.clone());
                if let Some(edges) = reverse.get(&node) {
                    stack.extend(edges.iter().cloned());
                }
            }
            if (members.len() > 1 || graph[&members[0]].contains(&members[0]))
                && members.iter().all(|path| {
                    self.session.as_ref().is_none_or(|(root, _)| root != path)
                        && checker::cycle_certifiable(&self.pending[path])
                })
            {
                for path in &members {
                    self.components.insert(path.clone(), self.members.len());
                }
                self.cyclic
                    .extend(members.iter().map(|p| p.display().to_string()));
                self.members.push(members);
            }
        }
        Ok(())
    }

    fn load(&mut self, path: &Path) -> io::Result<usize> {
        if let Some(index) = self.loaded.get(path) {
            return Ok(*index);
        }
        if let Some(&component) = self.components.get(path)
            && !self.interfaces.contains_key(path)
        {
            self.build_interfaces(component)?;
        }
        self.visiting.insert(path.to_owned());
        let Some(parsed) = self.pending.remove(path) else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "E_MODULE_NOT_FOUND: no planned module for {}; check the import spelling.",
                    path.display()
                ),
            ));
        };
        let session = self.session.as_ref().is_some_and(|(root, _)| root == path);
        let mut diagnostics = vec![];
        let entry = self.entry.as_deref() == Some(path);
        let module_name = if session {
            String::new()
        } else {
            self.module_name(path)
        };
        let valid_name = |name: &str| {
            name.split('.').all(|part| {
                !part.is_empty()
                    && part.chars().next().is_some_and(char::is_lowercase)
                    && part.chars().all(|c| c.is_lowercase() || c.is_ascii_digit())
            })
        };
        // The entry file keeps a usable name even in a folder such as `Documents`.
        let module_name = if entry && !session && !valid_name(&module_name) {
            let stem = path
                .file_stem()
                .map(|stem| stem.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            let cleaned: String = stem
                .chars()
                .map(|c| {
                    if c.is_lowercase() || c.is_ascii_digit() {
                        c
                    } else {
                        'x'
                    }
                })
                .collect();
            if cleaned.chars().next().is_some_and(char::is_lowercase) {
                cleaned
            } else {
                format!("main{cleaned}")
            }
        } else {
            module_name
        };
        if !session && self.in_package_code(path) {
            for node in parsed.root.children.iter().filter(|n| is_test(n, &parsed)) {
                diagnostics.push(diag(
                    node,
                    "E_TEST_LOCATION",
                    "A @test function belongs in the package's tests folder.",
                    "Move it to tests/, under the same path as the file it tests (Q61).",
                ));
            }
        }
        if !session && !entry && path != prelude_path(&self.root) && !valid_name(&module_name) {
            diagnostics.push(
                Diagnostic::new(
                    "E_MODULE_NAMING",
                    "Module paths use lowercase dotted names.",
                    parsed.root.span,
                )
                .with_hint("Rename the file or folder to use lowercase letters and digits."),
            );
        }
        let (environment, found) = self.imports(path, &parsed, None)?;
        diagnostics.extend(found);
        // A standard module checks the same way every time, so a session checks it once.
        let cached = if virtual_module(&module_name).is_some() {
            VIRTUAL_CHECKS.lock().unwrap().get(&module_name).cloned()
        } else {
            None
        };
        let mut checked = match cached {
            Some(checked) => checked,
            None => {
                let checked = checker::check_with_imports(&parsed, &module_name, environment);
                if virtual_module(&module_name).is_some() && checked.diagnostics.is_empty() {
                    VIRTUAL_CHECKS
                        .lock()
                        .unwrap()
                        .insert(module_name.clone(), checked.clone());
                }
                checked
            }
        };
        for found in self.cycle_errors.remove(path).unwrap_or_default() {
            if !checked
                .diagnostics
                .iter()
                .any(|d| d.code == found.code && d.span == found.span)
            {
                diagnostics.push(found);
            }
        }
        checked.diagnostics.splice(0..0, diagnostics);
        checked.environment.proofs.hide_cycle_files(&self.cyclic);
        let index = self.modules.len();
        self.modules.push(ModuleCheck { parsed, checked });
        self.visiting.remove(path);
        self.loaded.insert(path.to_owned(), index);
        Ok(index)
    }

    /// Builds the header interfaces of a certified import cycle before any
    /// member's body is checked. Each round binds every member's imports to the
    /// previous round's interfaces, so a type or signature that mentions another
    /// member's declarations settles after enough rounds. A component that does
    /// not settle within one round per declaration loses its certification and
    /// reports E_IMPORT_CYCLE_UNSUPPORTED.
    fn build_interfaces(&mut self, component: usize) -> io::Result<()> {
        let members = self.members[component].clone();
        let declarations: usize = members
            .iter()
            .map(|path| self.pending[path].root.children.len())
            .sum();
        let mut current = members
            .iter()
            .map(|path| {
                let skeleton =
                    checker::cycle_skeleton(&self.pending[path], &self.module_name(path));
                (path.clone(), skeleton)
            })
            .collect::<BTreeMap<_, _>>();
        let shape = |interfaces: &BTreeMap<PathBuf, Environment>| {
            interfaces
                .values()
                .map(|e| format!("{:?}", (&e.names, &e.definitions, &e.values, &e.calls)))
                .collect::<Vec<_>>()
        };
        for _ in 0..declarations + 2 {
            let mut next = BTreeMap::new();
            for path in &members {
                let Some(parsed) = self.pending.remove(path) else {
                    return Err(io::Error::new(
                        io::ErrorKind::NotFound,
                        format!(
                            "E_MODULE_NOT_FOUND: no planned module for {}; check the import spelling.",
                            path.display()
                        ),
                    ));
                };
                let imported = self.imports(path, &parsed, Some(&current));
                let module = self.module_name(path);
                self.pending.insert(path.clone(), parsed);
                let (environment, _) = imported?;
                let (interface, loops) =
                    checker::cycle_interface(&self.pending[path], &module, environment);
                let known = self.cycle_errors.entry(path.clone()).or_default();
                for found in loops {
                    if !known.iter().any(|d| d.span == found.span) {
                        known.push(found);
                    }
                }
                next.insert(path.clone(), interface);
            }
            if shape(&next) == shape(&current) {
                self.interfaces.extend(next);
                return Ok(());
            }
            current = next;
        }
        for path in &members {
            self.cyclic.remove(&path.display().to_string());
            self.components.remove(path);
        }
        Ok(())
    }

    /// Binds a file's imports. While a cycle's interfaces are being built,
    /// `rounds` holds the previous round's interface of each member.
    fn imports(
        &mut self,
        path: &Path,
        parsed: &Parsed,
        rounds: Option<&BTreeMap<PathBuf, Environment>>,
    ) -> io::Result<(Environment, Vec<Diagnostic>)> {
        let session = self.session.as_ref().is_some_and(|(root, _)| root == path);
        let package = self.package_folder(path);
        let source_root = self.source_root(path);
        let mut environment = Environment::default();
        let mut diagnostics = vec![];
        let mut after_imports = false;
        if parsed.is_ok() {
            for node in &parsed.root.children {
                if node.kind != K::Import {
                    after_imports = true;
                    continue;
                }
                if after_imports && !session {
                    diagnostics.push(diag(
                        node,
                        "E_IMPORT_ORDER",
                        "Imports must come before declarations and script statements.",
                        "Move this import to the start of the file.",
                    ));
                }
                let Some(head) = node.children.first() else {
                    continue;
                };
                let name = crate::lexer::path_name(parsed.source.slice(head.span));
                if matches!(name.as_str(), "c" | "python") {
                    let directories = [
                        path.parent().unwrap_or(&self.root).to_owned(),
                        source_root.clone(),
                    ];
                    foreign_import(
                        node,
                        parsed,
                        &directories,
                        &mut environment,
                        &mut diagnostics,
                    );
                    continue;
                }
                if !matches!(name.as_str(), "c" | "python")
                    && let Err((code, message, hint)) = self.local_import(path, &name)
                {
                    diagnostics.push(diag(node, code, &message, hint));
                    continue;
                }
                let imported = if let Some((file, _)) = virtual_module(&name) {
                    let target = virtual_path(&self.root, file);
                    let index = self.load(&target)?;
                    exported_environment(&self.modules[index], true)
                } else if let Ok(Some((search_root, local_name, candidates))) =
                    self.local_import(path, &name)
                {
                    if candidates.len() > 1 {
                        diagnostics.push(diag(
                            node,
                            "E_MODULE_AMBIGUOUS",
                            &format!(
                                "Expected one file for module `{name}`, found {}.",
                                candidates.len()
                            ),
                            "Check the module path relative to the supplied project root.",
                        ));
                        continue;
                    }
                    {
                        let target = &candidates[0];
                        let same_package = package == target.parent().unwrap_or(&self.root);
                        let _ = &search_root;
                        let _ = &local_name;
                        let same_cycle = self
                            .components
                            .get(path)
                            .is_some_and(|c| self.components.get(target) == Some(c));
                        if let Some(interface) = rounds.and_then(|rounds| rounds.get(target)) {
                            visible_interface(interface, same_package)
                        } else if same_cycle && self.interfaces.contains_key(target) {
                            // Members see each other only through their headers, so
                            // what one member accepts never depends on which loads first.
                            if !self.visiting.contains(target) {
                                self.load(target)?;
                            }
                            visible_interface(&self.interfaces[target], same_package)
                        } else if self.visiting.contains(target) {
                            diagnostics.push(diag(node, "E_IMPORT_CYCLE_UNSUPPORTED", "This cycle needs interfaces the compiler cannot certify yet.", "Modules in an import cycle may declare functions, classes, traits, enums and aliases with typed parameters and results. Calls inside requires, constants and theorems need a module outside the cycle."));
                            continue;
                        } else if self.visiting.len() >= 128 {
                            diagnostics.push(diag(
                                node,
                                "E_IMPORT_CYCLE_UNSUPPORTED",
                                "This import cycle or nesting depth is not supported yet.",
                                "The compiler cannot certify cyclic module signatures yet.",
                            ));
                            continue;
                        } else {
                            let index = self.load(target)?;
                            let imported = &self.modules[index];
                            if is_script(&imported.parsed) {
                                diagnostics.push(diag(node, "E_IMPORT_SCRIPT", &format!("`{name}` has top-level statements and cannot be imported."), "Move reusable declarations into a module with no top-level statements."));
                                continue;
                            }
                            exported_environment(
                                imported,
                                package == target.parent().unwrap_or(&self.root),
                            )
                        }
                    }
                } else if let Some(standard) = standard_module(&name) {
                    standard
                } else if let Some(standard) = standard_alias(&name) {
                    if let Some((file, _)) = virtual_module(&standard) {
                        let target = virtual_path(&self.root, file);
                        let index = self.load(&target)?;
                        exported_environment(&self.modules[index], true)
                    } else {
                        standard_module(&standard).unwrap_or_default()
                    }
                } else {
                    diagnostics.push(diag(
                        node,
                        "E_MODULE_NOT_FOUND",
                        &format!("No local module or standard module matches `{name}`."),
                        "Check the module path, or import the standard module by its full `std.` name.",
                    ));
                    continue;
                };
                environment.definitions.extend(imported.definitions.clone());
                merge_calls(&mut environment.calls, &imported.calls);
                environment.proofs.extend(&imported.proofs);
                let Some(names) = node.child(K::ImportNames) else {
                    bind_namespace(node, parsed, &imported, &mut environment, &mut diagnostics);
                    continue;
                };
                for selected in &names.children {
                    let selected_name = parsed.source.slice(selected.span);
                    let original_name = if session {
                        selected_name
                            .strip_prefix("session__")
                            .or_else(|| selected_name.strip_prefix("Session__"))
                            .and_then(|s| s.split_once('_'))
                            .map(|(_, name)| name)
                            .unwrap_or(selected_name)
                    } else {
                        selected_name
                    };
                    let value = imported.values.get(original_name);
                    let ty = imported.names.get(original_name);
                    let theorem = imported.theorems.get(original_name);
                    if value.is_none() && ty.is_none() && theorem.is_none() {
                        diagnostics.push(diag(selected, "E_PRIVATE_IMPORT", &format!("`{selected_name}` is not an accessible export of `{name}`."), "Mark the declaration public, or package for another file in the same folder."));
                        continue;
                    }
                    if let Some(value) = value
                        && environment
                            .values
                            .insert(selected_name.into(), value.clone())
                            .is_some()
                    {
                        diagnostics.push(diag(
                            selected,
                            "E_DUPLICATE_IMPORT",
                            &format!("`{selected_name}` is imported more than once."),
                            "Give imported declarations distinct names.",
                        ));
                    }
                    if let Some(ty) = ty
                        && environment
                            .names
                            .insert(selected_name.into(), ty.clone())
                            .is_some()
                    {
                        diagnostics.push(diag(
                            selected,
                            "E_DUPLICATE_IMPORT",
                            &format!("Type `{selected_name}` is imported more than once."),
                            "Import this type once.",
                        ));
                    }
                    if let Some(theorem) = theorem
                        && environment
                            .theorems
                            .insert(selected_name.into(), theorem.clone())
                            .is_some()
                    {
                        diagnostics.push(diag(
                            selected,
                            "E_DUPLICATE_IMPORT",
                            &format!("Theorem `{selected_name}` is imported more than once."),
                            "Import this theorem once.",
                        ));
                    }
                }
            }
        }
        if path != prelude_path(&self.root) {
            let index = self.load(&prelude_path(&self.root))?;
            let prelude = exported_environment(&self.modules[index], true);
            merge_prelude(&mut environment, &prelude);
        }
        Ok((environment, diagnostics))
    }

    /// The package whose folder holds `path`.
    fn package_of(&self, path: &Path) -> Option<&Package> {
        self.packages.iter().find(|p| path.starts_with(&p.root))
    }

    /// Where `path` resolves its own imports: its package's folder, or the root.
    fn source_root(&self, path: &Path) -> PathBuf {
        self.package_of(path)
            .map(|p| p.root.clone())
            .unwrap_or_else(|| self.root.clone())
    }

    /// A dependency's modules carry its name: `matrix/ops.kes` is `matrix.ops`.
    fn module_name(&self, path: &Path) -> String {
        for (module, file, _) in VIRTUAL_MODULES {
            if path == virtual_path(&self.root, file) {
                return module.into();
            }
        }
        match self.package_of(path) {
            Some(package) if package.root != self.root => {
                format!("{}.{}", package.name, module_name(path, &package.root))
            }
            _ => module_name(path, &self.root),
        }
    }

    /// The folder whose `package` members `path` may use. A test file counts as the
    /// folder it mirrors: `tests/geometry/x.kes` sees `geometry/` (Q62).
    fn package_folder(&self, path: &Path) -> PathBuf {
        let folder = path.parent().unwrap_or(&self.root).to_owned();
        if let Some(package) = self.package_of(path)
            && let Ok(rest) = folder.strip_prefix(package.root.join("tests"))
        {
            return package.root.join(rest);
        }
        folder
    }

    /// A code file of a package, outside its tests folder. A folder below the package
    /// with its own project file is another package.
    fn in_package_code(&self, path: &Path) -> bool {
        let Some(package) = self.package_of(path) else {
            return false;
        };
        !path.starts_with(package.root.join("tests"))
            && path
                .ancestors()
                .skip(1)
                .take_while(|folder| *folder != package.root)
                .all(|folder| !folder.join(PROJECT_FILE).is_file())
    }

    /// The local files an import may name: package-root-relative first, then
    /// relative to the importing file. Returns the folder used and the file
    /// candidates, empty when the name belongs to the standard library.
    #[allow(clippy::type_complexity)]
    fn local_import(
        &self,
        importer: &Path,
        name: &str,
    ) -> Result<Option<(PathBuf, String, Vec<PathBuf>)>, (&'static str, String, &'static str)> {
        let (base, local) = self.search_root(importer, name)?;
        let files = candidates(&base, &local);
        if !files.is_empty() {
            return Ok(Some((base, local, files)));
        }
        let Some(folder) = importer.parent() else {
            return Ok(None);
        };
        if folder != base {
            let files = candidates(folder, &local);
            if !files.is_empty() {
                return Ok(Some((folder.to_owned(), local, files)));
            }
        }
        Ok(None)
    }

    /// The folder an import searches and the module name inside it. An import that
    /// starts with a declared dependency's name reads that package (Q07).
    fn search_root(
        &self,
        importer: &Path,
        name: &str,
    ) -> Result<(PathBuf, String), (&'static str, String, &'static str)> {
        let base = self.source_root(importer);
        let Some(package) = self.package_of(importer) else {
            return Ok((base, name.to_owned()));
        };
        let (first, rest) = name.split_once('.').unwrap_or((name, ""));
        if package.depends.iter().any(|d| d.name == first) {
            let dependency = self.packages.iter().find(|p| p.name == first).unwrap();
            if base.join(first).exists() || base.join(format!("{first}.kes")).exists() {
                return Err((
                    "E_DEPENDENCY_SHADOW",
                    format!(
                        "`{first}` names both a dependency and a module or folder of this package."
                    ),
                    "Rename the local folder or module so the import has one meaning.",
                ));
            }
            if rest.is_empty() {
                return Err((
                    "E_MODULE_NOT_FOUND",
                    format!("`{first}` is a package; import one of its modules."),
                    "Write the module path inside the package, as in `import matrix.ops.{multiply}`.",
                ));
            }
            return Ok((dependency.root.clone(), rest.to_owned()));
        }
        if candidates(&base, name).is_empty()
            && let Some(parent) = package.root.parent()
            && parent.join(first).join(PROJECT_FILE).is_file()
        {
            return Err((
                "E_UNDECLARED_DEPENDENCY",
                format!(
                    "`{first}` is a package that `{}` does not depend on.",
                    package.name
                ),
                "Add `depends` with that name to kestrel.project.",
            ));
        }
        Ok((base, name.to_owned()))
    }
}

fn candidates(root: &Path, name: &str) -> Vec<PathBuf> {
    let parts = name.split('.').collect::<Vec<_>>();
    let mut candidates = BTreeSet::new();
    // Both folder modules and existing dotted filenames are represented in this repository.
    for split in 0..parts.len() {
        let mut candidate = root.to_owned();
        for part in &parts[..split] {
            candidate.push(part);
        }
        candidate.push(format!("{}.kes", parts[split..].join(".")));
        if let Ok(canonical) = candidate.canonical()
            && canonical.is_file()
        {
            candidates.insert(canonical);
        }
    }
    candidates.into_iter().collect()
}

/// A top-level function marked `@test`.
pub fn is_test(node: &Node, parsed: &Parsed) -> bool {
    node.kind == K::Function
        && node.children.iter().any(|child| {
            child.kind == K::Attribute
                && child
                    .children
                    .first()
                    .is_some_and(|name| parsed.source.slice(name.span) == "test")
        })
}

fn module_name(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .with_extension("")
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(".")
}

pub fn is_script(parsed: &Parsed) -> bool {
    parsed.root.children.iter().any(|node| {
        !matches!(
            node.kind,
            K::Import
                | K::Function
                | K::Class
                | K::Trait
                | K::Enum
                | K::Alias
                | K::Const
                | K::Theorem
                | K::Axiom
                | K::Example
        )
    })
}

fn exported_environment(module: &ModuleCheck, same_package: bool) -> Environment {
    let mut result = Environment {
        definitions: module.checked.environment.definitions.clone(),
        calls: module.checked.environment.calls.clone(),
        proofs: module.checked.environment.proofs.clone(),
        ..Environment::default()
    };
    for node in &module.parsed.root.children {
        let name = if node.kind == K::Const {
            node.descendants()
                .find(|n| n.kind == K::Name && n.children.is_empty())
        } else {
            node.child(K::Name)
        };
        let Some(name) = name.map(|n| module.parsed.source.slice(n.span)) else {
            continue;
        };
        if matches!(node.kind, K::Theorem | K::Axiom) {
            let modifier = |word: &str| {
                node.children
                    .iter()
                    .any(|n| n.kind == K::Modifier && module.parsed.source.slice(n.span) == word)
            };
            let visibility = if modifier("public") {
                Visibility::Public
            } else if modifier("package") {
                Visibility::Package
            } else {
                Visibility::Private
            };
            if accessible(visibility, same_package) {
                let id = crate::resolution::DeclarationId {
                    file: module.parsed.source.name.clone(),
                    span: node.span,
                };
                result.theorems.insert(name.into(), id);
            }
            continue;
        }
        // A function from `extend List` is stored and exported as `List.take`.
        let value_name = match node.child(K::ExtendTarget) {
            Some(target) => format!(
                "{}.{name}",
                module.parsed.source.slice(target.children[0].span)
            ),
            None => name.to_owned(),
        };
        if let Some(symbol) = module.checked.environment.values.get(&value_name)
            && accessible(symbol.visibility, same_package)
        {
            result.values.insert(value_name, symbol.clone());
        }
        if let Some(id) = module.checked.environment.names.get(name)
            && module
                .checked
                .environment
                .definitions
                .get(id)
                .is_some_and(|d| accessible(d.visibility, same_package))
        {
            result.names.insert(name.into(), id.clone());
        }
    }
    result
}

/// Adds imported call facts. A fact from a checked body is never replaced by
/// one built from an import cycle's headers, which only overapproximates it.
fn merge_calls(
    calls: &mut BTreeMap<(String, String), checker::CallFacts>,
    imported: &BTreeMap<(String, String), checker::CallFacts>,
) {
    for (key, facts) in imported {
        if !(facts.header && calls.get(key).is_some_and(|known| !known.header)) {
            calls.insert(key.clone(), facts.clone());
        }
    }
}

/// What an importer may see of a cyclic member's header interface.
fn visible_interface(interface: &Environment, same_package: bool) -> Environment {
    let mut interface = interface.clone();
    interface
        .values
        .retain(|_, symbol| accessible(symbol.visibility, same_package));
    let definitions = &interface.definitions;
    interface.names.retain(|_, id| {
        definitions
            .get(id)
            .is_some_and(|d| accessible(d.visibility, same_package))
    });
    interface
}

fn accessible(visibility: Visibility, same_package: bool) -> bool {
    visibility == Visibility::Public || same_package && visibility == Visibility::Package
}

fn bind_namespace(
    node: &Node,
    parsed: &Parsed,
    imported: &Environment,
    environment: &mut Environment,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let path = crate::lexer::path_name(parsed.source.slice(node.children[0].span));
    let alias = node
        .children
        .get(1)
        .map(|name| parsed.source.slice(name.span))
        .unwrap_or_else(|| path.rsplit('.').next().unwrap_or(&path));
    if alias == "this" {
        diagnostics.push(diag(
            node,
            "E_RESERVED_NAME",
            "`this` names the current object, so it cannot name an imported module.",
            "Choose a different import alias.",
        ));
        return;
    }
    if environment.values.contains_key(alias)
        || environment.names.contains_key(alias)
        || crate::checker::builtin_arity(alias).is_some()
        || matches!(alias, "Nat" | "RuntimeError")
    {
        diagnostics.push(diag(
            node,
            "E_DUPLICATE_IMPORT",
            &format!("Import namespace `{alias}` conflicts with an existing name."),
            "Choose a different import alias.",
        ));
        return;
    }
    environment.values.insert(
        alias.into(),
        Symbol {
            ty: Type::Namespace(alias.into()),
            quantified: BTreeSet::new(),
            mutable: false,
            visibility: Visibility::Private,
            access: None,
            origin: None,
            reference: None,
        },
    );
    for (name, value) in &imported.values {
        if name.contains('.') && name.chars().next().is_some_and(char::is_uppercase) {
            if environment
                .values
                .get(name)
                .is_some_and(|previous| previous.ty != value.ty)
            {
                diagnostics.push(diag(
                    node,
                    "E_AMBIGUOUS_EXTENSION",
                    &format!("Several imports provide extension `{name}`."),
                    "Import only one implementation of this extension.",
                ));
            } else {
                environment.values.insert(name.clone(), value.clone());
            }
        }
        environment
            .values
            .insert(format!("{alias}.{name}"), value.clone());
    }
    for (name, ty) in &imported.names {
        environment
            .names
            .insert(format!("{alias}.{name}"), ty.clone());
        // A whole-module import also brings the module's type names into scope,
        // so `import std.kesdas as kd` lets a signature say `DataFrame` as well
        // as `kd.DataFrame`. Values still need the qualifier: this only adds the
        // type namespace, not unqualified functions or constants.
        match environment.names.get(name) {
            None => {
                environment.names.insert(name.clone(), ty.clone());
            }
            Some(existing) if existing == ty => {}
            // Built-in names such as List or Tensor keep the standard meaning.
            Some(existing) if !existing.contains('.') => {}
            Some(_) => {
                diagnostics.push(diag(
                    node,
                    "E_AMBIGUOUS_TYPE",
                    &format!("The type `{name}` is exported by more than one imported module."),
                    &format!(
                        "Write `{alias}.{name}` for the type from this import, or use a selective import."
                    ),
                ));
            }
        }
    }
    for (name, theorem) in &imported.theorems {
        environment
            .theorems
            .insert(format!("{alias}.{name}"), theorem.clone());
    }
}
fn diag(node: &Node, code: &'static str, message: &str, hint: &str) -> Diagnostic {
    Diagnostic::new(code, message, node.span).with_hint(hint)
}

/// A builtin function type whose calls need `with Effects`.
fn effectful(parameters: Vec<Type>, result: Type) -> Type {
    match Type::function(parameters, result) {
        Type::Function(mut function) => {
            function.effects = true;
            Type::Function(function)
        }
        other => other,
    }
}

fn standard_module(name: &str) -> Option<Environment> {
    if virtual_module(name).is_some() {
        return Some(virtual_environment(name));
    }
    if let Some(env) = crate::numerical::environment(name) {
        return Some(env);
    }
    let mut env = Environment::default();
    let t = Type::Parameter("T".into());
    let namespace = name;
    let mut add = |name: &str, mut ty: Type| {
        let target = crate::resolution::Callable::Builtin(format!("{namespace}.{name}"));
        match &mut ty {
            Type::Function(function) => function.callable = Some(target),
            Type::Overloaded(functions) => {
                for function in functions {
                    function.callable = Some(target.clone());
                }
            }
            _ => {}
        }
        env.values.insert(
            name.into(),
            Symbol {
                ty,
                quantified: BTreeSet::new(),
                mutable: false,
                visibility: Visibility::Public,
                access: None,
                origin: None,
                reference: Some(crate::resolution::Reference::Builtin(format!(
                    "{namespace}.{name}"
                ))),
            },
        );
    };
    match name {
        "std.memory" => {
            for name in ["Arena", "ArenaRef"] {
                env.names.insert(name.into(), name.into());
            }
        }
        "std.collections" => {
            for name in [
                "List",
                "Array",
                "Map",
                "MutMap",
                "Set",
                "MutSet",
                "SortedSet",
            ] {
                env.names.insert(name.into(), name.into());
            }
        }
        "std.c" => {
            for name in crate::interop::C_TYPES {
                env.names.insert(name.into(), name.into());
            }
            for name in ["CPointer", "CConstPointer", "CVoid"] {
                env.names.insert(name.into(), name.into());
            }
        }
        "std.python" => {
            env.names.insert("PythonValue".into(), "PythonValue".into());
        }
        "std.core" => {
            for name in [
                "Option",
                "Tensor",
                "DynTensor",
                "Variable",
                "Error",
                "RuntimeError",
                "Text",
                "Equals",
                "Comparable",
                "Iterable",
                "Iterator",
                "Multipliable",
                "RightMultipliable",
                "Box",
            ] {
                env.names.insert(name.into(), name.into());
            }
        }
        "std.test" => {
            let fun = FunctionType {
                parameters: vec![
                    Parameter {
                        name: Some("a".into()),
                        ty: t.clone(),
                        default: false,
                        passing: Passing::Borrow,
                    },
                    Parameter {
                        name: Some("b".into()),
                        ty: t,
                        default: false,
                        passing: Passing::Borrow,
                    },
                ],
                result: Box::new(Type::named("void")),
                bounds: BTreeMap::from([("T".into(), vec![Type::named("Equals")])]),
                requirements: vec![],
                effects: false,
                receiver_mutates: false,
                callable: None,
                receiver: None,
            };
            let mut not_equal = fun.clone();
            not_equal.callable = None;
            add(
                "assertError",
                Type::function(vec![Type::Parameter("E".into())], Type::named("void")),
            );
            add("assertEq", Type::Function(fun));
            add("assertNotEq", Type::Function(not_equal));
            for name in ["assertTrue", "assertFalse"] {
                add(
                    name,
                    Type::function(vec![Type::named("Bool")], Type::named("void")),
                );
            }
            add(
                "assertNear",
                Type::function(vec![Type::named("double"); 3], Type::named("void")),
            );
        }
        "std.process" => {
            add("args", Type::applied("List", vec![Type::named("String")]));
            let string = || Type::named("String");
            add(
                "exit",
                effectful(vec![Type::named("Int")], Type::named("void")),
            );
            add(
                "env",
                effectful(vec![string()], Type::applied("Option", vec![string()])),
            );
            add("cwd", effectful(vec![], string()));
            // run(command) runs a shell command and gives its exit status and standard output.
            add(
                "run",
                effectful(
                    vec![string()],
                    Type::union(
                        Type::Tuple(vec![Type::named("Int"), string()]),
                        Type::named("Error"),
                    ),
                ),
            );
        }
        "std.bytes" => {
            let bytes = || Type::named("Bytes");
            let text = || Type::named("String");
            let int = || Type::named("Int");
            add(
                "fromList",
                Type::function(
                    vec![Type::applied("List", vec![int()])],
                    Type::union(bytes(), Type::named("Error")),
                ),
            );
            add("fromUtf8", Type::function(vec![text()], bytes()));
            add(
                "decodeUtf8",
                Type::function(vec![bytes()], Type::union(text(), Type::named("Error"))),
            );
            add(
                "toList",
                Type::function(vec![bytes()], Type::applied("List", vec![int()])),
            );
            add("toHex", Type::function(vec![bytes()], text()));
            env.names.insert("Bytes".into(), "Bytes".into());
        }
        // Files and folders (REVIEW2 section 4). Each call needs `with Effects`.
        "std.fs" => {
            let string = || Type::named("String");
            let fallible = |ty: Type| Type::union(ty, Type::named("Error"));
            add("readFile", effectful(vec![string()], fallible(string())));
            add(
                "readBytes",
                effectful(vec![string()], fallible(Type::named("Bytes"))),
            );
            for name in ["writeBytes", "appendBytes"] {
                add(
                    name,
                    effectful(
                        vec![string(), Type::named("Bytes")],
                        fallible(Type::named("void")),
                    ),
                );
            }
            for name in ["writeFile", "appendFile"] {
                add(
                    name,
                    effectful(vec![string(), string()], fallible(Type::named("void"))),
                );
            }
            for name in ["exists", "isDir"] {
                add(name, effectful(vec![string()], Type::named("Bool")));
            }
            add(
                "listDir",
                effectful(
                    vec![string()],
                    fallible(Type::applied("List", vec![string()])),
                ),
            );
            for name in ["createDirs", "remove"] {
                add(
                    name,
                    effectful(vec![string()], fallible(Type::named("void"))),
                );
            }
            for name in ["rename", "copy"] {
                add(
                    name,
                    effectful(vec![string(), string()], fallible(Type::named("void"))),
                );
            }
            add("tempDir", effectful(vec![], fallible(string())));
        }
        // Console output without a newline, standard error, and a prompt.
        "std.io" => {
            let string = || Type::named("String");
            for name in ["write", "eprint"] {
                add(name, effectful(vec![string()], Type::named("void")));
            }
            add("flush", effectful(vec![], Type::named("void")));
            add(
                "input",
                effectful(vec![string()], Type::union(string(), Type::named("Error"))),
            );
        }
        // The engine behind std.regex: a pattern's error text, and one match's byte offsets.
        "std.rx" => {
            let string = || Type::named("String");
            add("check", Type::function(vec![string()], string()));
            add(
                "find",
                Type::function(
                    vec![string(), string(), Type::named("Int")],
                    Type::applied("List", vec![Type::named("Int")]),
                ),
            );
        }
        // Wall time in seconds, a monotonic clock in nanoseconds, and sleep.
        "std.time" => {
            add("now", effectful(vec![], Type::named("double")));
            add("monotonic", effectful(vec![], Type::named("Int")));
            add(
                "sleep",
                effectful(vec![Type::named("double")], Type::named("void")),
            );
        }
        "std.math" => {
            for name in crate::numerical::UNARY {
                if matches!(*name, "sqrt" | "abs" | "floor" | "ceil" | "round") {
                    continue;
                }
                add(
                    name,
                    Type::function(vec![Type::named("double")], Type::named("double")),
                );
            }
            for name in ["PI", "E"] {
                add(name, Type::named("double"));
            }
            add(
                "sqrt",
                Type::function(vec![Type::named("double")], Type::named("double")),
            );
            add(
                "pow",
                Type::function(vec![Type::named("double"); 2], Type::named("double")),
            );
            for name in ["floor", "ceil", "round"] {
                add(
                    name,
                    Type::function(vec![Type::named("double")], Type::named("double")),
                );
            }
            for name in ["abs", "min", "max"] {
                let arity = if name == "abs" { 1 } else { 2 };
                let functions = ["Int", "double", "BigInt"]
                    .into_iter()
                    .map(
                        |ty| match Type::function(vec![Type::named(ty); arity], Type::named(ty)) {
                            Type::Function(fun) => fun,
                            _ => unreachable!(),
                        },
                    )
                    .collect();
                add(name, Type::Overloaded(functions));
            }
        }
        "std.str" => {
            for name in ["trim", "toUpper", "toLower"] {
                add(
                    name,
                    Type::function(vec![Type::named("String")], Type::named("String")),
                );
            }
            add(
                "length",
                Type::function(vec![Type::named("String")], Type::named("Int")),
            );
            add(
                "contains",
                Type::function(vec![Type::named("String"); 2], Type::named("Bool")),
            );
            add(
                "split",
                Type::function(
                    vec![Type::named("String"); 2],
                    Type::applied("List", vec![Type::named("String")]),
                ),
            );
        }
        _ => return None,
    }
    if name == "std.math" {
        for name in crate::numerical::UNARY {
            let Some(symbol) = env.values.get_mut(*name) else {
                continue;
            };
            let Type::Function(mut tensor) = Type::function(
                vec![crate::types::tensor_type(None)],
                crate::types::tensor_type(None),
            ) else {
                unreachable!()
            };
            tensor.callable = Some(crate::resolution::Callable::Builtin(format!(
                "std.math.{name}"
            )));
            let mut functions = match &symbol.ty {
                Type::Function(f) => vec![f.clone()],
                Type::Overloaded(fs) => fs.clone(),
                _ => vec![],
            };
            functions.push(tensor);
            symbol.ty = Type::Overloaded(functions);
        }
    }
    if name == "std.core" {
        env.names
            .insert("RuntimeError".into(), "std.core.RuntimeError".into());
    }
    Some(env)
}

/// For isolated sources without filesystem imports. Import-aware checking uses check_path.
/// The header or Python module named in a foreign import, without its quotes.
pub fn foreign_name(node: &Node, parsed: &Parsed) -> Option<String> {
    let literal = node.children.get(1)?;
    let text = parsed.source.slice(literal.span);
    crate::strings::decode(&text[1..text.len() - 1], literal.span.start + 1, false).ok()
}

/// `import c "header.h"` adds the header's functions; each call must sit in `unsafe`.
fn foreign_import(
    node: &Node,
    parsed: &Parsed,
    directories: &[PathBuf],
    environment: &mut Environment,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let kind = parsed.source.slice(node.children[0].span);
    let Some(target) = foreign_name(node, parsed) else {
        diagnostics.push(diag(
            node,
            "E_IMPORT",
            "This foreign import needs a quoted name.",
            "Write import c \"header.h\".",
        ));
        return;
    };
    if kind == "python" {
        // The module is imported the first time the program uses it (Q46).
        let local = match node.children.get(2) {
            Some(alias) => parsed.source.slice(alias.span).to_owned(),
            None => target.clone(),
        };
        let valid = local.chars().next().is_some_and(|c| c.is_ascii_lowercase())
            && local.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        if !valid {
            diagnostics.push(diag(
                node,
                "E_IMPORT",
                &format!("`{target}` needs a local name."),
                "Write import python \"module\" as name, with a lowercase name.",
            ));
            return;
        }
        if environment.values.contains_key(&local) {
            diagnostics.push(diag(
                node,
                "E_DUPLICATE_IMPORT",
                &format!("`{local}` is imported more than once."),
                "Give the Python module a distinct local name.",
            ));
            return;
        }
        environment.values.insert(
            local,
            Symbol {
                ty: Type::named("PythonValue"),
                quantified: BTreeSet::new(),
                mutable: false,
                visibility: Visibility::Private,
                access: None,
                origin: None,
                reference: Some(crate::resolution::Reference::Builtin(format!(
                    "python:{target}"
                ))),
            },
        );
        return;
    }
    let (functions, unsupported) = match crate::interop::read_header(&target, directories) {
        Ok(header) => header,
        Err(message) => {
            diagnostics.push(diag(
                node,
                "E_C_HEADER",
                &message,
                "Check the header name, and that clang is on PATH.",
            ));
            return;
        }
    };
    if let Some(path) = crate::interop::header_path(&target, directories) {
        environment.c_headers.insert(target.clone(), path);
    }
    for function in functions {
        for ty in function
            .parameters
            .iter()
            .chain(std::iter::once(&function.result))
        {
            ty.map(&mut |part| {
                if let Type::Named(name, _) = part
                    && ["c.struct.", "c.union.", "c.enum."]
                        .iter()
                        .any(|prefix| name.starts_with(prefix))
                {
                    let tag = name.rsplit('.').next().unwrap();
                    if environment.names.get(tag).is_some_and(|old| old != name) {
                        diagnostics.push(diag(
                            node,
                            "E_DUPLICATE_IMPORT",
                            &format!("C tag `{tag}` conflicts with an imported type."),
                            "Use a fixed-signature C wrapper with distinct tag names.",
                        ));
                    } else {
                        environment.names.insert(tag.to_owned(), name.clone());
                    }
                }
                None
            });
        }
        let mut ty = Type::function(function.parameters, function.result);
        let reference = format!("c:{}", function.name);
        if let Type::Function(signature) = &mut ty {
            // Running C code is outside work.
            signature.effects = true;
            signature.callable = Some(crate::resolution::Callable::Builtin(reference.clone()));
        }
        if let Some(existing) = environment.values.get(&function.name) {
            // Two headers may declare one C function: the Windows C runtime declares
            // abs in both math.h and stdlib.h. The same signature is the same import.
            if existing.ty == ty {
                continue;
            }
            diagnostics.push(diag(
                node,
                "E_DUPLICATE_IMPORT",
                &format!(
                    "`{}` from `{target}` has the same name as another import.",
                    function.name
                ),
                "Import the other declaration under its module path, or leave it out.",
            ));
            continue;
        }
        environment.values.insert(
            function.name.clone(),
            Symbol {
                ty,
                quantified: BTreeSet::new(),
                mutable: false,
                visibility: Visibility::Private,
                access: None,
                origin: None,
                reference: Some(crate::resolution::Reference::Builtin(reference)),
            },
        );
    }
    for (name, reason) in unsupported {
        environment.foreign.entry(name).or_insert(reason);
    }
}

/// Adds the prelude's public names and the math names to a file's environment, without an import.
/// A local declaration with the same name is kept, so a program may define its own Stack (Q112)
/// or max (Q114). The math names are read only after the file's own values, so they never clash
/// with a declaration or a local binding.
fn merge_prelude(environment: &mut Environment, prelude: &Environment) {
    environment.definitions.extend(prelude.definitions.clone());
    merge_calls(&mut environment.calls, &prelude.calls);
    environment.proofs.extend(&prelude.proofs);
    for (name, ty) in &prelude.names {
        environment
            .names
            .entry(name.clone())
            .or_insert_with(|| ty.clone());
    }
    // The prelude's `extend` functions, such as `List.take`, are methods everywhere.
    for (name, symbol) in &prelude.values {
        if name
            .split_once('.')
            .is_some_and(|(head, _)| head.starts_with(|c: char| c.is_ascii_uppercase()))
        {
            environment
                .values
                .entry(name.clone())
                .or_insert_with(|| symbol.clone());
        }
    }
    if let Some(math) = standard_module("std.math") {
        for (name, symbol) in math.values {
            if matches!(
                name.as_str(),
                "sqrt" | "abs" | "min" | "max" | "pow" | "floor" | "ceil" | "round" | "PI" | "E"
            ) {
                environment.implicit.entry(name).or_insert(symbol);
            }
        }
    }
}

/// The checked environment of a virtual standard module, for imports and for text
/// checked without the loader.
fn virtual_environment(name: &str) -> Environment {
    static CACHE: Mutex<BTreeMap<String, Environment>> = Mutex::new(BTreeMap::new());
    if let Some(environment) = CACHE.lock().unwrap().get(name) {
        return environment.clone();
    }
    let (_, source) = virtual_module(name).expect("a virtual module");
    let parsed = parse_file(name, source);
    let (mut environment, _) = source_imports(&parsed);
    merge_prelude(&mut environment, &prelude_environment());
    let checked = checker::check_with_imports(&parsed, name, environment);
    let exported = exported_environment(&ModuleCheck { parsed, checked }, true);
    CACHE
        .lock()
        .unwrap()
        .insert(name.to_owned(), exported.clone());
    exported
}

/// The prelude as an environment, for a file checked from text rather than loaded from disk.
fn prelude_environment() -> Environment {
    let parsed = parse_file("kestrel_prelude.kes", PRELUDE_SOURCE);
    let checked = checker::check_with_imports(&parsed, "", Environment::default());
    exported_environment(&ModuleCheck { parsed, checked }, true)
}

pub fn check_source(name: impl Into<String>, text: impl Into<String>) -> ModuleCheck {
    let parsed = parse_file(name, text);
    let (mut environment, diagnostics) = source_imports(&parsed);
    // The editor checks its buffer as text, so the prelude and math names must reach it too.
    merge_prelude(&mut environment, &prelude_environment());
    let mut checked = checker::check_with_imports(&parsed, "", environment);
    checked.diagnostics.splice(0..0, diagnostics);
    ModuleCheck { parsed, checked }
}

pub(crate) fn source_imports(parsed: &Parsed) -> (Environment, Vec<Diagnostic>) {
    let mut environment = Environment::default();
    let mut diagnostics = vec![];
    let mut after_imports = false;
    for import in &parsed.root.children {
        if import.kind != K::Import {
            after_imports = true;
            continue;
        }
        if after_imports {
            diagnostics.push(diag(
                import,
                "E_IMPORT_ORDER",
                "Imports must come before declarations and script statements.",
                "Move this import to the start of the file.",
            ));
        }
        let Some(head) = import.children.first() else {
            continue;
        };
        let name = crate::lexer::path_name(parsed.source.slice(head.span));
        if matches!(name.as_str(), "c" | "python") {
            let directories = std::env::current_dir().into_iter().collect::<Vec<_>>();
            foreign_import(
                import,
                parsed,
                &directories,
                &mut environment,
                &mut diagnostics,
            );
            continue;
        }
        let standard = standard_module(&name)
            .or_else(|| standard_alias(&name).and_then(|full| standard_module(&full)));
        if let Some(standard) = standard {
            environment.definitions.extend(standard.definitions.clone());
            environment.calls.extend(standard.calls.clone());
            // Most standard modules have no theorems. Copying their proof
            // tables for every import is wasted work and deep clones.
            if !standard.theorems.is_empty() {
                environment.proofs.extend(&standard.proofs);
            }
            if let Some(names) = import.child(K::ImportNames) {
                for name in &names.children {
                    let name_text = parsed.source.slice(name.span);
                    if let Some(value) = standard.values.get(name_text) {
                        environment.values.insert(name_text.into(), value.clone());
                    } else if let Some(ty) = standard.names.get(name_text) {
                        environment.names.insert(name_text.into(), ty.clone());
                    } else if let Some(theorem) = standard.theorems.get(name_text) {
                        environment
                            .theorems
                            .insert(name_text.into(), theorem.clone());
                    } else {
                        diagnostics.push(diag(
                            name,
                            "E_PRIVATE_IMPORT",
                            "This standard module does not export the selected name.",
                            "Use a specified standard-library name.",
                        ));
                    }
                }
            } else {
                environment.definitions.extend(standard.definitions.clone());
                environment.calls.extend(standard.calls.clone());
                bind_namespace(
                    import,
                    parsed,
                    &standard,
                    &mut environment,
                    &mut diagnostics,
                );
            }
        } else {
            diagnostics.push(diag(
                import,
                "E_IMPORT_ROOT_REQUIRED",
                "This import needs filesystem module resolution.",
                "Use `kestrel check --root PATH FILE`, or the check_path library entry point.",
            ));
        }
    }
    (environment, diagnostics)
}
