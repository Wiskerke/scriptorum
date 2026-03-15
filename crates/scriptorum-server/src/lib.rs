pub mod api;
pub mod storage;

use api::AppState;
use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post, put};
use axum::Router;
use scriptorum_core::protocol::API_VERSION;
use std::path::Path;
use std::sync::Arc;
use storage::Storage;
use tokio::sync::Mutex;
use tower_http::trace::TraceLayer;

/// Build the Axum router with the given storage and archive directories.
pub fn build_app(storage_dir: &Path, archive_dir: &Path) -> anyhow::Result<Router> {
    let storage = Storage::new(storage_dir.to_path_buf(), archive_dir.to_path_buf())?;
    let state: AppState = Arc::new(Mutex::new(storage));

    let api = Router::new()
        .route("/health", get(api::health))
        .route("/sync/diff", post(api::sync_diff))
        .route("/files/*path", get(api::get_file).put(api::put_file))
        .route("/archive/*path", put(api::put_archive))
        .with_state(state);

    Ok(Router::new()
        .nest(&format!("/api/{API_VERSION}"), api)
        // Axum's default body limit is 2MB; disable it so large .note files can be uploaded
        .layer(DefaultBodyLimit::disable())
        .layer(TraceLayer::new_for_http()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use scriptorum_core::protocol::{Manifest, SyncDiff, API_VERSION};

    fn api(path: &str) -> String {
        format!("/api/{API_VERSION}/{path}")
    }
    use tempfile::TempDir;
    use tower::ServiceExt;

    fn test_app(dir: &std::path::Path) -> Router {
        let archive = dir.join("archive");
        build_app(dir, &archive).unwrap()
    }

    #[tokio::test]
    async fn health_check() {
        let dir = TempDir::new().unwrap();
        let app = test_app(dir.path());

        let resp = app
            .oneshot(
                Request::builder()
                    .uri(api("health"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "ok");
    }

    #[tokio::test]
    async fn put_and_get_file() {
        let dir = TempDir::new().unwrap();
        let app = test_app(dir.path());

        let put_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(api("files/notes/test.txt"))
                    .body(Body::from("hello world"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(put_resp.status(), StatusCode::OK);

        let get_resp = app
            .oneshot(
                Request::builder()
                    .uri(api("files/notes/test.txt"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(get_resp.status(), StatusCode::OK);
        assert!(get_resp.headers().contains_key("X-SHA256"));

        let body = axum::body::to_bytes(get_resp.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(&body[..], b"hello world");
    }

    #[tokio::test]
    async fn put_with_checksum_verification() {
        let dir = TempDir::new().unwrap();
        let app = test_app(dir.path());

        let correct_sha = scriptorum_core::checksum::sha256_bytes(b"test data");

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(api("files/check.txt"))
                    .header("X-SHA256", &correct_sha)
                    .body(Body::from("test data"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let resp = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(api("files/check2.txt"))
                    .header("X-SHA256", "wrong_hash")
                    .body(Body::from("test data"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn sync_diff_empty() {
        let dir = TempDir::new().unwrap();
        let app = test_app(dir.path());

        let manifest = Manifest { files: vec![] };
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(api("sync/diff"))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&manifest).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let diff: SyncDiff = serde_json::from_slice(&body).unwrap();
        assert!(diff.client.to_upload.is_empty());
        assert!(diff.client.to_download.is_empty());
    }

    #[tokio::test]
    async fn sync_diff_detects_server_file() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("existing.txt"), "server file").unwrap();

        let app = test_app(dir.path());

        let manifest = Manifest { files: vec![] };
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(api("sync/diff"))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&manifest).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let diff: SyncDiff = serde_json::from_slice(&body).unwrap();
        assert!(diff.client.to_upload.is_empty());
        assert_eq!(diff.client.to_download.len(), 1);
        assert_eq!(diff.client.to_download[0].path, "existing.txt");
    }

    #[tokio::test]
    async fn get_nonexistent_file_returns_500() {
        let dir = TempDir::new().unwrap();
        let app = test_app(dir.path());

        let resp = app
            .oneshot(
                Request::builder()
                    .uri(api("files/nope.txt"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
