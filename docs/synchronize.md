# Sync Logic

This document describes how Scriptorum synchronises the client's note directory
with the server. The goal is to keep both sides identical while handling the
full range of real-world scenarios: independent edits, server-side renames and
deletes, conflicts, and duplicate content.

---

## Overview

Synchronisation happens in two phases:

1. **Diff phase** — the client sends its file manifest to the server
   (`POST /api/v1/sync/diff`). The server computes a `SyncDiff` that describes
   exactly what must happen to bring both sides into agreement, and returns it.

2. **Transfer phase** — the client executes the diff in order:
   1. Upload client's conflicted files to `/api/v1/conflicted/{path}`
   2. Delete locally (`to_delete`)
   3. Rename locally (`to_rename`)
   4. Upload to server (`to_upload`, via `PUT /api/v1/files/{path}`)
   5. Download from server (`to_download`, via `GET /api/v1/files/{path}`)

All diff logic lives in `compute_diff` in `crates/scriptorum-core/src/sync.rs`.

---

## Data Structures

### Manifest

A `Manifest` is a flat list of `FileEntry` values, one per file:

```
FileEntry {
    path:     "Daily/2026-02-17.note"   // relative to note root
    sha256:   "a3f5..."                  // lowercase hex
    size:     12345                      // bytes
    modified: 1740000000                 // Unix timestamp (seconds)
}
```

The client scans its local directory to build a manifest, then sends it to the
server. The server builds its own manifest from disk at the same time.

### SyncDiff

The diff returned by the server contains:

| Field               | Who acts on it | What it means |
|---------------------|----------------|---------------|
| `to_upload`         | client         | Upload these files to the server |
| `to_download`       | client         | Download these files from the server |
| `to_delete`         | client         | Delete these paths locally |
| `to_rename`         | client         | Apply these `{from → to}` renames locally |
| `to_delete_on_server` | server (in `apply_diff_to_ledger`) | Delete these stale server paths |
| `server_conflicts`  | server (in `apply_diff_to_ledger`) | Move the server's version of these files to the conflicted folder |
| `client_conflicts`  | client         | Upload the client's version of these files to `/api/v1/conflicted/` |

### The Ledger

The ledger is a server-side map of `path → sha256` that records the content
hash at the time the server last received each file from the client. It is the
key to distinguishing "who changed this file" from "both sides changed this
file."

- After a successful upload (`PUT /api/v1/files/{path}`), the server records
  `ledger[path] = sha256`.
- After computing a diff, `apply_diff_to_ledger` updates the ledger to reflect
  renames, downloads, and server-side deletes.

The ledger is stored on disk as `.ledger.json` inside the storage directory and
survives server restarts.

---

## Diff Algorithm

`compute_diff` iterates over the client's files and classifies each one, then
handles any remaining server files.

### Case A — Same path, same content

```
client: note.txt  sha=abc
server: note.txt  sha=abc
```

Nothing to do. The path is marked as matched.

### Case B — Same path, different content (conflict)

```
client: note.txt  sha=NEW_C  mtime=200
server: note.txt  sha=NEW_S  mtime=100
```

Both sides have a file at the same path but with different content. The diff
must pick a winner and optionally preserve the loser. The decision is made in
priority order:

1. **Contaminated chain** (see below) → client always wins.
2. **Ledger says client is unchanged** (`ledger[path] == client sha`) → client
   didn't touch it, so the server changed it → **download**.
3. **Ledger says server is unchanged** (`ledger[path] == server sha`) → server
   still has the last-known version, so the client changed it → **upload**.
4. **Both changed, or no ledger entry** → fall back to mtime: higher mtime wins;
   ties go to the client (`>=`).

#### Conflict protection

When a winner is chosen, the loser's version is preserved in the conflicted
folder *only if the loser actually changed independently* (i.e. the ledger does
not already match the loser's sha — if it does, the loser is just the shared
known-good baseline, which is already safe).

- If **client wins** (upload): server's version goes to `server_conflicts`. The
  server moves it to the conflicted folder during `apply_diff_to_ledger`, before
  any uploads are processed.
- If **server wins** (download): client's version goes to `client_conflicts`.
  The client uploads it to `PUT /api/v1/conflicted/{conflicted_path}` first,
  before downloading the server's version.

No conflict entry is created when the loser's sha matches the ledger, because
the ledger confirms the loser never changed — it is the pre-change baseline that
both sides already have.

### Case C — Client file exists at a different server path (rename detection)

```
client: note.txt    sha=abc
server: Archive/note.txt  sha=abc    (same content, different path)
```

The server has the content but at a different path. This means the server
renamed it. The client follows the rename locally.

There are two sub-cases that override the simple rename:

**Sub-case C1 — Client also renamed the file**

If the ledger associates `sha=abc` with a *different* path than the client's
current path, the client independently renamed the file to its current location.
The client's rename takes precedence: the client uploads to its new path and
the old server path is deleted.

```
ledger: old_name.txt → abc
client: new_name.txt → abc    (client renamed old_name → new_name)
server: Archive/old_name.txt → abc  (server renamed old_name → Archive/old_name)
→ client wins: upload new_name.txt, delete Archive/old_name.txt
```

**Sub-case C2 — Rename target is occupied on the client**

If the server's target path already holds a different file on the client,
applying the rename would overwrite it. The rename is skipped and the client
simply uploads its version.

```
client: a.txt → sha=1
        b.txt → sha=2       ← rename target is occupied
server: b.txt → sha=1       ← server renamed a → b, but client has b already
→ upload a.txt, keep b.txt as-is
```

### Case D — Client file not on server

The client has a file at a path the server does not have at all, *and* the
content does not appear anywhere on the server.

- **Ledger matches** (`ledger[path] == client sha`): the server had this file,
  the client hasn't changed it, and now it's gone. The server deleted it →
  **delete locally**.
- **No ledger match** (or no entry): the client has a new or independently
  changed file → **upload**.

The second rule is also the bootstrap case: on first sync there is no ledger at
all, so every client file is treated as new and uploaded.

### Remaining server files

After processing all client files, any server file not yet matched is handled:

- **Stale rename target**: if the server file's sha was in the ledger under some
  paths, and *all* of those paths are now absent from the client, the file is a
  server-side rename of something the client has since deleted. It is cleaned up
  server-side (`to_delete_on_server`).
- **Otherwise**: a file the client doesn't have yet → **download**.

### Deduplication of server deletes

If a path appears in both `to_delete_on_server` and `to_upload`, the upload
will overwrite it anyway, so the delete is dropped.

---

## Contaminated Chain Detection

Consider this scenario:

```
ledger: a.txt → sha=1,  b.txt → sha=2
client: a.txt → sha=1,  b.txt → sha=X   (client modified b)
server: a.txt → sha=2,  b.txt → sha=1   (server swapped a and b)
```

The server swapped the content of `a.txt` and `b.txt` (each now holds the
other's last-known hash). If we naively applied the ledger rule:

- `a.txt`: ledger=1, server=2, client=1 → "server changed a" → download server's
  version (sha=2) into `a.txt` → **overwrites client's `a.txt`**
- `b.txt`: ledger=2, server=1, client=X → "client changed b" → upload → OK

But downloading sha=2 into `a.txt` and having sha=2 also at `b.txt` would leave
a duplicate. More importantly, if the client had also changed `a.txt` we would
silently overwrite it.

The contamination check detects groups of server files that have shuffled each
other's ledger-tracked content (a "swap chain"). If *any* file in such a group
was independently modified by the client, the entire group is marked
**contaminated** and the client wins all of them, bypassing the ledger rules.

The detection works by building an undirected graph: for each server path `P`
whose content changed to sha `S`, if `S` was the ledger-known content of some
other path `L`, add an edge between `P` and `L`. Connected components where any
node has a client modification are marked contaminated.

---

## Conflicted Folder

When a conflict is detected, the losing version is preserved in a separate
server-side directory (the *conflicted folder*, configured with `--conflicted-dir`,
default `./conflicted`).

### Path assignment (`conflict_dest_path`)

Assigning a destination path for a conflicted file follows these rules in order:

1. **Already on disk**: if the conflicted folder already contains a file with
   the same sha256, no move is needed — the content is already safe. The diff
   entry is marked `already_present = true` and no action is taken.

2. **Already assigned this sync**: if the same sha was assigned a conflicted
   path earlier in the same diff computation (e.g. two files with identical
   content both lose), reuse the same destination. The content only needs to
   be stored once.

3. **Original path is free**: if the original path (e.g. `note.txt`) is not
   already taken in the conflicted folder, use it as-is. This is the common
   case.

4. **Path collision**: if the original path is already occupied, generate
   `{stem}_{unix_timestamp}{ext}` (e.g. `note_1740000000.note`), preserving
   the directory structure.

### Who moves what

- **`server_conflicts`** are handled by the server during `apply_diff_to_ledger`
  (called within the same lock as `sync_diff`, before any `PUT` requests arrive).
  The server calls `fs::rename` to move the file atomically.

- **`client_conflicts`** are handled by the client at the start of the transfer
  phase, before applying any deletes, renames, or downloads. The client reads
  the local file and `PUT`s it to `/api/v1/conflicted/{conflicted_path}`. The
  conflicted endpoint writes the data but does **not** update the ledger.

---

## Complete Example

```
Initial state (after first sync):
  notes/: note.txt  sha=v1
  ledger: note.txt → v1

Between syncs:
  Server admin edits note.txt → sha=v3  (mtime advances)
  User edits note.txt on device → sha=v2  (mtime advances even more)

Diff computation:
  client: note.txt sha=v2 mtime=300
  server: note.txt sha=v3 mtime=200
  ledger: note.txt → v1

  Case B. ledger=v1 ≠ client=v2 ≠ server=v3 → both changed → mtime wins.
  client mtime (300) > server mtime (200) → client wins → upload.

  server_conflicts: [{original_path: "note.txt", conflicted_path: "note.txt", already_present: false}]
  to_upload: [note.txt sha=v2]

apply_diff_to_ledger (on server, within sync_diff handler):
  → moves notes/note.txt (sha=v3) to conflicted/note.txt

Transfer phase (client):
  server_conflicts reported to user (informational only, server already handled it)
  PUT /api/v1/files/note.txt with sha=v2
  → server writes v2 to notes/note.txt, ledger[note.txt] = v2

Final state:
  notes/:     note.txt sha=v2   (client's version wins)
  conflicted/: note.txt sha=v3  (server's displaced version preserved)
  ledger:     note.txt → v2
  client:     note.txt sha=v2
```

---

## Edge Cases Summary

| Scenario | Outcome |
|----------|---------|
| File only on client | Upload |
| File only on server | Download |
| Same file, same content | Nothing |
| Same file, client newer (mtime) | Upload |
| Same file, server newer (mtime) | Download; client version saved in conflicted |
| Same file, client changed, server unchanged (ledger) | Upload |
| Same file, server changed, client unchanged (ledger) | Download; no conflict entry (loser = baseline) |
| Same file, both changed, same mtime | Upload (client wins ties) |
| Server renamed file, client unchanged | Client renames locally |
| Server renamed file, client also renamed | Client's rename wins; upload to client path, delete server path |
| Server renamed file, target occupied on client | Rename skipped; client uploads its version |
| Server deleted file, client unchanged (ledger confirms) | Client deletes locally |
| Server deleted file, client modified since | Client uploads |
| No ledger, file missing on server | Upload (bootstrap case) |
| Server shuffled content between files, client modified any of them | All in the swap group: client wins |
| Conflict, losing version already in conflicted folder | `already_present = true`; no duplicate stored |
| Multiple files with same sha both lose in one sync | Single conflicted entry; content stored once |
