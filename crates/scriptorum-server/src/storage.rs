use anyhow::{Context, Result};
use scriptorum_core::checksum::sha256_file;
use scriptorum_core::protocol::Manifest;
use scriptorum_core::scanner::scan_directory;
use std::fs;
use std::path::PathBuf;

/// Manages file storage on disk and manifest tracking.
pub struct Storage {
    root: PathBuf,
}

impl Storage {
    pub fn new(root: PathBuf) -> Result<Self> {
        fs::create_dir_all(&root)
            .with_context(|| format!("creating storage dir {}", root.display()))?;
        Ok(Self { root })
    }

    /// Build the current manifest by scanning the storage directory.
    pub fn manifest(&self) -> Result<Manifest> {
        scan_directory(&self.root)
    }

    /// Read a file's contents. Path is relative to the storage root.
    pub fn read_file(&self, rel_path: &str) -> Result<Vec<u8>> {
        let full = self.resolve(rel_path)?;
        fs::read(&full).with_context(|| format!("reading {}", full.display()))
    }

    /// Write a file's contents. Path is relative to the storage root.
    /// Creates parent directories as needed. Returns the SHA256 of what was written.
    pub fn write_file(&self, rel_path: &str, data: &[u8]) -> Result<String> {
        let full = self.resolve(rel_path)?;
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating dir {}", parent.display()))?;
        }
        fs::write(&full, data).with_context(|| format!("writing {}", full.display()))?;
        sha256_file(&full)
    }

    /// Resolve a relative path to an absolute path within the storage root.
    /// Rejects paths that escape the root via `..`.
    fn resolve(&self, rel_path: &str) -> Result<PathBuf> {
        let full = self.root.join(rel_path);
        let canonical_root = self
            .root
            .canonicalize()
            .with_context(|| format!("canonicalizing root {}", self.root.display()))?;
        // For new files that don't exist yet, we check the parent
        let check_path = if full.exists() {
            full.canonicalize()?
        } else {
            let parent = full.parent().context("no parent")?;
            fs::create_dir_all(parent)?;
            let canon_parent = parent.canonicalize()?;
            canon_parent.join(full.file_name().context("no filename")?)
        };
        anyhow::ensure!(
            check_path.starts_with(&canonical_root),
            "path traversal: {} escapes {}",
            rel_path,
            self.root.display()
        );
        Ok(full)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn write_and_read() {
        let dir = TempDir::new().unwrap();
        let storage = Storage::new(dir.path().to_path_buf()).unwrap();

        storage.write_file("hello.txt", b"hello world").unwrap();
        let data = storage.read_file("hello.txt").unwrap();
        assert_eq!(data, b"hello world");
    }

    #[test]
    fn write_nested() {
        let dir = TempDir::new().unwrap();
        let storage = Storage::new(dir.path().to_path_buf()).unwrap();

        storage.write_file("sub/deep/file.txt", b"deep").unwrap();
        assert_eq!(storage.read_file("sub/deep/file.txt").unwrap(), b"deep");
    }

    #[test]
    fn manifest_reflects_files() {
        let dir = TempDir::new().unwrap();
        let storage = Storage::new(dir.path().to_path_buf()).unwrap();

        storage.write_file("a.txt", b"aaa").unwrap();
        storage.write_file("b.txt", b"bbb").unwrap();

        let manifest = storage.manifest().unwrap();
        assert_eq!(manifest.files.len(), 2);
    }

    #[test]
    fn path_traversal_rejected() {
        let dir = TempDir::new().unwrap();
        let storage = Storage::new(dir.path().to_path_buf()).unwrap();

        assert!(storage.write_file("../escape.txt", b"nope").is_err());
    }

    #[test]
    fn read_nonexistent_errors() {
        let dir = TempDir::new().unwrap();
        let storage = Storage::new(dir.path().to_path_buf()).unwrap();
        assert!(storage.read_file("nope.txt").is_err());
    }
}
