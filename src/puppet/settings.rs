use async_trait::async_trait;
use config::Config as Cfg;
use directories;
use pptr::prelude::*;
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use crate::portfolio::RiskMode;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Asset {
    pub id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub struct Config {
    #[serde(default)]
    pub sl_nstd: usize,
    #[serde(default)]
    pub sl_max_dd: f64,
    #[serde(default)]
    pub risk_mode: RiskMode,
    #[serde(default)]
    pub risk: f64,
    #[serde(default)]
    pub risk_free: f64,
    #[serde(skip)]
    pub file_path: Option<String>,
    #[serde(skip_serializing)]
    pub username: String,
    #[serde(skip_serializing)]
    pub password: String,
    pub assets: Vec<Asset>,
    pub disabled_assets: Option<Vec<Asset>>,
}

impl Config {
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
            let inner = Config::default();
            let toml = toml::to_string_pretty(&inner).unwrap();
            tokio::fs::write(format!("{config_dir}/Config.toml"), toml)
                .await
                .expect("Can't write config");
            info!("Created config at {}", config_dir);
        }
        let cfg = Cfg::builder()
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
        let mut config = cfg
            .try_deserialize::<Config>()
            .expect("Can't deserialize config");
        config.file_path = Some(format!("{config_dir}/Config.toml"));

        config
    }
}

#[derive(Debug, Clone, Default)]
pub struct Settings;

#[async_trait]
impl Lifecycle for Settings {
    type Supervision = OneToOne;

    async fn reset(&self, _ctx: &Context) -> Result<Self, CriticalError> {
        Ok(Self)
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
        if let Some(config) = ctx.get_resource::<Config>() {
            let path = config.file_path.as_ref().unwrap().clone();
            let toml = toml::to_string_pretty(&config).unwrap();
            tokio::fs::write(&path, toml).await.map_err(|e| {
                error!("Can't save config: {}", e);
                ctx.critical_error(&e)
            })?;
            info!("Saved config to {}", path);
        };
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
pub struct GetAssets;

#[async_trait]
impl Handler<GetAssets> for Settings {
    type Response = Vec<Asset>;
    type Executor = SequentialExecutor;
    async fn handle_message(
        &mut self,
        msg: GetAssets,
        ctx: &Context,
    ) -> Result<Self::Response, PuppetError> {
        Ok(ctx.expect_resource::<Config>().assets)
    }
}

#[derive(Debug, Clone)]
pub struct AddAsset {
    pub id: String,
}

impl From<AddAsset> for Asset {
    fn from(msg: AddAsset) -> Self {
        Self { id: msg.id }
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
        ctx.with_resource_mut(|config: &mut Config| {
            config.assets.push(msg.into());
        })
        .unwrap();
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

        let removal_info = ctx
            .with_resource(|config: &Config| {
                config
                    .assets
                    .iter()
                    .enumerate()
                    .find_map(|(pos, asset)| (asset.id == msg.0).then(|| (pos, asset.clone())))
            })
            .flatten();
        let changed = removal_info.map_or(false, |(pos, asset)| {
            ctx.with_resource_mut(|config: &mut Config| {
                config
                    .disabled_assets
                    .get_or_insert_with(Vec::new)
                    .push(asset);
                config.assets.remove(pos);
                true
            })
            .unwrap_or_default()
        });
        if changed {
            ctx.send::<Self, _>(SaveSettings).await?;
        }
        Ok(())
    }
}
