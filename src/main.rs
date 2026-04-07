mod app;
mod cache;
mod config;
mod data;
mod error;
mod feed;

use config::Config;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let config = Config::from_env();
    let app = app::build_app(&config);

    let addr = format!("0.0.0.0:{}", config.port);
    tracing::info!("OpenRss listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
