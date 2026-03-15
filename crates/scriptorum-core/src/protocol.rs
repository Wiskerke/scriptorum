use serde::{Deserialize, Serialize};

pub const API_VERSION: &str = "v2";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileEntry {
    /// Relative path from the note root (e.g. "Daily/2026-02-17.note")
    pub path: String,
    /// SHA256 hex digest of file contents
    pub sha256: String,
    /// File size in bytes
    pub size: u64,
    /// Last modified time as Unix timestamp (seconds)
    pub modified: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Manifest {
    pub files: Vec<FileEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RenameEntry {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArchiveEntry {
    pub original_path: String,
    pub archive_path: String,
    pub already_present: bool,
}

/// Actions the client should perform after receiving a SyncDiff.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClientDiff {
    /// Files the client should upload to the server
    pub to_upload: Vec<FileEntry>,
    /// Files the client should download from the server
    pub to_download: Vec<FileEntry>,
    /// Paths the client should delete locally
    pub to_delete: Vec<String>,
    /// Renames the client should apply locally
    pub to_rename: Vec<RenameEntry>,
    /// Upload the client's conflict-losing version to PUT /api/{API_VERSION}/archive/{archive_path}
    pub conflicts: Vec<ArchiveEntry>,
}

/// Actions the server performs during `apply_diff_to_ledger` (before any client uploads arrive).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServerDiff {
    /// Stale server paths to delete (rename leftovers of client-deleted files)
    pub to_delete: Vec<String>,
    /// Move the server's conflict-losing versions to archive/conflicts/
    pub conflicts: Vec<ArchiveEntry>,
    /// Move client-deleted synced files to the archive root
    pub deleted: Vec<ArchiveEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SyncDiff {
    /// Actions the client should perform
    pub client: ClientDiff,
    /// Actions the server performs internally (applied in apply_diff_to_ledger)
    pub server: ServerDiff,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_entry_roundtrip() {
        let entry = FileEntry {
            path: "Daily/note.txt".into(),
            sha256: "abc123".into(),
            size: 42,
            modified: 1700000000,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let parsed: FileEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry, parsed);
    }

    #[test]
    fn manifest_roundtrip() {
        let manifest = Manifest {
            files: vec![FileEntry {
                path: "test.note".into(),
                sha256: "deadbeef".into(),
                size: 100,
                modified: 1700000000,
            }],
        };
        let json = serde_json::to_string(&manifest).unwrap();
        let parsed: Manifest = serde_json::from_str(&json).unwrap();
        assert_eq!(manifest.files.len(), parsed.files.len());
        assert_eq!(manifest.files[0], parsed.files[0]);
    }
}
