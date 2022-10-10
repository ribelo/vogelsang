use async_recursion::async_recursion;
use chrono::prelude::*;
use color_eyre::{eyre::eyre, Result};
use reqwest::{header, Url};
use serde::Deserialize;

use std::collections::HashMap;

use crate::client::SharedClient;

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CashMovement {
    balance: Balance,
    change: f64,
    currency: String,
    date: DateTime<FixedOffset>,
    #[serde(rename ="description")]
    movement_type: CashMovementType,
    id: i32,
    order_id: Option<String>,
    product_id: Option<i32>,
    #[serde(rename ="type")]
    transaction_type: TransactionType,
    value_date: DateTime<FixedOffset>,
}

#[derive(Debug, Deserialize)]
#[serde(from = "String")]
pub enum CashMovementType {
    Dividend(String),
    FxWithdrawal(String),
    DividentFee(String),
    FxCredit(String),
    Interest(String),
    BankWithdrawal(String),
    Deposit(String),
    TransactionFee(String),
    TransactionSell(String),
    TransactionBuy(String),
    UnknownFee(String),
    UnknownInteres(String),
    Unknown(String),
}

#[derive(Debug, Deserialize)]
pub enum TransactionType {
    #[serde(rename ="CASH_TRANSACTION")]
    Cash,
    #[serde(rename ="TRANSACTION")]
    NoCash,
    #[serde(rename ="CASH_FUND_TRANSACTION")]
    Fund,
    #[serde(rename ="PAYMENT")]
    Payment,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Balance {
    cash_fund: Option<Vec<CashFund>>,
    total: f64,
    unsettled_cash: f64,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct CashFund {
    id: i32,
    participation: f64,
    price: f64,
}

pub struct ParseMovementTypeError;

impl From<String> for CashMovementType {
    fn from(s: String) -> Self {
        if s == "Dywidenda" {
            CashMovementType::Dividend(s)
        } else if s == "FX Withdrawal" {
            CashMovementType::FxWithdrawal(s)
        } else if s == "Podatek Dywidendowy" {
            CashMovementType::DividentFee(s)
        } else if s == "FX Credit" {
            CashMovementType::FxCredit(s)
        } else if s == "Odsetki" {
            CashMovementType::Interest(s)
        } else if s == "Wypłata" {
            CashMovementType::BankWithdrawal(s)
        } else if s == "Depozyt" {
            CashMovementType::Deposit(s)
        } else if s.to_lowercase().contains("opłata transakcyjna") {
            CashMovementType::TransactionFee(s)
        } else if s.to_lowercase().contains("sprzedaż") {
            CashMovementType::TransactionSell(s)
        } else if s.to_lowercase().contains("kupno") {
            CashMovementType::TransactionBuy(s)
        } else if s.to_lowercase().contains("fee") {
            CashMovementType::UnknownFee(s)
        } else if s.to_lowercase().contains("interest") {
            CashMovementType::UnknownInteres(s)
        } else {
            CashMovementType::Unknown(s)
        }
    }
}

impl SharedClient {
    #[async_recursion]
    pub async fn account_state(
        &self,
        from_date: &NaiveDate,
        to_date: &NaiveDate,
    ) -> Result<Vec<CashMovement>> {
        let inner = self.inner.try_lock().unwrap();
        match (
            &inner.session_id,
            &inner.account,
            &inner.paths.reporting_url,
        ) {
            (Some(session_id), Some(account), Some(reporting_url)) => {
                let url = Url::parse(reporting_url)?.join("v6/accountoverview")?;
                let req = inner
                    .http_client
                    .get(url)
                    .query(&[
                        ("sessionId", &session_id),
                        ("intAccount", &&format!("{}", account.int_account)),
                        ("fromDate", &&from_date.format("%d/%m/%Y").to_string()),
                        ("toDate", &&to_date.format("%d/%m/%Y").to_string()),
                    ])
                    .header(header::REFERER, &inner.paths.referer);
                let res = req.send().await?;
                match res.error_for_status() {
                    Ok(res) => {
                        let mut body = res
                            .json::<HashMap<String, HashMap<String, Vec<CashMovement>>>>()
                            .await?;
                        let mut data = body.remove("data").ok_or(eyre!("data key not found"))?;
                        let state = data
                            .remove("cashMovements")
                            .ok_or(eyre!("cashMovements key not found"))?;
                        Ok(state)
                    }
                    Err(err) => match err.status().unwrap().as_u16() {
                        401 => {
                            drop(inner);
                            self.login().await?.account_state(from_date, to_date).await
                        }
                        _ => Err(eyre!(err)),
                    },
                }
            }
            (None, _, _) => {
                drop(inner);
                self.login().await?.account_state(from_date, to_date).await
            }
            (Some(_), _, _) => {
                drop(inner);
                self.login()
                    .await?
                    .fetch_account_data()
                    .await?
                    .account_state(from_date, to_date)
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
    async fn account_state() {
        let username = std::env::args().nth(2).expect("no username given");
        let password = std::env::args().nth(3).expect("no password given");
        let mut builder = ClientBuilder::default();
        let client = builder
            .username(&username)
            .password(&password)
            .build()
            .unwrap();
        let state = client
            .account_state(
                &NaiveDate::from_ymd(2022, 1, 1),
                &NaiveDate::from_ymd(2022, 12, 31),
            )
            .await
            .unwrap();
        dbg!(state);
    }
}
