#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_sign_loss)]

use anyhow::Result;
use tracing::info;

pub mod cli;
pub mod portfolio;
pub mod puppet;
pub mod server;

use crate::cli::CliExt;

#[derive(Debug, Clone)]
pub struct App;

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    #[must_use]
    pub const fn new() -> Self {
        Self {}
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().pretty().init();
    info!("Starting Vogelsang...");

    let app = App::new();
    app.run().await.unwrap();
    Ok(())
}
