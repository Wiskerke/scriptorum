use scriptorum_core::client::perform_sync;
use std::fs;
use tempfile::TempDir;

/// Start the server on a random port and return the URL.
async fn start_server(storage_dir: &std::path::Path) -> String {
    let app = scriptorum_server::build_app(storage_dir).unwrap();
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
    let url = start_server(server_dir.path()).await;

    // Create files on the client side
    fs::write(client_dir.path().join("note1.txt"), "hello").unwrap();
    fs::create_dir_all(client_dir.path().join("sub")).unwrap();
    fs::write(client_dir.path().join("sub/note2.txt"), "world").unwrap();

    // Sync: client -> server
    let result = tokio::task::spawn_blocking({
        let url = url.clone();
        let path = client_dir.path().to_path_buf();
        move || perform_sync(&url, &path, |msg| eprintln!("  {msg}"))
    })
    .await
    .unwrap()
    .unwrap();

    assert_eq!(result.uploaded, 2);
    assert_eq!(result.downloaded, 0);

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
    let url = start_server(server_dir.path()).await;

    // Pre-populate server storage
    fs::write(server_dir.path().join("from_server.txt"), "server data").unwrap();
    fs::create_dir_all(server_dir.path().join("deep")).unwrap();
    fs::write(
        server_dir.path().join("deep/nested.txt"),
        "nested data",
    )
    .unwrap();

    // Sync: server -> client
    let result = tokio::task::spawn_blocking({
        let url = url.clone();
        let path = client_dir.path().to_path_buf();
        move || perform_sync(&url, &path, |msg| eprintln!("  {msg}"))
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
    let url = start_server(server_dir.path()).await;

    // Server has one file, client has another
    fs::write(server_dir.path().join("server_note.txt"), "from server").unwrap();
    fs::write(client_dir.path().join("client_note.txt"), "from client").unwrap();

    let result = tokio::task::spawn_blocking({
        let url = url.clone();
        let path = client_dir.path().to_path_buf();
        move || perform_sync(&url, &path, |msg| eprintln!("  {msg}"))
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
    let url = start_server(server_dir.path()).await;

    fs::write(client_dir.path().join("note.txt"), "content").unwrap();

    // First sync: uploads the file
    let result = tokio::task::spawn_blocking({
        let url = url.clone();
        let path = client_dir.path().to_path_buf();
        move || perform_sync(&url, &path, |_| {})
    })
    .await
    .unwrap()
    .unwrap();
    assert_eq!(result.uploaded, 1);

    // Second sync: nothing to do
    let result = tokio::task::spawn_blocking({
        let url = url.clone();
        let path = client_dir.path().to_path_buf();
        move || perform_sync(&url, &path, |_| {})
    })
    .await
    .unwrap()
    .unwrap();
    assert_eq!(result.uploaded, 0);
    assert_eq!(result.downloaded, 0);
}

#[tokio::test]
async fn sync_after_client_modifies_file() {
    let server_dir = TempDir::new().unwrap();
    let client_dir = TempDir::new().unwrap();
    let url = start_server(server_dir.path()).await;

    fs::write(client_dir.path().join("note.txt"), "v1").unwrap();

    // First sync
    let result = tokio::task::spawn_blocking({
        let url = url.clone();
        let path = client_dir.path().to_path_buf();
        move || perform_sync(&url, &path, |_| {})
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
        move || perform_sync(&url, &path, |msg| eprintln!("  {msg}"))
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
