use std::sync::Arc;

use async_trait::async_trait;
use chrono::NaiveDate;
use degiro_rs::{
    api::{
        orders::{
            CreateOrderRequestBuilder, DeleteOrderRequestBuilder, ModifyOrderRequest,
            ModifyOrderRequestBuilder, Order, Orders,
        },
        portfolio::Portfolio,
        search::{QueryProduct, QueryProductDetails},
        transactions::Transactions,
    },
    client::{Client, ClientBuilder, ClientError, ClientStatus},
    util::{OrderTimeType, OrderType, Period, TransactionType},
};
use pptr::prelude::*;
use reqwest::cookie::CookieStore;
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};

use crate::puppet::{
    db::{Db, DeleteData},
    settings::{Asset, Config, DeleteAsset},
};

use super::settings::Settings;

#[derive(Debug, Clone)]
pub struct Degiro {
    pub username: String,
    pub password: String,
    pub client: Client,
    pub is_authorizing: (bool, Arc<tokio::sync::Notify>),
}

impl Degiro {
    pub fn new<U: AsRef<str>, P: AsRef<str>>(
        username: U,
        password: P,
    ) -> Result<Self, reqwest::Error> {
        let secrets = {
            let base_dir = directories::BaseDirs::new().expect("Can't get base dirs");
            let config_dir = base_dir
                .data_local_dir()
                .join("vogelsang")
                .to_str()
                .expect("Can't convert path")
                .to_owned();
            let path = config_dir + "/secrets.json";
            std::fs::read_to_string(path)
                .map(|s| serde_json::from_str::<Secrets>(&s).expect("Can't deserialize secrets"))
        };

        let mut client_builder = ClientBuilder::default()
            .username(username.as_ref())
            .password(password.as_ref());

        let client = match secrets {
            Ok(secrets) => {
                let cursor = std::io::Cursor::new(secrets.cookies_json);
                client_builder.cookie_jar =
                    Some(Arc::new(reqwest_cookie_store::CookieStoreMutex::new(
                        reqwest_cookie_store::CookieStore::load_json(cursor).unwrap(),
                    )));
                let client = client_builder.build().unwrap();
                {
                    let mut inner = client.inner.lock().unwrap();
                    inner.session_id = secrets.session_id;
                    inner.status = ClientStatus::Restricted;
                }
                client
            }
            Err(_) => client_builder.build().unwrap(),
        };

        Ok(Self {
            username: username.as_ref().to_owned(),
            password: password.as_ref().to_owned(),
            client,
            is_authorizing: Default::default(),
        })
    }
}

#[async_trait]
impl Lifecycle for Degiro {
    type Supervision = OneToOne;

    async fn reset(&self, ctx: &Context) -> Result<Self, CriticalError> {
        Self::new(&self.username, &self.password).map_err(|e| {
            error!("Failed to reset handler: {}", e);
            CriticalError {
                puppet: ctx.pid,
                message: e.to_string(),
            }
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Initialize;

#[async_trait]
impl Handler<Initialize> for Degiro {
    type Response = ();

    type Executor = ConcurrentExecutor;

    async fn handle_message(
        &mut self,
        _msg: Initialize,
        ctx: &Context,
    ) -> Result<Self::Response, PuppetError> {
        if self.client.inner.lock().unwrap().session_id.is_empty() {
            let cloned_ctx = ctx.clone();
            tokio::spawn(async move {
                cloned_ctx.ask::<Self, _>(Authorize).await.unwrap();
                info!("Handler initialized");
            });
            Ok(())
        } else if let Err(e) = ctx.ask::<Degiro, _>(GetAccountConfig).await? {
            error!(error = %e, "Failed to fetch account config");
            match e {
                ClientError::Unauthorized => Ok(ctx.send::<Self, _>(Authorize).await?),
                e => return Err(ctx.critical_error(&e)),
            }
        } else {
            info!("Fetched account config");
            info!("Handler initialized");
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Authorize;

#[async_trait]
impl Handler<Authorize> for Degiro {
    type Response = ();

    type Executor = ConcurrentExecutor;

    async fn handle_message(
        &mut self,
        _msg: Authorize,
        ctx: &Context,
    ) -> Result<Self::Response, PuppetError> {
        if self.is_authorizing.0 {
            warn!("Already authorizing, waiting for previous authorization to finish...");
            self.is_authorizing.1.notified().await;
            return Ok(());
        }

        info!("Authorizing...");
        self.is_authorizing.0 = true;
        self.client.authorize().await.map_err(|e| {
            error!("Failed to authorize: {}", e);
            self.is_authorizing.0 = false;
            self.is_authorizing.1.notify_waiters();
            ctx.critical_error(&e)
        })?;

        self.is_authorizing.0 = false;
        self.is_authorizing.1.notify_waiters();
        ctx.ask::<Degiro, _>(StoreSecrets).await.unwrap();
        info!("Successfully authorized.");
        Ok(())
    }
}

#[derive(Debug)]
pub struct GetAccountConfig;

#[async_trait]
impl Handler<GetAccountConfig> for Degiro {
    type Response = Result<(), ClientError>;
    type Executor = ConcurrentExecutor;
    async fn handle_message(
        &mut self,
        _msg: GetAccountConfig,
        _ctx: &Context,
    ) -> Result<Self::Response, PuppetError> {
        info!("Fetching account config...");
        Ok(self.client.account_config().await)
    }
}

#[derive(Clone, Debug)]
pub struct FetchData {
    pub id: Option<String>,
}

#[async_trait]
impl Handler<FetchData> for Degiro {
    type Response = ();

    type Executor = ConcurrentExecutor;

    async fn handle_message(
        &mut self,
        msg: FetchData,
        ctx: &Context,
    ) -> Result<Self::Response, PuppetError> {
        if let Some(id) = &msg.id {
            info!(id = %id, "Fetching data for asset");
            let mut isin = String::new();

            match self.client.product(id).await {
                Ok(product) => {
                    isin = product.inner.isin.clone();
                    ctx.ask::<Db, _>(product.inner.clone()).await.map_err(|e| {
                        error!(error = %e, id = %id, "Failed to send 'put product'");
                        ctx.critical_error(&e)
                    })?;
                }
                Err(_e @ ClientError::Unauthorized) => {
                    warn!(id = %id, "Handler unauthorized, attempting authorization...");
                    ctx.ask::<Self, _>(Authorize).await.map_err(|e| {
                        error!(error = %e, "Failed to authorize");
                        ctx.critical_error(&e)
                    })?;
                    return ctx.ask::<Self, _>(msg.clone()).await.map_err(|e| {
                        error!(error = %e, id = %id, "Failed to resend message");
                        ctx.critical_error(&e)
                    });
                }
                Err(e) => {
                    error!(error = %e, id = %id, "Failed to fetch product data");
                }
            };

            match self.client.quotes(id, Period::P50Y, Period::P1M).await {
                Ok(quotes) => {
                    info!(id = %id, "Fetched {} candles", quotes.time.len());
                    ctx.ask::<Db, _>(quotes).await.map_err(|e| {
                        error!(error = %e, id = %id, "Failed to send 'put candles'");
                        ctx.critical_error(&e)
                    })?;
                }
                Err(e) => {
                    error!(error = %e, id = %id, "Failed to fetch quotes");
                    warn!(id = %id, "Removing asset from settings and database");
                    ctx.ask::<Settings, _>(DeleteAsset(id.clone()))
                        .await
                        .map_err(|e| {
                            error!(error = %e, id = %id, "Failed to remove asset from settings");
                            ctx.critical_error(&e)
                        })?;
                    ctx.ask::<Db, _>(DeleteData(id.clone()))
                        .await
                        .map_err(|e| {
                            error!(error = %e, id = %id, "Failed to delete asset from database");
                            ctx.critical_error(&e)
                        })?;
                }
            }

            match self.client.financial_statements(id, &isin).await {
                Ok(financial_reports) => {
                    ctx.ask::<Db, _>(financial_reports).await.map_err(|e| {
                        error!(error = %e, id = %id, "Failed to send 'put financial reports'");
                        ctx.critical_error(&e)
                    })?;
                }
                Err(e) => {
                    error!(error = %e, id = %id, "Failed to fetch financial reports");
                    warn!(id = %id, "Removing asset from settings and database");
                    // ctx.ask::<Settings, _>(DeleteAsset(id.clone())).await.map_err(|e| {
                    //     error!(error = %e, id = %id, asset_name = %asset_name, "Failed to remove asset from settings");
                    //     ctx.critical_error(&e)
                    // })?;
                    // ctx.ask::<Db, _>(DeleteData(id.clone())).await.map_err(|e| {
                    //     error!(error = %e, id = %id, asset_name = %asset_name, "Failed to delete asset from database");
                    //     ctx.critical_error(&e)
                    // })?;
                }
            }

            match self.client.company_ratios(id, &isin).await {
                Ok(company_ratios) => {
                    let () = Box::pin(ctx.send::<Db, _>(company_ratios))
                        .await
                        .map_err(|e| {
                            error!(error = %e, id = %id, "Failed to send 'put company ratios'");
                            ctx.critical_error(&e)
                        })?;
                }
                Err(e) => {
                    error!(error = %e, id = %id, "Failed to fetch company ratios");
                    warn!(id = %id, "Removing asset from settings and database");
                    // ctx.ask::<Settings, _>(DeleteAsset(id.clone())).await.map_err(|e| {
                    //     error!(error = %e, id = %id, asset_name = %asset_name, "Failed to remove asset from settings");
                    //     ctx.critical_error(&e)
                    // })?;
                    // ctx.ask::<Db, _>(DeleteData(id.clone())).await.map_err(|e| {
                    //     error!(error = %e, id = %id, asset_name = %asset_name, "Failed to delete asset from database");
                    //     ctx.critical_error(&e)
                    // })?;
                }
            }
            info!(id = %id, "Finished fetching data for");
        } else {
            info!("Fetching data for all assets");
            ctx.ask::<Self, _>(Authorize).await.map_err(|e| {
                error!(error = %e, "Failed to authorize");
                ctx.critical_error(&e)
            })?;
            let config = ctx.expect_resource::<Config>();
            for Asset { id } in &config.assets {
                let msg = FetchData {
                    id: Some(id.to_string()),
                };
                ctx.ask::<Self, _>(msg).await.map_err(|e| {
                    error!(error = %e, id = %id, "Failed to resend message");
                    ctx.critical_error(&e)
                })?;
            }
            info!("Finished fetching data for all assets");
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct GetPortfolio;

#[async_trait]
impl Handler<GetPortfolio> for Degiro {
    type Response = Portfolio;

    type Executor = ConcurrentExecutor;

    async fn handle_message(
        &mut self,
        msg: GetPortfolio,
        ctx: &Context,
    ) -> Result<Self::Response, PuppetError> {
        info!("Fetching portfolio...");
        match self.client.portfolio().await {
            Ok(portfolio) => Ok(portfolio),
            Err(ClientError::Unauthorized) => {
                warn!("Handler unauthorized, attempting authorization...");
                ctx.ask::<Self, _>(Authorize).await.map_err(|e| {
                    error!(error = %e, "Failed to authorize");
                    ctx.critical_error(&e)
                })?;
                ctx.ask::<Self, _>(msg.clone()).await.map_err(|e| {
                    error!(error = %e, "Failed to resend message");
                    ctx.critical_error(&e)
                })
            }
            Err(e) => {
                error!(error = %e, "Failed to fetch portfolio: {}", e);
                Err(ctx.critical_error(&e))
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct GetTransactions {
    pub from_date: NaiveDate,
    pub to_date: NaiveDate,
}

#[async_trait]
impl Handler<GetTransactions> for Degiro {
    type Response = Transactions;

    type Executor = ConcurrentExecutor;

    async fn handle_message(
        &mut self,
        msg: GetTransactions,
        ctx: &Context,
    ) -> Result<Self::Response, PuppetError> {
        info!("Fetching transactions...");
        match self.client.transactions(msg.from_date, msg.to_date).await {
            Ok(transactions) => Ok(transactions),
            Err(ClientError::Unauthorized) => {
                warn!("Handler unauthorized, attempting authorization...");
                ctx.ask::<Self, _>(Authorize).await.map_err(|e| {
                    error!(error = %e, "Failed to authorize");
                    ctx.critical_error(&e)
                })?;
                ctx.ask::<Self, _>(msg.clone()).await.map_err(|e| {
                    error!(error = %e, "Failed to resend message");
                    ctx.critical_error(&e)
                })
            }
            Err(e) => {
                error!(error = %e, "Failed to fetch transactions: {}", e);
                Err(ctx.critical_error(&e))
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GetOrders;

#[async_trait]
impl Handler<GetOrders> for Degiro {
    type Response = Orders;

    type Executor = ConcurrentExecutor;

    async fn handle_message(
        &mut self,
        msg: GetOrders,
        ctx: &Context,
    ) -> Result<Self::Response, PuppetError> {
        info!("Fetching GetOrders...");
        match self.client.orders().await {
            Ok(orders) => Ok(orders),
            Err(ClientError::Unauthorized) => {
                warn!("Handler unauthorized, attempting authorization...");
                ctx.ask::<Self, _>(Authorize).await.map_err(|e| {
                    error!(error = %e, "Failed to authorize");
                    ctx.critical_error(&e)
                })?;
                ctx.ask::<Self, _>(msg).await.map_err(|e| {
                    error!(error = %e, "Failed to resend message");
                    ctx.critical_error(&e)
                })
            }
            Err(e) => {
                error!(error = %e, "Failed to fetch transactions: {}", e);
                Err(ctx.critical_error(&e))
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct SearchInstruments {
    pub query: String,
}

#[async_trait]
impl Handler<SearchInstruments> for Degiro {
    type Response = Vec<QueryProductDetails>;

    type Executor = ConcurrentExecutor;

    async fn handle_message(
        &mut self,
        msg: SearchInstruments,
        ctx: &Context,
    ) -> Result<Self::Response, PuppetError> {
        info!("Searching instruments...");
        let res = self
            .client
            .search()
            .query(&msg.query)
            .limit(32)
            .send()
            .await;
        match res {
            Ok(products) => {
                info!("Found {} products", products.len());
                Ok(products.into_iter().map(|p| p.inner).collect())
            }
            Err(ClientError::Unauthorized) => {
                warn!("Handler unauthorized, attempting authorization...");
                ctx.ask::<Self, _>(Authorize).await.map_err(|e| {
                    error!(error = %e, "Failed to authorize");
                    ctx.critical_error(&e)
                })?;
                ctx.ask::<Self, _>(msg).await.map_err(|e| {
                    error!(error = %e, "Failed to resend message");
                    ctx.critical_error(&e)
                })
            }
            Err(e) => {
                error!(error = %e, "Failed to fetch transactions: {}", e);
                Err(ctx.critical_error(&e))
            }
        }
    }
}

#[async_trait]
impl Handler<DeleteOrderRequestBuilder> for Degiro {
    type Response = ();

    type Executor = ConcurrentExecutor;

    async fn handle_message(
        &mut self,
        msg: DeleteOrderRequestBuilder,
        ctx: &Context,
    ) -> Result<Self::Response, PuppetError> {
        let order_id = msg.id.clone().unwrap();
        info!(order_id = %order_id, "Deleting order");
        msg.client(self.client.clone())
            .build()
            .map_err(|e| {
                error!(order_id = %order_id, error = %e, "Failed to build DeleteOrderRequest");
                ctx.non_critical_error(&e)
            })?
            .send()
            .await
            .map_err(|e| {
                error!(order_id = %order_id, error = %e, "Failed to delete order");
                ctx.critical_error(&e)
            })?;
        Ok(())
    }
}

#[async_trait]
impl Handler<ModifyOrderRequestBuilder> for Degiro {
    type Response = ();

    type Executor = ConcurrentExecutor;

    async fn handle_message(
        &mut self,
        msg: ModifyOrderRequestBuilder,
        ctx: &Context,
    ) -> Result<Self::Response, PuppetError> {
        let order_id = msg.id.clone().unwrap();
        info!(order_id = %order_id, "Modifing order");
        msg.client(self.client.clone())
            .build()
            .map_err(|e| {
                error!(order_id = %order_id, error = %e, "Failed to build ModifyOrderRequest");
                ctx.non_critical_error(&e)
            })?
            .send()
            .await
            .map_err(|e| {
                error!(order_id = %order_id, error = %e, "Failed to modify order");
                ctx.critical_error(&e)
            })?;
        Ok(())
    }
}

#[async_trait]
impl Handler<CreateOrderRequestBuilder> for Degiro {
    type Response = ();

    type Executor = SequentialExecutor;

    async fn handle_message(
        &mut self,
        msg: CreateOrderRequestBuilder,
        ctx: &Context,
    ) -> Result<Self::Response, PuppetError> {
        let product_id = msg.product_id.clone().unwrap();
        info!(product_id = %product_id, "Creating order");
        let res = msg
            .client(self.client.clone())
            .build()
            .map_err(|e| {
                error!(product_id = %product_id, error = %e, "Failed to build CreateOrderRequest");
                ctx.non_critical_error(&e)
            })?
            .send()
            .await
            .map_err(|e| {
                error!(product_id = %product_id, error = %e, "Failed to create order");
                ctx.critical_error(&e)
            })?;
        dbg!(res);
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Secrets {
    session_id: String,
    cookies_json: Vec<u8>,
}

#[derive(Debug)]
pub struct StoreSecrets;

#[async_trait]
impl Handler<StoreSecrets> for Degiro {
    type Response = ();

    type Executor = SequentialExecutor;

    async fn handle_message(
        &mut self,
        _msg: StoreSecrets,
        ctx: &Context,
    ) -> Result<Self::Response, PuppetError> {
        info!("Storing secrets...");
        let base_dir = directories::BaseDirs::new().expect("Can't get base dirs");
        let config_dir = base_dir
            .data_local_dir()
            .join("vogelsang")
            .to_str()
            .expect("Can't convert path")
            .to_owned();
        let cookies_jar = self
            .client
            .inner
            .lock()
            .unwrap()
            .cookie_jar
            .lock()
            .unwrap()
            .clone();
        let mut cookies_json = Vec::new();
        cookies_jar.save_incl_expired_and_nonpersistent_json(&mut cookies_json);
        dbg!(&cookies_json);
        let secrets = Secrets {
            session_id: self.client.inner.lock().unwrap().session_id.clone(),
            cookies_json,
        };
        let path = config_dir + "/secrets.json";
        let content = serde_json::to_string(&secrets).expect("Can't serialize secrets");
        tokio::fs::write(&path, content).await.map_err(|e| {
            error!("Can't save secrets: {}", e);
            ctx.critical_error(&e)
        })?;

        info!("Secrets stored.");
        Ok(())
    }
}
