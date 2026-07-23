use std::env;
use std::sync::Arc;

use anyhow::Context;
use nnj_grammar::server::{build_analyzer, router, serve, LocalCatalogMode};
use tokio::net::TcpListener;

const BIND_ADDR: &str = "127.0.0.1:7878";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cwd = env::current_dir().context("failed to read working directory")?;
    let (analyzer, mode) =
        build_analyzer(&cwd).context("failed to initialize the grammar analyzer")?;

    match mode {
        LocalCatalogMode::EmbeddedOnly => {
            println!("nnj-grammar-server: loaded embedded-only grammar catalog");
        }
        LocalCatalogMode::Combined(path) => {
            println!(
                "nnj-grammar-server: loaded combined grammar catalog ({})",
                path.display()
            );
        }
    }

    let listener = TcpListener::bind(BIND_ADDR)
        .await
        .with_context(|| format!("failed to bind {BIND_ADDR}"))?;
    println!("nnj-grammar-server: listening on http://{BIND_ADDR}");

    serve(listener, router(Arc::new(analyzer))).await
}
