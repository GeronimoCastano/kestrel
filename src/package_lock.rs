//! Reproducible Git package pins and the per-user source cache.
use crate::modules::{GitSource, PROJECT_FILE, Package, PackageDependency, read_package};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

const LOCK_FILE: &str = "kestrel.lock";
const LOCK_VERSION: u64 = 1;
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, PartialEq, Eq)]
struct LockedPackage {
    name: String,
    url: String,
    reference: String,
    commit: String,
    tree_hash: String,
}

#[derive(Clone, Debug, Default)]
struct Lockfile {
    packages: BTreeMap<String, LockedPackage>,
}

impl Lockfile {
    fn read(root: &Path) -> Result<Self, String> {
        let path = root.join(LOCK_FILE);
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(error) => return Err(format!("Cannot read {}: {error}", path.display())),
        };
        let json: serde_json::Value = serde_json::from_str(&text)
            .map_err(|error| format!("{} is not valid JSON: {error}", path.display()))?;
        if json.get("version").and_then(serde_json::Value::as_u64) != Some(LOCK_VERSION) {
            return Err(format!(
                "{} has an unsupported lockfile version; expected {LOCK_VERSION}.",
                path.display()
            ));
        }
        let entries = json
            .get("packages")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| format!("{} must contain a `packages` array.", path.display()))?;
        let mut packages = BTreeMap::new();
        for entry in entries {
            let field = |name: &str| {
                entry
                    .get(name)
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
                    .ok_or_else(|| {
                        format!("{} has a package entry without `{name}`.", path.display())
                    })
            };
            let package = LockedPackage {
                name: field("name")?,
                url: field("url")?,
                reference: field("reference")?,
                commit: field("commit")?,
                tree_hash: field("tree_hash")?,
            };
            if !valid_package_name(&package.name)
                || !valid_object_id(&package.commit)
                || !package.tree_hash.starts_with("sha256:")
                || package.tree_hash.len() != "sha256:".len() + 64
                || !package.tree_hash["sha256:".len()..]
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit())
            {
                return Err(format!(
                    "{} has an invalid lock entry for `{}`.",
                    path.display(),
                    package.name
                ));
            }
            validate_git_url(&package.url).map_err(|error| {
                format!(
                    "{} has an invalid source URL for `{}`: {error}",
                    path.display(),
                    package.name
                )
            })?;
            if packages
                .insert(package.name.clone(), package.clone())
                .is_some()
            {
                return Err(format!(
                    "{} contains more than one pin for `{}`.",
                    path.display(),
                    package.name
                ));
            }
        }
        Ok(Self { packages })
    }

    fn write(&self, root: &Path) -> Result<(), String> {
        let path = root.join(LOCK_FILE);
        let packages: Vec<_> = self
            .packages
            .values()
            .map(|package| {
                serde_json::json!({
                    "name": package.name,
                    "url": package.url,
                    "reference": package.reference,
                    "commit": package.commit,
                    "tree_hash": package.tree_hash,
                })
            })
            .collect();
        let text = serde_json::to_vec_pretty(&serde_json::json!({
            "version": LOCK_VERSION,
            "packages": packages,
        }))
        .map_err(|error| format!("Cannot encode {}: {error}", path.display()))?;
        let temp = unique_path(root, "kestrel.lock.tmp");
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|error| format!("Cannot create {}: {error}", temp.display()))?;
        let result = (|| {
            file.write_all(&text)?;
            file.write_all(b"\n")?;
            file.sync_all()?;
            fs::rename(&temp, &path)
        })();
        if let Err(error) = result {
            let _ = fs::remove_file(&temp);
            return Err(format!("Cannot write {}: {error}", path.display()));
        }
        Ok(())
    }
}

/// Fetches newly declared Git sources while preserving existing exact pins.
pub fn fetch_packages(root: &Path) -> Result<usize, String> {
    synchronize(root, None)
}

/// Moves the pin for one named Git package; all other existing pins stay fixed.
pub fn update_package(root: &Path, name: &str) -> Result<usize, String> {
    if !valid_package_name(name) {
        return Err(format!("`{name}` is not a package name."));
    }
    synchronize(root, Some(name))
}

fn synchronize(root: &Path, update: Option<&str>) -> Result<usize, String> {
    let root = root
        .canonicalize()
        .map_err(|error| format!("Cannot open package folder {}: {error}", root.display()))?;
    let Some(package) = read_package(&root)? else {
        return Err(format!("{} has no {PROJECT_FILE}.", root.display()));
    };
    let old = Lockfile::read(&root)?;
    let mut sync = SyncState {
        old,
        new: Lockfile::default(),
        selected: BTreeMap::new(),
        completed: BTreeSet::new(),
        stack: vec![],
        update,
        update_found: false,
        cache_root: cache_root()?,
    };
    let identity = local_identity(&package.root);
    sync.enter_root(package, identity)?;
    if let Some(name) = update
        && !sync.update_found
    {
        return Err(format!("`{name}` is not a Git dependency of this package."));
    }
    sync.new.write(&root)?;
    Ok(sync.new.packages.len())
}

struct Selection {
    identity: String,
    description: String,
    path: String,
}

struct SyncState<'a> {
    old: Lockfile,
    new: Lockfile,
    selected: BTreeMap<String, Selection>,
    completed: BTreeSet<String>,
    stack: Vec<String>,
    update: Option<&'a str>,
    update_found: bool,
    cache_root: PathBuf,
}

impl SyncState<'_> {
    fn enter_root(&mut self, package: Package, identity: String) -> Result<(), String> {
        let path = package.name.clone();
        self.record(&package.name, identity.clone(), identity.clone(), &path)?;
        self.completed
            .insert(format!("{}\0{}", package.name, identity));
        self.stack.push(package.name.clone());
        for dependency in package.depends.clone() {
            self.visit_dependency(&package, &dependency, &path)?;
        }
        self.stack.pop();
        Ok(())
    }

    fn visit_dependency(
        &mut self,
        parent: &Package,
        dependency: &PackageDependency,
        graph_path: &str,
    ) -> Result<(), String> {
        let path = format!("{graph_path} -> {}", dependency.name);
        if self.stack.contains(&dependency.name) {
            return Err(format!("Packages depend on each other in a cycle: {path}."));
        }
        if let Some(source) = &dependency.git {
            let identity = git_identity(source);
            self.record(
                &dependency.name,
                identity.clone(),
                format!("{} at {:?}", source.url, source.reference),
                &path,
            )?;
            if self
                .completed
                .contains(&format!("{}\0{}", dependency.name, identity))
            {
                return Ok(());
            }
            let (locked, source_root) = self.pin(source, &dependency.name, &path)?;
            self.new
                .packages
                .insert(dependency.name.clone(), locked.clone());
            let found = read_package(&source_root)?.ok_or_else(|| {
                format!(
                    "The cached source for `{}` has no {PROJECT_FILE}.",
                    dependency.name
                )
            })?;
            self.visit_package(found, identity, &path, dependency)?;
            return Ok(());
        }

        let parent_dir = parent.root.parent().map(Path::to_owned).unwrap_or_default();
        let folder = parent_dir.join(&dependency.name);
        let identity = local_identity(&folder);
        self.record(
            &dependency.name,
            identity.clone(),
            format!("local folder {}", folder.display()),
            &path,
        )?;
        if self
            .completed
            .contains(&format!("{}\0{}", dependency.name, identity))
        {
            return Ok(());
        }
        let found = match read_package(&folder)? {
            Some(found) if found.name == dependency.name => found,
            Some(found) => {
                return Err(format!(
                    "{}: the package is named `{}`, but `{}` finds it as `{}`. `depends` finds a package by its folder, so the two names must match.",
                    folder.join(PROJECT_FILE).display(),
                    found.name,
                    parent.name,
                    dependency.name
                ));
            }
            None => {
                let source_note = if parent.root.starts_with(&self.cache_root) {
                    " A cached Git package must declare transitive dependencies with `from` and `at`."
                } else {
                    ""
                };
                return Err(format!(
                    "`{}` depends on `{}`, but {} has no {PROJECT_FILE}.{source_note}",
                    parent.name,
                    dependency.name,
                    folder.display()
                ));
            }
        };
        self.visit_package(found, identity, &path, dependency)
    }

    fn visit_package(
        &mut self,
        package: Package,
        identity: String,
        graph_path: &str,
        required_as: &PackageDependency,
    ) -> Result<(), String> {
        if package.name != required_as.name {
            let source = package.root.join(PROJECT_FILE);
            return Err(format!(
                "{}: the package is named `{}`, but the dependency is named `{}`.",
                source.display(),
                package.name,
                required_as.name
            ));
        }
        let visit_key = format!("{}\0{}", package.name, identity);
        if self.completed.contains(&visit_key) {
            return Ok(());
        }
        self.completed.insert(visit_key);
        self.stack.push(package.name.clone());
        for dependency in package.depends.clone() {
            self.visit_dependency(&package, &dependency, graph_path)?;
        }
        self.stack.pop();
        Ok(())
    }

    fn record(
        &mut self,
        name: &str,
        identity: String,
        description: String,
        graph_path: &str,
    ) -> Result<(), String> {
        if let Some(previous) = self.selected.get(name) {
            if previous.identity != identity {
                return Err(version_conflict(
                    name,
                    previous,
                    &Selection {
                        identity,
                        description,
                        path: graph_path.to_owned(),
                    },
                ));
            }
        } else {
            self.selected.insert(
                name.to_owned(),
                Selection {
                    identity,
                    description,
                    path: graph_path.to_owned(),
                },
            );
        }
        Ok(())
    }

    fn pin(
        &mut self,
        source: &GitSource,
        name: &str,
        graph_path: &str,
    ) -> Result<(LockedPackage, PathBuf), String> {
        validate_git_url(&source.url)
            .map_err(|error| format!("E_PACKAGE_SOURCE: `{name}` at {graph_path}: {error}"))?;
        let update_this = self.update == Some(name);
        if update_this {
            self.update_found = true;
        }
        let old = self
            .old
            .packages
            .get(name)
            .filter(|entry| entry.url == source.url && entry.reference == source.reference);
        let reusable = old.filter(|_| !update_this);
        let mut clone: Option<GitClone> = None;
        let commit = if let Some(entry) = reusable {
            entry.commit.clone()
        } else {
            let repo = GitClone::clone_source(&source.url, name)?;
            let commit = repo.resolve(&source.reference, name)?;
            clone = Some(repo);
            commit
        };
        let destination = cache_path(&self.cache_root, name, &source.url, &commit);
        let expected = old.filter(|entry| entry.commit == commit);
        if let Some(entry) = expected
            && source_root(&destination).is_dir()
        {
            match verify_cached(&destination, entry) {
                Ok(()) => return Ok((entry.clone(), source_root(&destination))),
                Err(_) => remove_cache(&destination)?,
            }
        }
        if path_exists(&destination)? {
            remove_cache(&destination)?;
        }
        let repo = match clone {
            Some(repo) => repo,
            None => GitClone::clone_source(&source.url, name)?,
        };
        let new_entry = materialize(
            &repo,
            &destination,
            name,
            &source.url,
            &source.reference,
            &commit,
        )?;
        if let Some(entry) = expected
            && entry.tree_hash != new_entry.tree_hash
        {
            remove_cache(&destination)?;
            return Err(format!(
                "The cached source tree for `{name}` does not match the hash in kestrel.lock."
            ));
        }
        let entry = expected.cloned().unwrap_or(new_entry);
        let source_root = source_root(&destination);
        Ok((entry, source_root))
    }
}

/// Loads source dependencies only from the committed lockfile and verified cache.
pub fn load_packages(root: &Path) -> Result<Vec<Package>, String> {
    let root = root
        .canonicalize()
        .map_err(|error| format!("Cannot open package folder {}: {error}", root.display()))?;
    let Some(package) = read_package(&root)? else {
        return Ok(vec![]);
    };
    let lock = Lockfile::read(&root)?;
    let cache_root = cache_root()?;
    let mut loader = LoadState {
        lock,
        cache_root,
        packages: vec![],
        selected: BTreeMap::new(),
        completed: BTreeSet::new(),
        stack: vec![],
    };
    let identity = local_identity(&package.root);
    loader.record(&package.name, identity.clone(), identity, &package.name)?;
    loader.completed.insert(format!(
        "{}\0{}",
        package.name,
        local_identity(&package.root)
    ));
    loader.stack.push(package.name.clone());
    loader.packages.push(package.clone());
    for dependency in package.depends.clone() {
        loader.visit_dependency(&package, &dependency, &package.name)?;
    }
    Ok(loader.packages)
}

struct LoadState {
    lock: Lockfile,
    cache_root: PathBuf,
    packages: Vec<Package>,
    selected: BTreeMap<String, Selection>,
    completed: BTreeSet<String>,
    stack: Vec<String>,
}

impl LoadState {
    fn visit_dependency(
        &mut self,
        parent: &Package,
        dependency: &PackageDependency,
        graph_path: &str,
    ) -> Result<(), String> {
        let path = format!("{graph_path} -> {}", dependency.name);
        if self.stack.contains(&dependency.name) {
            return Err(format!("Packages depend on each other in a cycle: {path}."));
        }
        if let Some(source) = &dependency.git {
            let identity = git_identity(source);
            self.record(
                &dependency.name,
                identity.clone(),
                format!("{} at {:?}", source.url, source.reference),
                &path,
            )?;
            if self
                .completed
                .contains(&format!("{}\0{}", dependency.name, identity))
            {
                return Ok(());
            }
            let entry = self.lock.packages.get(&dependency.name).ok_or_else(|| {
                format!(
                    "E_PACKAGE_LOCK: `{}` has no pin in {}; run `kestrel fetch` first.",
                    dependency.name, LOCK_FILE
                )
            })?;
            if entry.url != source.url || entry.reference != source.reference {
                return Err(format!(
                    "E_PACKAGE_LOCK: the declared source for `{}` changed; run `kestrel fetch` to update {}.",
                    dependency.name, LOCK_FILE
                ));
            }
            let directory = cache_path(
                &self.cache_root,
                &dependency.name,
                &entry.url,
                &entry.commit,
            );
            verify_cached(&directory, entry)?;
            let source_root = source_root(&directory);
            let package = read_package(&source_root)?.ok_or_else(|| {
                format!(
                    "The cached source for `{}` has no {PROJECT_FILE}.",
                    dependency.name
                )
            })?;
            self.visit_package(package, identity, &path, dependency)
        } else {
            let parent_dir = parent.root.parent().map(Path::to_owned).unwrap_or_default();
            let folder = parent_dir.join(&dependency.name);
            let identity = local_identity(&folder);
            self.record(
                &dependency.name,
                identity.clone(),
                format!("local folder {}", folder.display()),
                &path,
            )?;
            if self
                .completed
                .contains(&format!("{}\0{}", dependency.name, identity))
            {
                return Ok(());
            }
            let found = match read_package(&folder)? {
                Some(found) if found.name == dependency.name => found,
                Some(found) => {
                    return Err(format!(
                        "{}: the package is named `{}`, but `{}` finds it as `{}`. `depends` finds a package by its folder, so the two names must match.",
                        folder.join(PROJECT_FILE).display(),
                        found.name,
                        parent.name,
                        dependency.name
                    ));
                }
                None => {
                    let source_note = if parent.root.starts_with(&self.cache_root) {
                        " A cached Git package must declare transitive dependencies with `from` and `at`."
                    } else {
                        ""
                    };
                    return Err(format!(
                        "`{}` depends on `{}`, but {} has no {PROJECT_FILE}.{source_note}",
                        parent.name,
                        dependency.name,
                        folder.display()
                    ));
                }
            };
            self.visit_package(found, identity, &path, dependency)
        }
    }

    fn visit_package(
        &mut self,
        package: Package,
        identity: String,
        graph_path: &str,
        required_as: &PackageDependency,
    ) -> Result<(), String> {
        if package.name != required_as.name {
            return Err(format!(
                "{}: the package is named `{}`, but the dependency is named `{}`.",
                package.root.join(PROJECT_FILE).display(),
                package.name,
                required_as.name
            ));
        }
        let visit_key = format!("{}\0{}", package.name, identity);
        if self.completed.contains(&visit_key) {
            return Ok(());
        }
        self.completed.insert(visit_key);
        self.stack.push(package.name.clone());
        self.packages.push(package.clone());
        for dependency in package.depends.clone() {
            self.visit_dependency(&package, &dependency, graph_path)?;
        }
        self.stack.pop();
        Ok(())
    }

    fn record(
        &mut self,
        name: &str,
        identity: String,
        description: String,
        graph_path: &str,
    ) -> Result<(), String> {
        if let Some(previous) = self.selected.get(name) {
            if previous.identity != identity {
                return Err(version_conflict(
                    name,
                    previous,
                    &Selection {
                        identity,
                        description,
                        path: graph_path.to_owned(),
                    },
                ));
            }
        } else {
            self.selected.insert(
                name.to_owned(),
                Selection {
                    identity,
                    description,
                    path: graph_path.to_owned(),
                },
            );
        }
        Ok(())
    }
}

fn version_conflict(name: &str, first: &Selection, second: &Selection) -> String {
    format!(
        "E_PACKAGE_VERSION: `{name}` has incompatible pins: {} through `{}` and {} through `{}`. A build can use only one version of a package.",
        first.description, first.path, second.description, second.path
    )
}

fn git_identity(source: &GitSource) -> String {
    format!("git\0{}\0{}", source.url, source.reference)
}

fn local_identity(path: &Path) -> String {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_owned());
    format!("local\0{}", canonical.display())
}

fn valid_package_name(name: &str) -> bool {
    !name.contains('.') && crate::identifiers::valid(name, "value identifier")
}

fn valid_object_id(commit: &str) -> bool {
    matches!(commit.len(), 40 | 64) && commit.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn validate_git_url(url: &str) -> Result<(), String> {
    let accepted = ["https://", "http://", "ssh://", "git://", "file://"];
    if !accepted.iter().any(|prefix| url.starts_with(prefix)) {
        return Err("Git URLs must use https, http, ssh, git, or file.".into());
    }
    if url.contains('\0') || url.contains('\n') || url.contains('\r') {
        return Err("The Git URL contains a control character.".into());
    }
    if let Some(path) = url.strip_prefix("file://")
        && path.is_empty()
    {
        return Err("A file URL needs a path.".into());
    }
    Ok(())
}

fn cache_root() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os("XDG_CACHE_HOME").filter(|path| !path.is_empty()) {
        return Ok(PathBuf::from(path).join("kestrel/packages"));
    }
    #[cfg(target_os = "macos")]
    if let Some(home) = env::var_os("HOME").filter(|path| !path.is_empty()) {
        return Ok(PathBuf::from(home).join("Library/Caches/kestrel/packages"));
    }
    #[cfg(target_os = "windows")]
    if let Some(local) = env::var_os("LOCALAPPDATA").filter(|path| !path.is_empty()) {
        return Ok(PathBuf::from(local).join("Kestrel/cache/packages"));
    }
    if let Some(home) = env::var_os("HOME").filter(|path| !path.is_empty()) {
        return Ok(PathBuf::from(home).join(".cache/kestrel/packages"));
    }
    Err("Cannot find the per-user cache folder (set XDG_CACHE_HOME or HOME).".into())
}

fn cache_path(root: &Path, name: &str, url: &str, commit: &str) -> PathBuf {
    let url_hash = format!("{:x}", Sha256::digest(url.as_bytes()));
    root.join(name).join(url_hash).join(commit)
}

fn source_root(cache_directory: &Path) -> PathBuf {
    cache_directory.join("source")
}

fn verify_cached(directory: &Path, entry: &LockedPackage) -> Result<(), String> {
    let source = source_root(directory);
    let metadata = fs::symlink_metadata(directory).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            format!(
                "The cached source for `{}` is missing; run `kestrel fetch` while online.",
                entry.name
            )
        } else {
            format!("Cannot inspect cached package `{}`: {error}", entry.name)
        }
    })?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(format!(
            "The cache entry for `{}` is not a real directory.",
            entry.name
        ));
    }
    let hash = tree_hash(&source)
        .map_err(|error| format!("Cannot verify cached package `{}`: {error}", entry.name))?;
    if hash != entry.tree_hash {
        return Err(format!(
            "E_PACKAGE_HASH: the cached source tree for `{}` has changed (expected {}, found {}). Run `kestrel fetch` to restore it.",
            entry.name, entry.tree_hash, hash
        ));
    }
    Ok(())
}

fn tree_hash(root: &Path) -> Result<String, String> {
    let metadata = fs::symlink_metadata(root)
        .map_err(|error| format!("{} does not exist: {error}", root.display()))?;
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "{} is a symlink, not a package tree.",
            root.display()
        ));
    }
    let root = root
        .canonicalize()
        .map_err(|error| format!("{} does not exist: {error}", root.display()))?;
    if !root.is_dir() {
        return Err(format!("{} is not a directory.", root.display()));
    }
    let mut hasher = Sha256::new();
    hash_directory(&root, &root, &mut hasher)?;
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

fn hash_directory(root: &Path, directory: &Path, hasher: &mut Sha256) -> Result<(), String> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| format!("Cannot read {}: {error}", directory.display()))?
        .map(|entry| entry.map_err(|error| error.to_string()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Cannot list {}: {error}", directory.display()))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name();
        let _name = name
            .to_str()
            .ok_or_else(|| format!("{} contains a non-UTF-8 file name.", path.display()))?;
        let relative = path
            .strip_prefix(root)
            .map_err(|_| format!("{} is outside the package cache.", path.display()))?;
        let relative = relative
            .to_str()
            .ok_or_else(|| format!("{} is not a UTF-8 path.", path.display()))?
            .replace('\\', "/");
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("Cannot inspect {}: {error}", path.display()))?;
        if metadata.file_type().is_symlink() {
            let target = fs::read_link(&path)
                .map_err(|error| format!("Cannot read symlink {}: {error}", path.display()))?;
            let target_text = target
                .to_str()
                .ok_or_else(|| format!("{} has a non-UTF-8 symlink target.", path.display()))?;
            if target.is_absolute() {
                return Err(format!(
                    "{} points outside the package tree.",
                    path.display()
                ));
            }
            let resolved = path
                .canonicalize()
                .map_err(|_| format!("{} is a broken symlink.", path.display()))?;
            if !resolved.starts_with(root) {
                return Err(format!(
                    "{} points outside the package tree.",
                    path.display()
                ));
            }
            hash_record(hasher, b'L', &relative, target_text.as_bytes());
        } else if metadata.is_dir() {
            hash_record(hasher, b'D', &relative, &[]);
            hash_directory(root, &path, hasher)?;
        } else if metadata.is_file() {
            let mut file = fs::File::open(&path)
                .map_err(|error| format!("Cannot read {}: {error}", path.display()))?;
            hasher.update(b"F");
            hash_bytes(hasher, relative.as_bytes());
            let length = metadata.len();
            hasher.update(length.to_be_bytes());
            let mut buffer = [0; 64 * 1024];
            loop {
                let count = file
                    .read(&mut buffer)
                    .map_err(|error| format!("Cannot read {}: {error}", path.display()))?;
                if count == 0 {
                    break;
                }
                hasher.update(&buffer[..count]);
            }
        } else {
            return Err(format!(
                "{} is not a regular file or directory.",
                path.display()
            ));
        }
    }
    Ok(())
}

fn hash_record(hasher: &mut Sha256, kind: u8, path: &str, content: &[u8]) {
    hasher.update([kind]);
    hash_bytes(hasher, path.as_bytes());
    hash_bytes(hasher, content);
}

fn hash_bytes(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn materialize(
    repo: &GitClone,
    destination: &Path,
    name: &str,
    url: &str,
    reference: &str,
    commit: &str,
) -> Result<LockedPackage, String> {
    fs::create_dir_all(destination.parent().unwrap_or(destination))
        .map_err(|error| format!("Cannot create the Kestrel package cache: {error}"))?;
    let staging = unique_path(destination.parent().unwrap_or(destination), "package.tmp");
    fs::create_dir(&staging).map_err(|error| {
        format!(
            "Cannot create cache staging folder {}: {error}",
            staging.display()
        )
    })?;
    let source = staging.join("source");
    let result = (|| {
        repo.ensure_commit(commit, url, name)?;
        repo.check_no_submodules(commit, url, name)?;
        repo.checkout(commit, &source, name, url)?;
        let tree_hash = tree_hash(&source)?;
        let package = LockedPackage {
            name: name.to_owned(),
            url: url.to_owned(),
            reference: reference.to_owned(),
            commit: commit.to_owned(),
            tree_hash,
        };
        if path_exists(destination)? {
            remove_cache(destination)?;
        }
        fs::rename(&staging, destination).map_err(|error| {
            format!(
                "Cannot install cached source for `{name}` at {}: {error}",
                destination.display()
            )
        })?;
        Ok(package)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}

struct GitClone {
    _temp: TempDirectory,
    repo: PathBuf,
}

impl GitClone {
    fn clone_source(url: &str, name: &str) -> Result<Self, String> {
        validate_git_url(url).map_err(|error| format!("E_PACKAGE_SOURCE: {error}"))?;
        let temp = TempDirectory::new("git-source")?;
        let repo = temp.path.join("repo");
        fs::create_dir_all(temp.path.join("empty-hooks"))
            .map_err(|error| format!("Cannot isolate Git hooks: {error}"))?;
        let output = clean_git_command()
            .arg("-c")
            .arg(format!(
                "core.hooksPath={}",
                temp.path.join("empty-hooks").display()
            ))
            .args(["clone", "--quiet", "--no-checkout", "--", url])
            .arg(&repo)
            .output()
            .map_err(|error| format!("Cannot start Git while fetching `{name}`: {error}"))?;
        ensure_success(output, "clone", url, name)?;
        Ok(Self { _temp: temp, repo })
    }

    fn git(&self, args: &[String]) -> Result<Output, String> {
        let mut command = clean_git_command();
        command.arg("-C").arg(&self.repo);
        command.args(args);
        command.env("GIT_TERMINAL_PROMPT", "0");
        command
            .output()
            .map_err(|error| format!("Cannot run Git: {error}"))
    }

    fn resolve(&self, reference: &str, name: &str) -> Result<String, String> {
        if reference.is_empty()
            || reference.contains('\0')
            || reference.contains('\n')
            || reference.contains('\r')
        {
            return Err(format!(
                "E_PACKAGE_SOURCE: `{name}` has an invalid Git ref."
            ));
        }
        let mut candidates = vec![reference.to_owned()];
        if !reference.starts_with("refs/") {
            candidates.extend([
                format!("refs/tags/{reference}"),
                format!("refs/remotes/origin/{reference}"),
                format!("refs/heads/{reference}"),
            ]);
        }
        for candidate in candidates {
            let object = format!("{candidate}^{{commit}}");
            let output = self.git(&[
                "rev-parse".into(),
                "--verify".into(),
                "--end-of-options".into(),
                object,
            ])?;
            if !output.status.success() {
                continue;
            }
            let commit = String::from_utf8_lossy(&output.stdout)
                .trim()
                .to_ascii_lowercase();
            if !valid_object_id(&commit) {
                return Err(format!("Git resolved `{name}` to an invalid object id."));
            }
            return Ok(commit);
        }
        Err(format!(
            "Git could not resolve ref `{reference}` for `{name}` in the source repository."
        ))
    }

    fn ensure_commit(&self, commit: &str, url: &str, name: &str) -> Result<(), String> {
        if !valid_object_id(commit) {
            return Err(format!("`{name}` has an invalid commit in its lockfile."));
        }
        let object = format!("{commit}^{{commit}}");
        let output = self.git(&["cat-file".into(), "-e".into(), object])?;
        ensure_success(output, "verify commit", url, name)
    }

    fn check_no_submodules(&self, commit: &str, url: &str, name: &str) -> Result<(), String> {
        let output = self.git(&[
            "ls-tree".into(),
            "-r".into(),
            "-z".into(),
            "--full-tree".into(),
            commit.to_owned(),
        ])?;
        ensure_success(output.clone(), "inspect tree", url, name)?;
        for entry in output.stdout.split(|byte| *byte == 0) {
            if entry.starts_with(b"160000 commit ") {
                return Err(format!(
                    "`{name}` contains a Git submodule; submodules are not fetched."
                ));
            }
        }
        Ok(())
    }

    fn checkout(&self, commit: &str, target: &Path, name: &str, url: &str) -> Result<(), String> {
        fs::create_dir_all(target)
            .map_err(|error| format!("Cannot create source folder for `{name}`: {error}"))?;
        fs::create_dir_all(self.hooks_path())
            .map_err(|error| format!("Cannot isolate Git hooks: {error}"))?;
        let mut command = clean_git_command();
        command
            .arg("-C")
            .arg(&self.repo)
            .arg("-c")
            .arg(format!("core.hooksPath={}", self.hooks_path().display()))
            .arg(format!("--work-tree={}", target.display()))
            .args(["checkout", "--force", "--detach", commit]);
        let config = self._temp.path.join("empty.gitconfig");
        fs::write(&config, "").map_err(|error| format!("Cannot create Git config: {error}"))?;
        command.env("GIT_CONFIG_GLOBAL", config);
        command.env("GIT_CONFIG_NOSYSTEM", "1");
        command.env("GIT_ATTR_NOSYSTEM", "1");
        let output = command
            .output()
            .map_err(|error| format!("Cannot check out `{name}`: {error}"))?;
        ensure_success(output, "check out", url, name)
    }

    fn hooks_path(&self) -> PathBuf {
        self._temp.path.join("empty-hooks")
    }
}

fn clean_git_command() -> Command {
    let mut command = Command::new("git");
    for variable in [
        "GIT_CONFIG_COUNT",
        "GIT_CONFIG_PARAMETERS",
        "GIT_CONFIG_SYSTEM",
        "GIT_CONFIG_GLOBAL",
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_COMMON_DIR",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_PREFIX",
        "GIT_CEILING_DIRECTORIES",
        "GIT_DISCOVERY_ACROSS_FILESYSTEM",
        "GIT_NAMESPACE",
    ] {
        command.env_remove(variable);
    }
    command.env("GIT_TERMINAL_PROMPT", "0");
    command
}

struct TempDirectory {
    path: PathBuf,
}

impl TempDirectory {
    fn new(label: &str) -> Result<Self, String> {
        let path = unique_path(&env::temp_dir(), &format!("kestrel-{label}"));
        fs::create_dir(&path).map_err(|error| {
            format!("Cannot create temporary folder {}: {error}", path.display())
        })?;
        Ok(Self { path })
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn unique_path(parent: &Path, prefix: &str) -> PathBuf {
    let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
    parent.join(format!("{prefix}.{}.{}", std::process::id(), serial))
}

fn path_exists(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("Cannot inspect {}: {error}", path.display())),
    }
}

fn remove_cache(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("Cannot inspect cache entry {}: {error}", path.display()))?;
    if metadata.file_type().is_symlink() {
        return fs::remove_file(path)
            .map_err(|error| format!("Cannot remove cache link {}: {error}", path.display()));
    }
    if !metadata.is_dir() {
        return Err(format!(
            "Cache entry {} is not a real directory.",
            path.display()
        ));
    }
    fs::remove_dir_all(path).map_err(|error| {
        format!(
            "Cannot remove stale cache entry {}: {error}",
            path.display()
        )
    })
}

fn ensure_success(output: Output, operation: &str, url: &str, name: &str) -> Result<(), String> {
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr = stderr.replace(url, "source URL");
    let detail = stderr.trim();
    Err(if detail.is_empty() {
        format!("Git could not {operation} `{name}`.")
    } else {
        format!("Git could not {operation} `{name}`: {detail}")
    })
}
