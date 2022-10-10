use axum::{http::header::HeaderMap, routing::get, Router};
use color_eyre::{eyre::eyre, Result};
use std::net::SocketAddr;
use vogelsang::cmd;
use vogelsang::settings::Settings;

#[tokio::main]
async fn main() -> Result<()> {
    let settings = Settings::new(None);
    cmd::run(settings).await
}
