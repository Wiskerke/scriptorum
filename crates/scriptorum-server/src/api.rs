use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use scriptorum_core::checksum::sha256_bytes;
use scriptorum_core::protocol::{Manifest, SyncDiff};
use scriptorum_core::sync::compute_diff;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::storage::Storage;

pub type AppState = Arc<Mutex<Storage>>;

pub async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
}

pub async fn sync_diff(
    State(storage): State<AppState>,
    Json(client_manifest): Json<Manifest>,
) -> Result<Json<SyncDiff>, AppError> {
    let storage = storage.lock().await;
    let server_manifest = storage.manifest()?;
    let diff = compute_diff(&client_manifest, &server_manifest);
    Ok(Json(diff))
}

pub async fn get_file(
    State(storage): State<AppState>,
    Path(path): Path<String>,
) -> Result<Response, AppError> {
    let storage = storage.lock().await;
    let data = storage.read_file(&path)?;
    let sha256 = sha256_bytes(&data);

    Ok(Response::builder()
        .header("X-SHA256", sha256)
        .header("content-type", "application/octet-stream")
        .body(Body::from(data))
        .unwrap())
}

pub async fn put_file(
    State(storage): State<AppState>,
    Path(path): Path<String>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<impl IntoResponse, AppError> {
    let expected_sha256 = headers
        .get("X-SHA256")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let actual_sha256 = sha256_bytes(&body);

    if let Some(expected) = &expected_sha256 {
        if *expected != actual_sha256 {
            return Err(AppError::ChecksumMismatch {
                expected: expected.clone(),
                actual: actual_sha256,
            });
        }
    }

    let storage = storage.lock().await;
    storage.write_file(&path, &body)?;

    Ok((StatusCode::OK, Json(serde_json::json!({"sha256": actual_sha256}))))
}

// Error handling

pub enum AppError {
    Internal(anyhow::Error),
    ChecksumMismatch { expected: String, actual: String },
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError::Internal(err)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::Internal(err) => {
                let msg = format!("{err:#}");
                tracing::error!(%msg, "internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": msg})),
                )
                    .into_response()
            }
            AppError::ChecksumMismatch { expected, actual } => (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "checksum mismatch",
                    "expected": expected,
                    "actual": actual,
                })),
            )
                .into_response(),
        }
    }
}
