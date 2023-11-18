pub mod tcp;

use anyhow::Result;
use vogelsang::settings::Settings;
use vogelsang::App;

#[tokio::main]
async fn main() -> Result<()> {
    let settings = Settings::new(None);
    let degiro = degiro_rs::client::ClientBuilder::default()
        .username(&settings.username)
        .password(&settings.password)
        .build()?;
    let app = App { settings, degiro };
    app.run().await
}
