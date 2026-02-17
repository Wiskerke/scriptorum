use clap::Parser;
use std::net::SocketAddr;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "scriptorum-server", about = "Supernote sync server")]
struct Cli {
    /// Address to bind to
    #[arg(short, long, default_value = "0.0.0.0:3742")]
    bind: SocketAddr,

    /// Path to file storage directory
    #[arg(short, long, default_value = "./storage")]
    storage: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            EnvFilter::new("scriptorum_server=info,tower_http=info")
        }))
        .init();

    let cli = Cli::parse();

    let app = scriptorum_server::build_app(&cli.storage)?;

    let listener = tokio::net::TcpListener::bind(cli.bind).await?;
    tracing::info!("listening on {}", cli.bind);
    axum::serve(listener, app).await?;

    Ok(())
}
