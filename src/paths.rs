//! Canonical paths as the user would write them, plus the folders Kestrel installs
//! into and caches under.

use std::{
    env, io,
    path::{Path, PathBuf},
};

pub trait CanonicalPath {
    /// `fs::canonicalize`. On Windows a canonical path carries the `\\?\` prefix, which
    /// would then appear in every diagnostic; it is dropped when the rest is a plain
    /// drive path within the usual length limit, where the prefix changes nothing.
    fn canonical(&self) -> io::Result<PathBuf>;
}

impl CanonicalPath for Path {
    fn canonical(&self) -> io::Result<PathBuf> {
        let found = self.canonicalize()?;
        if cfg!(windows)
            && let Some(rest) = found.to_str().and_then(|text| text.strip_prefix(r"\\?\"))
            && plain_drive_path(rest)
        {
            return Ok(PathBuf::from(rest));
        }
        Ok(found)
    }
}

/// `C:\...` under 260 characters, with no component Windows treats as a device name.
fn plain_drive_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 3
        && path.len() < 260
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && bytes[2] == b'\\'
        && !path[3..].split('\\').any(|part| {
            let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
            matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
                || ((stem.starts_with("COM") || stem.starts_with("LPT"))
                    && stem.len() == 4
                    && stem.as_bytes()[3].is_ascii_digit())
                || part.ends_with('.')
                || part.ends_with(' ')
        })
}

/// A tool shipped with the compiler. The Windows install keeps `kestrel.exe` in `bin`,
/// Clang in `toolchain/bin` and Z3 in `z3` beside it. `None` means the tool comes
/// from `PATH`.
pub fn bundled(tool: &str) -> Option<PathBuf> {
    let executable = env::current_exe().ok()?;
    let root = executable.parent()?.parent()?;
    bundled_at(root, tool)
}

/// `bundled`, for a compiler that does not have to be this process's executable.
pub fn bundled_at(root: &Path, tool: &str) -> Option<PathBuf> {
    bundled_at_with(root, tool, env::consts::EXE_SUFFIX)
}

/// `bundled_at`, with the suffix of the platform being described rather than the one
/// this copy was built for.
pub fn bundled_at_with(root: &Path, tool: &str, suffix: &str) -> Option<PathBuf> {
    let file = format!("{tool}{suffix}");
    [root.join("toolchain").join("bin"), root.join(tool)]
        .into_iter()
        .map(|folder| folder.join(&file))
        .find(|path| path.is_file())
}

/// The operating system a build targets. The release tarballs are built for one of
/// these three, and the installers lay their tools out differently on each.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    MacOs,
    Linux,
    Windows,
}

impl Platform {
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::MacOs
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else {
            Self::Linux
        }
    }

    /// The name people use, as in `kestrel doctor` output.
    pub fn name(self) -> &'static str {
        match self {
            Self::MacOs => "macOS",
            Self::Linux => "Linux",
            Self::Windows => "Windows",
        }
    }

    /// The release target, such as `aarch64-apple-darwin`.
    pub fn target(self) -> String {
        let architecture = env::consts::ARCH;
        match self {
            Self::MacOs => format!("{architecture}-apple-darwin"),
            Self::Linux => format!("{architecture}-unknown-linux-gnu"),
            Self::Windows => format!("{architecture}-pc-windows-msvc"),
        }
    }

    /// What an executable is called on this platform, after the bare name.
    pub fn executable_suffix(self) -> &'static str {
        match self {
            Self::Windows => ".exe",
            Self::MacOs | Self::Linux => "",
        }
    }
}

/// The folder Git packages are cloned into, or the reason there is none. `paths` is
/// `XDG_CACHE_HOME`, `home` is `HOME`, and `local` is `LOCALAPPDATA`, which only
/// Windows reads.
pub fn cache_root(
    platform: Platform,
    xdg_cache_home: Option<&Path>,
    home: Option<&Path>,
    local_app_data: Option<&Path>,
) -> Result<PathBuf, String> {
    let nonempty = |path: &&Path| !path.as_os_str().is_empty();
    if let Some(path) = xdg_cache_home.filter(nonempty) {
        return Ok(path.join("kestrel/packages"));
    }
    if platform == Platform::MacOs
        && let Some(home) = home.filter(nonempty)
    {
        return Ok(home.join("Library/Caches/kestrel/packages"));
    }
    if platform == Platform::Windows
        && let Some(local) = local_app_data.filter(nonempty)
    {
        return Ok(local.join("Kestrel/cache/packages"));
    }
    if let Some(home) = home.filter(nonempty) {
        return Ok(home.join(".cache/kestrel/packages"));
    }
    Err("Cannot find the per-user cache folder (set XDG_CACHE_HOME or HOME).".into())
}

#[cfg(test)]
mod tests {
    use super::plain_drive_path;
    #[test]
    fn only_plain_drive_paths_lose_the_verbatim_prefix() {
        assert!(plain_drive_path(r"C:\Users\ana\app\main.kes"));
        assert!(!plain_drive_path(r"UNC\server\share\main.kes"));
        assert!(!plain_drive_path(r"C:\work\CON\main.kes"));
        assert!(!plain_drive_path(r"C:\work\com1.txt"));
        assert!(!plain_drive_path(r"C:\work\trailing."));
        assert!(!plain_drive_path(&format!(r"C:\{}", "a".repeat(300))));
    }
}
