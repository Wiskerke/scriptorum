use scriptorum_core::client::perform_sync;
use std::fs;
use tempfile::TempDir;

/// Start the server on a random port and return the URL.
async fn start_server(storage_dir: &std::path::Path, conflicted_dir: &std::path::Path) -> String {
    let app = scriptorum_server::build_app(storage_dir, conflicted_dir).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

#[tokio::test]
async fn upload_files_to_empty_server() {
    let server_dir = TempDir::new().unwrap();
    let client_dir = TempDir::new().unwrap();
    let url = start_server(server_dir.path(), &server_dir.path().join("conflicted")).await;

    // Create files on the client side
    fs::write(client_dir.path().join("note1.txt"), "hello").unwrap();
    fs::create_dir_all(client_dir.path().join("sub")).unwrap();
    fs::write(client_dir.path().join("sub/note2.txt"), "world").unwrap();

    // Sync: client -> server
    let result = tokio::task::spawn_blocking({
        let url = url.clone();
        let path = client_dir.path().to_path_buf();
        move || perform_sync(&url, &path, None, |msg| eprintln!("  {msg}"))
    })
    .await
    .unwrap()
    .unwrap();

    assert_eq!(result.uploaded, 2);
    assert_eq!(result.downloaded, 0);
    assert_eq!(result.deleted, 0);
    assert_eq!(result.renamed, 0);

    // Verify files exist on server
    assert_eq!(
        fs::read_to_string(server_dir.path().join("note1.txt")).unwrap(),
        "hello"
    );
    assert_eq!(
        fs::read_to_string(server_dir.path().join("sub/note2.txt")).unwrap(),
        "world"
    );
}

#[tokio::test]
async fn download_files_from_server() {
    let server_dir = TempDir::new().unwrap();
    let client_dir = TempDir::new().unwrap();
    let url = start_server(server_dir.path(), &server_dir.path().join("conflicted")).await;

    // Pre-populate server storage
    fs::write(server_dir.path().join("from_server.txt"), "server data").unwrap();
    fs::create_dir_all(server_dir.path().join("deep")).unwrap();
    fs::write(server_dir.path().join("deep/nested.txt"), "nested data").unwrap();

    // Sync: server -> client
    let result = tokio::task::spawn_blocking({
        let url = url.clone();
        let path = client_dir.path().to_path_buf();
        move || perform_sync(&url, &path, None, |msg| eprintln!("  {msg}"))
    })
    .await
    .unwrap()
    .unwrap();

    assert_eq!(result.uploaded, 0);
    assert_eq!(result.downloaded, 2);

    assert_eq!(
        fs::read_to_string(client_dir.path().join("from_server.txt")).unwrap(),
        "server data"
    );
    assert_eq!(
        fs::read_to_string(client_dir.path().join("deep/nested.txt")).unwrap(),
        "nested data"
    );
}

#[tokio::test]
async fn bidirectional_sync() {
    let server_dir = TempDir::new().unwrap();
    let client_dir = TempDir::new().unwrap();
    let url = start_server(server_dir.path(), &server_dir.path().join("conflicted")).await;

    // Server has one file, client has another
    fs::write(server_dir.path().join("server_note.txt"), "from server").unwrap();
    fs::write(client_dir.path().join("client_note.txt"), "from client").unwrap();

    let result = tokio::task::spawn_blocking({
        let url = url.clone();
        let path = client_dir.path().to_path_buf();
        move || perform_sync(&url, &path, None, |msg| eprintln!("  {msg}"))
    })
    .await
    .unwrap()
    .unwrap();

    assert_eq!(result.uploaded, 1);
    assert_eq!(result.downloaded, 1);

    // Client should now have server's file
    assert_eq!(
        fs::read_to_string(client_dir.path().join("server_note.txt")).unwrap(),
        "from server"
    );
    // Server should now have client's file
    assert_eq!(
        fs::read_to_string(server_dir.path().join("client_note.txt")).unwrap(),
        "from client"
    );
}

#[tokio::test]
async fn no_changes_on_second_sync() {
    let server_dir = TempDir::new().unwrap();
    let client_dir = TempDir::new().unwrap();
    let url = start_server(server_dir.path(), &server_dir.path().join("conflicted")).await;

    fs::write(client_dir.path().join("note.txt"), "content").unwrap();

    // First sync: uploads the file
    let result = tokio::task::spawn_blocking({
        let url = url.clone();
        let path = client_dir.path().to_path_buf();
        move || perform_sync(&url, &path, None, |_| {})
    })
    .await
    .unwrap()
    .unwrap();
    assert_eq!(result.uploaded, 1);

    // Second sync: nothing to do
    let result = tokio::task::spawn_blocking({
        let url = url.clone();
        let path = client_dir.path().to_path_buf();
        move || perform_sync(&url, &path, None, |_| {})
    })
    .await
    .unwrap()
    .unwrap();
    assert_eq!(result.uploaded, 0);
    assert_eq!(result.downloaded, 0);
    assert_eq!(result.deleted, 0);
    assert_eq!(result.renamed, 0);
}

#[tokio::test]
async fn sync_after_client_modifies_file() {
    let server_dir = TempDir::new().unwrap();
    let client_dir = TempDir::new().unwrap();
    let url = start_server(server_dir.path(), &server_dir.path().join("conflicted")).await;

    fs::write(client_dir.path().join("note.txt"), "v1").unwrap();

    // First sync
    let result = tokio::task::spawn_blocking({
        let url = url.clone();
        let path = client_dir.path().to_path_buf();
        move || perform_sync(&url, &path, None, |_| {})
    })
    .await
    .unwrap()
    .unwrap();
    assert_eq!(result.uploaded, 1);

    // Modify the file on client (ensure mtime advances)
    std::thread::sleep(std::time::Duration::from_millis(1100));
    fs::write(client_dir.path().join("note.txt"), "v2").unwrap();

    // Second sync: should upload the modified file
    let result = tokio::task::spawn_blocking({
        let url = url.clone();
        let path = client_dir.path().to_path_buf();
        move || perform_sync(&url, &path, None, |msg| eprintln!("  {msg}"))
    })
    .await
    .unwrap()
    .unwrap();
    assert_eq!(result.uploaded, 1);
    assert_eq!(result.downloaded, 0);

    assert_eq!(
        fs::read_to_string(server_dir.path().join("note.txt")).unwrap(),
        "v2"
    );
}

/// Server admin renames a file (moves to Archive/). Client had the original.
/// After sync, client should have renamed the file locally.
#[tokio::test]
async fn server_rename_propagates_to_client() {
    let server_dir = TempDir::new().unwrap();
    let client_dir = TempDir::new().unwrap();
    let url = start_server(server_dir.path(), &server_dir.path().join("conflicted")).await;

    // Client uploads note.txt
    fs::write(client_dir.path().join("note.txt"), "my note").unwrap();
    let result = tokio::task::spawn_blocking({
        let url = url.clone();
        let path = client_dir.path().to_path_buf();
        move || perform_sync(&url, &path, None, |_| {})
    })
    .await
    .unwrap()
    .unwrap();
    assert_eq!(result.uploaded, 1);

    // Admin renames note.txt → Archive/note.txt on the server
    fs::create_dir_all(server_dir.path().join("Archive")).unwrap();
    fs::rename(
        server_dir.path().join("note.txt"),
        server_dir.path().join("Archive/note.txt"),
    )
    .unwrap();

    // Next sync: client should follow the rename
    let result = tokio::task::spawn_blocking({
        let url = url.clone();
        let path = client_dir.path().to_path_buf();
        move || perform_sync(&url, &path, None, |msg| eprintln!("  {msg}"))
    })
    .await
    .unwrap()
    .unwrap();

    assert_eq!(result.renamed, 1);
    assert_eq!(result.uploaded, 0);
    assert_eq!(result.downloaded, 0);
    assert_eq!(result.deleted, 0);

    // note.txt should be gone, Archive/note.txt should exist on client
    assert!(!client_dir.path().join("note.txt").exists());
    assert_eq!(
        fs::read_to_string(client_dir.path().join("Archive/note.txt")).unwrap(),
        "my note"
    );
}

/// Server admin deletes a file. Client had the same version.
/// After sync, client should delete it locally.
#[tokio::test]
async fn server_delete_propagates_to_client() {
    let server_dir = TempDir::new().unwrap();
    let client_dir = TempDir::new().unwrap();
    let url = start_server(server_dir.path(), &server_dir.path().join("conflicted")).await;

    // Client uploads note.txt
    fs::write(client_dir.path().join("note.txt"), "to be deleted").unwrap();
    let result = tokio::task::spawn_blocking({
        let url = url.clone();
        let path = client_dir.path().to_path_buf();
        move || perform_sync(&url, &path, None, |_| {})
    })
    .await
    .unwrap()
    .unwrap();
    assert_eq!(result.uploaded, 1);

    // Admin deletes the file on the server
    fs::remove_file(server_dir.path().join("note.txt")).unwrap();

    // Next sync: client should delete it
    let result = tokio::task::spawn_blocking({
        let url = url.clone();
        let path = client_dir.path().to_path_buf();
        move || perform_sync(&url, &path, None, |msg| eprintln!("  {msg}"))
    })
    .await
    .unwrap()
    .unwrap();

    assert_eq!(result.deleted, 1);
    assert_eq!(result.uploaded, 0);
    assert_eq!(result.downloaded, 0);
    assert_eq!(result.renamed, 0);

    assert!(!client_dir.path().join("note.txt").exists());
}

/// Client modifies a file while server admin renames it.
/// Client's version should win: it gets uploaded to the new path,
/// but the client should not rename its local file (it has new content).
#[tokio::test]
async fn client_change_wins_over_server_rename() {
    let server_dir = TempDir::new().unwrap();
    let client_dir = TempDir::new().unwrap();
    let url = start_server(server_dir.path(), &server_dir.path().join("conflicted")).await;

    // Client uploads note.txt (v1)
    fs::write(client_dir.path().join("note.txt"), "v1").unwrap();
    let result = tokio::task::spawn_blocking({
        let url = url.clone();
        let path = client_dir.path().to_path_buf();
        move || perform_sync(&url, &path, None, |_| {})
    })
    .await
    .unwrap()
    .unwrap();
    assert_eq!(result.uploaded, 1);

    // Admin renames note.txt → Archive/note.txt on server
    fs::create_dir_all(server_dir.path().join("Archive")).unwrap();
    fs::rename(
        server_dir.path().join("note.txt"),
        server_dir.path().join("Archive/note.txt"),
    )
    .unwrap();

    // Client modifies note.txt before next sync (new content = new sha256)
    std::thread::sleep(std::time::Duration::from_millis(1100));
    fs::write(client_dir.path().join("note.txt"), "v2 modified").unwrap();

    // Next sync: client's version wins — no rename, upload the modified file
    let result = tokio::task::spawn_blocking({
        let url = url.clone();
        let path = client_dir.path().to_path_buf();
        move || perform_sync(&url, &path, None, |msg| eprintln!("  {msg}"))
    })
    .await
    .unwrap()
    .unwrap();

    assert_eq!(result.renamed, 0);
    // note.txt is still present on client with v2 content
    assert_eq!(
        fs::read_to_string(client_dir.path().join("note.txt")).unwrap(),
        "v2 modified"
    );
    // Archive/note.txt (v1) comes down from server OR client uploads note.txt (v2)
    // Either way, no panic and no unintended rename happened.
}

/// Both sides independently change the same file. Client is newer → client wins.
/// Server's version must be preserved in the conflicted folder.
#[tokio::test]
async fn conflict_preserves_server_version() {
    let server_dir = TempDir::new().unwrap();
    let conflicted_dir = TempDir::new().unwrap();
    let client_dir = TempDir::new().unwrap();
    let url = start_server(server_dir.path(), conflicted_dir.path()).await;

    // Step 1: initial sync — both sides have v1
    fs::write(client_dir.path().join("note.txt"), "v1").unwrap();
    let result = tokio::task::spawn_blocking({
        let url = url.clone();
        let path = client_dir.path().to_path_buf();
        move || perform_sync(&url, &path, None, |_| {})
    })
    .await
    .unwrap()
    .unwrap();
    assert_eq!(result.uploaded, 1);

    // Step 2: server file is changed to v3 out-of-band
    std::thread::sleep(std::time::Duration::from_millis(100));
    fs::write(server_dir.path().join("note.txt"), "v3-server").unwrap();

    // Step 3: client modifies note.txt to v2, ensure client is newer
    std::thread::sleep(std::time::Duration::from_millis(1100));
    fs::write(client_dir.path().join("note.txt"), "v2-client").unwrap();

    // Sync: client wins (newer mtime) → uploads v2; server's v3 goes to conflicted
    let result = tokio::task::spawn_blocking({
        let url = url.clone();
        let path = client_dir.path().to_path_buf();
        move || perform_sync(&url, &path, None, |msg| eprintln!("  {msg}"))
    })
    .await
    .unwrap()
    .unwrap();

    assert_eq!(result.uploaded, 1);
    assert_eq!(result.downloaded, 0);
    assert_eq!(result.conflicts, 1);

    // Server notes dir has v2 (client's version)
    assert_eq!(
        fs::read_to_string(server_dir.path().join("note.txt")).unwrap(),
        "v2-client"
    );
    // Client has v2
    assert_eq!(
        fs::read_to_string(client_dir.path().join("note.txt")).unwrap(),
        "v2-client"
    );
    // Server conflicted dir has v3 (server's displaced version)
    let conflicted_note = conflicted_dir.path().join("note.txt");
    assert!(conflicted_note.exists(), "conflicted note.txt should exist");
    assert_eq!(fs::read_to_string(&conflicted_note).unwrap(), "v3-server");
}

/// Both sides independently change the same file. Server is newer → server wins.
/// Client's version must be uploaded to the server's conflicted folder.
#[tokio::test]
async fn conflict_preserves_client_version() {
    let server_dir = TempDir::new().unwrap();
    let conflicted_dir = TempDir::new().unwrap();
    let client_dir = TempDir::new().unwrap();
    let url = start_server(server_dir.path(), conflicted_dir.path()).await;

    // Step 1: initial sync — both sides have v1
    fs::write(client_dir.path().join("note.txt"), "v1").unwrap();
    let result = tokio::task::spawn_blocking({
        let url = url.clone();
        let path = client_dir.path().to_path_buf();
        move || perform_sync(&url, &path, None, |_| {})
    })
    .await
    .unwrap()
    .unwrap();
    assert_eq!(result.uploaded, 1);

    // Step 2: client modifies note.txt to v2
    std::thread::sleep(std::time::Duration::from_millis(100));
    fs::write(client_dir.path().join("note.txt"), "v2-client").unwrap();

    // Step 3: server file is changed to v3, ensure server is newer
    std::thread::sleep(std::time::Duration::from_millis(1100));
    fs::write(server_dir.path().join("note.txt"), "v3-server").unwrap();

    // Sync: server wins (newer mtime) → client downloads v3; client's v2 uploaded to conflicted
    let result = tokio::task::spawn_blocking({
        let url = url.clone();
        let path = client_dir.path().to_path_buf();
        move || perform_sync(&url, &path, None, |msg| eprintln!("  {msg}"))
    })
    .await
    .unwrap()
    .unwrap();

    assert_eq!(result.uploaded, 0);
    assert_eq!(result.downloaded, 1);
    assert_eq!(result.conflicts, 1);

    // Client now has v3 (server's version)
    assert_eq!(
        fs::read_to_string(client_dir.path().join("note.txt")).unwrap(),
        "v3-server"
    );
    // Server notes dir still has v3
    assert_eq!(
        fs::read_to_string(server_dir.path().join("note.txt")).unwrap(),
        "v3-server"
    );
    // Server conflicted dir has v2 (client's displaced version)
    let conflicted_note = conflicted_dir.path().join("note.txt");
    assert!(conflicted_note.exists(), "conflicted note.txt should exist");
    assert_eq!(fs::read_to_string(&conflicted_note).unwrap(), "v2-client");
}
