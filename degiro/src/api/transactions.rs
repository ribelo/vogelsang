use async_recursion::async_recursion;
use chrono::prelude::*;
use color_eyre::{eyre::eyre, Result};
use reqwest::{header, Url};
use serde::Deserialize;

use std::collections::HashMap;

use crate::client::SharedClient;
use crate::TransactionType;

#[derive(Debug, Deserialize)]
pub struct Transactions(Vec<Transaction>);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    pub auto_fx_fee_in_base_currency: f64,
    #[serde(rename ="buysell")]
    pub transaction_type: TransactionType,
    pub counter_party: Option<String>,
    pub date: DateTime<FixedOffset>,
    pub fee_in_base_currency: Option<f64>,
    pub fx_rate: f64,
    pub gross_fx_rate: f64,
    pub id: i32,
    pub nett_fx_rate: f64,
    pub order_type_id: Option<i8>,
    pub price: f64,
    pub product_id: i32,
    pub quantity: i32,
    pub total: f64,
    pub total_fees_in_base_currency: f64,
    pub total_in_base_currency: f64,
    pub total_plus_all_fees_in_base_currency: f64,
    pub total_plus_fee_in_base_currency: f64,
    pub trading_venue: Option<String>,
    pub transaction_type_id: i32,
    pub transfered: bool,
}

impl SharedClient {
    #[async_recursion]
    pub async fn transactions(
        &self,
        from_date: NaiveDate,
        to_date: NaiveDate,
    ) -> Result<Transactions> {
        let inner = self.inner.try_lock().unwrap();
        match (
            &inner.session_id,
            &inner.account,
            &inner.paths.reporting_url,
        ) {
            (Some(session_id), Some(account), Some(reporting_url)) => {
                let url = Url::parse(reporting_url)?.join(&inner.paths.transactions_path)?;
                let req = inner
                    .http_client
                    .get(url)
                    .query(&[
                        ("sessionId", session_id),
                        ("intAccount", &format!("{}", account.int_account)),
                        ("fromDate", &from_date.format("%d/%m/%Y").to_string()),
                        ("toDate", &to_date.format("%d/%m/%Y").to_string()),
                        ("groupTransactionsByOrder", &"1".to_string()),
                    ])
                    .header(header::REFERER, &inner.paths.referer);
                let res = req.send().await.unwrap();
                match res.error_for_status() {
                    Ok(res) => {
                        let mut m = res.json::<HashMap<String, Transactions>>().await?;
                        if let Some(data) = m.remove("data") {
                            Ok(data)
                        } else {
                            Err(eyre!("data key not found"))
                        }
                    }
                    Err(err) => match err.status().unwrap().as_u16() {
                        401 => {
                            drop(inner);
                            self.login().await?.transactions(from_date, to_date).await
                        }
                        _ => Err(eyre!(err)),
                    },
                }
            }
            (None, _, _) => {
                drop(inner);
                self.login().await?.transactions(from_date, to_date).await
            }
            (Some(_), _, _) => {
                drop(inner);
                self.login()
                    .await?
                    .fetch_account_data()
                    .await?
                    .transactions(from_date, to_date)
                    .await
            }
        }
    }
}

#[cfg(test)]
mod test {
    use chrono::NaiveDate;

    use crate::client::ClientBuilder;

    #[tokio::test]
    async fn transactions() {
        let username = std::env::args().nth(2).expect("no username given");
        let password = std::env::args().nth(3).expect("no password given");
        let mut builder = ClientBuilder::default();
        let client = builder
            .username(&username)
            .password(&password)
            .build()
            .unwrap();
        let transactions = client
            .transactions(
                NaiveDate::from_ymd(2021, 1, 1),
                NaiveDate::from_ymd(2022, 12, 31),
            )
            .await
            .unwrap();
        dbg!(transactions);
    }
}
