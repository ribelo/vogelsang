use crate::api::product::Product;
use crate::{
    client::{Client, ClientMsg},
    AllowedOrderTypes, OrderTimeTypes, ProductCategory,
};
use async_recursion::async_recursion;
use chrono::NaiveDate;
use color_eyre::{eyre::eyre, Result};
use derivative::Derivative;
use reqwest::{header, Url};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;
use tokio::time::Duration;

#[allow(dead_code)]
#[derive(Debug)]
pub struct QueryBuilder<'a> {
    query: String,
    symbol: Option<String>,
    limit: u32,
    offset: u32,
    client: Arc<&'a Client>,
}

#[derive(Deserialize, Derivative, Clone)]
#[derivative(Debug)]
#[serde(rename_all = "camelCase")]
pub struct QueryProduct {
    pub active: bool,
    pub buy_order_types: AllowedOrderTypes,
    pub category: ProductCategory,
    pub close_price: Option<f64>,
    pub close_price_date: Option<NaiveDate>,
    pub contract_size: f64,
    pub exchange_id: String,
    pub feed_quality: Option<String>,
    pub feed_quality_secondary: Option<String>,
    pub id: String,
    pub isin: String,
    pub name: String,
    pub only_eod_prices: bool,
    pub order_book_depth: Option<i32>,
    pub order_book_depth_secondary: Option<i32>,
    pub order_time_types: OrderTimeTypes,
    pub product_bit_types: Vec<String>,
    pub product_type: String,
    pub product_type_id: i32,
    pub quality_switch_free: Option<bool>,
    pub quality_switch_free_secondary: Option<bool>,
    pub quality_switchable: Option<bool>,
    pub quality_switchable_secondary: Option<bool>,
    pub sell_order_types: AllowedOrderTypes,
    pub symbol: String,
    pub tradable: bool,
    #[serde(skip)]
    pub(crate) client_tx: Option<Sender<ClientMsg>>,
}

impl QueryBuilder<'_> {
    pub fn query(&mut self, query: &str) -> &mut Self {
        self.query = query.to_uppercase();
        self
    }
    pub fn symbol(&mut self, symbol: &str) -> &mut Self {
        self.symbol = Some(symbol.to_uppercase());
        self
    }
    pub fn limit(&mut self, limit: u32) -> &mut Self {
        self.limit = limit;
        self
    }
    pub fn offset(&mut self, offset: u32) -> &mut Self {
        self.offset = offset;
        self
    }
    #[async_recursion]
    pub async fn send(&self) -> Result<Vec<QueryProduct>> {
        let client = &self.client.clone();
        match (
            &client.session_id,
            &client.account,
            &client.paths.products_search_url,
        ) {
            (Some(session_id), Some(account), Some(products_search_url)) => {
                let url = Url::parse(products_search_url)?
                    .join(products_search_url)?
                    .join("v5/products/lookup")?;
                let req = client
                    .http_client
                    .get(url)
                    .query(&[
                        ("intAccount", &account.int_account.to_string()),
                        ("sessionId", session_id),
                        ("searchText", &self.query),
                        ("limit", &self.limit.to_string()),
                        ("offset", &self.offset.to_string()),
                    ])
                    .header(header::REFERER, &client.paths.referer);
                let res = req.send().await.unwrap();
                match res.error_for_status() {
                    Ok(res) => {
                        let mut body = res.json::<Value>().await?;
                        if let Some(products) = body.get_mut("products") {
                            let mut products =
                                serde_json::from_value::<Vec<QueryProduct>>(products.take())?;
                            for mut p in products.iter_mut() {
                                p.client_tx = Some(client.tx.clone());
                            }
                            if let Some(symbol) = &self.symbol {
                                Ok(products
                                    .into_iter()
                                    .filter(|p| p.symbol == symbol.to_uppercase())
                                    .collect())
                            } else {
                                Ok(products)
                            }
                        } else {
                            Err(eyre!("products is empty"))
                        }
                    }
                    Err(err) => match err.status().unwrap().as_u16() {
                        401 => {
                            client.login().await?;
                            self.send().await
                        }
                        _ => Err(eyre!(err)),
                    },
                }
            }
            (None, _, _) => self.send().await,
            (Some(_), None, _) | (Some(_), _, None) => {
                client
                    .fetch_account_data()
                    .await?
                    .fetch_account_info()
                    .await?;
                self.send().await
            }
        }
    }
}

impl Client {
    pub fn search(&self) -> QueryBuilder {
        QueryBuilder {
            query: Default::default(),
            symbol: None,
            limit: 1,
            offset: 0,
            client: Arc::new(self),
        }
    }
}

impl QueryProduct {
    pub async fn product(&self) -> Result<Arc<Product>> {
        let (tx, rx) = oneshot::channel::<Result<Arc<Product>>>();
        self.client_tx
            .as_ref()
            .expect("channel don't exits")
            .send_timeout(
                ClientMsg::GetProduct {
                    id: self.id.clone(),
                    tx: Some(tx),
                },
                Duration::from_secs(10),
            )
            .await?;
        rx.await?
    }
}

#[cfg(test)]
mod test {
    use crate::client::ClientBuilder;

    #[tokio::test]
    async fn search() {
        let username = std::env::args().nth(2).expect("no username given");
        let password = std::env::args().nth(3).expect("no password given");
        let mut builder = ClientBuilder::default();
        let client = builder
            .username(&username)
            .password(&password)
            .build()
            .unwrap();
        let products = client
            .search()
            .query("CA8849037095")
            .limit(10)
            .symbol("TRI")
            .send()
            .await
            .unwrap();
        dbg!(products.first().unwrap());
    }
}
