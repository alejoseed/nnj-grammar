use std::env;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use nnj_grammar::logging::root_logger;
use nnj_grammar::server::{
    build_analyzer, router_with_logger, serve, serve_unrestricted, LocalCatalogMode,
};
use slog::info;
use tokio::net::TcpListener;

const DEFAULT_BIND_ADDR: &str = "127.0.0.1:7878";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // When NNJ_GRAMMAR_LOG_DIR is set, logs are also written there as one
    // file per day (YYYY-MM-DD.log).
    let log_dir = env::var_os("NNJ_GRAMMAR_LOG_DIR").map(PathBuf::from);
    let logger = root_logger(log_dir)?;

    let cwd = env::current_dir().context("failed to read working directory")?;
    let (analyzer, mode) =
        build_analyzer(&cwd).context("failed to initialize the grammar analyzer")?;

    match mode {
        LocalCatalogMode::EmbeddedOnly => {
            info!(logger, "loaded embedded-only grammar catalog");
        }
        LocalCatalogMode::Combined(path) => {
            info!(logger, "loaded combined grammar catalog";
                "local" => %path.display());
        }
    }

    // Setting NNJ_GRAMMAR_BIND is an explicit opt-out of the loopback-only
    // guard, for containers where loopback is unreachable from the host and
    // the port mapping controls exposure instead.
    let bind_override = env::var("NNJ_GRAMMAR_BIND").ok();
    let bind_addr = bind_override.as_deref().unwrap_or(DEFAULT_BIND_ADDR);

    let listener = TcpListener::bind(bind_addr)
        .await
        .with_context(|| format!("failed to bind {bind_addr}"))?;
    info!(logger, "listening"; "addr" => format!("http://{bind_addr}"));

    let app = router_with_logger(Arc::new(analyzer), logger.clone());
    let result = if bind_override.is_some() {
        serve_unrestricted(listener, app).await
    } else {
        serve(listener, app).await
    };
    info!(logger, "shutting down");
    result
}
