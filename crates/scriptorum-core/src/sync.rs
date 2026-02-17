use crate::protocol::{FileEntry, Manifest, SyncDiff};
use std::collections::HashMap;

/// Compute the sync diff between a local and remote manifest.
///
/// For files present on both sides with differing checksums,
/// the side with the more recent mtime wins (last-write-wins).
///
/// Returns a SyncDiff indicating which files the client should
/// upload (to_upload) and download (to_download).
pub fn compute_diff(local: &Manifest, remote: &Manifest) -> SyncDiff {
    let local_map: HashMap<&str, &FileEntry> =
        local.files.iter().map(|f| (f.path.as_str(), f)).collect();
    let remote_map: HashMap<&str, &FileEntry> =
        remote.files.iter().map(|f| (f.path.as_str(), f)).collect();

    let mut to_upload = Vec::new();
    let mut to_download = Vec::new();

    // Files only on local -> upload
    // Files on both with different checksums -> compare mtime
    for (path, local_entry) in &local_map {
        match remote_map.get(path) {
            None => to_upload.push((*local_entry).clone()),
            Some(remote_entry) => {
                if local_entry.sha256 != remote_entry.sha256 {
                    if local_entry.modified >= remote_entry.modified {
                        to_upload.push((*local_entry).clone());
                    } else {
                        to_download.push((*remote_entry).clone());
                    }
                }
            }
        }
    }

    // Files only on remote -> download
    for (path, remote_entry) in &remote_map {
        if !local_map.contains_key(path) {
            to_download.push((*remote_entry).clone());
        }
    }

    to_upload.sort_by(|a, b| a.path.cmp(&b.path));
    to_download.sort_by(|a, b| a.path.cmp(&b.path));

    SyncDiff {
        to_upload,
        to_download,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, sha256: &str, modified: u64) -> FileEntry {
        FileEntry {
            path: path.into(),
            sha256: sha256.into(),
            size: 100,
            modified,
        }
    }

    #[test]
    fn both_empty() {
        let diff = compute_diff(&Manifest::default(), &Manifest::default());
        assert!(diff.to_upload.is_empty());
        assert!(diff.to_download.is_empty());
    }

    #[test]
    fn identical_manifests() {
        let m = Manifest {
            files: vec![entry("a.txt", "abc", 1000)],
        };
        let diff = compute_diff(&m, &m);
        assert!(diff.to_upload.is_empty());
        assert!(diff.to_download.is_empty());
    }

    #[test]
    fn local_only_files_upload() {
        let local = Manifest {
            files: vec![entry("new.txt", "abc", 1000)],
        };
        let diff = compute_diff(&local, &Manifest::default());
        assert_eq!(diff.to_upload.len(), 1);
        assert_eq!(diff.to_upload[0].path, "new.txt");
        assert!(diff.to_download.is_empty());
    }

    #[test]
    fn remote_only_files_download() {
        let remote = Manifest {
            files: vec![entry("remote.txt", "abc", 1000)],
        };
        let diff = compute_diff(&Manifest::default(), &remote);
        assert!(diff.to_upload.is_empty());
        assert_eq!(diff.to_download.len(), 1);
        assert_eq!(diff.to_download[0].path, "remote.txt");
    }

    #[test]
    fn conflict_local_newer_uploads() {
        let local = Manifest {
            files: vec![entry("note.txt", "local_hash", 2000)],
        };
        let remote = Manifest {
            files: vec![entry("note.txt", "remote_hash", 1000)],
        };
        let diff = compute_diff(&local, &remote);
        assert_eq!(diff.to_upload.len(), 1);
        assert_eq!(diff.to_upload[0].sha256, "local_hash");
        assert!(diff.to_download.is_empty());
    }

    #[test]
    fn conflict_remote_newer_downloads() {
        let local = Manifest {
            files: vec![entry("note.txt", "local_hash", 1000)],
        };
        let remote = Manifest {
            files: vec![entry("note.txt", "remote_hash", 2000)],
        };
        let diff = compute_diff(&local, &remote);
        assert!(diff.to_upload.is_empty());
        assert_eq!(diff.to_download.len(), 1);
        assert_eq!(diff.to_download[0].sha256, "remote_hash");
    }

    #[test]
    fn conflict_same_mtime_local_wins() {
        let local = Manifest {
            files: vec![entry("note.txt", "local_hash", 1000)],
        };
        let remote = Manifest {
            files: vec![entry("note.txt", "remote_hash", 1000)],
        };
        let diff = compute_diff(&local, &remote);
        assert_eq!(diff.to_upload.len(), 1);
        assert!(diff.to_download.is_empty());
    }

    #[test]
    fn mixed_scenario() {
        let local = Manifest {
            files: vec![
                entry("both-same.txt", "aaa", 1000),
                entry("both-local-newer.txt", "bbb", 2000),
                entry("both-remote-newer.txt", "ccc", 1000),
                entry("local-only.txt", "ddd", 1000),
            ],
        };
        let remote = Manifest {
            files: vec![
                entry("both-same.txt", "aaa", 1000),
                entry("both-local-newer.txt", "xxx", 1500),
                entry("both-remote-newer.txt", "yyy", 2000),
                entry("remote-only.txt", "eee", 1000),
            ],
        };
        let diff = compute_diff(&local, &remote);

        let upload_paths: Vec<&str> = diff.to_upload.iter().map(|f| f.path.as_str()).collect();
        let download_paths: Vec<&str> = diff.to_download.iter().map(|f| f.path.as_str()).collect();

        assert!(upload_paths.contains(&"both-local-newer.txt"));
        assert!(upload_paths.contains(&"local-only.txt"));
        assert_eq!(upload_paths.len(), 2);

        assert!(download_paths.contains(&"both-remote-newer.txt"));
        assert!(download_paths.contains(&"remote-only.txt"));
        assert_eq!(download_paths.len(), 2);
    }
}
