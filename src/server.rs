use std::net::{SocketAddr, SocketAddrV4};

use atomic_take::AtomicTake;
use chrono::Duration;
use degiro_rs::api::product::ProductInner;
use erfurt::prelude::Candles;
use eventual::eve::Eve;
use futures::SinkExt;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::net::{TcpListener, TcpStream};
use tokio_stream::StreamExt;
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use tracing::error;

use crate::{
    events::{
        self, calculate_portfolio::CalculatePorftolio, single_allocation::GetSingleAllocation,
    },
    portfolio::RiskMode,
    App,
};

#[derive(Debug)]
pub struct ClientBuilder {
    pub(crate) addr: SocketAddr,
}

#[derive(Debug)]
pub struct Client {
    pub frame: Framed<TcpStream, LengthDelimitedCodec>,
    pub addr: SocketAddr,
}

pub struct Server {
    pub eve: Eve<App>,
    pub listener: TcpListener,
    pub addr: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum Request {
    Ping,
    Pong,
    Login,
    FetchData {
        id: Option<String>,
    },
    GetProduct {
        query: crate::data::products::ProductQuery,
    },
    GetCandles {
        query: crate::data::products::ProductQuery,
    },
    GetSingleAllocation {
        query: crate::data::products::ProductQuery,
        mode: RiskMode,
        risk: f64,
        risk_free: f64,
    },
    CalculatePortfolio {
        mode: RiskMode,
        risk: f64,
        risk_free: f64,
        freq: u32,
        money: f64,
        max_stocks: i32,
        min_rsi: Option<f64>,
        max_rsi: Option<f64>,
        min_class: Option<degiro_rs::util::ProductCategory>,
        max_class: Option<degiro_rs::util::ProductCategory>,
        short_sales_constraint: bool,
    },
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Deserialize, Serialize)]
pub enum Response {
    SendProduct { product: Option<ProductInner> },
    SendCandles { candles: Option<Candles> },
    SendSingleAllocation { single_allocation: Option<f64> },
    SendPortfolio { portfolio: Option<String> },
}

#[derive(Debug, Deserialize, Error, Serialize)]
pub enum MsgError {}

#[derive(Debug, Deserialize, Error, Serialize)]
pub enum ServerError {
    #[error("can't read from socket")]
    ReadError,
    #[error("can't write to socket")]
    WriteError,
    #[error("can't deserialize bincode")]
    DeserializeError,
    #[error("empty message")]
    EmptyMessage,
}

impl Server {
    pub async fn new(
        socket: impl Into<SocketAddrV4>,
        eve: Eve<App>,
    ) -> Result<Self, tokio::io::Error> {
        let addr = socket.into();
        let listener = TcpListener::bind(&addr).await?;
        Ok(Self {
            eve,
            listener,
            addr: addr.to_string(),
        })
    }
    pub async fn run(&mut self) {
        loop {
            let (socket, _) = self.listener.accept().await.unwrap();
            let mut frame = Framed::new(socket, LengthDelimitedCodec::new());
            // TODO:
            let (res_tx, mut res_rx) = tokio::sync::mpsc::unbounded_channel::<Option<Response>>();
            let eve = self.eve.clone();
            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        Some(msg) = res_rx.recv() => {
                            let bytes = bincode::serialize(&msg).unwrap();
                            frame.send(bytes.into()).await.unwrap();
                        }
                        framed = frame.next() => {
                            match framed {
                                Some(Ok(buf)) => {
                                    let req: Request = bincode::deserialize(&buf).unwrap();
                                    req.process(&eve, &res_tx).await;
                                }
                                Some(Err(err)) => {
                                    dbg!(err);
                                }
                                None => break,
                            }
                        }
                    }
                }
            });
        }
    }
}

impl ClientBuilder {
    pub fn new(socket: impl Into<SocketAddrV4>) -> Self {
        let addr = socket.into();
        Self { addr: addr.into() }
    }
    pub async fn build(&self) -> Result<Client, tokio::io::Error> {
        let socket = TcpStream::connect(&self.addr).await?;
        let frame = Framed::new(socket, LengthDelimitedCodec::new());
        Ok(Client {
            frame,
            addr: self.addr,
        })
    }
}
impl Client {
    pub async fn read(&mut self) -> Option<Response> {
        match tokio::time::timeout(Duration::seconds(60).to_std().unwrap(), self.frame.next()).await
        {
            Err(_) | Ok(None) | Ok(Some(Err(_))) => None,
            Ok(Some(Ok(buf))) => bincode::deserialize::<Option<Response>>(&buf).unwrap(),
        }
    }
    pub async fn write(&mut self, req: Request) -> Option<Response> {
        let bytes = bincode::serialize(&req).unwrap();
        self.frame.send(bytes.into()).await.unwrap();
        self.read().await
    }
}

// impl Client {
//     pub async fn read(&mut self) -> Option<Response> {
//         match self.frame.next().await {
//             Some(Ok(buf)) => bincode::deserialize::<Option<Response>>(&buf).unwrap(),
//             _ => None,
//         }
//     }
//     pub async fn write(&mut self, msg: Request) -> Option<Response> {
//         let bytes = bincode::serialize(&msg).unwrap();
//         self.frame.send(bytes.into()).await.unwrap();
//
//         match tokio::time::timeout(Duration::milliseconds(1000).to_std().unwrap(), self.read())
//             .await
//         {
//             Ok(maybe_msg) => maybe_msg,
//             Err(_) => None,
//         }
//     }
// }

impl Request {
    pub async fn process(
        self,
        eve: &Eve<App>,
        res_tx: &tokio::sync::mpsc::UnboundedSender<Option<Response>>,
    ) {
        match self {
            Request::Ping => todo!(),
            Request::Pong => todo!(),
            Request::Login => {
                let event = events::login::Login {};
                eve.dispatch(event).await.unwrap_or_else(|err| {
                    tracing::error!(error = %err, "Failed to dispatch login event");
                });
                res_tx.send(None).unwrap();
            }
            Request::FetchData { id } => {
                let event = events::fetch_data::FetchData { id, name: None };
                eve.dispatch(event).await.unwrap_or_else(|err| {
                    tracing::error!(error = %err, "Failed to dispatch fetch data event");
                });
                res_tx.send(None).unwrap();
            }
            Request::GetProduct { query } => {
                let (tx, rx) = tokio::sync::oneshot::channel();
                let event = events::get_product::GetProduct {
                    query,
                    tx: AtomicTake::new(tx),
                };
                eve.dispatch(event).await.unwrap_or_else(|err| {
                    tracing::error!(error = %err, "Failed to dispatch get product event");
                });
                if let Ok(product) = rx.await {
                    res_tx
                        .send(Some(Response::SendProduct { product }))
                        .unwrap();
                } else {
                    error!("Failed to get product. Response channel closed");
                    res_tx.send(None).unwrap();
                }
            }
            Request::GetCandles { query } => {
                let (tx, rx) = tokio::sync::oneshot::channel();
                let event = events::get_candles::GetCandles {
                    query,
                    tx: AtomicTake::new(tx),
                };
                eve.dispatch(event).await.unwrap_or_else(|err| {
                    tracing::error!(error = %err, "Failed to dispatch get candles event");
                });
                if let Ok(candles) = rx.await {
                    res_tx
                        .send(Some(Response::SendCandles { candles }))
                        .unwrap();
                } else {
                    error!("Failed to get candles. Response channel closed");
                    res_tx.send(None).unwrap();
                }
            }
            Request::GetSingleAllocation {
                query,
                mode,
                risk,
                risk_free,
            } => {
                let (tx, rx) = tokio::sync::oneshot::channel();
                let event = GetSingleAllocation {
                    query,
                    mode,
                    risk,
                    risk_free,
                    tx: AtomicTake::new(tx),
                };
                eve.dispatch(event).await.unwrap_or_else(|err| {
                    tracing::error!(error = %err, "Failed to dispatch get single allocation event");
                });
                if let Ok(single_allocation) = rx.await {
                    res_tx
                        .send(Some(Response::SendSingleAllocation { single_allocation }))
                        .unwrap();
                } else {
                    error!("Failed to get single allocation. Response channel closed");
                    res_tx.send(None).unwrap();
                }
            }
            Request::CalculatePortfolio {
                mode,
                risk,
                risk_free,
                freq,
                money,
                max_stocks,
                min_rsi,
                max_rsi,
                min_class,
                max_class,
                short_sales_constraint,
            } => {
                let (tx, rx) = tokio::sync::oneshot::channel();
                let event = CalculatePorftolio {
                    tx: AtomicTake::new(tx),
                    mode,
                    risk,
                    risk_free,
                    freq,
                    money,
                    max_stocks,
                    min_rsi,
                    max_rsi,
                    min_class,
                    max_class,
                    short_sales_constraint,
                };
                eve.dispatch(event).await.unwrap_or_else(|err| {
                    tracing::error!(error = %err, "Failed to dispatch calculate portfolio event");
                });
                if let Ok(portfolio) = rx.await {
                    res_tx
                        .send(Some(Response::SendPortfolio { portfolio }))
                        .unwrap();
                } else {
                    error!("Failed to get portfolio. Response channel closed");
                    res_tx.send(None).unwrap();
                }
            }
        }
    }
}
