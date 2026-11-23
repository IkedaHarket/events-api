#![allow(dead_code, unused_imports)]

use events_api::{infrastructure, presentation};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Load environment variables from .env file if available
    let _ = dotenvy::dotenv();

    // 2. Initialize structured logging with Tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,events_api=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // 3. Resolve host and port
    let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    let addr: SocketAddr = format!("{host}:{port}").parse()?;

    tracing::info!(%addr, "Initializing IkedaDev Events API...");

    // 4. Connect to PostgreSQL (Neon / Local)
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://postgres:postgres@localhost:5432/events_db?sslmode=disable".to_string()
    });

    let db_config = infrastructure::DatabaseConfig {
        url: database_url,
        ..Default::default()
    };

    let pool = infrastructure::create_pool(&db_config).await?;
    tracing::info!("PostgreSQL connection pool established with RLS transaction support.");

    let calendar_repo = Arc::new(infrastructure::SqlxCalendarRepository::new(pool.clone()));
    let event_repo = Arc::new(infrastructure::SqlxEventRepository::new(pool));
    let state = presentation::AppState::new(calendar_repo, event_repo);

    // 5. Configure middleware layers (CORS, Request Tracing)
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // 6. Assemble Axum Router
    let app = presentation::create_router(state)
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    // 7. Bind TCP listener and serve with graceful shutdown
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!(%addr, "Server successfully listening. Swagger UI available at http://{addr}/swagger-ui");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("Server shut down gracefully.");
    Ok(())
}

/// Listens for OS shutdown signals (Ctrl+C / SIGINT).
async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(err) = tokio::signal::ctrl_c().await {
            tracing::error!(error = %err, "Failed to install Ctrl+C signal handler");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(err) => {
                tracing::error!(error = %err, "Failed to install SIGTERM signal handler");
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}
