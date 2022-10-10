use async_recursion::async_recursion;
use color_eyre::{eyre::eyre, Result};
use reqwest::{header, Url};
use serde::Deserialize;
use serde_json::Value;
use std::{collections::HashMap, convert::TryInto, sync::Arc};
use strum::EnumString;
use thiserror::Error;
use tokio::time::{timeout, Duration};

use crate::{
    client::{Client, ClientMsg},
    money::{Currency, Money},
};

use super::product::Product;

#[derive(Debug, Deserialize)]
struct PortfolioObject {
    value: Vec<ValueObject>,
}

#[derive(Debug, Deserialize)]
struct ValueObject {
    #[serde(rename = "name")]
    elem_type: ElemType,
    value: Option<Value>,
}

#[derive(Debug, Deserialize, EnumString)]
#[serde(rename_all = "camelCase")]
enum ElemType {
    Id,
    PositionType,
    Size,
    Price,
    Value,
    AccruedInterest,
    PlBase,
    TodayPlBase,
    PortfolioValueCorrection,
    BreakEvenPrice,
    AverageFxRate,
    RealizedProductPl,
    RealizedFxPl,
    TodayRealizedProductPl,
    TodayRealizedFxPl,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default)]
pub struct Position {
    pub id: String,
    pub product: Option<Arc<Product>>,
    pub position_type: PositionType,
    pub size: f64,
    pub price: f64,
    pub currency: Currency,
    pub value: Money,
    pub accrued_interest: Option<f64>,
    pub base_value: Money,
    pub today_value: Money,
    pub portfolio_value_correction: f64,
    pub break_even_price: f64,
    pub average_fx_rate: f64,
    pub realized_product_profit: Money,
    pub realized_fx_profit: Money,
    pub today_realized_product_pl: Money,
    pub today_realized_fx_pl: Money,
    pub total_profit: Money,
    pub product_profit: Money,
    pub fx_profit: Money,
}

#[derive(Clone, Debug, Default)]
pub struct Portfolio(Vec<Position>);

impl Portfolio {
    pub fn add(&mut self, position: Position) -> &mut Self {
        self.0.push(position);
        self
    }
    pub fn value(&self) -> HashMap<&Currency, f64> {
        let mut m = HashMap::default();
        for p in &self.0 {
            let money = &p.value;
            let x = m.entry(&money.0).or_insert(0.0);
            *x += money.1;
        }
        m
    }
    pub fn base_value(&self) -> HashMap<&Currency, f64> {
        let mut m = HashMap::default();
        for p in &self.0 {
            let money = &p.base_value;
            let x = m.entry(&money.0).or_insert(0.0);
            *x += money.1;
        }
        m
    }
}

#[derive(Clone, Debug, Default, EnumString)]
#[strum(ascii_case_insensitive)]
pub enum PositionType {
    Cash,
    #[default]
    Product,
}

#[derive(Debug, Error)]
#[error("can't parse object {:#?}", 0)]
pub struct ParsePositionError(PortfolioObject);

impl TryFrom<PortfolioObject> for Position {
    type Error = ParsePositionError;

    fn try_from(obj: PortfolioObject) -> Result<Self, Self::Error> {
        let mut position = Position::default();
        let mut value = 0.0;
        for row in &obj.value {
            match row.elem_type {
                ElemType::Id => {
                    position.id = row.value.as_ref().unwrap().as_str().unwrap().to_string();
                }
                ElemType::PositionType => {
                    match row.value.as_ref().unwrap().as_str().unwrap().parse() {
                        Ok(val) => position.position_type = val,
                        Err(_) => return Err(ParsePositionError(obj)),
                    };
                }
                ElemType::Size => {
                    let val = row.value.as_ref().unwrap().as_f64().unwrap();
                    position.size = val;
                }
                ElemType::Price => {
                    position.price = row.value.as_ref().unwrap().as_f64().unwrap();
                }
                ElemType::Value => {
                    value = row.value.as_ref().unwrap().as_f64().unwrap();
                }
                ElemType::AccruedInterest => {
                    if let Some(s) = &row.value {
                        let val = s.as_f64().unwrap();
                        if val > 0.0 {
                            position.accrued_interest = Some(val);
                        }
                    }
                }
                ElemType::PlBase => {
                    match serde_json::from_value::<HashMap<String, f64>>(
                        row.value.as_ref().unwrap().clone(),
                    ) {
                        Ok(m) => match TryInto::<Money>::try_into(m) {
                            Ok(val) => {
                                position.currency = val.currency();
                                position.base_value = -val;
                            }
                            Err(_) => return Err(ParsePositionError(obj)),
                        },
                        Err(_) => return Err(ParsePositionError(obj)),
                    }
                }
                ElemType::TodayPlBase => {
                    match serde_json::from_value::<HashMap<String, f64>>(
                        row.value.as_ref().unwrap().clone(),
                    ) {
                        Ok(m) => match m.try_into() {
                            Ok(val) => position.today_value = val,
                            Err(_) => return Err(ParsePositionError(obj)),
                        },
                        Err(_) => return Err(ParsePositionError(obj)),
                    }
                }
                ElemType::PortfolioValueCorrection => {
                    if let Some(s) = &row.value {
                        let val = s.as_f64().unwrap();
                        position.portfolio_value_correction = val;
                    }
                }
                ElemType::BreakEvenPrice => {
                    if let Some(s) = &row.value {
                        let val = s.as_f64().unwrap();
                        position.break_even_price = val;
                    }
                }
                ElemType::AverageFxRate => {
                    if let Some(s) = &row.value {
                        let val = s.as_f64().unwrap();
                        position.average_fx_rate = val;
                    }
                }
                ElemType::RealizedProductPl => {
                    if let Some(s) = &row.value {
                        let val = s.as_f64().unwrap();
                        position.realized_product_profit = Money(position.currency.clone(), val);
                    }
                }
                ElemType::RealizedFxPl => {
                    if let Some(s) = &row.value {
                        let val = s.as_f64().unwrap();
                        position.realized_fx_profit = Money(position.currency.clone(), val);
                    }
                }
                ElemType::TodayRealizedProductPl => {
                    if let Some(s) = &row.value {
                        let val = s.as_f64().unwrap();
                        position.today_realized_product_pl = Money(position.currency.clone(), val);
                    }
                }
                ElemType::TodayRealizedFxPl => {
                    if let Some(s) = &row.value {
                        let val = s.as_f64().unwrap();
                        position.today_realized_fx_pl = Money(position.currency.clone(), val);
                    }
                }
            }
        }
        position.total_profit =
            -(position.today_value.clone() - position.base_value.clone()).unwrap();
        let profit = (position.price * position.size)
            - (position.break_even_price * position.size) / position.average_fx_rate;
        position.product_profit = Money(position.total_profit.currency(), profit);
        position.value = Money(position.currency.clone(), value);
        position.fx_profit = ((position.total_profit.clone() - position.product_profit.clone())
            .unwrap()
            - position.realized_fx_profit.clone())
        .unwrap();
        Ok(position)
    }
}

impl Client {
    #[async_recursion]
    pub async fn portfolio(&mut self) -> Result<Portfolio> {
        match (&self.session_id, &self.account, &self.paths.trading_url) {
            (Some(session_id), Some(account), Some(trading_url)) => {
                let url = Url::parse(trading_url)?
                    .join(&self.paths.generic_data_path)?
                    .join(&format!(
                        "{};jsessionid={}",
                        account.int_account, session_id
                    ))?;
                let req = self
                    .http_client
                    .get(url)
                    .query(&[("portfolio", 0)])
                    .header(header::REFERER, &self.paths.referer);
                let res = req.send().await.unwrap();
                match res.error_for_status() {
                    Ok(res) => {
                        let json = res.json::<Value>().await?;
                        let body = json
                            .get("portfolio")
                            .ok_or(eyre!("portfolio key not found"))?
                            .get("value")
                            .ok_or(eyre!("value key not found"))?;
                        let objs: Vec<PortfolioObject> = serde_json::from_value(body.clone())?;
                        let mut portfolio = Portfolio::default();
                        for obj in objs {
                            let mut p: Position = obj.try_into()?;
                            if let Ok(product) = self.product_by_id(&p.id).await {
                                p.product = Some(product.clone());
                                portfolio.add(p);
                            }
                        }
                        Ok(portfolio)
                    }
                    Err(err) => match err.status().unwrap().as_u16() {
                        401 => {
                            self.tx
                                .send_timeout(ClientMsg::Login, Duration::from_secs(10))
                                .await;
                            self.portfolio().await
                        }
                        _ => Err(eyre!(err)),
                    },
                }
            }
            (None, _, _) => {
                self.tx
                    .send_timeout(ClientMsg::Login, Duration::from_secs(10))
                    .await;
                self.portfolio().await
            }
            // (Some(_), _, _) => {
            //     self.tx
            //         .send_timeout(ClientMsg::Login, Duration::from_secs(10))
            //         .await;
            //     // self.fetch_account_data().await?.portfolio().await
            // }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::client::ClientBuilder;

    #[tokio::test]
    async fn current_portfolio() {
        let username = std::env::args().nth(2).expect("no username given");
        let password = std::env::args().nth(3).expect("no password given");
        let mut builder = ClientBuilder::default();
        let client = builder
            .username(&username)
            .password(&password)
            .build()
            .unwrap();
        let xs = client.portfolio().await.unwrap();
        dbg!(&xs);
        dbg!(&xs.value());
        dbg!(&xs.base_value());
    }
}
