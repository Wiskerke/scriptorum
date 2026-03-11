pub mod api;
pub mod storage;

use api::AppState;
use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};
use axum::Router;
use tower_http::trace::TraceLayer;
use std::path::Path;
use std::sync::Arc;
use storage::Storage;
use tokio::sync::Mutex;

/// Build the Axum router with the given storage directory.
pub fn build_app(storage_dir: &Path) -> anyhow::Result<Router> {
    let storage = Storage::new(storage_dir.to_path_buf())?;
    let state: AppState = Arc::new(Mutex::new(storage));

    Ok(Router::new()
        .route("/api/v1/health", get(api::health))
        .route("/api/v1/sync/diff", post(api::sync_diff))
        .route("/api/v1/files/*path", get(api::get_file).put(api::put_file))
        // Axum's default body limit is 2MB; disable it so large .note files can be uploaded
        .layer(DefaultBodyLimit::disable())
        .layer(TraceLayer::new_for_http())
        .with_state(state))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use scriptorum_core::protocol::{Manifest, SyncDiff};
    use tempfile::TempDir;
    use tower::ServiceExt;

    fn test_app(dir: &std::path::Path) -> Router {
        build_app(dir).unwrap()
    }

    #[tokio::test]
    async fn health_check() {
        let dir = TempDir::new().unwrap();
        let app = test_app(dir.path());

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/health")
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
                    .uri("/api/v1/files/notes/test.txt")
                    .body(Body::from("hello world"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(put_resp.status(), StatusCode::OK);

        let get_resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/files/notes/test.txt")
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
                    .uri("/api/v1/files/check.txt")
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
                    .uri("/api/v1/files/check2.txt")
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
                    .uri("/api/v1/sync/diff")
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
        assert!(diff.to_upload.is_empty());
        assert!(diff.to_download.is_empty());
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
                    .uri("/api/v1/sync/diff")
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
        assert!(diff.to_upload.is_empty());
        assert_eq!(diff.to_download.len(), 1);
        assert_eq!(diff.to_download[0].path, "existing.txt");
    }

    #[tokio::test]
    async fn get_nonexistent_file_returns_500() {
        let dir = TempDir::new().unwrap();
        let app = test_app(dir.path());

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/files/nope.txt")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
