use std::{collections::HashSet, fmt, sync::Arc};

use async_trait::async_trait;
use chrono::Datelike;
use comfy_table::{presets::UTF8_BORDERS_ONLY, Cell, Table};
use dashmap::DashMap;
use degiro_rs::{
    api::product::{Product, ProductDetails},
    util::{Period, ProductCategory, TransactionType},
};
use erfurt::candle::{Candles, CandlesExt};
use itertools::Itertools;
use master_of_puppets::prelude::*;
use qualsdorf::{
    average_drawdown::AverageDrawdownExt, rolling_economic_drawdown::RollingEconomicDrawdownExt,
    rsi::RsiExt, sharpe_ratio::SharpeRatioExt, Indicator,
};
use tracing::{error, info, warn};

use crate::{
    portfolio::{AssetsSeq, RiskMode, SingleAllocation},
    puppet::degiro::{Degiro, GetPortfolio},
};

use super::{
    db::{CandlesQuery, CompanyRatiosQuery, Db, FinanclaReportsQuery, ProductQuery},
    settings::Settings,
};

#[derive(Debug, Clone)]
pub struct Calculator {
    settings: Settings,
}

impl Calculator {
    pub fn new(settings: Settings) -> Self {
        Self { settings }
    }
}

#[async_trait]
impl Lifecycle for Calculator {
    type Supervision = OneToOne;

    async fn reset(&self, _puppeter: &Puppeter) -> Result<Self, CriticalError> {
        Ok(Self::new(self.settings.clone()))
    }
}

#[derive(Debug, Clone)]
pub struct GetSingleAllocation {
    pub query: CandlesQuery,
    pub mode: RiskMode,
    pub risk: f64,
    pub risk_free: f64,
}

#[async_trait]
impl Handler<GetSingleAllocation> for Calculator {
    type Response = Option<f64>;

    type Executor = ConcurrentExecutor;

    async fn handle_message(
        &mut self,
        msg: GetSingleAllocation,
        puppeter: &Puppeter,
    ) -> Result<Self::Response, PuppetError> {
        if let Some(candles) = puppeter.ask::<Db, _>(msg.query.clone()).await? {
            let allocation = candles
                .single_allocation(msg.mode, msg.risk, msg.risk_free, Period::P1Y, Period::P1M)
                .await
                .map_err(|e| {
                    error!(error = %e, "Failed to calculate single allocation");
                    CriticalError::new(puppeter.pid, e.to_string())
                })?;
            Ok(Some(allocation))
        } else {
            Ok(None)
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CalculatePortfolio {
    pub mode: RiskMode,
    pub risk: f64,
    pub risk_free: f64,
    pub freq: usize,
    pub money: f64,
    pub max_stocks: usize,
    pub min_rsi: Option<f64>,
    pub max_rsi: Option<f64>,
    pub min_class: Option<ProductCategory>,
    pub max_class: Option<ProductCategory>,
    pub short_sales_constraint: bool,
    pub roic_wacc_delta: Option<f64>,
}

#[derive(Debug)]
pub struct DataEntry {
    product: ProductDetails,
    candles: Candles,
    single_allocation: f64,
    redp_allocation: f64,
    sharpe_ratio: f64,
    redp: f64,
    avg_dd: f64,
    rsi: f64,
    roic: f64,
    wacc: f64,
}

#[derive(Debug, Clone)]
pub struct GetDataEntry {
    id: String,
    pub risk: f64,
    pub risk_free: f64,
    pub freq: usize,
}

#[async_trait]
impl Handler<GetDataEntry> for Calculator {
    type Response = Option<DataEntry>;

    type Executor = ConcurrentExecutor;

    async fn handle_message(
        &mut self,
        msg: GetDataEntry,
        puppeter: &Puppeter,
    ) -> Result<Self::Response, PuppetError> {
        let candles = puppeter
            .ask::<Db, _>(CandlesQuery::Id(msg.id.clone()))
            .await?;
        let product = puppeter
            .ask::<Db, _>(ProductQuery::Id(msg.id.clone()))
            .await?;
        let financials = puppeter
            .ask::<Db, _>(FinanclaReportsQuery::Id(msg.id.clone()))
            .await?;
        let ratios = puppeter
            .ask::<Db, _>(CompanyRatiosQuery::Id(msg.id.clone()))
            .await?;
        match (candles, product, financials, ratios) {
            (Some(candles), Some(product), Some(financials), Some(ratios)) => {
                if candles.time.len() >= msg.freq {
                    let candles = candles.take_last(msg.freq).unwrap();
                    let single_allocation = candles
                        .single_allocation(
                            RiskMode::STD,
                            msg.risk,
                            msg.risk_free,
                            Period::P1Y,
                            Period::P1M,
                        )
                        .await
                        .unwrap();
                    let sharpe_ratio = *candles
                        .sharpe_ratio(msg.freq, msg.risk_free)
                        .unwrap()
                        .last()
                        .unwrap();
                    let avg_dd = *candles.average_drawdown(msg.freq).unwrap().last().unwrap();
                    let rsi = *candles.rsi(msg.freq).unwrap().last().unwrap();
                    let redp = *candles
                        .rolling_economic_drawndown(msg.freq)
                        .unwrap()
                        .last()
                        .unwrap();
                    let Some(beta) = ratios.current_ratios.beta.value else {
                        warn!("No beta for {}", &product.id);
                        return Ok(None);
                    };
                    let current_year = chrono::Utc::now().year();
                    let Some(annual_report) = financials.get_annual(current_year - 1) else {
                        warn!("No annual report for {} in {}", &product.id, current_year);
                        dbg!(&financials);
                        return Ok(None);
                    };
                    let roic = annual_report.roic();
                    let capm = annual_report.capm_equity_cost(0.2, 0.05, beta);
                    let wacc = annual_report.wacc(capm);
                    let entry = DataEntry {
                        product,
                        candles,
                        single_allocation,
                        redp_allocation: 0.0,
                        sharpe_ratio,
                        avg_dd,
                        rsi,
                        redp,
                        roic,
                        wacc,
                    };
                    Ok(Some(entry))
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
    }
}

#[async_trait]
impl Handler<CalculatePortfolio> for Calculator {
    type Response = String;

    type Executor = ConcurrentExecutor;

    async fn handle_message(
        &mut self,
        msg: CalculatePortfolio,
        puppeter: &Puppeter,
    ) -> Result<Self::Response, PuppetError> {
        let data = DashMap::new();
        for (id, _) in self.settings.assets.iter() {
            let get_data_entry = GetDataEntry {
                id: id.clone(),
                risk: msg.risk,
                risk_free: msg.risk_free,
                freq: msg.freq,
            };
            if let Some(entry) = puppeter.ask::<Self, _>(get_data_entry).await? {
                data.insert(id.clone(), entry);
            }
        }
        let mut portfolio_calculator = PortfolioCalculator {
            mode: msg.mode,
            risk: msg.risk,
            risk_free: msg.risk_free,
            money: msg.money,
            max_stock: msg.max_stocks as i32,
            min_rsi: msg.min_rsi,
            max_rsi: msg.max_rsi,
            short_sales_constraint: msg.short_sales_constraint,
            roic_wacc_delta: msg.roic_wacc_delta,
            data: Arc::new(data),
        };
        portfolio_calculator.remove_invalid().calculate().await;
        Ok(portfolio_calculator.as_table().to_string())
    }
}

pub struct PortfolioCalculator {
    mode: RiskMode,
    risk: f64,
    risk_free: f64,
    money: f64,
    max_stock: i32,
    min_rsi: Option<f64>,
    max_rsi: Option<f64>,
    short_sales_constraint: bool,
    roic_wacc_delta: Option<f64>,
    pub data: Arc<DashMap<String, DataEntry>>,
}

impl PortfolioCalculator {
    pub fn blacklist(&self, id: &str) {
        self.data.remove(id);
    }

    pub fn remove_invalid(&mut self) -> &mut Self {
        let mut to_remove: HashSet<String> = HashSet::new();
        let max_time_month = self
            .data
            .iter()
            .filter_map(|entry| entry.value().candles.time.last().cloned())
            .max()
            .unwrap()
            .month();

        for entry in self.data.iter() {
            let id = entry.key();
            let DataEntry {
                candles,
                product,
                single_allocation,
                rsi,
                roic,
                wacc,
                ..
            } = entry.value();
            let last_candle_month = candles.time.last().unwrap().month();

            if last_candle_month != max_time_month {
                println!(
                    "Data is not up to date for {:>10} : {:<24.24} - last candle month: {} max month: {}",
                    id,
                    product.name,
                    last_candle_month,
                    &max_time_month,
                );
                to_remove.insert(id.clone());
            }

            if self.min_rsi.is_some() && self.max_rsi.is_some() {
                let min_rsi_value = self.min_rsi.unwrap();
                let max_rsi_value = self.max_rsi.unwrap();

                if *rsi < min_rsi_value || *rsi > max_rsi_value {
                    println!("RSI is out of range for {} : {}", id, product.name);
                    println!("Should be: {} < {} < {}", min_rsi_value, rsi, max_rsi_value);
                    to_remove.insert(id.clone());
                }
            }

            if product.close_price > self.money
                || (*single_allocation < 1.0 && self.short_sales_constraint)
            {
                to_remove.insert(id.clone());
            }

            if let Some(delta) = self.roic_wacc_delta {
                if *roic < wacc + delta {
                    to_remove.insert(id.clone());
                }
            }
        }

        for id in to_remove {
            self.blacklist(&id);
        }

        self
    }

    pub fn remove_worst(&self) {
        let min_key = {
            self.data
                .iter()
                .min_by(|a, b| {
                    let a_ratio = a.value().sharpe_ratio;
                    let b_ratio = b.value().sharpe_ratio;
                    a_ratio
                        .partial_cmp(&b_ratio)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|min_entry| min_entry.key().clone())
        };
        if let Some(id) = min_key {
            self.blacklist(&id);
        } else {
            println!("Cannot find min key");
        }
    }

    pub async fn calculate(&self) {
        let mut retry = 0;
        'outer: loop {
            if retry > 5 {
                panic!("Too many retries");
            }
            let stocks = self
                .data
                .iter()
                .map(|entry| {
                    let DataEntry {
                        product, candles, ..
                    } = entry.value();
                    (product.clone(), candles.clone())
                })
                .sorted_by_cached_key(|(p, _c)| p.id.clone())
                .collect_vec();

            let seq = AssetsSeq(stocks);
            let Ok(mut allocations) = seq
                .redp_multiple_allocation(
                    self.mode,
                    self.risk,
                    self.risk_free,
                    Period::P1Y,
                    Period::P1M,
                    self.short_sales_constraint,
                )
                .await
            else {
                retry += 1;
                self.remove_worst();
                continue 'outer;
            };

            allocations.sort_by(|(_, a), (_, b)| b.partial_cmp(a).unwrap());

            if allocations.len() > self.max_stock as usize {
                self.remove_worst();
                continue 'outer;
            };

            for (p, allocation) in allocations.iter() {
                let cash = self.money * allocation.abs();
                if cash < p.close_price {
                    self.blacklist(&p.id);
                    continue 'outer;
                };
            }

            for (p, allocation) in allocations {
                self.data.get_mut(&p.id).unwrap().redp_allocation = allocation;
            }
            let to_remove = self
                .data
                .iter()
                .filter_map(|entry| {
                    let id = entry.key();
                    let entry = entry.value();
                    if entry.redp_allocation == 0.0 {
                        Some(id.clone())
                    } else {
                        None
                    }
                })
                .collect_vec();
            to_remove.iter().for_each(|id| self.blacklist(id));

            break;
        }
    }

    pub fn as_table(&self) -> Table {
        let mut table = Table::new();
        let header = vec![
            "id",
            "name",
            "symbol",
            "allocation",
            "cash",
            "qty",
            "price",
            "sl",
            "sharpe",
            "avg dd",
            "roic",
            "wacc",
            "rsi",
            "redp",
            "class",
        ];
        table.set_header(header);
        table.load_preset(UTF8_BORDERS_ONLY);
        for entry in self
            .data
            .iter()
            .sorted_by(|a, b| b.redp_allocation.partial_cmp(&a.redp_allocation).unwrap())
        {
            let DataEntry {
                product,
                redp_allocation,
                sharpe_ratio,
                redp,
                avg_dd,
                roic,
                wacc,
                rsi,
                ..
            } = entry.value();
            let mode = if *redp_allocation > 0.0 {
                TransactionType::Buy
            } else {
                TransactionType::Sell
            };
            let stop_loss = if mode == TransactionType::Buy {
                product.close_price * (1.0 - (3.0 * avg_dd).min(self.risk))
            } else {
                product.close_price * (1.0 + (3.0 * avg_dd).min(self.risk))
            };
            let cash = self.money * redp_allocation.abs();
            let qty = (cash / product.close_price).round() as i64;
            table.add_row(vec![
                Cell::new(product.id.clone()),
                Cell::new(format!(
                    "{:<24}",
                    product.name.chars().take(24).collect::<String>()
                )),
                Cell::new(product.symbol.clone()),
                Cell::new(format!("{:.2}", redp_allocation)),
                Cell::new(format!("{:.2}", cash)),
                Cell::new(qty.to_string()),
                Cell::new(format!("{:.2}", product.close_price)),
                Cell::new(format!("{:.2}", stop_loss)),
                Cell::new(format!("{:.2}", sharpe_ratio)),
                Cell::new(format!("{:.2}", avg_dd)),
                Cell::new(format!("{:.2}", roic)),
                Cell::new(format!("{:.2}", wacc)),
                Cell::new(format!("{:.2}", rsi)),
                Cell::new(format!("{:.2}", redp)),
                Cell::new(product.category.to_string()),
            ]);
        }

        table
    }
}

#[derive(Debug, Clone)]
pub struct CalculateSl {
    pub n: usize,
}

#[async_trait]
impl Handler<CalculateSl> for Calculator {
    type Response = String;

    type Executor = ConcurrentExecutor;

    async fn handle_message(
        &mut self,
        msg: CalculateSl,
        puppeter: &Puppeter,
    ) -> Result<Self::Response, PuppetError> {
        info!("Calculating stop losses...");
        let portfolio = puppeter.ask::<Degiro, _>(GetPortfolio).await?;
        let mut table = comfy_table::Table::new();
        let header = vec![
            comfy_table::Cell::new("id"),
            comfy_table::Cell::new("name"),
            comfy_table::Cell::new("symbol"),
            comfy_table::Cell::new("date"),
            comfy_table::Cell::new("price"),
            comfy_table::Cell::new("avg dd").set_alignment(comfy_table::CellAlignment::Right),
            comfy_table::Cell::new("stop loss").set_alignment(comfy_table::CellAlignment::Right),
        ];
        table.set_header(header);
        table.load_preset(UTF8_BORDERS_ONLY);
        for position in portfolio.0.iter() {
            if position.inner.size <= 0.0 {
                continue;
            }
            let product = puppeter
                .ask::<Db, _>(ProductQuery::Id(position.inner.id.clone()))
                .await?;
            let candles = puppeter
                .ask::<Db, _>(CandlesQuery::Id(position.inner.id.clone()))
                .await?;
            if let (Some(product), Some(candles)) = (product, candles) {
                if let Some(avg_dd) = candles.average_drawdown(12) {
                    if let Some(Some(avg_dd_value)) = avg_dd.values.last() {
                        let last_price = candles.close.last().unwrap();
                        let stop_loss = last_price * (1.0 - avg_dd_value * msg.n as f64);
                        table.add_row(vec![
                            comfy_table::Cell::new(product.id.clone()),
                            comfy_table::Cell::new(format!(
                                "{:<24}",
                                product.name.chars().take(24).collect::<String>()
                            )),
                            comfy_table::Cell::new(product.symbol.clone()),
                            comfy_table::Cell::new(candles.time.last().unwrap().to_string()),
                            comfy_table::Cell::new(last_price)
                                .set_alignment(comfy_table::CellAlignment::Right),
                            comfy_table::Cell::new(format!("{:.2}", avg_dd_value))
                                .set_alignment(comfy_table::CellAlignment::Right),
                            comfy_table::Cell::new(format!("{:.2}", stop_loss))
                                .set_alignment(comfy_table::CellAlignment::Right),
                        ]);
                    }
                }
            } else {
                eprintln!("Failed to get data for {}", &position.inner.id);
            };
        }
        Ok(table.to_string())
    }
}

#[async_trait]
impl Handler<GetPortfolio> for Calculator {
    type Response = String;

    type Executor = ConcurrentExecutor;

    async fn handle_message(
        &mut self,
        msg: GetPortfolio,
        puppeter: &Puppeter,
    ) -> Result<Self::Response, PuppetError> {
        let portfolio = puppeter.ask::<Degiro, _>(GetPortfolio).await?;
        let mut table = comfy_table::Table::new();
        let header = vec![
            comfy_table::Cell::new("id"),
            comfy_table::Cell::new("name"),
            comfy_table::Cell::new("symbol"),
            comfy_table::Cell::new("size").set_alignment(comfy_table::CellAlignment::Right),
            comfy_table::Cell::new("price").set_alignment(comfy_table::CellAlignment::Right),
            comfy_table::Cell::new("value").set_alignment(comfy_table::CellAlignment::Right),
            comfy_table::Cell::new("profit").set_alignment(comfy_table::CellAlignment::Right),
            comfy_table::Cell::new("%").set_alignment(comfy_table::CellAlignment::Right),
            comfy_table::Cell::new("roic").set_alignment(comfy_table::CellAlignment::Right),
            comfy_table::Cell::new("wacc").set_alignment(comfy_table::CellAlignment::Right),
        ];
        table.set_header(header);
        table.load_preset(UTF8_BORDERS_ONLY);
        for position in portfolio.0.iter() {
            if position.inner.size <= 0.0 {
                continue;
            }
            let product = puppeter
                .ask::<Db, _>(ProductQuery::Id(position.inner.id.clone()))
                .await?;
            let financials = puppeter
                .ask::<Db, _>(FinanclaReportsQuery::Id(position.inner.id.clone()))
                .await?;
            let ratios = puppeter
                .ask::<Db, _>(CompanyRatiosQuery::Id(position.inner.id.clone()))
                .await?;
            if let (Some(product), Some(financials), Some(ratios)) = (product, financials, ratios) {
                let mut row = Vec::new();
                row.push(Cell::new(product.id.clone()));
                row.push(Cell::new(format!(
                    "{:<24}",
                    product.name.chars().take(24).collect::<String>()
                )));
                row.push(Cell::new(product.symbol.clone()));
                row.push(
                    Cell::new(position.inner.size).set_alignment(comfy_table::CellAlignment::Right),
                );
                row.push(
                    Cell::new(product.close_price).set_alignment(comfy_table::CellAlignment::Right),
                );
                row.push(
                    Cell::new(position.inner.value)
                        .set_alignment(comfy_table::CellAlignment::Right),
                );
                row.push(
                    Cell::new(position.inner.total_profit)
                        .set_alignment(comfy_table::CellAlignment::Right),
                );
                let profit_perc = position.inner.total_profit.amount
                    / (position.inner.size * position.inner.break_even_price);
                row.push(
                    Cell::new(format!("{:.2}%", profit_perc * 100.0))
                        .set_alignment(comfy_table::CellAlignment::Right),
                );

                let current_year = chrono::Utc::now().year();
                if let Some(annual_report) = financials.get_annual(current_year - 1) {
                    if let Some(beta) = ratios.current_ratios.beta.value {
                        let roic = annual_report.roic();
                        let capm = annual_report.capm_equity_cost(0.2, 0.05, beta);
                        let wacc = annual_report.wacc(capm);
                        row.push(
                            Cell::new(format!("{:.2}", roic))
                                .set_alignment(comfy_table::CellAlignment::Right),
                        );
                        row.push(
                            Cell::new(format!("{:.2}", wacc))
                                .set_alignment(comfy_table::CellAlignment::Right),
                        );
                    }
                }

                table.add_row(row);
            } else {
                eprintln!("Failed to get data for {}", &position.inner.id);
            };
        }
        Ok(table.to_string())
    }
}
