pub mod cli;
pub mod cmd;
pub mod data;
pub mod events;
pub mod portfolio;
pub mod server;
pub mod settings;
pub mod subs;

#[derive(Debug, Clone)]
pub struct App {
    pub settings: settings::Settings,
    pub degiro: degiro_rs::client::Client,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        let settings = settings::Settings::new(None);
        let degiro = degiro_rs::client::ClientBuilder::default()
            .username(&settings.username)
            .password(&settings.password)
            .build()
            .unwrap();
        Self { settings, degiro }
    }
}
