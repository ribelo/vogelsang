pub mod account;
pub mod api;
pub mod client;
pub mod money;
use chrono::Duration;
use color_eyre::eyre;
use std::{collections::HashSet, fmt::Display};
use thiserror::Error;

use serde::{Deserialize, Serialize};
use serde_repr::Serialize_repr;
use strum::{self, EnumString};

#[allow(dead_code, clippy::enum_variant_names)]
#[derive(Debug, Error)]
enum Error {
    #[error("no session_id")]
    NoSessionId,
    #[error("session_id terminated")]
    SessionIdTerminated,
    #[error("no account info")]
    NoAccountInfo,
    #[error("no account config")]
    NoAccountConfig,
    #[error(transparent)]
    Other(#[from] eyre::Error),
}

#[derive(Clone, Debug, Default, Deserialize, EnumString, PartialEq, Eq, Hash)]
pub enum Period {
    PT1S,
    PT1M,
    PT1H,
    P1D,
    P1W,
    P1M,
    P3M,
    P6M,
    #[default]
    P1Y,
    P3Y,
    P5Y,
    P50Y,
}

impl Display for Period {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Period {
    pub fn to_ms(&self) -> u64 {
        match &self {
            Self::PT1S => 1000,
            Self::PT1M => 1000 * 60,
            Self::PT1H => 1000 * 60 * 60,
            Self::P1D => 1000 * 60 * 60 * 24,
            Self::P1W => 1000 * 60 * 60 * 24 * 7,
            Self::P1M => 1000 * 60 * 60 * 24 * 30,
            Self::P3M => 1000 * 60 * 60 * 24 * 30 * 3,
            Self::P6M => 1000 * 60 * 60 * 24 * 30 * 6,
            Self::P1Y => 1000 * 60 * 60 * 24 * 365,
            Self::P3Y => 1000 * 60 * 60 * 24 * 365 * 3,
            Self::P5Y => 1000 * 60 * 60 * 24 * 365 * 5,
            Self::P50Y => 1000 * 60 * 60 * 24 * 365 * 50,
        }
    }
    pub fn to_duration(&self) -> Duration {
        Duration::milliseconds(self.to_ms() as i64)
    }
}

impl std::ops::Div for Period {
    type Output = u32;

    fn div(self, rhs: Self) -> Self::Output {
        (self.to_ms() / rhs.to_ms()) as u32
    }
}

#[derive(Debug, Default, Deserialize, PartialEq, Eq, Hash, EnumString, Clone, Serialize_repr)]
#[strum(ascii_case_insensitive)]
#[repr(u8)]
pub enum OrderType {
    #[default]
    #[serde(rename ="LIMIT")]
    Limit = 0,
    #[serde(rename ="STOPLIMIT")]
    StopLimit = 1,
    #[serde(rename ="MARKET")]
    Market = 2,
    #[serde(rename ="STOPLOSS")]
    StopLoss = 3,
    #[serde(rename ="TRAILINGSTOP")]
    TrailingStop = 4,
}

#[allow(dead_code)]
#[derive(Debug, Default, Deserialize, Clone)]
pub struct AllowedOrderTypes(HashSet<OrderType>);

impl AllowedOrderTypes {
    pub fn has(&self, x: OrderType) -> bool {
        self.0.contains(&x)
    }
}

#[derive(Clone, Debug, Deserialize, EnumString)]
#[strum(ascii_case_insensitive)]
pub enum ProductCategory {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Hash, EnumString, Serialize_repr)]
#[strum(ascii_case_insensitive)]
#[repr(u8)]
pub enum OrderTimeType {
    #[default]
    #[serde(rename(deserialize = "DAY"))]
    Day = 1,
    #[serde(rename(deserialize = "GTC"))]
    Permanent = 3,
}

#[derive(Clone, Debug, Deserialize)]
pub struct OrderTimeTypes(HashSet<OrderTimeType>);

impl OrderTimeTypes {
    pub fn has(&self, x: OrderTimeType) -> bool {
        self.0.contains(&x)
    }
}

#[derive(Debug, Deserialize)]
pub enum ProductType {
    #[serde(rename = "STOCK")]
    Stock,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub enum TransactionType {
    #[default]
    #[serde(rename(deserialize = "B", serialize = "BUY"))]
    Buy,
    #[serde(rename(deserialize = "S", serialize = "SELL"))]
    Sell,
}
