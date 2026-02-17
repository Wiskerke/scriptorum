use crate::checksum::sha256_file;
use crate::protocol::{FileEntry, Manifest};
use anyhow::{Context, Result};
use std::path::Path;
use std::time::UNIX_EPOCH;
use walkdir::WalkDir;

/// Scan a directory recursively, producing a Manifest of all files.
/// Paths in the manifest are relative to `root`.
pub fn scan_directory(root: &Path) -> Result<Manifest> {
    let mut files = Vec::new();

    for entry in WalkDir::new(root).follow_links(false) {
        let entry = entry.with_context(|| format!("walking {}", root.display()))?;
        if !entry.file_type().is_file() {
            continue;
        }

        let abs_path = entry.path();
        let rel_path = abs_path
            .strip_prefix(root)
            .with_context(|| format!("stripping prefix from {}", abs_path.display()))?;

        let metadata = entry.metadata().context("reading metadata")?;
        let modified = metadata
            .modified()
            .context("reading mtime")?
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let size = metadata.len();

        let sha256 = sha256_file(abs_path)
            .with_context(|| format!("hashing {}", abs_path.display()))?;

        // Use forward slashes for cross-platform consistency
        let path_str = rel_path
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");

        files.push(FileEntry {
            path: path_str,
            sha256,
            size,
            modified,
        });
    }

    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(Manifest { files })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn scan_empty_dir() {
        let dir = TempDir::new().unwrap();
        let manifest = scan_directory(dir.path()).unwrap();
        assert!(manifest.files.is_empty());
    }

    #[test]
    fn scan_flat_dir() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.txt"), "aaa").unwrap();
        fs::write(dir.path().join("b.txt"), "bbb").unwrap();

        let manifest = scan_directory(dir.path()).unwrap();
        assert_eq!(manifest.files.len(), 2);
        assert_eq!(manifest.files[0].path, "a.txt");
        assert_eq!(manifest.files[1].path, "b.txt");
        assert_eq!(manifest.files[0].size, 3);
    }

    #[test]
    fn scan_nested_dir() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("sub/deep")).unwrap();
        fs::write(dir.path().join("top.txt"), "top").unwrap();
        fs::write(dir.path().join("sub/mid.txt"), "mid").unwrap();
        fs::write(dir.path().join("sub/deep/bottom.txt"), "bottom").unwrap();

        let manifest = scan_directory(dir.path()).unwrap();
        assert_eq!(manifest.files.len(), 3);

        let paths: Vec<&str> = manifest.files.iter().map(|f| f.path.as_str()).collect();
        assert!(paths.contains(&"top.txt"));
        assert!(paths.contains(&"sub/mid.txt"));
        assert!(paths.contains(&"sub/deep/bottom.txt"));
    }

    #[test]
    fn scan_nonexistent_dir_errors() {
        assert!(scan_directory(Path::new("/nonexistent/path")).is_err());
    }
}
