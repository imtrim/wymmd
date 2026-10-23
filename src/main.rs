mod app;
mod css;
mod http;
mod js;
mod metrics;
mod minifier;
mod trace_writer;

use crate::{
    app::{app, parse_max_body_size},
    trace_writer::StdioTracingWriter,
};
use tokio::signal::unix::{SignalKind, signal};
use tracing::info;
use tracing_subscriber::{EnvFilter, prelude::*};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_flat_json::FlatJsonLayer::new(StdioTracingWriter))
        .init();

    let port = std::env::var("PORT").ok().unwrap_or("3000".to_string());
    let raw_max_body_size = std::env::var("MAX_BODY_SIZE").ok();
    let max_body_size = parse_max_body_size(raw_max_body_size.as_deref());
    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    info!(port, max_body_size, "app starting");
    axum::serve(listener, app(max_body_size))
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

async fn shutdown_signal() {
    let mut sigint = signal(SignalKind::interrupt()).unwrap();
    let mut sigterm = signal(SignalKind::terminate()).unwrap();
    tokio::select! {
        _ = sigint.recv() => info!("SIGINT recv"),
        _ = sigterm.recv() => info!("SIGTERM recv"),
    }
}
