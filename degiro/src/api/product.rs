use std::{collections::HashMap, fmt::Debug, rc::Weak, sync::Arc, sync::Weak};

use async_recursion::async_recursion;
use chrono::NaiveDate;
use color_eyre::{eyre::eyre, Result};
use dashmap::DashMap;
use derivative::Derivative;
use erfurt::candle::Candles;
use reqwest::{header, Url};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::{client::SharedClient, client::Client, AllowedOrderTypes, OrderTimeTypes, Period, ProductCategory};

#[derive(Deserialize, Derivative, Clone)]
#[derivative(Debug)]
#[serde(rename_all = "camelCase")]
pub struct Product {
    pub active: bool,
    pub buy_order_types: AllowedOrderTypes,
    pub category: ProductCategory,
    pub close_price: f64,
    pub close_price_date: NaiveDate,
    pub contract_size: f64,
    pub exchange_id: String,
    pub feed_quality: Option<String>,
    pub feed_quality_secondary: Option<String>,
    pub id: String,
    pub isin: String,
    pub name: String,
    pub only_eod_prices: bool,
    pub order_book_depth: i32,
    pub order_book_depth_secondary: Option<i32>,
    pub order_time_types: OrderTimeTypes,
    pub product_bit_types: Vec<String>,
    pub product_type: String,
    pub product_type_id: i32,
    pub quality_switch_free: bool,
    pub quality_switch_free_secondary: Option<bool>,
    pub quality_switchable: bool,
    pub quality_switchable_secondary: Option<bool>,
    pub sell_order_types: AllowedOrderTypes,
    pub symbol: String,
    pub tradable: bool,
    pub vwd_id: String,
    pub vwd_id_secondary: Option<String>,
    pub vwd_identifier_type: String,
    pub vwd_identifier_type_secondary: Option<String>,
    pub vwd_module_id: i32,
    pub vwd_module_id_secondary: Option<i32>,
    #[serde(skip)]
    #[derivative(Debug = "ignore")]
    pub(crate) client: Option<Weak<Client>>,
}

impl SharedClient {
    #[async_recursion]
    pub async fn fetch_products<T>(&self, ids: T) -> Result<()>
    where
        T: Serialize + Sized + Send + Debug + Sync,
    {
        let inner = self.inner.try_lock().unwrap();
        match (
            &inner.session_id,
            &inner.account,
            &inner.paths.products_search_url,
        ) {
            (Some(session_id), Some(account), Some(products_search_url)) => {
                let url = Url::parse(products_search_url)?
                    .join(products_search_url)?
                    .join("v5/products/info")?;
                let req = inner
                    .http_client
                    .post(url)
                    .query(&[
                        ("intAccount", account.int_account.to_string()),
                        ("sessionId", session_id.to_string()),
                    ])
                    .json(&ids)
                    .header(header::REFERER, &inner.paths.referer);
                let res = req.send().await.unwrap();
                match res.error_for_status() {
                    Ok(res) => {
                        let mut body = res
                            .json::<HashMap<String, HashMap<String, Product>>>()
                            .await?;
                        let m = body.remove("data").ok_or(eyre!("data key not found"))?;
                        for (k, mut v) in m.into_iter() {
                            v.client = Some(self.clone());
                            self.products_cache.insert(k, Arc::new(v));
                        }
                        Ok(())
                    }
                    Err(err) => Err(eyre!(err)),
                }
            }
            (None, _, _) => {
                drop(inner);
                self.login().await?.fetch_products(ids).await
            }
            (Some(_), None, _) => {
                drop(inner);
                self.fetch_account_data().await?.fetch_products(ids).await
            }
            (Some(_), Some(_), None) => {
                drop(inner);
                self.fetch_account_config().await?.fetch_products(ids).await
            }
        }
    }
    #[async_recursion]
    pub async fn product_by_id(&self, id: &str) -> Result<Arc<Product>> {
        if let Some(product) = self.products_cache.get(id).as_deref() {
            Ok(product.clone())
        } else {
            self.fetch_products(&[id]).await?;
            self.product_by_id(id).await
        }
    }
    pub async fn product_by_symbol(&self, symbol: &str) -> Result<Arc<Product>> {
        let mut query_products = self
            .search()
            .query(symbol)
            .symbol(symbol)
            .limit(10)
            .send()
            .await?;
        if let Some(query_product) = query_products.pop() {
            let product = query_product.product().await?;
            let id = product.id.clone();
            self.products_cache.insert(product.id.clone(), product);
            self.product_by_id(&id).await
        } else {
            Err(eyre!("can't find product {}", symbol))
        }
    }
}

impl Product {
    pub async fn candles(&self, period: &Period, interval: &Period) -> Result<Candles> {
        if let Some(quotes) = self
            .quotes
            .upgrade()
            .ok_or_else(|| eyre!("can't upgrade quotes"))?
            .get(&(period.clone(), interval.clone()))
            .as_deref()
        {
            Ok(quotes.clone())
        } else {
            let quotes = self
                .client
                .as_ref()
                .unwrap()
                .quotes(&self.id, period, interval)
                .await?;

            self.quotes
                .upgrade()
                .ok_or_else(|| eyre!("can't upgrade quotes"))?
                .insert((period.clone(), interval.clone()), quotes.clone());

            Ok(quotes)
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{client::ClientBuilder, Period};

    #[tokio::test]
    async fn product_ids() {
        let username = std::env::args().nth(2).expect("no username given");
        let password = std::env::args().nth(3).expect("no password given");
        let mut builder = ClientBuilder::default();
        let client = builder
            .username(&username)
            .password(&password)
            .build()
            .unwrap();
        client.fetch_products(&["17461000"]).await.unwrap();
    }
    #[tokio::test]
    async fn product_one_id() {
        let username = std::env::args().nth(2).expect("no username given");
        let password = std::env::args().nth(3).expect("no password given");
        let mut builder = ClientBuilder::default();
        let client = builder
            .username(&username)
            .password(&password)
            .build()
            .unwrap();
        let product = client.product_by_id("17461000").await.unwrap();
        dbg!(product);
    }
    #[tokio::test]
    async fn product_candles() {
        let username = std::env::args().nth(2).expect("no username given");
        let password = std::env::args().nth(3).expect("no password given");
        let mut builder = ClientBuilder::default();
        let client = builder
            .username(&username)
            .password(&password)
            .build()
            .unwrap();
        let product = client.product_by_symbol("msft").await.unwrap();
        let candles = product.candles(&Period::P1Y, &Period::P1M).await.unwrap();
        dbg!(candles);
    }
}
