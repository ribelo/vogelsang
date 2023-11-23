use anyhow::Result;
use eventual::eve::{Eve, EveBuilder};
use eventual::event_handler::{self, State};
use eventual::Event;
use tokio::signal;
use tracing::info;
use vogelsang::events::authorize::Authorize;
use vogelsang::events::calculate_portfolio::CalculatePorftolio;
use vogelsang::events::fetch_data::FetchData;
use vogelsang::events::get_candles::GetCandles;
use vogelsang::events::get_product::GetProduct;
use vogelsang::events::login::Login;
use vogelsang::events::single_allocation::GetSingleAllocation;
use vogelsang::settings::Settings;
use vogelsang::subs;
use vogelsang::{cli::CliExt, events, App};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().pretty().init();
    info!("Starting Vogelsang...");

    let settings = Settings::new(None);
    let degiro = degiro_rs::client::ClientBuilder::default()
        .username(&settings.username)
        .password(&settings.password)
        .build()?;
    let app = App { settings, degiro };
    let eve = EveBuilder::new(app)
        .reg_handler::<Login, _, _>(events::login::login)?
        .reg_handler::<Authorize, _, _>(events::authorize::authorize)?
        .reg_handler::<FetchData, _, _>(events::fetch_data::fetch_data)?
        .reg_handler::<GetProduct, _, _>(events::get_product::get_product)?
        .reg_handler::<GetCandles, _, _>(events::get_candles::get_candles)?
        .reg_handler::<GetSingleAllocation, _, _>(events::single_allocation::get_single_allocation)?
        .reg_handler::<CalculatePorftolio, _, _>(events::calculate_portfolio::calculate_portfolio)?
        .reg_memo(subs::products::products)
        .build()?;
    eve.run().await.unwrap();
    Ok(())
}
