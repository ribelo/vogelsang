use std::sync::Arc;

use async_trait::async_trait;
use chrono::NaiveDate;
use degiro_rs::{
    api::{
        orders::Orders,
        portfolio::Portfolio,
        search::{QueryProduct, QueryProductDetails},
        transactions::Transactions,
    },
    client::{Client, ClientBuilder, ClientError},
    util::Period,
};
use pptr::prelude::*;
use tracing::{error, info, warn};

use crate::puppet::{
    db::{Db, DeleteData},
    settings::{Asset, DeleteAsset, GetSettings},
};

use super::settings::Settings;

#[derive(Debug, Clone)]
pub struct Degiro {
    pub username: String,
    pub password: String,
    pub client: Client,
}

impl Degiro {
    pub fn new<U: AsRef<str>, P: AsRef<str>>(
        username: U,
        password: P,
    ) -> Result<Self, reqwest::Error> {
        let client = ClientBuilder::default()
            .username(username.as_ref())
            .password(password.as_ref())
            .build()?;
        Ok(Self {
            username: username.as_ref().to_owned(),
            password: password.as_ref().to_owned(),
            client,
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
        info!("Authorizing...");
        self.client.authorize().await.map_err(|e| {
            error!("Failed to authorize: {}", e);
            ctx.critical_error(&e)
        })?;

        info!("Successfully authorized.");
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct FetchData {
    pub id: Option<String>,
    pub name: Option<String>,
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
            let mut asset_name = msg.name.clone().unwrap_or_else(|| "Unknown".to_owned());
            info!(id = %id, %asset_name, "Fetching data for asset");
            let mut isin = String::new();

            match self.client.product(id).await {
                Ok(product) => {
                    isin = product.inner.isin.clone();
                    asset_name = product.inner.symbol.clone();
                    ctx
                        .ask::<Db, _>(product.inner.as_ref().clone())
                        .await
                        .map_err(|e| {
                            error!(error = %e, id = %id, asset_name = %asset_name, "Failed to send 'put product'");
                            ctx.critical_error(&e)
                        })?;
                }
                Err(_e @ ClientError::Unauthorized) => {
                    warn!(id = %id, asset_name = %asset_name, "Handler unauthorized, attempting authorization...");
                    ctx.ask::<Self, _>(Authorize).await.map_err(|e| {
                        error!(error = %e, "Failed to authorize");
                        ctx.critical_error(&e)
                    })?;
                    return ctx.ask::<Self, _>(msg.clone()).await.map_err(|e| {
                        error!(error = %e, id = %id, asset_name = %asset_name, "Failed to resend message");
                        ctx.critical_error(&e)
                    });
                }
                Err(e) => {
                    error!(error = %e, id = %id, asset_name = %asset_name, "Failed to fetch product data");
                }
            };

            match self.client.quotes(id, Period::P50Y, Period::P1M).await {
                Ok(quotes) => {
                    info!(id = %id, asset_name = %asset_name, "Fetched {} candles", quotes.time.len());
                    ctx.ask::<Db, _>(quotes.clone()).await.map_err(|e| {
                        error!(error = %e, id = %id, asset_name = %asset_name, "Failed to send 'put candles'");
                        ctx.critical_error(&e)
                    })?;
                }
                Err(e) => {
                    error!(error = %e, id = %id, asset_name = %asset_name, "Failed to fetch quotes");
                    warn!(id = %id, asset_name = %asset_name, "Removing asset from settings and database");
                    ctx.ask::<Settings, _>(DeleteAsset(id.clone())).await.map_err(|e| {
                        error!(error = %e, id = %id, asset_name = %asset_name, "Failed to remove asset from settings");
                        ctx.critical_error(&e)
                    })?;
                    ctx.ask::<Db, _>(DeleteData(id.clone())).await.map_err(|e| {
                        error!(error = %e, id = %id, asset_name = %asset_name, "Failed to delete asset from database");
                        ctx.critical_error(&e)
                    })?;
                }
            }

            match self.client.financial_statements(id, &isin).await {
                Ok(financial_reports) => {
                    ctx
                        .ask::<Db, _>(financial_reports)
                        .await
                        .map_err(|e| {
                            error!(error = %e, id = %id, asset_name = %asset_name, "Failed to send 'put financial reports'");
                            ctx.critical_error(&e)
                        })?;
                }
                Err(e) => {
                    error!(error = %e, id = %id, asset_name = %asset_name, "Failed to fetch financial reports");
                    warn!(id = %id, asset_name = %asset_name, "Removing asset from settings and database");
                    ctx.ask::<Settings, _>(DeleteAsset(id.clone())).await.map_err(|e| {
                        error!(error = %e, id = %id, asset_name = %asset_name, "Failed to remove asset from settings");
                        ctx.critical_error(&e)
                    })?;
                    ctx.ask::<Db, _>(DeleteData(id.clone())).await.map_err(|e| {
                        error!(error = %e, id = %id, asset_name = %asset_name, "Failed to delete asset from database");
                        ctx.critical_error(&e)
                    })?;
                }
            }

            match self.client.company_ratios(id, &isin).await {
                Ok(company_ratios) => {
                    let () = Box::pin(ctx.send::<Db, _>(company_ratios)).await.map_err(|e| {
                        error!(error = %e, id = %id, asset_name = %asset_name, "Failed to send 'put company ratios'");
                        ctx.critical_error(&e)
                    })?;
                }
                Err(e) => {
                    error!(error = %e, id = %id, asset_name = %asset_name, "Failed to fetch company ratios");
                    warn!(id = %id, asset_name = %asset_name, "Removing asset from settings and database");
                    ctx.ask::<Settings, _>(DeleteAsset(id.clone())).await.map_err(|e| {
                        error!(error = %e, id = %id, asset_name = %asset_name, "Failed to remove asset from settings");
                        ctx.critical_error(&e)
                    })?;
                    ctx.ask::<Db, _>(DeleteData(id.clone())).await.map_err(|e| {
                        error!(error = %e, id = %id, asset_name = %asset_name, "Failed to delete asset from database");
                        ctx.critical_error(&e)
                    })?;
                }
            }
            info!(id = %id, asset_name = %asset_name, "Finished fetching data for");
        } else {
            info!("Fetching data for all assets");
            ctx.ask::<Self, _>(Authorize).await.map_err(|e| {
                error!(error = %e, "Failed to authorize");
                ctx.critical_error(&e)
            })?;
            let settings = ctx.ask::<Settings, _>(GetSettings).await.map_err(|e| {
                error!(error = %e, "Failed to get settings");
                ctx.critical_error(&e)
            })?;
            for Asset { id, name } in &settings.assets {
                let msg = FetchData {
                    id: Some(id.to_string()),
                    name: Some(name.clone()),
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
            Ok(products) => Ok(products.into_iter().map(|p| p.inner).collect()),
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
