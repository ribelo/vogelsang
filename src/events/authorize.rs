use eventual::{eve::Eve, event::Event, Event};
use tracing::info;

use crate::App;

#[derive(Debug, Clone, Event)]
pub struct Authorize {}

pub async fn authorize(_event: Event<Authorize>, eve: Eve<App>) {
    info!("Authorizing...");
    eve.state.degiro.authorize().await.unwrap();
    info!("Successfully authorized.");
}
