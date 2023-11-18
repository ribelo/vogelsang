use std::sync::{Arc, Mutex};

use crate::{portfolio::SingleAllocation, settings::Settings};
use comfy_table::{presets::UTF8_BORDERS_ONLY, CellAlignment, Table};
use degiro_rs::{
    api::{account::AccountConfigExt, login::Authorize, product::ProductExt},
    client::Client,
};
use anyhow::Result;
use erfurt::candle::Candles;
use futures::future;
use itertools::Itertools;
use qualsdorf::{sharpe_ratio::SharpeRatioExt, Value, sortino_ratio::SortinoRatioExt, rolling_economic_drawdown::RollingEconomicDrawdownExt, maximum_drawdown::MaximumDrawdownExt, average_drawdown::AverageDrawdownExt};
use tokio::task::JoinHandle;

struct TableRow {
    id: String,
    name: String,
    sharpe_ratio: f64,
    sortino_ratio: f64,
    max_dd: f64,
    avg_dd: f64,
    redp: f64,
    allocation: f64,
}

pub async fn run(settings: &Settings) -> Result<()> {
    let settings = Arc::new(settings.to_owned());
    let client = Arc::new(
        Client::new_from_env()
            .login()
            .await?
            .account_config()
            .await?,
    );
    let mut table = Table::new();
    table.load_preset(UTF8_BORDERS_ONLY);
    table.set_header(vec!["id", "name", "sharpe", "sortino", "max dd", 
                     "avg dd",
                     "redp", "allocation"]);
    let rows: Arc<Mutex<Vec<TableRow>>> = Arc::new(Mutex::new(Vec::new()));
    let mut tasks: Vec<JoinHandle<()>> = Vec::new();

    for (id, name) in settings.assets.iter() {
        println!("Fetching product {} {}", &id, &name);
        let id = id.clone();
        let name = name.clone();
        let client = client.clone();
        let rows = rows.clone();
        let settings = settings.clone();
        let freq = settings.period.div(&settings.interval);
        let task = tokio::spawn(async move {
            let Ok(product) = client.product(&id).await else {
                return println!("Could not fetch product {} {}", &id, &name);
            };
            let Ok(quotes) = product.quotes(
                &settings.period,
                &settings.interval
                ).await else {
                return println!("Could not fetch quotes for {} {}", &id, &name);
            };
            let candles: Candles = quotes.into();
            
            let sharpe_ratio = candles.sharpe_ratio(freq, settings.risk_free)
                .map_or(0.0, |x| *x.value().unwrap_or(&0.0));

            let sortino_ratio = candles.sortino_ratio(freq, settings.risk_free, 0.0)
                .map_or(0.0, |x| *x.value().unwrap_or(&0.0));

            let max_dd = candles.maximum_drawdown(freq)
                .map_or(0.0, |x| *x.value().unwrap_or(&0.0));

            let avg_dd = candles.average_drawdown(freq)
                .map_or(0.0, |x| *x.value().unwrap_or(&0.0));

            let redp = candles.rolling_economic_drawndown(freq)
                .map_or(0.0, |x| *x.value().unwrap_or(&0.0));


            let Ok(allocation) = candles 
                .single_allocation(
                    settings.risk,
                    settings.risk_free,
                    &settings.period,
                    &settings.interval
                )
                .await else {
                    return println!("Could not calculate single allocation for {} {}", &id, &name);
                };

            let row = TableRow {
                id: id.to_string(),
                name: name.to_string(),
                sharpe_ratio,
                sortino_ratio,
                max_dd,
                avg_dd,
                redp,
                allocation,
            };
            rows.lock().unwrap().push(row);
        });
        tasks.push(task);
    }
    future::join_all(tasks).await;
    rows.lock().unwrap().iter().sorted_by(|a, b| {
        b.sharpe_ratio
            .partial_cmp(&a.sharpe_ratio)
            .unwrap_or(std::cmp::Ordering::Equal)
    }).for_each(|row| {
        table.add_row(vec![
            row.id.clone(),
            row.name.clone(),
            format!("{:.2}", row.sharpe_ratio),
            format!("{:.2}", row.sortino_ratio),
            format!("{:.2}", row.max_dd),
            format!("{:.2}", row.avg_dd),
            format!("{:.2}", row.redp),
            format!("{:.2}", row.allocation),
        ]);
    });
    for column in table.column_iter_mut() {
        if column.index > 1 {
            column.set_cell_alignment(CellAlignment::Right)
        }
    }
    println!("{}", table);
    Ok(())
}
