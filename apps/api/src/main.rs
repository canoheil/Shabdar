//! Shabdar API — process entry point (docs/PHASE1_PLAN.md).
//!
//! Application code lives in the library (src/lib.rs) so integration tests can
//! reuse it; main only wires the process: logs, config, pool, migrations,
//! bind, serve. Task 5 replaces the fallback with routes::router().

use std::net::SocketAddr;

use axum::{middleware, Router};
use shabdar_api::{config::Config, db, request_id};

/// Placeholder handler: the literal string `ok`.
async fn ok() -> &'static str {
    "ok"
}

#[tokio::main]
async fn main() {
    // Log filter from RUST_LOG; default level is info. ARCHITECTURE.md §5:
    // tracing carries the dev OTP print in later tasks.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    // Fail fast on missing or invalid env (Config contract in the plan).
    let cfg = Config::from_env().unwrap_or_else(|e| {
        eprintln!("configuration error: {e}");
        std::process::exit(2);
    });

    // The pool connects lazily; migrations are the first real database contact.
    // Startup fails here when the database is unreachable or a migration errors.
    let pool = db::create_pool(&cfg);
    db::run_migrations(&pool)
        .await
        .expect("database migrations failed");

    let addr: SocketAddr = cfg.bind_addr.parse().expect("BIND_ADDR must be host:port");

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
