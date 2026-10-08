//! Shabdar API — Task 1 scaffold (docs/PHASE1_PLAN.md).
//!
//! Binds BIND_ADDR (default 127.0.0.1:8080, ARCHITECTURE.md §2) and answers
//! every request with `ok`. Task 5 replaces the fallback with routes::router().

mod config;
mod error;
mod request_id;

use std::net::SocketAddr;

use axum::{middleware, Router};

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

    // Fail fast on missing or invalid env (Config contract in docs/PHASE1_PLAN.md).
    let cfg = config::Config::from_env().unwrap_or_else(|e| {
        eprintln!("configuration error: {e}");
        std::process::exit(2);
    });
    let addr: SocketAddr = cfg.bind_addr.parse().expect("BIND_ADDR must be host:port");

    // Task 1 fallback still answers `ok`; Task 5 swaps in routes::router().
    // The request-id layer wraps every request, fallback included.
    let app = Router::new()
        .fallback(ok)
        .layer(middleware::from_fn(request_id::request_id_middleware));

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
