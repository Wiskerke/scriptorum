use crate::checksum::sha256_bytes;
use crate::protocol::SyncDiff;
use crate::scanner::scan_directory;
use anyhow::{Context, Result};
use std::io::Read;
use std::path::Path;
use std::sync::Arc;

/// TLS configuration for mTLS connections (PEM-encoded strings).
pub struct TlsConfig {
    pub ca_cert_pem: String,
    pub client_cert_pem: String,
    pub client_key_pem: String,
}

fn build_tls_config(tls: &TlsConfig) -> Result<rustls::ClientConfig> {
    use rustls::pki_types::PrivateKeyDer;
    use std::io::Cursor;

    // Load CA cert into root store
    let ca_certs: Vec<_> = rustls_pemfile::certs(&mut Cursor::new(&tls.ca_cert_pem))
        .collect::<std::result::Result<_, _>>()
        .context("parsing CA certificates")?;
    let mut root_store = rustls::RootCertStore::empty();
    for cert in ca_certs {
        root_store
            .add(cert)
            .context("adding CA cert to root store")?;
    }

    // Load client cert chain
    let client_certs: Vec<_> =
        rustls_pemfile::certs(&mut Cursor::new(&tls.client_cert_pem))
            .collect::<std::result::Result<_, _>>()
            .context("parsing client certificates")?;

    // Load client private key
    let client_key: PrivateKeyDer =
        rustls_pemfile::private_key(&mut Cursor::new(&tls.client_key_pem))
            .context("parsing client private key")?
            .context("no private key found")?;

    let config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_client_auth_cert(client_certs, client_key)
        .context("building TLS client config with client auth")?;

    Ok(config)
}

/// Summary of a completed sync operation.
#[derive(Debug)]
pub struct SyncResult {
    pub uploaded: usize,
    pub downloaded: usize,
    pub messages: Vec<String>,
}

/// Perform a full sync of `note_dir` against the server at `server_url`.
///
/// 1. Scans the local directory to build a manifest
/// 2. POSTs the manifest to get a SyncDiff
/// 3. Uploads files the server needs
/// 4. Downloads files the client needs
///
/// The optional `on_progress` callback receives status messages.
/// If `tls` is `Some`, the connection uses mTLS with the provided certificates.
pub fn perform_sync<F>(
    server_url: &str,
    note_dir: &Path,
    tls: Option<&TlsConfig>,
    mut on_progress: F,
) -> Result<SyncResult>
where
    F: FnMut(&str),
{
    let mut messages = Vec::new();
    let mut report = |msg: &str| {
        messages.push(msg.to_string());
        on_progress(msg);
    };

    report("Scanning local files...");
    let local_manifest = scan_directory(note_dir)?;
    report(&format!("Found {} local files", local_manifest.files.len()));

    report("Computing sync diff...");
    let mut builder = ureq::AgentBuilder::new().timeout(std::time::Duration::from_secs(30));
    if let Some(tls) = tls {
        let rustls_config = build_tls_config(tls)?;
        builder = builder.tls_config(Arc::new(rustls_config));
    }
    let agent = builder.build();
    let diff: SyncDiff = agent
        .post(&format!("{server_url}/api/v1/sync/diff"))
        .set("Content-Type", "application/json")
        .send_string(&serde_json::to_string(&local_manifest)?)
        .context("POST /sync/diff failed")?
        .into_json()
        .context("parsing SyncDiff response")?;

    report(&format!(
        "To upload: {}, to download: {}",
        diff.to_upload.len(),
        diff.to_download.len()
    ));

    for entry in &diff.to_upload {
        report(&format!("Uploading {}", entry.path));
        let file_path = note_dir.join(&entry.path);
        let data = std::fs::read(&file_path)
            .with_context(|| format!("reading {}", file_path.display()))?;
        let sha = sha256_bytes(&data);
        agent
            .put(&format!("{server_url}/api/v1/files/{}", entry.path))
            .set("X-SHA256", &sha)
            .set("Content-Type", "application/octet-stream")
            .send_bytes(&data)
            .with_context(|| format!("uploading {}", entry.path))?;
    }

    for entry in &diff.to_download {
        report(&format!("Downloading {}", entry.path));
        let file_path = note_dir.join(&entry.path);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let resp = agent
            .get(&format!("{server_url}/api/v1/files/{}", entry.path))
            .call()
            .with_context(|| format!("downloading {}", entry.path))?;
        let mut data = Vec::new();
        resp.into_reader().read_to_end(&mut data)?;
        std::fs::write(&file_path, &data)?;
    }

    report("Sync complete!");

    Ok(SyncResult {
        uploaded: diff.to_upload.len(),
        downloaded: diff.to_download.len(),
        messages,
    })
}
