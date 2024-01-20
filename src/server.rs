use core::fmt;
use std::{
    net::{SocketAddr, SocketAddrV4},
    sync::Arc,
};

use async_trait::async_trait;
use atomic_take::AtomicTake;
use chrono::Duration;
use degiro_rs::api::{financial_statements::FinancialReports, product::ProductDetails};
use erfurt::prelude::Candles;
use futures::SinkExt;
use master_of_puppets::{
    executor::SequentialExecutor,
    prelude::*,
    puppet::Lifecycle,
    supervision::strategy::{OneForAll, OneToOne},
};
use serde::{Deserialize, Serialize};
use strum::Display;
use thiserror::Error;
use tokio::{
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};
use tokio_stream::StreamExt;
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use tracing::{error, info};

use crate::{
    portfolio::RiskMode,
    puppet::{
        db::{CandlesQuery, CleanUp, Db, FinanclaReportsQuery, ProductQuery},
        degiro::{Authorize, Degiro, FetchData, GetPortfolio},
        portfolio::{CalculatePortfolio, CalculateSl, Calculator, GetSingleAllocation},
    },
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

#[derive(Debug, Clone)]
pub struct Server {
    pub listener: Arc<TcpListener>,
    pub addr: String,
    pub handle: Option<Arc<JoinHandle<()>>>,
}

#[async_trait]
impl Lifecycle for Server {
    type Supervision = OneToOne;

    async fn reset(&self, puppeter: &Puppeter) -> Result<Self, CriticalError> {
        let socket: SocketAddrV4 = self
            .addr
            .parse()
            .map_err(|_| CriticalError::new(puppeter.pid, "Can't parse address"))?;

        Self::new(socket)
            .await
            .map_err(|e| CriticalError::new(puppeter.pid, e.to_string()))
    }

    async fn on_init(&mut self, puppeter: &Puppeter) -> Result<(), PuppetError> {
        let cloned_self = self.clone();
        let cloned_puppeter = puppeter.clone();
        info!("Starting server on {}", self.addr);
        let handle = tokio::spawn(async move {
            loop {
                let (socket, _) = cloned_self.listener.accept().await.unwrap();
                let mut frame = Framed::new(socket, LengthDelimitedCodec::new());
                // TODO:
                let (res_tx, mut res_rx) =
                    tokio::sync::mpsc::unbounded_channel::<Option<Response>>();
                let cloned_puppeter = cloned_puppeter.clone();
                tokio::spawn(async move {
                    loop {
                        tokio::select! {
                            Some(msg) = res_rx.recv() => {
                                info!(msg = ?msg, "Sending message");
                                let bytes = bincode::serialize(&msg).unwrap();
                                frame.send(bytes.into()).await.unwrap();
                            }
                            framed = frame.next() => {
                                match framed {
                                    Some(Ok(buf)) => {
                                        let req: Request = bincode::deserialize(&buf).unwrap();
                                        info!(req =? req, "Received message");
                                        req.process(&res_tx, &cloned_puppeter).await;
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
        });
        self.handle = Some(Arc::new(handle));
        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub enum Request {
    Ping,
    Pong,
    Authorize,
    FetchData {
        id: Option<String>,
    },
    GetProduct {
        query: ProductQuery,
    },
    GetFinancials {
        query: ProductQuery,
    },
    GetCandles {
        query: ProductQuery,
    },
    GetSingleAllocation {
        query: ProductQuery,
        mode: RiskMode,
        risk: f64,
        risk_free: f64,
    },
    CalculatePortfolio {
        mode: RiskMode,
        risk: f64,
        risk_free: f64,
        freq: usize,
        money: f64,
        max_stocks: usize,
        min_rsi: Option<f64>,
        max_rsi: Option<f64>,
        min_class: Option<degiro_rs::util::ProductCategory>,
        max_class: Option<degiro_rs::util::ProductCategory>,
        short_sales_constraint: bool,
        roic_wacc_delta: Option<f64>,
    },
    RecalculateSl {
        n: usize,
    },
    GetPortfolio,
    CleanUp,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Deserialize, Serialize)]
pub enum Response {
    SendProduct {
        product: Option<ProductDetails>,
    },
    SendFinancials {
        financials: Option<FinancialReports>,
    },
    SendCandles {
        candles: Option<Candles>,
    },
    SendSingleAllocation {
        single_allocation: Option<f64>,
    },
    SendPortfolio {
        portfolio: Option<String>,
    },
    SendRecalcucatetSl {
        table: Option<String>,
    },
    SendPortfolioSl {
        table: Option<String>,
    },
    SendCleanUp,
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
    pub async fn new(socket: impl Into<SocketAddrV4>) -> Result<Self, tokio::io::Error> {
        let addr = socket.into();
        let listener = TcpListener::bind(&addr).await?;
        Ok(Self {
            listener: Arc::new(listener),
            addr: addr.to_string(),
            handle: None,
        })
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
        res_tx: &tokio::sync::mpsc::UnboundedSender<Option<Response>>,
        puppeter: &Puppeter,
    ) {
        match self {
            Request::Ping => todo!(),
            Request::Pong => todo!(),
            Request::Authorize => {
                puppeter
                    .ask::<Degiro, _>(Authorize)
                    .await
                    .unwrap_or_else(|err| {
                        tracing::error!(error = %err, "Failed to authorize");
                    });
                res_tx.send(None).unwrap();
            }
            Request::FetchData { id } => {
                let msg = FetchData { id, name: None };
                puppeter.send::<Degiro, _>(msg).await.unwrap_or_else(|err| {
                    tracing::error!(error = %err, "Failed to fetch data");
                });
                res_tx.send(None).unwrap();
            }
            Request::GetProduct { query } => {
                let product = puppeter.ask::<Db, _>(query).await.unwrap_or_else(|err| {
                    tracing::error!(error = %err, "Failed to get product");
                    None
                });
                res_tx
                    .send(Some(Response::SendProduct { product }))
                    .unwrap();
            }
            Request::GetFinancials { query } => {
                let financials = puppeter
                    .ask::<Db, _>(FinanclaReportsQuery::from(query))
                    .await
                    .unwrap_or_else(|err| {
                        tracing::error!(error = %err, "Failed to get product");
                        None
                    });
                res_tx
                    .send(Some(Response::SendFinancials { financials }))
                    .unwrap();
            }
            Request::GetCandles { query } => {
                let candles = puppeter
                    .ask::<Db, _>(CandlesQuery::from(query))
                    .await
                    .unwrap_or_else(|err| {
                        tracing::error!(error = %err, "Failed to get product");
                        None
                    });
                res_tx
                    .send(Some(Response::SendCandles { candles }))
                    .unwrap();
            }
            Request::GetSingleAllocation {
                query,
                mode,
                risk,
                risk_free,
            } => {
                let msg = GetSingleAllocation {
                    query: query.into(),
                    mode,
                    risk,
                    risk_free,
                };
                let allocation = puppeter
                    .ask::<Calculator, _>(msg)
                    .await
                    .unwrap_or_else(|err| {
                        tracing::error!(error = %err, "Failed to get single allocation");
                        None
                    });
                res_tx
                    .send(Some(Response::SendSingleAllocation {
                        single_allocation: allocation,
                    }))
                    .unwrap();
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
                roic_wacc_delta,
            } => {
                let msg = CalculatePortfolio {
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
                    roic_wacc_delta,
                };
                let portfolio = puppeter.ask::<Calculator, _>(msg).await.ok();
                res_tx
                    .send(Some(Response::SendPortfolio { portfolio }))
                    .unwrap();
            }
            Request::RecalculateSl { n } => {
                let msg = CalculateSl { n };
                let table = puppeter.ask::<Calculator, _>(msg).await.ok();
                res_tx
                    .send(Some(Response::SendRecalcucatetSl { table }))
                    .unwrap();
            }
            Request::GetPortfolio => {
                let msg = GetPortfolio;
                let portfolio = puppeter.ask::<Calculator, _>(msg).await.ok();
                res_tx
                    .send(Some(Response::SendPortfolio { portfolio }))
                    .unwrap();
            }
            Request::CleanUp => {
                let msg = CleanUp;
                puppeter.send::<Db, _>(msg).await.ok();
                res_tx.send(Some(Response::SendCleanUp)).unwrap();
            }
        }
    }
}
