use std::sync::Arc;

use async_recursion::async_recursion;
use chrono::{prelude::*, Duration};
use color_eyre::{eyre::eyre, Result};
use erfurt::candle::Candles;
use reqwest::{header, Url};
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::oneshot;

use crate::{
    client::{Client, ClientMsg},
    Period,
};

use super::product::Product;

#[derive(Debug, Deserialize)]
struct Quotes(Vec<Ohlc>);

// #[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct Ohlc {
    n: u64,
    o: f64,
    h: f64,
    l: f64,
    c: f64,
}

impl Quotes {
    fn as_candles(&self, symbol: &str, start: DateTime<Utc>, interval: &Period) -> Result<Candles> {
        let mut candles = Candles {
            symbol: symbol.to_uppercase(),
            ..Default::default()
        };
        for x in self.0.iter() {
            let shift = Duration::milliseconds((interval.to_ms() * x.n) as i64);
            let dt = start
                .checked_add_signed(shift)
                .ok_or(eyre!("can't shift datetime"))?;
            candles.time.push(dt);
            candles.open.push(x.o);
            candles.high.push(x.h);
            candles.low.push(x.l);
            candles.close.push(x.c);
        }
        Ok(candles)
    }
}

impl Client {
    #[async_recursion]
    pub async fn quotes(&self, id: &str, period: &Period, interval: &Period) -> Result<Arc<Candles>> {
        let product = self.product_by_id(id).await?;

        match self.client_id {
            Some(client_id) => {
                let url = Url::parse(&self.paths.price_data_url)?;
                let req = self
                    .http_client
                    .get(url)
                    .query(&[
                        ("requestid", 1.to_string()),
                        ("format", "json".to_string()),
                        ("resolution", interval.to_string()),
                        ("period", period.to_string()),
                        ("series", format!("ohlc:issueid:{}", &product.vwd_id)),
                        ("userToken", client_id.to_string()),
                    ])
                    .header(header::REFERER, &self.paths.referer);
                let res = req.send().await.unwrap();
                match res {
                    res if res.status().is_success() => {
                        let mut json = res.json::<Value>().await?;
                        let v = json
                            .get_mut("start")
                            .ok_or(eyre!("can't get start value"))?;
                        let start = serde_json::from_value::<NaiveDateTime>(v.take())?;
                        let start: DateTime<Utc> = DateTime::from_utc(start, Utc);
                        let series = json.get_mut("series").ok_or(eyre!("can't get series"))?;
                        let arr = series.as_array().ok_or(eyre!("value is not array"))?;
                        let obj = arr.first().ok_or(eyre!("can't get first elem"))?;
                        let data = obj.get("data").ok_or(eyre!("can't get data"))?;
                        let quotes = serde_json::from_value::<Quotes>(data.clone())?;
                        let candles = quotes.as_candles(&product.symbol, start, interval)?;
                        let candles = Arc::new(candles);
                        self.quotes_cache.insert(
                            (id.to_string(), period.clone(), interval.clone()),
                            candles,
                        );
                        Ok(candles)
                    }
                    res if res.status().as_u16() == 401 => {
                        let candles = self.login().await?.quotes(id, period, interval).await?;
                        Ok(candles)
                    }
                    res => Err(eyre!(res.error_for_status_ref().unwrap_err())),
                }
            }
            None => {
                self.fetch_account_config()
                    .await?
                    .quotes(id, period, interval)
                    .await
            }
        }
    }
}

impl Product {
    async fn quotes(&self, period: &Period, interval: &Period) -> Result<Arc<Candles>> {
        let (tx, rx) = oneshot::channel::<Result<Arc<Candles>>>();
        &self.client_tx.as_ref()
            .expect("channel don't exists")
            .send_timeout(
                ClientMsg::GetCandles {
                    id: self.id.clone(),
                    period: period.clone(),
                    interval: interval.clone(),
                    tx: Some(tx),
                },
                tokio::time::Duration::from_secs(10),
            )
            .await?;
        rx.await?
    }
}

#[cfg(test)]
mod test {
    use crate::{client::ClientBuilder, Period};

    #[tokio::test]
    async fn quotes() {
        let username = std::env::args().nth(2).expect("no username given");
        let password = std::env::args().nth(3).expect("no password given");
        let mut builder = ClientBuilder::default();
        let client = builder
            .username(&username)
            .password(&password)
            .build()
            .unwrap();
        let product = client.product_by_symbol("msft").await.unwrap();
        let x = product
            .quotes(&Period::P1Y, &Period::P1M)
            .await
            .unwrap()
            .last()
            .unwrap();
        dbg!(x);
    }
}
