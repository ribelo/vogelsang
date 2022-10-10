use config::Config;
use degiro::{money::Money, Period};
use serde::Deserialize;

#[derive(Debug, Default, Deserialize, PartialEq)]
pub struct Settings {
    pub username: String,
    pub password: String,
    pub risk_free: f64,
    pub risk: f64,
    pub max_stock: u32,
    pub money: Money,
    pub period: Period,
    pub interval: Period,
    pub stocks: Vec<(String, String, String)>,
}

impl Settings {
    pub fn new(path: Option<&str>) -> Self {
        let path = path.unwrap_or("Config");
        let settings = Config::builder()
            .add_source(config::File::with_name(path))
            .add_source(
                config::Environment::with_prefix("VOG")
                    .try_parsing(true)
                    .separator("_")
                    .list_separator(" "),
            )
            .build()
            .expect("Can't load config");
        settings.try_deserialize::<Settings>().expect("Can't deserialize config")
    }
}
