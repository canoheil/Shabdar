//! Shabdar API — Task 1 scaffold (docs/PHASE1_PLAN.md).
//!
//! Binds BIND_ADDR (default 127.0.0.1:8080, ARCHITECTURE.md §2) and answers
//! every request with `ok`. Task 5 replaces the fallback with routes::router().

use std::net::SocketAddr;

use axum::Router;

/// Placeholder handler: the literal string `ok`.
async fn ok() -> &'static str {
    "ok"
}

#[tokio::main]
async fn main() {
    // Log filter from RUST_LOG; default level is info. architecture.md §5: tracing
    // carries the dev OTP print in later tasks.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".into());
    let addr: SocketAddr = bind_addr.parse().expect("BIND_ADDR must be host:port");

    // Task 1: one fallback route answers `ok` for every path, including GET /.
    let app = Router::new().fallback(ok);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind BIND_ADDR");
    tracing::info!(%addr, "shabdar-api listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            // ctrl-c only; add SIGTERM when a deployment actually sends it.
            tokio::signal::ctrl_c().await.expect("ctrl-c handler");
        })
        .await
        .expect("server error");
}
