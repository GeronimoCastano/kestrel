//! Canonical paths as the user would write them.

use std::{
    io,
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
