use async_recursion::async_recursion;
use chrono::prelude::*;
use color_eyre::{eyre::eyre, Report, Result};
use reqwest::{header, Url};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use strum::EnumString;

use crate::client::SharedClient;
use crate::money::Currency;
use crate::{OrderTimeType, OrderType, TransactionType};
use derivative::Derivative;

#[derive(Debug, Deserialize)]
struct OrderObject {
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
    Date,
    ProductId,
    #[serde(rename = "product")]
    Symbol,
    ContractType,
    ContractSize,
    Currency,
    #[serde(rename = "buysell")]
    TransactionType,
    Size,
    Quantity,
    Price,
    StopPrice,
    TotalOrderValue,
    OrderTypeId,
    OrderTimeTypeId,
    OrderType,
    OrderTimeType,
    IsModifiable,
    IsDeletable,
}

#[allow(dead_code)]
#[derive(Debug, Default, Deserialize)]
pub struct HistoricOrder {
    id: String,
    date: NaiveDateTime,
    product_id: u32,
    #[serde(rename = "product")]
    symbol: String,
    contract_type: u32,
    contract_size: f64,
    currency: Currency,
    #[serde(rename = "buysell")]
    transaction_type: TransactionType,
    size: f64,
    quantity: f64,
    price: f64,
    stop_price: f64,
    total_order_value: f64,
    order_type_id: u32,
    order_time_type_id: u32,
    order_type: OrderType,
    order_time_type: OrderTimeType,
    is_modifiable: bool,
    is_deletable: bool,
}

impl TryFrom<OrderObject> for HistoricOrder {
    type Error = Report;

    fn try_from(obj: OrderObject) -> Result<Self, Self::Error> {
        let mut order = HistoricOrder::default();
        for row in &obj.value {
            match row.elem_type {
                ElemType::Id => {
                    if let Some(s) = &row.value {
                        let val = s.as_str().ok_or(eyre!("val is not string"))?;
                        order.id = val.to_string();
                    }
                }
                ElemType::Date => {
                    if let Some(s) = &row.value {
                        let val = serde_json::from_value::<NaiveDateTime>(dbg!(s).clone())?;
                        order.date = val;
                    }
                }
                ElemType::ProductId => {
                    if let Some(s) = &row.value {
                        let val = s.as_u64().ok_or(eyre!("val is not u64"))?;
                        order.product_id = val as u32;
                    }
                }
                ElemType::Symbol => {
                    if let Some(s) = &row.value {
                        let val = s.as_str().ok_or(eyre!("val is not string"))?;
                        order.symbol = val.to_string();
                    }
                }
                ElemType::ContractType => {
                    if let Some(s) = &row.value {
                        let val = s.as_u64().ok_or(eyre!("val is not u64"))?;
                        order.contract_type = val as u32;
                    }
                }
                ElemType::ContractSize => {
                    if let Some(s) = &row.value {
                        let val = s.as_f64().ok_or(eyre!("val is not f64"))?;
                        order.contract_size = val;
                    }
                }
                ElemType::Currency => {
                    if let Some(s) = &row.value {
                        let val = serde_json::from_value::<Currency>(s.clone())?;
                        order.currency = val;
                    }
                }
                ElemType::TransactionType => {
                    if let Some(s) = &row.value {
                        let val = serde_json::from_value::<TransactionType>(s.clone())?;
                        order.transaction_type = val;
                    }
                }
                ElemType::Size => {
                    if let Some(s) = &row.value {
                        let val = s.as_f64().ok_or(eyre!("val is not f64"))?;
                        order.size = val;
                    }
                }
                ElemType::Quantity => {
                    if let Some(s) = &row.value {
                        let val = s.as_f64().ok_or(eyre!("val is not f64"))?;
                        order.quantity = val;
                    }
                }
                ElemType::Price => {
                    if let Some(s) = &row.value {
                        let val = s.as_f64().ok_or(eyre!("val is not f64"))?;
                        order.price = val;
                    }
                }
                ElemType::StopPrice => {
                    if let Some(s) = &row.value {
                        let val = s.as_f64().ok_or(eyre!("val is not f64"))?;
                        order.stop_price = val;
                    }
                }
                ElemType::TotalOrderValue => {
                    if let Some(s) = &row.value {
                        let val = s.as_f64().ok_or(eyre!("val is not f64"))?;
                        order.total_order_value = val;
                    }
                }
                ElemType::OrderTypeId => {
                    if let Some(s) = &row.value {
                        let val = s.as_u64().ok_or(eyre!("val is not u64"))?;
                        order.order_type_id = val as u32;
                    }
                }
                ElemType::OrderTimeTypeId => {
                    if let Some(s) = &row.value {
                        let val = s.as_u64().ok_or(eyre!("val is not u64"))?;
                        order.order_time_type_id = val as u32;
                    }
                }
                ElemType::OrderType => {
                    if let Some(s) = &row.value {
                        let val = serde_json::from_value::<OrderType>(s.clone())?;
                        order.order_type = val;
                    }
                }
                ElemType::OrderTimeType => {
                    if let Some(s) = &row.value {
                        let val = serde_json::from_value::<OrderTimeType>(s.clone())?;
                        order.order_time_type = val;
                    }
                }
                ElemType::IsModifiable => {
                    if let Some(s) = &row.value {
                        let val = s.as_bool().ok_or(eyre!("val is not bool"))?;
                        order.is_modifiable = val;
                    }
                }
                ElemType::IsDeletable => {
                    if let Some(s) = &row.value {
                        let val = s.as_bool().ok_or(eyre!("val is not bool"))?;
                        order.is_deletable = val;
                    }
                }
            }
        }

        Ok(order)
    }
}

pub struct HistoricOrders(Vec<HistoricOrder>);

#[derive(Serialize, Derivative)]
#[serde(rename_all = "camelCase")]
#[derivative(Debug)]
pub struct Order {
    #[serde(rename = "buySell")]
    transaction_type: TransactionType,
    order_type: OrderType,
    price: f64,
    product_id: String,
    size: i64,
    stop_price: f64,
    time_type: OrderTimeType,
    #[derivative(Debug = "ignore")]
    #[serde(skip)]
    client: Option<SharedClient>,
}

impl SharedClient {
    #[async_recursion]
    pub async fn orders(&self) -> Result<HistoricOrders> {
        let inner = self.inner.try_lock().unwrap();
        match (&inner.session_id, &inner.account, &inner.paths.trading_url) {
            (Some(session_id), Some(account), Some(trading_url)) => {
                let url = Url::parse(trading_url)?.join(&format!(
                    "v5/update/{};jsessionid={}",
                    account.int_account, session_id
                ))?;
                let req = inner
                    .http_client
                    .get(url)
                    .query(&[
                        ("sessionId", session_id),
                        ("orders", &0.to_string()),
                        ("transactions", &0.to_string()),
                    ])
                    .header(header::REFERER, &inner.paths.referer);
                let res = req.send().await.unwrap();
                match res.error_for_status() {
                    Ok(res) => {
                        let json = res.json::<Value>().await?;
                        let body = json
                            .get("orders")
                            .ok_or(eyre!("orders key not found"))?
                            .get("value")
                            .ok_or(eyre!("value key not found"))?;
                        let objs: Vec<OrderObject> = serde_json::from_value(body.clone())?;
                        let mut orders = Vec::new();
                        for obj in objs {
                            let o: HistoricOrder = obj.try_into()?;
                            orders.push(o)
                        }
                        Ok(HistoricOrders(orders))
                    }
                    Err(err) => match err.status().unwrap().as_u16() {
                        401 => {
                            drop(inner);
                            Ok(self.login().await?.orders().await?)
                        }
                        _ => Err(eyre!(err)),
                    },
                }
            }
            (None, _, _) => {
                drop(inner);
                self.login().await?.orders().await
            }
            (Some(_), _, _) => {
                drop(inner);
                self.login()
                    .await?
                    .fetch_account_data()
                    .await?
                    .orders()
                    .await
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::client::ClientBuilder;

    use super::Order;

    #[tokio::test]
    async fn orders() {
        let username = std::env::args().nth(2).expect("no username given");
        let password = std::env::args().nth(3).expect("no password given");
        let mut builder = ClientBuilder::default();
        let client = builder
            .username(&username)
            .password(&password)
            .build()
            .unwrap();
        client.orders().await.unwrap();
    }
    #[test]
    fn market_buy_order() {
        let order = Order {
            order_type: crate::OrderType::Market,
            transaction_type: crate::TransactionType::Buy,
            price: 1.23,
            product_id: "id".to_string(),
            size: 1,
            stop_price: 1.23,
            time_type: crate::OrderTimeType::Day,
            client: None,
        };
        println!("{}", serde_json::to_string_pretty(&order).unwrap());
    }
    #[test]
    fn market_sell_order() {
        let order = Order {
            order_type: crate::OrderType::Market,
            transaction_type: crate::TransactionType::Sell,
            price: 1.23,
            product_id: "id".to_string(),
            size: 1,
            stop_price: 1.23,
            time_type: crate::OrderTimeType::Day,
            client: None,
        };
        println!("{}", serde_json::to_string_pretty(&order).unwrap());
    }
}
