use crate::protocol::{
    ArchiveEntry, ClientDiff, FileEntry, Manifest, RenameEntry, ServerDiff, SyncDiff,
};
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

/// Determine the destination path for a file being moved to the archive directory.
///
/// `subdir` is the subdirectory within the archive (e.g. `"conflicts"` for conflict losers,
/// or `""` for files moved directly to the archive root).
///
/// `assignments` maps `sha256 → archive_path` for paths already assigned in the current sync.
/// Returns `(path, already_present)` where `already_present` is true if the file is already
/// anywhere in the archive directory (any subdirectory).
pub fn archive_dest_path(
    original: &str,
    sha: &str,
    subdir: &str,
    archive: &Manifest,
    assignments: &mut HashMap<String, String>,
) -> (String, bool) {
    // Already on disk in archive folder (search entire archive regardless of path prefix)
    for file in &archive.files {
        if file.sha256 == sha {
            return (file.path.clone(), true);
        }
    }
    // Already assigned in this sync — reuse the same dest
    if let Some(existing) = assignments.get(sha) {
        return (existing.clone(), false);
    }
    // Candidate path: place under subdir if provided
    let candidate = if subdir.is_empty() {
        original.to_string()
    } else {
        format!("{subdir}/{original}")
    };
    // Use candidate path if free
    let occupied: HashSet<&str> = assignments.values().map(|s| s.as_str()).collect();
    let on_disk: HashSet<&str> = archive.files.iter().map(|f| f.path.as_str()).collect();
    if !occupied.contains(candidate.as_str()) && !on_disk.contains(candidate.as_str()) {
        assignments.insert(sha.to_string(), candidate.clone());
        return (candidate, false);
    }
    // Generate a unique name using the current unix timestamp
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let p = std::path::Path::new(original);
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or(original);
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{e}"))
        .unwrap_or_default();
    let stamped = format!("{stem}_{ts}{ext}");
    let dir = p
        .parent()
        .and_then(|par| par.to_str())
        .filter(|s| !s.is_empty());
    let dest = match (subdir.is_empty(), dir) {
        (true, Some(parent)) => format!("{parent}/{stamped}"),
        (true, None) => stamped,
        (false, Some(parent)) => format!("{subdir}/{parent}/{stamped}"),
        (false, None) => format!("{subdir}/{stamped}"),
    };
    assignments.insert(sha.to_string(), dest.clone());
    (dest, false)
}

/// Compute the sync diff between client and server manifests.
///
/// `ledger` maps paths the server has previously received from the client
/// (path → sha256 at upload time). Used to detect server-side deletions and
/// to determine which side owns a change when the same path diverges.
///
/// Rules:
/// - Same path, same sha256: in sync, nothing to do.
/// - Same path, different sha256: ledger decides who changed (if available), else mtime wins.
/// - Client file sha256 matches a server file at a different path: server renamed it;
///   client renames locally — unless the client also renamed the file (ledger shows a
///   different origin path), in which case the client's path wins.
/// - Client file not on server and ledger shows same sha256 was uploaded: server deleted it.
/// - Client file not on server, no ledger entry: client has new/changed file to upload.
/// - Server file unaccounted for: download it, unless it is a stale rename of a
///   ledger-tracked file that the client has since deleted (→ delete or archive server-side).
pub fn compute_diff(
    client: &Manifest,
    server: &Manifest,
    ledger: &HashMap<String, String>,
    archive: &Manifest,
) -> SyncDiff {
    // Build maps for quick lookup
    let client_map: HashMap<&str, &FileEntry> =
        client.files.iter().map(|f| (f.path.as_str(), f)).collect();
    let server_map: HashMap<&str, &FileEntry> =
        server.files.iter().map(|f| (f.path.as_str(), f)).collect();

    // Reverse ledger: sha256 → set of paths the ledger associates with that sha.
    let mut ledger_by_sha: HashMap<&str, HashSet<&str>> = HashMap::new();
    for (path, sha) in ledger {
        ledger_by_sha
            .entry(sha.as_str())
            .or_default()
            .insert(path.as_str());
    }

    // Detect "contaminated chains": groups of server files whose content was shuffled among
    // themselves (each got another ledger-tracked path's sha), where at least one file in the
    // group was also changed by the client. When this happens, following any of the server's
    // moves risks overwriting the client's work, so the client's version wins for all of them.
    //
    // Build a graph: for each server path P whose sha changed AND whose new sha came from
    // another ledger path L, add an undirected edge between P and L.
    let mut chain_graph: HashMap<&str, HashSet<&str>> = HashMap::new();
    for server_file in &server.files {
        let path = server_file.path.as_str();
        let sha = server_file.sha256.as_str();
        if let Some(ledger_sha) = ledger.get(path) {
            if sha != ledger_sha.as_str() {
                if let Some(sources) = ledger_by_sha.get(sha) {
                    for &src in sources {
                        chain_graph.entry(path).or_default().insert(src);
                        chain_graph.entry(src).or_default().insert(path);
                    }
                }
            }
        }
    }
    // BFS: find connected components, mark those containing a client-changed path.
    let mut contaminated_paths: HashSet<&str> = HashSet::new();
    let mut visited: HashSet<&str> = HashSet::new();
    let chain_starts: Vec<&str> = chain_graph.keys().copied().collect();
    for start in chain_starts {
        if visited.contains(start) {
            continue;
        }
        let mut component: Vec<&str> = Vec::new();
        let mut stack = vec![start];
        while let Some(node) = stack.pop() {
            if visited.insert(node) {
                component.push(node);
                if let Some(neighbors) = chain_graph.get(node) {
                    for &n in neighbors {
                        if !visited.contains(n) {
                            stack.push(n);
                        }
                    }
                }
            }
        }
        let is_contaminated = component.iter().any(|&p| {
            let client_sha = client_map.get(p).map(|e| e.sha256.as_str());
            let ledger_sha = ledger.get(p).map(|s| s.as_str());
            client_sha != ledger_sha
        });
        if is_contaminated {
            contaminated_paths.extend(component);
        }
    }

    // Index server files by sha256 for rename detection.
    // Only include server entries NOT already matched by the same path+sha256 on the client.
    let server_by_sha: HashMap<&str, &FileEntry> = server
        .files
        .iter()
        .filter(|s| {
            // Exclude if client has the same path with the same sha256 (already in sync)
            client_map
                .get(s.path.as_str())
                .is_none_or(|c| c.sha256 != s.sha256)
        })
        .map(|s| (s.sha256.as_str(), s))
        .collect();

    let mut to_upload = Vec::new();
    let mut to_download = Vec::new();
    let mut to_delete = Vec::new();
    let mut to_rename = Vec::new();
    let mut to_delete_on_server = Vec::new();
    let mut server_conflicts: Vec<ArchiveEntry> = Vec::new();
    let mut client_conflicts: Vec<ArchiveEntry> = Vec::new();
    let mut server_deleted: Vec<ArchiveEntry> = Vec::new();
    let mut archive_assignments: HashMap<String, String> = HashMap::new();

    // Tracks server paths that have been "claimed" by a client file
    let mut matched_server_paths: HashSet<&str> = HashSet::new();

    for client_entry in &client.files {
        let path = client_entry.path.as_str();

        match server_map.get(path) {
            Some(server_entry) if server_entry.sha256 == client_entry.sha256 => {
                // Case A: in sync at same path
                matched_server_paths.insert(path);
            }
            Some(server_entry) => {
                // Case B: same path, different content.
                // If this path is part of a contaminated server shuffle chain, client wins.
                // Otherwise, prefer ledger-based attribution (who actually changed it) over mtime.
                matched_server_paths.insert(path);
                let upload = if contaminated_paths.contains(path) {
                    true // part of contaminated server shuffle chain → client wins
                } else {
                    match ledger.get(path) {
                        Some(ledger_sha) if ledger_sha == &client_entry.sha256 => false, // client unchanged → download
                        Some(ledger_sha) if ledger_sha == &server_entry.sha256 => true, // server unchanged → upload
                        _ => client_entry.modified >= server_entry.modified, // both changed → mtime
                    }
                };
                if upload {
                    let ledger_sha = ledger.get(path).map(|s| s.as_str());
                    if ledger_sha != Some(server_entry.sha256.as_str()) {
                        let (apath, present) = archive_dest_path(
                            path,
                            &server_entry.sha256,
                            "conflicts",
                            archive,
                            &mut archive_assignments,
                        );
                        server_conflicts.push(ArchiveEntry {
                            original_path: path.to_string(),
                            archive_path: apath,
                            already_present: present,
                        });
                    }
                    to_upload.push(client_entry.clone());
                } else {
                    let ledger_sha = ledger.get(path).map(|s| s.as_str());
                    if ledger_sha != Some(client_entry.sha256.as_str()) {
                        let (apath, present) = archive_dest_path(
                            path,
                            &client_entry.sha256,
                            "conflicts",
                            archive,
                            &mut archive_assignments,
                        );
                        client_conflicts.push(ArchiveEntry {
                            original_path: path.to_string(),
                            archive_path: apath,
                            already_present: present,
                        });
                    }
                    to_download.push((*server_entry).clone());
                }
            }
            None => {
                // No server file at the same path — check for rename or delete
                if let Some(server_entry) = server_by_sha.get(client_entry.sha256.as_str()) {
                    // Case C: server has this sha256 at a different path → possible rename.
                    let target_path = server_entry.path.as_str();
                    let sha = client_entry.sha256.as_str();

                    // If the ledger associates this sha with other paths (not the current
                    // client path), the client has renamed the file to its current path.
                    // Client's rename wins; the server's copy at target_path is stale.
                    let client_renamed =
                        ledger_by_sha.get(sha).is_some_and(|lp| !lp.contains(path));

                    if client_renamed {
                        to_upload.push(client_entry.clone());
                        to_delete_on_server.push(server_entry.path.clone());
                        matched_server_paths.insert(target_path);
                    } else if client_map.contains_key(target_path) {
                        // Rename target is occupied on the client — skip rename, upload instead
                        to_upload.push(client_entry.clone());
                    } else {
                        to_rename.push(RenameEntry {
                            from: client_entry.path.clone(),
                            to: server_entry.path.clone(),
                        });
                        matched_server_paths.insert(target_path);
                    }
                } else {
                    // Case D: no server match at all
                    let ledger_sha = ledger.get(path).map(|s| s.as_str());
                    if ledger_sha == Some(client_entry.sha256.as_str()) {
                        // Server deleted it and client hasn't changed it
                        to_delete.push(client_entry.path.clone());
                    } else {
                        to_upload.push(client_entry.clone());
                    }
                }
            }
        }
    }

    // Remaining server files not claimed by any client file.
    for server_entry in &server.files {
        if !matched_server_paths.contains(server_entry.path.as_str()) {
            // If this sha was previously tracked in the ledger, and *all* of the paths
            // that carried it are now absent from the client, the server file is either:
            // - A directly-tracked file the client deleted (path is in ledger paths) → archive
            // - A stale rename of something the client has since deleted → delete from server
            let sha = server_entry.sha256.as_str();
            let path = server_entry.path.as_str();
            let stale_paths = ledger_by_sha
                .get(sha)
                .filter(|lp| lp.iter().all(|p| !client_map.contains_key(p)));

            if let Some(lp) = stale_paths {
                if lp.contains(path) {
                    // Client deleted this synced file → archive it on server
                    let (apath, present) =
                        archive_dest_path(path, sha, "", archive, &mut archive_assignments);
                    server_deleted.push(ArchiveEntry {
                        original_path: path.to_string(),
                        archive_path: apath,
                        already_present: present,
                    });
                } else {
                    // Stale rename leftover → delete
                    to_delete_on_server.push(server_entry.path.clone());
                }
            } else {
                to_download.push(server_entry.clone());
            }
        }
    }

    // Upload paths override both to_delete_on_server and server_deleted.
    let upload_paths: HashSet<&str> = to_upload.iter().map(|f| f.path.as_str()).collect();
    to_delete_on_server.retain(|p| !upload_paths.contains(p.as_str()));
    server_deleted.retain(|e| !upload_paths.contains(e.original_path.as_str()));

    // server_deleted paths take precedence over to_delete_on_server.
    let server_deleted_paths: HashSet<&str> = server_deleted
        .iter()
        .map(|e| e.original_path.as_str())
        .collect();
    to_delete_on_server.retain(|p| !server_deleted_paths.contains(p.as_str()));

    to_upload.sort_by(|a, b| a.path.cmp(&b.path));
    to_download.sort_by(|a, b| a.path.cmp(&b.path));
    to_delete.sort();
    to_rename.sort_by(|a, b| a.from.cmp(&b.from));
    to_delete_on_server.sort();

    SyncDiff {
        client: ClientDiff {
            to_upload,
            to_download,
            to_delete,
            to_rename,
            conflicts: client_conflicts,
        },
        server: ServerDiff {
            to_delete: to_delete_on_server,
            conflicts: server_conflicts,
            deleted: server_deleted,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, sha256: &str, modified: u64) -> FileEntry {
        FileEntry {
            path: path.into(),
            sha256: sha256.into(),
            size: 0,
            modified,
        }
    }

    fn no_ledger() -> HashMap<String, String> {
        HashMap::new()
    }

    /// Parse a `{path_letters}{hash_digits}` token into `(path, hash, mtime)`.
    ///
    /// The numeric suffix is both the hash string and the mtime value —
    /// a higher number means a newer file.
    fn parse_tok(tok: &str) -> (String, String, u64) {
        let i = tok
            .find(|c: char| c.is_ascii_digit())
            .unwrap_or_else(|| panic!("token `{tok}` must contain a digit"));
        let path = tok[..i].to_string();
        let hash = tok[i..].to_string();
        let mtime: u64 = hash.parse().unwrap_or(1);
        (path, hash, mtime)
    }

    /// Compact table DSL for sync tests.
    ///
    /// `input` has one row per file (newline-separated), with three
    /// whitespace-separated tokens:
    ///
    /// ```text
    /// client  ledger  server
    /// ```
    ///
    /// Each token is `-` (absent) or `{path}{hash}`, e.g. `a1` or `note3`.
    /// The letter prefix is the file path; the numeric suffix is both the hash
    /// and the mtime — a higher digit means a newer file.
    ///
    /// `expected` is a space-separated list of `{path}{hash}` tokens representing
    /// the desired final state on *both* client and server after applying the diff.
    /// An empty string means all files should be gone.
    fn check(input: &str, expected: &str) {
        let mut client_files: Vec<FileEntry> = Vec::new();
        let mut server_files: Vec<FileEntry> = Vec::new();
        let mut ledger: HashMap<String, String> = HashMap::new();

        for line in input.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let tokens: Vec<&str> = line.split_whitespace().collect();
            assert_eq!(
                tokens.len(),
                3,
                "each row needs 3 tokens: client ledger server"
            );
            let (ct, lt, st) = (tokens[0], tokens[1], tokens[2]);

            if ct != "-" {
                let (path, hash, mtime) = parse_tok(ct);
                client_files.push(FileEntry {
                    path,
                    sha256: hash,
                    size: 0,
                    modified: mtime,
                });
            }
            if lt != "-" {
                let (path, hash, _) = parse_tok(lt);
                ledger.insert(path, hash);
            }
            if st != "-" {
                let (path, hash, mtime) = parse_tok(st);
                server_files.push(FileEntry {
                    path,
                    sha256: hash,
                    size: 0,
                    modified: mtime,
                });
            }
        }

        let diff = compute_diff(
            &Manifest {
                files: client_files.clone(),
            },
            &Manifest {
                files: server_files.clone(),
            },
            &ledger,
            &Manifest::default(),
        );

        // Simulate applying the diff on the client side
        let mut final_client: HashMap<String, String> = client_files
            .iter()
            .map(|f| (f.path.clone(), f.sha256.clone()))
            .collect();
        for path in &diff.client.to_delete {
            final_client.remove(path);
        }
        for r in &diff.client.to_rename {
            if let Some(hash) = final_client.remove(&r.from) {
                final_client.insert(r.to.clone(), hash);
            }
        }
        for f in &diff.client.to_download {
            final_client.insert(f.path.clone(), f.sha256.clone());
        }

        // Simulate applying the diff on the server side
        let mut final_server: HashMap<String, String> = server_files
            .iter()
            .map(|f| (f.path.clone(), f.sha256.clone()))
            .collect();
        for f in &diff.client.to_upload {
            final_server.insert(f.path.clone(), f.sha256.clone());
        }
        for path in &diff.server.to_delete {
            final_server.remove(path);
        }
        for entry in &diff.server.deleted {
            final_server.remove(&entry.original_path);
        }

        let expected_state: HashMap<String, String> = expected
            .split_whitespace()
            .map(|tok| {
                let (path, hash, _) = parse_tok(tok);
                (path, hash)
            })
            .collect();

        assert_eq!(
            final_client, expected_state,
            "client state mismatch\ndiff: {diff:?}"
        );
        assert_eq!(
            final_server, expected_state,
            "server state mismatch\ndiff: {diff:?}"
        );
    }

    #[test]
    fn both_empty() {
        check("", "");
    }

    #[test]
    fn identical_manifests() {
        check("a1 - a1", "a1");
    }

    #[test]
    fn local_only_files_upload() {
        check("a1 - -", "a1");
    }

    #[test]
    fn remote_only_files_download() {
        check("- - a1", "a1");
    }

    #[test]
    fn conflict_local_newer_uploads() {
        check("n3 - n1", "n3");
    }

    #[test]
    fn conflict_remote_newer_downloads() {
        check("n1 - n3", "n3");
    }

    #[test]
    fn conflict_same_mtime_local_wins() {
        // Same mtime, different content: `>=` means local wins.
        // Can't express same-mtime-different-hash in the DSL (the numeric suffix
        // encodes both hash and mtime), so we fall back to explicit entries here.
        let local = Manifest {
            files: vec![entry("n", "hash_a", 1000)],
        };
        let remote = Manifest {
            files: vec![entry("n", "hash_b", 1000)],
        };
        let diff = compute_diff(&local, &remote, &no_ledger(), &Manifest::default());
        assert_eq!(diff.client.to_upload.len(), 1);
        assert!(diff.client.to_download.is_empty());
    }

    #[test]
    fn mixed_scenario() {
        // a: in sync on both sides
        // b: client newer  → upload
        // c: server newer  → download
        // d: client only   → upload
        // e: server only   → download
        check(
            "a1 - a1
             b3 - b2
             c2 - c4
             d5 - -
             -  - e6",
            "a1 b3 c4 d5 e6",
        );
    }

    // --- Rename detection ---

    #[test]
    fn server_rename_detected() {
        // Server renamed "a" → "b" (same hash, different path)
        check("a1 - b1", "b1");
    }

    #[test]
    fn rename_target_occupied_client_uploads() {
        // Server has hash "1" at "b", but client already has "b" with a different hash
        // → rename target is occupied, so "a" is uploaded unchanged instead of renamed
        check(
            "a1 - -
             b2 - b1",
            "a1 b2",
        );
    }

    // --- Ledger-based deletion detection ---

    #[test]
    fn server_deleted_client_unchanged() {
        // Ledger confirms "1" was the last uploaded hash → server deleted it → delete locally
        check("a1 a1 -", "");
    }

    #[test]
    fn server_deleted_but_client_modified_uploads() {
        // Client changed "a" (now hash "2"), ledger records "1" as last upload, server deleted it
        // → client wins, upload
        check("a2 a1 -", "a2");
    }

    #[test]
    fn bootstrap_no_ledger_uploads_new_files() {
        // No ledger entry → treat as new file and upload, not delete
        // (contrast with server_deleted_client_unchanged where ledger confirms deletion)
        check("a1 - -", "a1");
    }

    #[test]
    fn move_conflict() {
        // When rename on both server and client, follow client
        check("b1 a1 c1", "b1")
    }

    #[test]
    fn rename_switch_around() {
        // Two files are switched around on the server
        check(
            "a1 a1 a2
             b2 b2 b1",
            "a2 b1",
        );

        // Three files are switched around on server
        check(
            "a1 a1 a2
             b2 b2 b3
             c3 c3 c1",
            "a2 b3 c1",
        );
    }

    #[test]
    fn rename_switch_around_on_client() {
        // Two files are switched around on the server
        check(
            "a2 a1 a1
             b1 b2 b2",
            "a2 b1",
        );
    }

    #[test]
    fn rename_on_both_sides() {
        // Client has a rename and the server has a differen conflicting rename
        // In that case the client has precedence, and the servers side cannot happen
        check(
            "a2 a1 a1
             b1 b2 b3
             c3 c3 b3",
            "a2 b1 c3",
        );
        // Even a bit more complicated, there are now three files involved on the server side
        // Which means that in principle one rename could happen, but would result in files being
        // lost. The best behavior would be to ignore all renames on the server side.
        check(
            "a2 a1 a1
             b1 b2 b3
             c3 c3 c4
             d4 d4 d2",
            "a2 b1 c3 d4",
        );
    }

    #[test]
    fn duplicates() {
        // If there is a duplicate (two files with the same hatch)
        check(
            "a1 a1 -
             b1 b1 b1",
            "b1",
        );
        check(
            "a1 a1 a1
             b1 b1 -",
            "a1",
        );
        check(
            "a1 a1 -
             b1 b1 -
             - - c1",
            "c1",
        );
        check(
            "- a1 -
             - b1 -
             c1 - -
             - - d1
             - - e1",
            "c1",
        )
    }

    #[test]
    fn client_delete_archives_on_server() {
        // Client had "a" in ledger, deleted it → server should archive it, not delete
        // Ledger shows a1 was uploaded; client no longer has it; server still has it
        let client = Manifest { files: vec![] };
        let server = Manifest {
            files: vec![entry("a", "1", 1)],
        };
        let mut ledger = HashMap::new();
        ledger.insert("a".to_string(), "1".to_string());

        let diff = compute_diff(&client, &server, &ledger, &Manifest::default());

        assert!(diff.client.to_download.is_empty());
        assert!(diff.server.to_delete.is_empty());
        assert_eq!(diff.server.deleted.len(), 1);
        assert_eq!(diff.server.deleted[0].original_path, "a");
        assert_eq!(diff.server.deleted[0].archive_path, "a");
        assert!(!diff.server.deleted[0].already_present);
    }

    #[test]
    fn stale_rename_leftover_still_deleted() {
        // Server renamed a→b (ledger has a→sha1), client deleted a.
        // Server's copy at b is a stale rename leftover → delete, not archive.
        let client = Manifest { files: vec![] };
        let server = Manifest {
            files: vec![entry("b", "1", 1)],
        };
        let mut ledger = HashMap::new();
        ledger.insert("a".to_string(), "1".to_string());

        let diff = compute_diff(&client, &server, &ledger, &Manifest::default());

        assert!(diff.client.to_download.is_empty());
        assert!(diff.server.deleted.is_empty());
        assert_eq!(diff.server.to_delete, vec!["b".to_string()]);
    }
}
