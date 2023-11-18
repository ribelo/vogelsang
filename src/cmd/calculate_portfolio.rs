use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    ops::Deref,
    sync::{Arc, Mutex},
};

use anyhow::Result;
use chrono::Datelike;
use comfy_table::{presets::UTF8_BORDERS_ONLY, Cell, CellAlignment, Color, Table};
use dashmap::{DashMap, DashSet};
use degiro_rs::{
    api::{
        account::AccountConfigExt,
        login::Authorize,
        product::{Product, ProductExt},
    },
    client::{
        client_status::{self, Authorized},
        Client,
    },
    util::{Period, ProductCategory, TransactionType},
};
use erfurt::prelude::*;
use futures::future;
use itertools::Itertools;
use qualsdorf::{
    average_drawdown::AverageDrawdownExt, rolling_economic_drawdown::RollingEconomicDrawdownExt,
    rsi::RsiExt, sharpe_ratio::SharpeRatioExt, Indicator, Value,
};

use crate::{
    data::candles::{self, CandlesHandler},
    portfolio::{AssetsSeq, RiskMode, SingleAllocation},
    prelude::*,
    settings::Settings,
    App,
};

struct TableRow {
    id: String,
    name: String,
    symbol: String,
    allocation: f64,
    cash: f64,
    qty: i64,
    price: f64,
    stop_loss: f64,
    avg_dd: f64,
    rsi: f64,
    redp: f64,
    category: ProductCategory,
}

#[derive(Debug)]
pub struct DataEntry {
    product: Product,
    candles: Candles,
    single_allocation: f64,
    redp_allocation: f64,
    sharpe_ratio: f64,
    redp: f64,
    avg_dd: f64,
    rsi: f64,
}

pub struct PortfolioCalculator {
    mode: RiskMode,
    freq: u32,
    risk: f64,
    risk_free: f64,
    money: f64,
    max_stock: i32,
    min_rsi: Option<f64>,
    max_rsi: Option<f64>,
    min_class: Option<ProductCategory>,
    max_class: Option<ProductCategory>,
    short_sales_constraint: bool,
    pub data: Arc<DashMap<String, DataEntry>>,
}

impl App<degiro_rs::client::client_status::Authorized> {
    pub async fn portfolio_calculator(
        self,
        mode: RiskMode,
        risk: f64,
        risk_free: f64,
        freq: u32,
        money: f64,
        max_stock: i32,
        min_rsi: Option<f64>,
        max_rsi: Option<f64>,
        min_class: Option<ProductCategory>,
        max_class: Option<ProductCategory>,
        short_sales_constraint: bool,
    ) -> PortfolioCalculator {
        let data = Arc::new(DashMap::new());
        let mut set = tokio::task::JoinSet::new();
        for (id, _) in self.settings.assets.iter() {
            let id = id.clone();
            let stocks = Arc::clone(&data);
            let candles_handler = self.candles_handler(&id);
            let product_handler = self.product_handler(&id);
            set.spawn(async move {
                let candles = candles_handler.take().await;
                let product = product_handler.take().await;
                match (candles, product) {
                    (Ok(candles), Ok(product)) => {
                        if candles.time.len() >= freq as usize {
                            let candles = candles.take_last(freq as usize).unwrap();
                            let single_allocation = candles
                                .single_allocation(
                                    RiskMode::STD,
                                    risk,
                                    risk_free,
                                    &Period::P1Y,
                                    &Period::P1M,
                                )
                                .await
                                .unwrap();
                            let sharpe_ratio = *candles
                                .sharpe_ratio(freq as usize, risk_free)
                                .unwrap()
                                .last()
                                .unwrap();
                            let avg_dd = *candles
                                .average_drawdown(freq as usize)
                                .unwrap()
                                .last()
                                .unwrap();
                            let rsi = *candles.rsi(freq as usize).unwrap().last().unwrap();
                            let redp = *candles
                                .rolling_economic_drawndown(freq as usize)
                                .unwrap()
                                .last()
                                .unwrap();
                            let entry = DataEntry {
                                product,
                                candles,
                                single_allocation,
                                redp_allocation: 0.0,
                                sharpe_ratio,
                                avg_dd,
                                rsi,
                                redp,
                            };
                            stocks.insert(id, entry);
                        } else {
                            println!("Not enough data for {}", &id);
                        }
                    }
                    (Ok(_), Err(_)) => {
                        println!("Cannot fetch product data for {}", &id);
                    }
                    (Err(_), Ok(_)) => {
                        println!("Cannot fetch candles data for {}", &id);
                    }
                    (Err(_), Err(_)) => {
                        println!("Cannot fetch candles and product data for {}", &id);
                    }
                }
            });
        }

        while (set.join_next().await).is_some() {}
        PortfolioCalculator {
            mode,
            freq,
            risk,
            risk_free,
            money,
            max_stock,
            min_rsi,
            max_rsi,
            min_class,
            max_class,
            short_sales_constraint,
            data,
        }
    }
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
        let min_rsi = self.min_rsi.unwrap_or(0.0);
        let max_rsi = self.max_rsi.unwrap_or(100.0);
        for entry in self.data.iter() {
            let id = entry.key();
            let DataEntry {
                candles,
                product,
                single_allocation,
                rsi,
                ..
            } = entry.value();
            let last_candle_month = candles.time.last().unwrap().month();

            if last_candle_month != max_time_month {
                println!(
                    "Data is not up to date for {:>10} : {:<24.24} - last candle month: {} max month: {}",
                    id,
                    product.inner.name,
                    last_candle_month,
                    &max_time_month,
                );
                to_remove.insert(id.clone());
            } else if self.min_rsi.is_some() || self.max_rsi.is_some() {
                if *rsi < min_rsi || *rsi > max_rsi {
                    println!("RSI is out of range for {} : {}", id, product.inner.name);
                    println!("Should be: {} < {} < {}", min_rsi, rsi, max_rsi);
                    to_remove.insert(id.clone());
                }
            } else if product.inner.close_price > self.money
                || (*single_allocation < 1.0 && self.short_sales_constraint)
            {
                to_remove.insert(id.clone());
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
                .sorted_by_cached_key(|(p, _c)| p.inner.id.clone())
                .collect_vec();

            let seq = AssetsSeq(stocks);
            let Ok(mut allocations) = seq
                .redp_multiple_allocation(
                    self.mode.clone(),
                    self.risk,
                    self.risk_free,
                    &Period::P1Y,
                    &Period::P1M,
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
                if cash < p.inner.close_price {
                    self.blacklist(&p.inner.id);
                    continue 'outer;
                };
            }

            for (p, allocation) in allocations {
                self.data.get_mut(&p.inner.id).unwrap().redp_allocation = allocation;
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
                rsi,
                ..
            } = entry.value();
            let mode = if *redp_allocation > 0.0 {
                TransactionType::Buy
            } else {
                TransactionType::Sell
            };
            let stop_loss = if mode == TransactionType::Buy {
                product.inner.close_price * (1.0 - (3.0 * avg_dd).min(self.risk))
            } else {
                product.inner.close_price * (1.0 + (3.0 * avg_dd).min(self.risk))
            };
            let cash = self.money * redp_allocation.abs();
            let qty = (cash / product.inner.close_price).round() as i64;
            table.add_row(vec![
                Cell::new(product.inner.id.clone()),
                Cell::new(format!(
                    "{:<24}",
                    product.inner.name.chars().take(24).collect::<String>()
                )),
                Cell::new(product.inner.symbol.clone()),
                Cell::new(format!("{:.2}", redp_allocation)),
                Cell::new(format!("{:.2}", cash)),
                Cell::new(qty.to_string()),
                Cell::new(format!("{:.2}", product.inner.close_price)),
                Cell::new(format!("{:.2}", stop_loss)),
                Cell::new(format!("{:.2}", sharpe_ratio)),
                Cell::new(format!("{:.2}", avg_dd)),
                Cell::new(format!("{:.2}", rsi)),
                Cell::new(format!("{:.2}", redp)),
                Cell::new(product.inner.category.to_string()),
            ]);
        }

        table
    }
}
