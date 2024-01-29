use async_trait::async_trait;
use config::Config;
use master_of_puppets::prelude::*;
use serde::{Deserialize, Serialize};
use tracing::{error, info};

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub struct Settings {
    #[serde(skip)]
    pub file_path: Option<String>,
    pub username: String,
    pub password: String,
    pub assets: Vec<(String, String)>,
    pub disabled_assets: Option<Vec<(String, String)>>,
}

impl Settings {
    #[must_use]
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
        let mut settings = settings
            .try_deserialize::<Self>()
            .expect("Can't deserialize config");
        settings.file_path = Some(path.to_owned());
        settings
    }
}

#[async_trait]
impl Lifecycle for Settings {
    type Supervision = OneToOne;

    async fn reset(&self, _puppeter: &Puppeter) -> Result<Self, CriticalError> {
        Ok(Self::new(self.file_path.as_deref()))
    }
}

#[derive(Debug, Clone)]
pub struct SaveSettings;

#[async_trait]
impl Handler<SaveSettings> for Settings {
    type Response = ();
    type Executor = ConcurrentExecutor;
    async fn handle_message(
        &mut self,
        _msg: SaveSettings,
        _puppeter: &Puppeter,
    ) -> Result<Self::Response, PuppetError> {
        let path = format!("{}.toml", self.file_path.as_ref().unwrap());
        let toml = toml::to_string_pretty(self).unwrap();
        tokio::fs::write(&path, toml).await.map_err(|e| {
            error!("Can't save config: {}", e);
            CriticalError::new(_puppeter.pid, e.to_string())
        })?;
        info!("Saved config to {}", path);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GetSettings;

#[async_trait]
impl Handler<GetSettings> for Settings {
    type Response = Self;

    type Executor = SequentialExecutor;

    async fn handle_message(
        &mut self,
        _msg: GetSettings,
        _puppeter: &Puppeter,
    ) -> Result<Self::Response, PuppetError> {
        Ok(self.clone())
    }
}

#[derive(Debug, Clone)]
pub struct DeleteAsset(pub String);

#[async_trait]
impl Handler<DeleteAsset> for Settings {
    type Response = ();
    type Executor = SequentialExecutor;
    async fn handle_message(
        &mut self,
        msg: DeleteAsset,
        puppeter: &Puppeter,
    ) -> Result<Self::Response, PuppetError> {
        info!("Removing asset: {:?}", msg.0);
        if let Some(pos) = self.assets.iter().position(|x| x.0 == msg.0) {
            let asset = self.assets.remove(pos);
            if let Some(disabled_assets) = &mut self.disabled_assets {
                disabled_assets.push(asset);
            } else {
                self.disabled_assets = Some(vec![asset]);
            }
            puppeter.send::<Self, _>(SaveSettings).await?;
        }
        Ok(())
    }
}
