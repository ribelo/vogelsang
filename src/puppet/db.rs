use std::{collections::HashSet, fmt};

use async_trait::async_trait;
use degiro_rs::api::{
    company_ratios::CompanyRatios, financial_statements::FinancialReports, product::ProductDetails,
    quotes::Quotes,
};
use erfurt::prelude::Candles;
use master_of_puppets::prelude::*;
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use super::settings::{GetSettings, Settings};

#[derive(Clone)]
pub struct Db {
    pub env: heed::Env,
    pub candles: heed::Database<heed::types::Str, heed::types::SerdeBincode<Candles>>,
    pub products: heed::Database<heed::types::Str, heed::types::SerdeBincode<ProductDetails>>,
    pub financial_reports:
        heed::Database<heed::types::Str, heed::types::SerdeBincode<FinancialReports>>,
    pub company_ratios: heed::Database<heed::types::Str, heed::types::SerdeBincode<CompanyRatios>>,
}

impl fmt::Debug for Db {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Db").finish()
    }
}

impl Db {
    #[must_use]
    pub fn new() -> Self {
        std::fs::create_dir_all("vogelsang.mdb").expect("Failed to create db directory.");
        let env = heed::EnvOpenOptions::new()
            .map_size(1024 * 1024 * 1024) // 1GB
            .max_dbs(10)
            .open("vogelsang.mdb")
            .unwrap();
        let candles = env.create_database(Some("candles")).unwrap();
        let products = env.create_database(Some("products")).unwrap();
        let financial_reports = env.create_database(Some("financial_reports")).unwrap();
        let company_ratios = env.create_database(Some("company_ratios")).unwrap();
        Self {
            env,
            candles,
            products,
            financial_reports,
            company_ratios,
        }
    }
}

impl Default for Db {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Lifecycle for Db {
    type Supervision = OneToOne;

    async fn reset(&self, _puppeter: &Puppeter) -> Result<Self, CriticalError> {
        Ok(Self::new())
    }
}

#[async_trait]
impl Handler<ProductDetails> for Db {
    type Response = ();

    type Executor = SequentialExecutor;

    async fn handle_message(
        &mut self,
        msg: ProductDetails,
        puppeter: &Puppeter,
    ) -> Result<Self::Response, PuppetError> {
        info!(id = msg.id, symbol = msg.symbol, "Saving product.");
        let mut wtx = self
            .env
            .write_txn()
            .map_err(|e| puppeter.critical_error(&e))?;
        self.products.put(&mut wtx, &msg.id, &msg).map_err(|e| {
            error!(
                id = msg.id,
                symbol = msg.symbol,
                error = %e,
                "Failed to save product."
            );
            puppeter.critical_error(&e)
        })?;
        wtx.commit().map_err(|e| puppeter.critical_error(&e))
    }
}

#[async_trait]
impl Handler<Quotes> for Db {
    type Response = ();

    type Executor = SequentialExecutor;

    async fn handle_message(
        &mut self,
        msg: Quotes,
        puppeter: &Puppeter,
    ) -> Result<Self::Response, PuppetError> {
        info!(id = msg.id, "Saving candles.");
        let mut wtx = self
            .env
            .write_txn()
            .map_err(|e| puppeter.critical_error(&e))?;
        let candles = Candles::from(msg.clone());
        self.candles.put(&mut wtx, &msg.id, &candles).map_err(|e| {
            error!(
                id = msg.id,
                error = %e,
                "Failed to save candles."
            );
            puppeter.critical_error(&e)
        })?;
        wtx.commit().map_err(|e| puppeter.critical_error(&e))
    }
}

#[async_trait]
impl Handler<FinancialReports> for Db {
    type Response = ();
    type Executor = SequentialExecutor;
    async fn handle_message(
        &mut self,
        msg: FinancialReports,
        puppeter: &Puppeter,
    ) -> Result<Self::Response, PuppetError> {
        info!(id = msg.id, "Saving financial reports.");
        let mut wtx = self
            .env
            .write_txn()
            .map_err(|e| puppeter.critical_error(&e))?;
        self.financial_reports
            .put(&mut wtx, &msg.id, &msg)
            .map_err(|e| {
                error!(
                    id = msg.id,
                    error = %e,
                    "Failed to save financial reports."
                );
                puppeter.critical_error(&e)
            })?;
        wtx.commit().map_err(|e| puppeter.critical_error(&e))
    }
}

#[async_trait]
impl Handler<CompanyRatios> for Db {
    type Response = ();
    type Executor = SequentialExecutor;
    async fn handle_message(
        &mut self,
        msg: CompanyRatios,
        puppeter: &Puppeter,
    ) -> Result<Self::Response, PuppetError> {
        info!(id = msg.id, "Saving company ratios.");
        let mut wtx = self
            .env
            .write_txn()
            .map_err(|e| puppeter.critical_error(&e))?;
        self.company_ratios
            .put(&mut wtx, &msg.id, &msg)
            .map_err(|e| {
                error!(
                    id = msg.id,
                    error = %e,
                    "Failed to save company ratios."
                );
                puppeter.critical_error(&e)
            })?;
        wtx.commit().map_err(|e| puppeter.critical_error(&e))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProductQuery {
    Id(String),
    Symbol(String),
    Name(String),
}

#[async_trait]
impl Handler<ProductQuery> for Db {
    type Response = Option<ProductDetails>;

    type Executor = ConcurrentExecutor;

    async fn handle_message(
        &mut self,
        msg: ProductQuery,
        puppeter: &Puppeter,
    ) -> Result<Self::Response, PuppetError> {
        let rtxn = self
            .env
            .read_txn()
            .map_err(|e| puppeter.critical_error(&e))?;
        match msg {
            ProductQuery::Id(id) => {
                return self
                    .products
                    .get(&rtxn, &id)
                    .map_err(|e| puppeter.critical_error(&e));
            }
            ProductQuery::Symbol(symbol) => {
                let mut iter = self
                    .products
                    .iter(&rtxn)
                    .map_err(|e| puppeter.critical_error(&e))?;
                while let Some(Ok((_, product))) = iter.next() {
                    println!("{:?}", product.symbol);
                    if product.symbol.to_lowercase() == symbol.to_lowercase() {
                        return Ok(Some(product));
                    }
                }
            }
            ProductQuery::Name(name) => {
                let rgx = regex::Regex::new(&format!("(?i){name}")).unwrap();
                let mut iter = self
                    .products
                    .iter(&rtxn)
                    .map_err(|e| puppeter.critical_error(&e))?;
                while let Some(Ok((_, product))) = iter.next() {
                    if rgx.is_match(&product.name) {
                        return Ok(Some(product));
                    }
                }
            }
        }
        Ok(None)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CandlesQuery {
    Id(String),
    Symbol(String),
    Name(String),
}

impl From<ProductQuery> for CandlesQuery {
    fn from(value: ProductQuery) -> Self {
        match value {
            ProductQuery::Id(id) => Self::Id(id),
            ProductQuery::Symbol(symbol) => Self::Symbol(symbol),
            ProductQuery::Name(name) => Self::Name(name),
        }
    }
}

#[async_trait]
impl Handler<CandlesQuery> for Db {
    type Response = Option<Candles>;

    type Executor = ConcurrentExecutor;

    async fn handle_message(
        &mut self,
        msg: CandlesQuery,
        puppeter: &Puppeter,
    ) -> Result<Self::Response, PuppetError> {
        match msg {
            CandlesQuery::Id(id) => {
                let rtxn = self
                    .env
                    .read_txn()
                    .map_err(|e| puppeter.critical_error(&e))?;
                return self
                    .candles
                    .get(&rtxn, &id)
                    .map_err(|e| puppeter.critical_error(&e));
            }
            CandlesQuery::Symbol(symbol) => {
                let new_msg = {
                    let rtxn = self
                        .env
                        .read_txn()
                        .map_err(|e| puppeter.critical_error(&e))?;
                    let mut iter = self
                        .products
                        .iter(&rtxn)
                        .map_err(|e| puppeter.critical_error(&e))?;
                    iter.find_map(|res| {
                        res.ok()
                            .filter(|(_, product)| {
                                product.symbol.to_lowercase() == symbol.to_lowercase()
                            })
                            .map(|(_, product)| CandlesQuery::Id(product.id))
                    })
                };
                if let Some(msg) = new_msg {
                    return puppeter
                        .ask::<Self, _>(msg)
                        .await
                        .map_err(|e| puppeter.critical_error(&e));
                }
                return Ok(None);
            }
            CandlesQuery::Name(name) => {
                let rgx = regex::Regex::new(&format!("(?i){name}")).unwrap();
                let new_msg = {
                    let rtxn = self
                        .env
                        .read_txn()
                        .map_err(|e| puppeter.critical_error(&e))?;
                    let mut iter = self
                        .products
                        .iter(&rtxn)
                        .map_err(|e| puppeter.critical_error(&e))?;
                    iter.find_map(|res| {
                        res.ok()
                            .filter(|(_, product)| rgx.is_match(&product.name))
                            .map(|(_, product)| CandlesQuery::Id(product.id))
                    })
                };
                if let Some(msg) = new_msg {
                    return puppeter
                        .ask::<Self, _>(msg)
                        .await
                        .map_err(|e| puppeter.critical_error(&e));
                }
                return Ok(None);
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FinanclaReportsQuery {
    Id(String),
    Symbol(String),
    Name(String),
}

impl From<ProductQuery> for FinanclaReportsQuery {
    fn from(value: ProductQuery) -> Self {
        match value {
            ProductQuery::Id(id) => Self::Id(id),
            ProductQuery::Symbol(symbol) => Self::Symbol(symbol),
            ProductQuery::Name(name) => Self::Name(name),
        }
    }
}

#[async_trait]
impl Handler<FinanclaReportsQuery> for Db {
    type Response = Option<FinancialReports>;
    type Executor = ConcurrentExecutor;
    async fn handle_message(
        &mut self,
        msg: FinanclaReportsQuery,
        puppeter: &Puppeter,
    ) -> Result<Self::Response, PuppetError> {
        match msg {
            FinanclaReportsQuery::Id(id) => {
                let rtxn = self
                    .env
                    .read_txn()
                    .map_err(|e| puppeter.critical_error(&e))?;
                return self
                    .financial_reports
                    .get(&rtxn, &id)
                    .map_err(|e| puppeter.critical_error(&e));
            }
            FinanclaReportsQuery::Symbol(symbol) => {
                let new_msg = {
                    let rtxn = self
                        .env
                        .read_txn()
                        .map_err(|e| puppeter.critical_error(&e))?;
                    let mut iter = self
                        .products
                        .iter(&rtxn)
                        .map_err(|e| puppeter.critical_error(&e))?;
                    iter.find_map(|res| {
                        res.ok()
                            .filter(|(_, product)| {
                                product.symbol.to_lowercase() == symbol.to_lowercase()
                            })
                            .map(|(_, product)| FinanclaReportsQuery::Id(product.id))
                    })
                };
                if let Some(msg) = new_msg {
                    return puppeter
                        .ask::<Self, _>(msg)
                        .await
                        .map_err(|e| puppeter.critical_error(&e));
                }
                return Ok(None);
            }
            FinanclaReportsQuery::Name(name) => {
                let rgx = regex::Regex::new(&format!("(?i){name}")).unwrap();
                let new_msg = {
                    let rtxn = self
                        .env
                        .read_txn()
                        .map_err(|e| puppeter.critical_error(&e))?;
                    let mut iter = self
                        .products
                        .iter(&rtxn)
                        .map_err(|e| puppeter.critical_error(&e))?;
                    iter.find_map(|res| {
                        res.ok()
                            .filter(|(_, product)| rgx.is_match(&product.name))
                            .map(|(_, product)| FinanclaReportsQuery::Id(product.id))
                    })
                };
                if let Some(msg) = new_msg {
                    return puppeter
                        .ask::<Self, _>(msg)
                        .await
                        .map_err(|e| puppeter.critical_error(&e));
                }
                return Ok(None);
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompanyRatiosQuery {
    Id(String),
    Symbol(String),
    Name(String),
}

impl From<ProductQuery> for CompanyRatiosQuery {
    fn from(value: ProductQuery) -> Self {
        match value {
            ProductQuery::Id(id) => Self::Id(id),
            ProductQuery::Symbol(symbol) => Self::Symbol(symbol),
            ProductQuery::Name(name) => Self::Name(name),
        }
    }
}

#[async_trait]
impl Handler<CompanyRatiosQuery> for Db {
    type Response = Option<CompanyRatios>;
    type Executor = ConcurrentExecutor;
    async fn handle_message(
        &mut self,
        msg: CompanyRatiosQuery,
        puppeter: &Puppeter,
    ) -> Result<Self::Response, PuppetError> {
        match msg {
            CompanyRatiosQuery::Id(id) => {
                let rtxn = self
                    .env
                    .read_txn()
                    .map_err(|e| puppeter.critical_error(&e))?;
                return self
                    .company_ratios
                    .get(&rtxn, &id)
                    .map_err(|e| puppeter.critical_error(&e));
            }
            CompanyRatiosQuery::Symbol(symbol) => {
                let new_msg = {
                    let rtxn = self
                        .env
                        .read_txn()
                        .map_err(|e| puppeter.critical_error(&e))?;
                    let mut iter = self
                        .products
                        .iter(&rtxn)
                        .map_err(|e| puppeter.critical_error(&e))?;
                    iter.find_map(|res| {
                        res.ok()
                            .filter(|(_, product)| {
                                product.symbol.to_lowercase() == symbol.to_lowercase()
                            })
                            .map(|(_, product)| CompanyRatiosQuery::Id(product.id))
                    })
                };
                if let Some(msg) = new_msg {
                    return puppeter
                        .ask::<Self, _>(msg)
                        .await
                        .map_err(|e| puppeter.critical_error(&e));
                }
                return Ok(None);
            }
            CompanyRatiosQuery::Name(name) => {
                let rgx = regex::Regex::new(&format!("(?i){name}")).unwrap();
                let new_msg = {
                    let rtxn = self
                        .env
                        .read_txn()
                        .map_err(|e| puppeter.critical_error(&e))?;
                    let mut iter = self
                        .products
                        .iter(&rtxn)
                        .map_err(|e| puppeter.critical_error(&e))?;
                    iter.find_map(|res| {
                        res.ok()
                            .filter(|(_, product)| rgx.is_match(&product.name))
                            .map(|(_, product)| CompanyRatiosQuery::Id(product.id))
                    })
                };
                if let Some(msg) = new_msg {
                    return puppeter
                        .ask::<Self, _>(msg)
                        .await
                        .map_err(|e| puppeter.critical_error(&e));
                }
                return Ok(None);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct DeleteData(pub String);

#[async_trait]
impl Handler<DeleteData> for Db {
    type Response = ();
    type Executor = ConcurrentExecutor;
    async fn handle_message(
        &mut self,
        msg: DeleteData,
        puppeter: &Puppeter,
    ) -> Result<Self::Response, PuppetError> {
        info!(id = %msg.0, "Deleting data.");
        let mut wtx = self
            .env
            .write_txn()
            .map_err(|e| puppeter.critical_error(&e))?;
        self.candles
            .delete(&mut wtx, &msg.0)
            .map_err(|e| puppeter.critical_error(&e))?;
        self.products
            .delete(&mut wtx, &msg.0)
            .map_err(|e| puppeter.critical_error(&e))?;
        self.financial_reports
            .delete(&mut wtx, &msg.0)
            .map_err(|e| puppeter.critical_error(&e))?;
        self.company_ratios
            .delete(&mut wtx, &msg.0)
            .map_err(|e| puppeter.critical_error(&e))?;
        wtx.commit().map_err(|e| puppeter.critical_error(&e))
    }
}

#[derive(Debug, Clone)]
pub struct CleanUp;

#[async_trait]
impl Handler<CleanUp> for Db {
    type Response = ();

    type Executor = ConcurrentExecutor;

    async fn handle_message(
        &mut self,
        _msg: CleanUp,
        puppeter: &Puppeter,
    ) -> Result<Self::Response, PuppetError> {
        let settings = puppeter
            .ask::<Settings, _>(GetSettings)
            .await
            .map_err(|e| {
                error!(error = %e, "Failed to get settings");
                puppeter.critical_error(&e)
            })?;

        let assets = settings
            .assets
            .iter()
            .map(|(id, _)| id.clone())
            .collect::<HashSet<_>>();

        let to_delete = {
            let rtxn = self
                .env
                .read_txn()
                .map_err(|e| puppeter.critical_error(&e))?;

            let iter = self
                .products
                .iter(&rtxn)
                .map_err(|e| puppeter.critical_error(&e))?;

            iter.filter_map(|res| {
                let (id, _) = res.unwrap();
                (!assets.contains(id)).then(|| id.to_owned())
            })
            .collect::<HashSet<_>>()
        };

        for id in to_delete {
            puppeter
                .ask::<Self, _>(DeleteData(id))
                .await
                .map_err(|e| puppeter.critical_error(&e))?;
        }

        Ok(())
    }
}
