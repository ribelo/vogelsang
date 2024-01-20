use async_trait::async_trait;
use degiro_rs::{
    api::{
        company_ratios::CompanyRatios,
        financial_statements::FinancialReports,
        portfolio::Portfolio,
        product::{Product, ProductDetails},
        quotes::Quotes,
    },
    client::{Client, ClientBuilder, ClientError},
    util::Period,
};
use erfurt::candle::Candles;
use master_of_puppets::{executor::SequentialExecutor, prelude::*};
use tracing::{error, info, warn};

use crate::puppet::{
    db::{Db, DeleteData},
    settings::{DeleteAsset, GetSettings},
};

use super::settings::Settings;

#[derive(Debug, Clone)]
pub struct Degiro {
    pub username: String,
    pub password: String,
    pub client: Client,
}

impl Degiro {
    pub fn new(username: impl AsRef<str>, password: impl AsRef<str>) -> Self {
        let client = ClientBuilder::default()
            .username(username.as_ref())
            .password(password.as_ref())
            .build()
            .unwrap();
        Self {
            username: username.as_ref().to_string(),
            password: password.as_ref().to_string(),
            client,
        }
    }
}

#[async_trait]
impl Lifecycle for Degiro {
    type Supervision = OneToOne;

    async fn reset(&self, _puppeter: &Puppeter) -> Result<Self, CriticalError> {
        Ok(Self::new(&self.username, &self.password))
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
        puppeter: &Puppeter,
    ) -> Result<Self::Response, PuppetError> {
        info!("Authorizing...");
        self.client.authorize().await.map_err(|e| {
            error!("Failed to authorize: {}", e);
            PuppetError::Critical(CriticalError::new(puppeter.pid, e.to_string()))
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
        puppeter: &Puppeter,
    ) -> Result<Self::Response, PuppetError> {
        if let Some(id) = &msg.id {
            let mut asset_name = msg.name.clone().unwrap_or_else(|| "Unknown".to_string());
            info!(id = %id, %asset_name, "Fetching data for asset");
            let mut isin = String::new();

            match self.client.product(id).await {
                Ok(product) => {
                    isin = product.inner.isin.clone();
                    asset_name = product.inner.symbol.clone();
                    puppeter
                        .send::<Db, _>(product.inner.as_ref().clone())
                        .await
                        .map_err(|e| {
                            error!(error = %e, id = %id, asset_name = %asset_name, "Failed to send 'put product'");
                            puppeter.critical_error(e)
                        })?;
                }
                Err(e @ ClientError::Unauthorized) => {
                    warn!(id = %id, asset_name = %asset_name, "Handler unauthorized, attempting authorization...");
                    puppeter.ask::<Self, _>(Authorize).await.map_err(|e| {
                        error!(error = %e, "Failed to authorize");
                        puppeter.critical_error(e)
                    })?;
                    puppeter.send::<Self, _>(msg.clone()).await.map_err(|e| {
                        error!(error = %e, id = %id, asset_name = %asset_name, "Failed to resend message");
                        puppeter.critical_error(e)
                    })?;

                    return Err(puppeter.non_critical_error(e).into());
                }
                Err(e) => {
                    error!(error = %e, id = %id, asset_name = %asset_name, "Failed to fetch product data")
                }
            };

            match self.client.quotes(id, Period::P50Y, Period::P1M).await {
                Ok(quotes) => {
                    info!(id = %id, asset_name = %asset_name, "Fetched {} candles", quotes.time.len());
                    puppeter.send::<Db, _>(quotes.clone()).await.map_err(|e| {
                        error!(error = %e, id = %id, asset_name = %asset_name, "Failed to send 'put candles'");
                        puppeter.critical_error(e)
                    })?;
                }
                Err(e) => {
                    error!(error = %e, id = %id, asset_name = %asset_name, "Failed to fetch quotes");
                    warn!(id = %id, asset_name = %asset_name, "Removing asset from settings and database");
                    puppeter.ask::<Settings, _>(DeleteAsset(id.clone())).await.map_err(|e| {
                        error!(error = %e, id = %id, asset_name = %asset_name, "Failed to remove asset from settings");
                        puppeter.critical_error(e)
                    })?;
                    puppeter.ask::<Db, _>(DeleteData(id.clone())).await.map_err(|e| {
                        error!(error = %e, id = %id, asset_name = %asset_name, "Failed to delete asset from database");
                        puppeter.critical_error(e)
                    })?;
                }
            }

            match self.client.financial_statements(id, &isin).await {
                Ok(financial_reports) => {
                    puppeter
                        .send::<Db, _>(financial_reports)
                        .await
                        .map_err(|e| {
                            error!(error = %e, id = %id, asset_name = %asset_name, "Failed to send 'put financial reports'");
                            puppeter.critical_error(e)
                        })?;
                }
                Err(e) => {
                    error!(error = %e, id = %id, asset_name = %asset_name, "Failed to fetch financial reports");
                    warn!(id = %id, asset_name = %asset_name, "Removing asset from settings and database");
                    puppeter.ask::<Settings, _>(DeleteAsset(id.clone())).await.map_err(|e| {
                        error!(error = %e, id = %id, asset_name = %asset_name, "Failed to remove asset from settings");
                        puppeter.critical_error(e)
                    })?;
                    puppeter.ask::<Db, _>(DeleteData(id.clone())).await.map_err(|e| {
                        error!(error = %e, id = %id, asset_name = %asset_name, "Failed to delete asset from database");
                        puppeter.critical_error(e)
                    })?;
                }
            }

            match self.client.company_ratios(id, &isin).await {
                Ok(company_ratios) => {
                    puppeter.send::<Db, _>(company_ratios).await.map_err(|e| {
                        error!(error = %e, id = %id, asset_name = %asset_name, "Failed to send 'put company ratios'");
                        puppeter.critical_error(e)
                    })?;
                }
                Err(e) => {
                    error!(error = %e, id = %id, asset_name = %asset_name, "Failed to fetch company ratios");
                    warn!(id = %id, asset_name = %asset_name, "Removing asset from settings and database");
                    puppeter.ask::<Settings, _>(DeleteAsset(id.clone())).await.map_err(|e| {
                        error!(error = %e, id = %id, asset_name = %asset_name, "Failed to remove asset from settings");
                        puppeter.critical_error(e)
                    })?;
                    puppeter.ask::<Db, _>(DeleteData(id.clone())).await.map_err(|e| {
                        error!(error = %e, id = %id, asset_name = %asset_name, "Failed to delete asset from database");
                        puppeter.critical_error(e)
                    })?;
                }
            }

            Ok(())
        } else {
            info!("Fetching data for all assets");
            puppeter.ask::<Self, _>(Authorize).await.map_err(|e| {
                error!(error = %e, "Failed to authorize");
                puppeter.critical_error(e)
            })?;
            let settings = puppeter
                .ask::<Settings, _>(GetSettings)
                .await
                .map_err(|e| {
                    error!(error = %e, "Failed to get settings");
                    puppeter.critical_error(e)
                })?;
            for (id, name) in settings.assets.iter() {
                let msg = FetchData {
                    id: Some(id.to_string()),
                    name: Some(name.clone()),
                };
                puppeter.send::<Self, _>(msg).await.map_err(|e| {
                    error!(error = %e, id = %id, "Failed to resend message");
                    puppeter.critical_error(e)
                })?;
            }
            info!("Finished fetching data for all assets");
            Ok(())
        }
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
        puppeter: &Puppeter,
    ) -> Result<Self::Response, PuppetError> {
        info!("Fetching portfolio");
        match self.client.portfolio().await {
            Ok(portfolio) => Ok(portfolio),
            Err(e @ ClientError::Unauthorized) => {
                warn!("Handler unauthorized, attempting authorization...");
                puppeter.ask::<Self, _>(Authorize).await.map_err(|e| {
                    error!(error = %e, "Failed to authorize");
                    puppeter.critical_error(e)
                })?;
                puppeter.send::<Self, _>(msg.clone()).await.map_err(|e| {
                    error!(error = %e, "Failed to resend message");
                    puppeter.critical_error(e)
                })?;
                Err(puppeter.non_critical_error(e).into())
            }
            Err(e) => {
                error!(error = %e, "Failed to fetch portfolio: {}", e);
                Err(puppeter.critical_error(e).into())
            }
        }
    }
}
