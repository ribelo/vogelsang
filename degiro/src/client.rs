use std::sync::Arc;

use color_eyre::Result;
use dashmap::DashMap;
use derivative::Derivative;
use erfurt::candle::Candles;
use tokio::sync::Mutex;

use crate::{account::Account, api::product::Product, Period};

#[allow(dead_code)]
#[derive(Clone, Debug, Derivative)]
#[derivative(Default)]
pub struct Paths {
    #[derivative(Default(value = r#""https://trader.degiro.nl/trader/".to_string()"#))]
    pub(crate) referer: String,
    #[derivative(Default(value = r#""login/secure/login".to_string()"#))]
    pub(crate) login_url_path: String,
    #[derivative(Default(value = r#""v5/checkorder".to_string()"#))]
    pub(crate) create_order_path: String,
    #[derivative(Default(value = r#""v4/transactions".to_string()"#))]
    pub(crate) transactions_path: String,
    #[derivative(Default(value = r#""settings/user".to_string()"#))]
    pub(crate) web_user_settings_path: String,
    #[derivative(Default(value = r#""login/secure/config".to_string()"#))]
    pub(crate) account_config_path: String,
    #[derivative(Default(value = r#""document/download/".to_string()"#))]
    pub(crate) base_report_download_uri: String,
    #[derivative(Default(value = r#""https://trader.degiro.nl/".to_string()"#))]
    pub(crate) base_api_url: String,
    #[derivative(Default(value = r#""newsfeed/v2/top_news_preview".to_string()"#))]
    pub(crate) top_news_path: String,
    #[derivative(Default(value = r#""settings/web".to_string()"#))]
    pub(crate) web_settings_path: String,
    #[derivative(Default(value = r#""newsfeed/v2/latest_news".to_string()"#))]
    pub(crate) latests_news_path: String,
    #[derivative(Default(value = r#""v5/account/info/".to_string()"#))]
    pub(crate) account_info_path: String,
    #[derivative(Default(value = r#""v5/stocks".to_string()"#))]
    pub(crate) stocks_search_path: String,
    #[derivative(Default(
        value = r#""https://charting.vwdservices.com/hchart/v1/deGiro/data.js".to_string()"#
    ))]
    pub(crate) price_data_url: String,
    #[derivative(Default(value = r#""trading/secure/logout".to_string()"#))]
    pub(crate) logout_url_path: String,
    #[derivative(Default(value = r#""v5/update/".to_string()"#))]
    pub(crate) generic_data_path: String,
    #[derivative(Default(
        value = r#""https://charting.vwdservices.com/hchart/v1/deGiro/data.js".to_string()"#
    ))]
    pub(crate) chart_data_url: String,
    pub(crate) products_search_url: Option<String>,
    pub(crate) pa_url: Option<String>,
    pub(crate) trading_url: Option<String>,
    pub(crate) reporting_url: Option<String>,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct Client {
    pub(crate) username: String,
    pub(crate) password: String,
    pub(crate) session_id: Option<String>,
    pub(crate) client_id: Option<i32>,
    pub account: Option<Account>,
    pub(crate) paths: Paths,
    pub(crate) http_client: reqwest::Client,
    pub(crate) products_cache: Arc<DashMap<String, Arc<Product>>>,
    pub(crate) quotes_cache: Arc<DashMap<(String, Period, Period), Candles>>,
}

#[derive(Clone, Debug)]
pub struct SharedClient {
    pub inner: Arc<Mutex<Client>>,
}

#[allow(dead_code)]
#[derive(Debug, Default)]
pub struct ClientBuilder {
    username: Option<String>,
    password: Option<String>,
}

impl ClientBuilder {
    pub fn username(&mut self, username: &str) -> &mut Self {
        self.username = Some(username.to_string());

        self
    }

    pub fn password(&mut self, password: &str) -> &mut Self {
        self.password = Some(password.to_string());

        self
    }

    pub fn build(&self) -> Result<SharedClient> {
        let http_client = reqwest::ClientBuilder::new()
            .https_only(true)
            .cookie_store(true)
            .build()?;
        let client = SharedClient::new(
            self.username.as_ref().unwrap().to_string(),
            self.password.as_ref().unwrap().to_string(),
            http_client,
        );
        Ok(client)
    }
}

impl Client {
    pub fn new(username: String, password: String, http_client: reqwest::Client) -> Self {
        Self {
            username,
            password,
            session_id: None,
            client_id: None,
            account: None,
            paths: Paths::default(),
            http_client,
            products_cache: Arc::new(DashMap::new()),
            quotes_cache: Arc::new(DashMap::new()),
        }
    }
}

impl SharedClient {
    pub fn new(username: String, password: String, http_client: reqwest::Client) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Client::new(
                username,
                password,
                http_client,
            ))),
        }
    }
}
