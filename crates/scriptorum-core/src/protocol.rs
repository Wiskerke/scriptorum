use serde::{Deserialize, Serialize};

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
pub struct ConflictEntry {
    pub original_path: String,
    pub conflicted_path: String,
    pub already_present: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SyncDiff {
    /// Files the client should upload to the server
    pub to_upload: Vec<FileEntry>,
    /// Files the client should download from the server
    pub to_download: Vec<FileEntry>,
    /// Paths the client should delete locally
    #[serde(default)]
    pub to_delete: Vec<String>,
    /// Renames the client should apply locally
    #[serde(default)]
    pub to_rename: Vec<RenameEntry>,
    /// Paths the server should delete (stale renames of files the client has since deleted)
    #[serde(default)]
    pub to_delete_on_server: Vec<String>,
    /// Server will move its version of these to the conflicted folder (during PUT)
    #[serde(default)]
    pub server_conflicts: Vec<ConflictEntry>,
    /// Client should upload its version of these to /api/v1/conflicted/{conflicted_path}
    #[serde(default)]
    pub client_conflicts: Vec<ConflictEntry>,
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
