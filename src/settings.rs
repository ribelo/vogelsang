use config::Config;
use degiro_rs::{money::Money, util::Period};
use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Settings {
    pub username: String,
    pub password: String,
    pub data_path: String,
    pub assets: Vec<(String, String)>,
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
        settings
            .try_deserialize::<Settings>()
            .expect("Can't deserialize config")
    }
}
