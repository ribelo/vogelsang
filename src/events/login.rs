use eventual::{eve::Eve, event::Event, Event};

use crate::App;

#[derive(Debug, Clone, Event)]
pub struct Login {}

pub async fn login(_event: Event<Login>, eve: Eve<App>) {
    println!("Logging in...");
    eve.state.degiro.login().await.unwrap();
}
