//! Shabdar API — process entry point (docs/PHASE1_PLAN.md).
//!
//! Application code lives in the library (src/lib.rs) so integration tests can
//! reuse it; main only wires the process: logs, config, pool, migrations,
//! state, router, serve.

use std::net::SocketAddr;

use shabdar_api::ratelimit::RateLimiter;
use shabdar_api::{config::Config, db, routes, state::AppState};

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

    let state = AppState {
        pool,
        config: cfg,
        limiter: RateLimiter::new(),
    };
    let app = routes::router(state);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind BIND_ADDR");
    tracing::info!(%addr, "shabdar-api listening");

    // Connect info gives handlers the socket peer, which is the rate-limit key
    // when TRUST_PROXY is off (docs/PHASE5_PLAN.md).
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async {
        // ctrl-c only; add SIGTERM when a deployment actually sends it.
        tokio::signal::ctrl_c().await.expect("ctrl-c handler");
    })
    .await
    .expect("server error");
}
