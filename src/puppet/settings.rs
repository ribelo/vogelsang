use async_trait::async_trait;
use config::Config;
use directories;
use pptr::prelude::*;
use serde::{Deserialize, Serialize};
use tracing::{error, info};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Asset {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub struct Settings {
    #[serde(default)]
    pub sl_nstd: usize,
    #[serde(default)]
    pub sl_max_percent: f64,
    #[serde(skip)]
    pub file_path: Option<String>,
    #[serde(skip_serializing)]
    pub username: String,
    #[serde(skip_serializing)]
    pub password: String,
    pub assets: Vec<Asset>,
    pub disabled_assets: Option<Vec<Asset>>,
}

impl Settings {
    #[must_use]
    pub async fn new() -> Self {
        let base_dir = directories::BaseDirs::new().expect("Can't get base dirs");
        let config_dir = base_dir
            .config_local_dir()
            .join("vogelsang")
            .to_str()
            .expect("Can't convert path")
            .to_owned();
        if !std::path::Path::new(&format!("{config_dir}/Config.toml")).exists() {
            tokio::fs::create_dir_all(&config_dir)
                .await
                .expect("Can't create config dir");
            let settings = Self::default();
            let toml = toml::to_string_pretty(&settings).unwrap();
            tokio::fs::write(format!("{config_dir}/Config.toml"), toml)
                .await
                .expect("Can't write config");
            info!("Created config at {}", config_dir);
        }
        let settings = Config::builder()
            .set_default(
                "username",
                std::env::var("DEGIRO_LOGIN").unwrap_or_default(),
            )
            .expect("Can't set default username")
            .set_default(
                "password",
                std::env::var("DEGIRO_PASSWORD").unwrap_or_default(),
            )
            .expect("Can't set default password")
            .add_source(config::File::with_name(&format!("{config_dir}/Config")))
            .build()
            .expect("Can't load config");
        let mut settings = settings
            .try_deserialize::<Self>()
            .expect("Can't deserialize config");
        settings.file_path = Some(format!("{config_dir}/Config.toml"));
        settings
    }
}

#[async_trait]
impl Lifecycle for Settings {
    type Supervision = OneToOne;

    async fn reset(&self, _ctx: &Context) -> Result<Self, CriticalError> {
        Ok(Self::new().await)
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
        ctx: &Context,
    ) -> Result<Self::Response, PuppetError> {
        let path = self.file_path.as_ref().unwrap();
        let toml = toml::to_string_pretty(self).unwrap();
        tokio::fs::write(&path, toml).await.map_err(|e| {
            error!("Can't save config: {}", e);
            ctx.critical_error(&e)
        })?;
        info!("Saved config to {}", path);
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ReplaceSettings(pub Settings);

#[async_trait]
impl Handler<ReplaceSettings> for Settings {
    type Response = ();
    type Executor = ConcurrentExecutor;
    async fn handle_message(
        &mut self,
        msg: ReplaceSettings,
        ctx: &Context,
    ) -> Result<Self::Response, PuppetError> {
        *self = msg.0;
        ctx.ask::<Self, _>(SaveSettings).await?;
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
        _ctx: &Context,
    ) -> Result<Self::Response, PuppetError> {
        Ok(self.clone())
    }
}

#[derive(Debug, Clone)]
pub struct AddAsset {
    pub id: String,
    pub name: String,
}

impl From<AddAsset> for Asset {
    fn from(msg: AddAsset) -> Self {
        Self {
            id: msg.id,
            name: msg.name,
        }
    }
}

#[async_trait]
impl Handler<AddAsset> for Settings {
    type Response = ();

    type Executor = SequentialExecutor;

    async fn handle_message(
        &mut self,
        msg: AddAsset,
        ctx: &Context,
    ) -> Result<Self::Response, PuppetError> {
        info!("Adding asset: {:?}", msg);
        self.assets.push(msg.into());
        ctx.send::<Self, _>(SaveSettings).await?;
        Ok(())
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
        ctx: &Context,
    ) -> Result<Self::Response, PuppetError> {
        info!("Removing asset: {:?}", msg.0);
        if let Some(pos) = self.assets.iter().position(|x| x.id == msg.0) {
            let asset = self.assets.remove(pos);
            if let Some(disabled_assets) = &mut self.disabled_assets {
                disabled_assets.push(asset);
            } else {
                self.disabled_assets = Some(vec![asset]);
            }
            ctx.send::<Self, _>(SaveSettings).await?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct GetAssets;

#[async_trait]
impl Handler<GetAssets> for Settings {
    type Response = Vec<Asset>;
    type Executor = SequentialExecutor;
    async fn handle_message(
        &mut self,
        _msg: GetAssets,
        _ctx: &Context,
    ) -> Result<Self::Response, PuppetError> {
        Ok(self.assets.clone())
    }
}
