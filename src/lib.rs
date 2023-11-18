pub mod cli;
pub mod cmd;
pub mod data;
pub mod portfolio;
pub mod prelude;
pub mod settings;

pub struct App<T: degiro_rs::client::client_status::Status> {
    pub settings: settings::Settings,
    pub degiro: degiro_rs::client::Client<T>,
}
