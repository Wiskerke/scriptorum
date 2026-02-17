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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SyncDiff {
    /// Files the client should upload to the server
    pub to_upload: Vec<FileEntry>,
    /// Files the client should download from the server
    pub to_download: Vec<FileEntry>,
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
