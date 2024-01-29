use std::sync::Arc;

use dashmap::DashMap;
use degiro_rs::{
    api::product::Product,
    util::{Period, ProductCategory, TransactionType},
};
use erfurt::prelude::*;
use itertools::Itertools;
use qualsdorf::{
    average_drawdown::AverageDrawdownExt, rolling_economic_drawdown::RollingEconomicDrawdownExt,
    rsi::RsiExt, sharpe_ratio::SharpeRatioExt, Indicator, Value,
};

use crate::{
    portfolio::{AssetsSeq, RiskMode, SingleAllocation},
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

// impl<'a> App {
//     pub async fn portfolio_calculator(
//         self,
//         mode: RiskMode,
//         risk: f64,
//         risk_free: f64,
//         freq: u32,
//         money: f64,
//         max_stocks: i32,
//         min_rsi: Option<f64>,
//         max_rsi: Option<f64>,
//         min_class: Option<ProductCategory>,
//         max_class: Option<ProductCategory>,
//         short_sales_constraint: bool,
//     ) -> PortfolioCalculator {
//         let data = Arc::new(DashMap::new());
//         let mut set = tokio::task::JoinSet::new();
//         for (id, _) in self.settings.assets.iter() {
//             let id = id.clone();
//             let stocks = Arc::clone(&data);
//             let candles_handler = self.candles_handler(&id);
//             let product_handler = self.product_handler(&id);
//             set.spawn(async move {
//                 let candles = candles_handler.take().await;
//                 let product = product_handler.take().await;
//                 match (candles, product) {
//                     (Ok(candles), Ok(product)) => {
//                         if candles.time.len() >= freq as usize {
//                             let candles = candles.take_last(freq as usize).unwrap();
//                             let single_allocation = candles
//                                 .single_allocation(
//                                     RiskMode::STD,
//                                     risk,
//                                     risk_free,
//                                     Period::P1Y,
//                                     Period::P1M,
//                                 )
//                                 .await
//                                 .unwrap();
//                             let sharpe_ratio = *candles
//                                 .sharpe_ratio(freq as usize, risk_free)
//                                 .unwrap()
//                                 .last()
//                                 .unwrap();
//                             let avg_dd = *candles
//                                 .average_drawdown(freq as usize)
//                                 .unwrap()
//                                 .last()
//                                 .unwrap();
//                             let rsi = *candles.rsi(freq as usize).unwrap().last().unwrap();
//                             let redp = *candles
//                                 .rolling_economic_drawndown(freq as usize)
//                                 .unwrap()
//                                 .last()
//                                 .unwrap();
//                             let entry = DataEntry {
//                                 product,
//                                 candles,
//                                 single_allocation,
//                                 redp_allocation: 0.0,
//                                 sharpe_ratio,
//                                 avg_dd,
//                                 rsi,
//                                 redp,
//                             };
//                             stocks.insert(id, entry);
//                         } else {
//                             println!("Not enough data for {}", &id);
//                         }
//                     }
//                     (Ok(_), Err(_)) => {
//                         println!("Cannot fetch product data for {}", &id);
//                     }
//                     (Err(_), Ok(_)) => {
//                         println!("Cannot fetch candles data for {}", &id);
//                     }
//                     (Err(_), Err(_)) => {
//                         println!("Cannot fetch candles and product data for {}", &id);
//                     }
//                 }
//             });
//         }
//
//         while (set.join_next().await).is_some() {}
//         PortfolioCalculator {
//             mode,
//             freq,
//             risk,
//             risk_free,
//             money,
//             max_stock: max_stocks,
//             min_rsi,
//             max_rsi,
//             min_class,
//             max_class,
//             short_sales_constraint,
//             data,
//         }
//     }
// }
