use crate::api::portfolio::Portfolio;
use crate::{account::Account, api::product::Product, Period};
use color_eyre::{Report, Result};
use dashmap::DashMap;
use derivative::Derivative;
use erfurt::candle::Candles;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::{oneshot, RwLock, Mutex};
use tokio::task::JoinHandle;

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

#[derive(Debug)]
pub enum ClientMsg {
    Login,
    GetAccountInfo,
    GetAccountData {
        tx: Option<oneshot::Sender<Result<Account>>>,
    },
    GetPortfolio {
        tx: Option<oneshot::Sender<Result<Portfolio>>>,
    },
    GetProduct {
        id: String,
        tx: Option<oneshot::Sender<Result<Arc<Product>>>>,
    },
    GetCandles {
        id: String,
        period: Period,
        interval: Period,
        tx: Option<oneshot::Sender<Result<Arc<Candles>>>>,
    },
}

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("Unauthorized")]
    Unauthorized,
    #[error("No account info")]
    NoAccountInfo,
    #[error("No account config")]
    NoAccountConfig,
    #[error("No account data")]
    NoAccountData,
    #[error(transparent)]
    Unknown(#[from] Report),
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct ClientInner {
    pub(crate) username: String,
    pub(crate) password: String,
    pub(crate) session_id: Arc<RwLock<Option<String>>>,
    pub(crate) client_id: Arc<RwLock<Option<i32>>>,
    pub account: Arc<RwLock<Option<Account>>>,
    pub portfolio: Arc<RwLock<Option<Portfolio>>>,
    pub(crate) paths: Arc<RwLock<Paths>>,
    pub(crate) http_client: Arc<reqwest::Client>,
    pub(crate) products_cache: Arc<DashMap<String, Arc<Product>>>,
    pub(crate) quotes_cache: Arc<DashMap<(String, Period, Period), Arc<Candles>>>,
    pub(crate) tx: Sender<ClientMsg>,
}

#[derive(Clone, Debug)]
pub struct Client {
    pub(crate) inner: Arc<Mutex<ClientInner>>,
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

    pub fn build(&self) -> Result<ClientInner> {
        let http_client = reqwest::ClientBuilder::new()
            .https_only(true)
            .cookie_store(true)
            .build()?;
        let client = ClientInner::new(
            self.username.as_ref().unwrap().to_string(),
            self.password.as_ref().unwrap().to_string(),
            http_client,
        );
        Ok(client)
    }
}

impl ClientInner {
    pub fn new(username: String, password: String, http_client: reqwest::Client) -> Self {
        let (tx, rx) = channel(1024);
        let mut client = Self {
            username,
            password,
            session_id: None,
            client_id: None,
            account: Arc::new(RwLock::new(None)),
            portfolio: Arc::new(RwLock::new(None)),
            paths: Paths::default(),
            http_client,
            products_cache: Arc::new(DashMap::new()),
            quotes_cache: Arc::new(DashMap::new()),
            tx,
        };
        // let handler = client.msg_handler(rx);
        // tokio::spawn(async {
        //     while let Ok(x) = shutdown_rx.await {
        //         drop(rx);
        //         drop(shutdown_tx);
        //     }
        // });
        client
    }

    fn msg_handler(&mut self, mut rx: Receiver<ClientMsg>) -> JoinHandle<()> {
        let mut client = self.clone();
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                use ClientMsg::*;
                match msg {
                    Login => {
                        if let Err(err) = client.login().await {
                            log::error!("{}", err);
                        }
                    }
                    GetCandles {
                        id,
                        period,
                        interval,
                        tx,
                    } => {
                        if let Some(quotes) =
                            client.quotes_cache.get(&(id, period, interval)).as_deref()
                        {
                            if let Some(tx) = tx {
                                tx.send(Ok(quotes.clone()));
                            };
                        } else {
                            let quotes = client.quotes(&id, &period, &interval).await;
                            if let Some(tx) = tx {
                                tx.send(quotes);
                            };
                        }
                    }
                    GetProduct { id, tx } => {
                        if let Some(product) = client.products_cache.get(&id).as_deref() {
                            if let Some(tx) = tx {
                                tx.send(Ok(product.clone()));
                            };
                        } else {
                            let product = client.product_by_id(&id).await;
                            if let Some(tx) = tx {
                                tx.send(product);
                            };
                        }
                    }
                    _ => {
                        unimplemented!()
                    }
                };
            }
        })
    }
}
