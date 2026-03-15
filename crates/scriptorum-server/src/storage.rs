use anyhow::{Context, Result};
use scriptorum_core::checksum::sha256_file;
use scriptorum_core::protocol::{Manifest, SyncDiff};
use scriptorum_core::scanner::scan_directory;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

const LEDGER_FILENAME: &str = ".ledger.json";

/// Manages file storage on disk and manifest tracking.
pub struct Storage {
    root: PathBuf,
    archive_dir: PathBuf,
    ledger: HashMap<String, String>,
}

impl Storage {
    pub fn new(root: PathBuf, archive_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&root)
            .with_context(|| format!("creating storage dir {}", root.display()))?;
        fs::create_dir_all(&archive_dir)
            .with_context(|| format!("creating archive dir {}", archive_dir.display()))?;

        let ledger_path = root.join(LEDGER_FILENAME);
        let ledger = if ledger_path.exists() {
            let data = fs::read_to_string(&ledger_path)
                .with_context(|| format!("reading ledger {}", ledger_path.display()))?;
            serde_json::from_str(&data)
                .with_context(|| format!("parsing ledger {}", ledger_path.display()))?
        } else {
            HashMap::new()
        };

        Ok(Self {
            root,
            archive_dir,
            ledger,
        })
    }

    /// Build the current manifest by scanning the storage directory.
    /// Excludes the ledger file itself.
    pub fn manifest(&self) -> Result<Manifest> {
        let mut manifest = scan_directory(&self.root)?;
        manifest.files.retain(|f| f.path != LEDGER_FILENAME);
        Ok(manifest)
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

    /// Build a manifest by scanning the archive directory.
    pub fn archive_manifest(&self) -> Result<Manifest> {
        scan_directory(&self.archive_dir)
    }

    /// Write a file to the archive directory. Returns the SHA256 of what was written.
    pub fn write_archive(&self, rel_path: &str, data: &[u8]) -> Result<String> {
        let full = self.resolve_archive(rel_path)?;
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating dir {}", parent.display()))?;
        }
        fs::write(&full, data).with_context(|| format!("writing {}", full.display()))?;
        sha256_file(&full)
    }

    /// Move a file from the notes root to the archive directory.
    pub fn move_to_archive(&self, notes_rel: &str, archive_rel: &str) -> Result<()> {
        let src = self.resolve(notes_rel)?;
        let dst = self.archive_dir.join(archive_rel);
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating dir {}", parent.display()))?;
        }
        fs::rename(&src, &dst)
            .with_context(|| format!("moving {} to archive {}", notes_rel, archive_rel))
    }

    /// Record a successful client upload in the ledger and persist it.
    pub fn record_upload(&mut self, path: &str, sha256: &str) -> Result<()> {
        self.ledger.insert(path.to_string(), sha256.to_string());
        self.save_ledger()
    }

    /// Return a reference to the current ledger snapshot.
    pub fn ledger_snapshot(&self) -> &HashMap<String, String> {
        &self.ledger
    }

    /// Update the ledger to reflect what the server told the client to do in a diff.
    ///
    /// - `to_download` entries: record path → sha256 (client will have these files)
    /// - `to_rename` entries: remove `from` path, add `to` path → sha256
    /// - Stale entries: remove ledger entries where the path is absent from both
    ///   the client manifest and the server filesystem (deletion was successfully applied)
    ///
    /// Note: `to_delete` entries are NOT removed from the ledger so deletion is idempotent.
    pub fn apply_diff_to_ledger(&mut self, diff: &SyncDiff, client: &Manifest) -> Result<()> {
        // Move server files displaced by conflicts to the archive/conflicts/ folder.
        for entry in &diff.server.conflicts {
            if !entry.already_present {
                self.move_to_archive(&entry.original_path, &entry.archive_path)?;
                tracing::info!(
                    original = %entry.original_path,
                    archive = %entry.archive_path,
                    "server conflict: moved server version to archive/conflicts/"
                );
            } else {
                tracing::info!(
                    original = %entry.original_path,
                    archive = %entry.archive_path,
                    "server conflict: version already in archive, skipping move"
                );
            }
        }

        // Move client-deleted synced files to the archive root.
        for entry in &diff.server.deleted {
            if !entry.already_present {
                self.move_to_archive(&entry.original_path, &entry.archive_path)?;
                tracing::info!(
                    original = %entry.original_path,
                    archive = %entry.archive_path,
                    "client deleted: moved server copy to archive"
                );
            } else {
                tracing::info!(
                    original = %entry.original_path,
                    archive = %entry.archive_path,
                    "client deleted: version already in archive, skipping move"
                );
            }
            self.ledger.remove(&entry.original_path);
        }

        let client_paths: HashMap<&str, &str> = client
            .files
            .iter()
            .map(|f| (f.path.as_str(), f.sha256.as_str()))
            .collect();

        for path in &diff.server.to_delete {
            let full_path = self.root.join(path.as_str());
            if full_path.exists() {
                fs::remove_file(&full_path)
                    .with_context(|| format!("deleting stale file {}", full_path.display()))?;
            }
            self.ledger.remove(path);
        }

        for entry in &diff.client.to_download {
            self.ledger.insert(entry.path.clone(), entry.sha256.clone());
        }

        for rename in &diff.client.to_rename {
            let sha = self.ledger.remove(&rename.from).unwrap_or_default();
            // Look up the actual sha from the server manifest if not in ledger
            let sha = if sha.is_empty() {
                // Find it in client map (the client has the original sha at from_path)
                client_paths
                    .get(rename.from.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_default()
            } else {
                sha
            };
            if !sha.is_empty() {
                self.ledger.insert(rename.to.clone(), sha);
            }
        }

        // Clean up stale entries: path absent from client manifest and server filesystem
        let stale_keys: Vec<String> = self
            .ledger
            .keys()
            .filter(|path| {
                !client_paths.contains_key(path.as_str()) && !self.root.join(path.as_str()).exists()
            })
            .cloned()
            .collect();
        for key in stale_keys {
            self.ledger.remove(&key);
        }

        self.save_ledger()
    }

    fn save_ledger(&self) -> Result<()> {
        let ledger_path = self.root.join(LEDGER_FILENAME);
        let data = serde_json::to_string(&self.ledger).context("serializing ledger")?;
        fs::write(&ledger_path, data)
            .with_context(|| format!("writing ledger {}", ledger_path.display()))
    }

    /// Resolve a relative path to an absolute path within the archive directory.
    /// Rejects paths that escape via `..`.
    fn resolve_archive(&self, rel_path: &str) -> Result<PathBuf> {
        let full = self.archive_dir.join(rel_path);
        let canonical_root = self.archive_dir.canonicalize().with_context(|| {
            format!("canonicalizing archive dir {}", self.archive_dir.display())
        })?;
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
            self.archive_dir.display()
        );
        Ok(full)
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

    fn test_storage(dir: &TempDir) -> Storage {
        let archive = dir.path().join("archive");
        Storage::new(dir.path().to_path_buf(), archive).unwrap()
    }

    #[test]
    fn write_and_read() {
        let dir = TempDir::new().unwrap();
        let storage = test_storage(&dir);

        storage.write_file("hello.txt", b"hello world").unwrap();
        let data = storage.read_file("hello.txt").unwrap();
        assert_eq!(data, b"hello world");
    }

    #[test]
    fn write_nested() {
        let dir = TempDir::new().unwrap();
        let storage = test_storage(&dir);

        storage.write_file("sub/deep/file.txt", b"deep").unwrap();
        assert_eq!(storage.read_file("sub/deep/file.txt").unwrap(), b"deep");
    }

    #[test]
    fn manifest_reflects_files() {
        let dir = TempDir::new().unwrap();
        let storage = test_storage(&dir);

        storage.write_file("a.txt", b"aaa").unwrap();
        storage.write_file("b.txt", b"bbb").unwrap();

        let manifest = storage.manifest().unwrap();
        assert_eq!(manifest.files.len(), 2);
    }

    #[test]
    fn manifest_excludes_ledger() {
        let dir = TempDir::new().unwrap();
        let mut storage = test_storage(&dir);

        storage.write_file("a.txt", b"aaa").unwrap();
        storage.record_upload("a.txt", "sha_a").unwrap();

        let manifest = storage.manifest().unwrap();
        assert_eq!(manifest.files.len(), 1);
        assert!(!manifest.files.iter().any(|f| f.path == LEDGER_FILENAME));
    }

    #[test]
    fn path_traversal_rejected() {
        let dir = TempDir::new().unwrap();
        let storage = test_storage(&dir);

        assert!(storage.write_file("../escape.txt", b"nope").is_err());
    }

    #[test]
    fn read_nonexistent_errors() {
        let dir = TempDir::new().unwrap();
        let storage = test_storage(&dir);
        assert!(storage.read_file("nope.txt").is_err());
    }

    #[test]
    fn ledger_persists_across_reload() {
        let dir = TempDir::new().unwrap();
        let archive = dir.path().join("archive");
        {
            let mut storage = Storage::new(dir.path().to_path_buf(), archive.clone()).unwrap();
            storage.record_upload("a.txt", "sha_a").unwrap();
        }
        let storage = Storage::new(dir.path().to_path_buf(), archive).unwrap();
        assert_eq!(
            storage.ledger_snapshot().get("a.txt").map(|s| s.as_str()),
            Some("sha_a")
        );
    }
}
