use std::{
    net::{SocketAddr, SocketAddrV4},
    sync::Arc,
};

use async_trait::async_trait;
use chrono::{Duration, NaiveDate};
use comfy_table::presets::UTF8_BORDERS_ONLY;
use degiro_rs::api::{financial_statements::FinancialReports, product::ProductDetails};
use erfurt::prelude::Candles;
use futures::SinkExt;
use master_of_puppets::{
    message::ServiceCommand, prelude::*, puppet::Lifecycle, supervision::strategy::OneToOne,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::net::{TcpListener, TcpStream};
use tokio_stream::StreamExt;
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use tracing::{error, info};

use crate::{
    portfolio::RiskMode,
    puppet::{
        db::{CandlesQuery, CleanUp, Db, FinanclaReportsQuery, ProductQuery},
        degiro::{Authorize, Degiro, FetchData, GetOrders, GetPortfolio, GetTransactions},
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
}

#[async_trait]
impl Lifecycle for Server {
    type Supervision = OneToOne;

    async fn reset(&self, puppeter: &Puppeter) -> Result<Self, CriticalError> {
        let socket: SocketAddrV4 = self.addr.parse().map_err(|_err| CriticalError {
            puppet: puppeter.pid,
            message: "Can't parse address".to_string(),
        })?;

        Self::new(socket).await.map_err(|e| CriticalError {
            puppet: puppeter.pid,
            message: e.to_string(),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub enum Request {
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
        min_dd: Option<f64>,
        max_dd: Option<f64>,
        min_class: Option<degiro_rs::util::ProductCategory>,
        max_class: Option<degiro_rs::util::ProductCategory>,
        short_sales_constraint: bool,
        min_roic: Option<f64>,
        roic_wacc_delta: Option<f64>,
    },
    RecalculateSl {
        nstd: usize,
        max_percent: Option<f64>,
    },
    GetPortfolio,
    GetTransactions {
        from_date: NaiveDate,
        to_date: NaiveDate,
    },
    GetOrders,
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
    SendTransactions {
        table: Option<String>,
    },
    SendOrders {
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

#[derive(Debug)]
pub struct RunServer;

impl Server {
    pub async fn new<T: Into<SocketAddrV4> + Send>(socket: T) -> Result<Self, tokio::io::Error> {
        let addr = socket.into();
        let listener = TcpListener::bind(&addr).await?;
        Ok(Self {
            listener: Arc::new(listener),
            addr: addr.to_string(),
        })
    }
}

#[async_trait]
impl Handler<RunServer> for Server {
    type Response = ();

    type Executor = SequentialExecutor;

    async fn handle_message(
        &mut self,
        _msg: RunServer,
        puppeter: &Puppeter,
    ) -> Result<Self::Response, PuppetError> {
        info!("Starting server on {}", self.addr);
        let cloned_self = self.clone();
        let cloned_puppeter = puppeter.clone();
        tokio::spawn(async move {
            loop {
                let Ok((socket, _)) = cloned_self.listener.accept().await else {
                    let err = cloned_puppeter.critical_error("Can't accept connection");
                    let _ = cloned_puppeter
                        .send_command::<Self>(ServiceCommand::ReportFailure {
                            pid: cloned_puppeter.pid,
                            error: err,
                        })
                        .await;
                    break;
                };
                let mut frame = Framed::new(socket, LengthDelimitedCodec::new());
                // TODO:
                let (res_tx, mut res_rx) =
                    tokio::sync::mpsc::unbounded_channel::<Option<Response>>();
                let cloned_puppeter = cloned_puppeter.clone();
                tokio::spawn(async move {
                    loop {
                        tokio::select! {
                            Some(msg) = res_rx.recv() => {
                                let Ok(bytes) = bincode::serialize(&msg) else {
                                    return Err(cloned_puppeter.critical_error("Can't serialize message"))
                                };
                                if frame.send(bytes.into()).await.is_err() {
                                    return Err(cloned_puppeter.critical_error( "Can't send message"))
                                };
                            }
                            framed = frame.next() => {
                                match framed {
                                    Some(Ok(buf)) => {
                                        let Ok(req) = bincode::deserialize::<Request>(&buf) else {
                                            return Err(cloned_puppeter.critical_error("Can't deserialize message"))
                                        };
                                        info!(req =? req, "Received message");
                                        req.process(&res_tx, &cloned_puppeter).await;
                                    }
                                    Some(Err(err)) => {
                                        eprintln!("{err}");
                                    }
                                    None => break Ok(()),
                                }
                            }
                        }
                    }
                });
            }
        });
        Ok(())
    }
}

impl ClientBuilder {
    pub fn new<T: Into<SocketAddrV4>>(socket: T) -> Self {
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
            Err(_) | Ok(None | Some(Err(_))) => None,
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
            Self::Authorize => {
                puppeter
                    .ask::<Degiro, _>(Authorize)
                    .await
                    .unwrap_or_else(|err| {
                        tracing::error!(error = %err, "Failed to authorize");
                    });
                res_tx.send(None).unwrap();
            }
            Self::FetchData { id } => {
                let msg = FetchData { id, name: None };
                puppeter.send::<Degiro, _>(msg).await.unwrap_or_else(|err| {
                    tracing::error!(error = %err, "Failed to fetch data");
                });
                res_tx.send(None).unwrap();
            }
            Self::GetProduct { query } => {
                let product = puppeter.ask::<Db, _>(query).await.unwrap_or_else(|err| {
                    tracing::error!(error = %err, "Failed to get product");
                    None
                });
                res_tx
                    .send(Some(Response::SendProduct { product }))
                    .unwrap();
            }
            Self::GetFinancials { query } => {
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
            Self::GetCandles { query } => {
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
            Self::GetSingleAllocation {
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
            Self::CalculatePortfolio {
                mode,
                risk,
                risk_free,
                freq,
                money,
                max_stocks,
                min_rsi,
                max_rsi,
                min_dd,
                max_dd,
                min_class,
                max_class,
                short_sales_constraint,
                min_roic,
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
                    min_dd,
                    max_dd,
                    min_class,
                    max_class,
                    short_sales_constraint,
                    min_roic,
                    roic_wacc_delta,
                };
                let portfolio = puppeter.ask::<Calculator, _>(msg).await.ok();
                res_tx
                    .send(Some(Response::SendPortfolio { portfolio }))
                    .unwrap();
            }
            Self::RecalculateSl { nstd, max_percent } => {
                let msg = CalculateSl { nstd, max_percent };
                let table = puppeter.ask::<Calculator, _>(msg).await.ok();
                res_tx
                    .send(Some(Response::SendRecalcucatetSl { table }))
                    .unwrap();
            }
            Self::GetPortfolio => {
                let msg = GetPortfolio;
                let portfolio = puppeter.ask::<Calculator, _>(msg).await.ok();
                res_tx
                    .send(Some(Response::SendPortfolio { portfolio }))
                    .unwrap();
            }
            Self::GetTransactions { from_date, to_date } => {
                let msg = GetTransactions { from_date, to_date };
                let transactions = puppeter.ask::<Degiro, _>(msg).await.ok();
                let mut table = comfy_table::Table::new();
                let header = vec![
                    comfy_table::Cell::new("id"),
                    comfy_table::Cell::new("product id"),
                    comfy_table::Cell::new("transaction type"),
                    comfy_table::Cell::new("transaction type id"),
                    comfy_table::Cell::new("order type id"),
                    comfy_table::Cell::new("price")
                        .set_alignment(comfy_table::CellAlignment::Right),
                    comfy_table::Cell::new("total")
                        .set_alignment(comfy_table::CellAlignment::Right),
                ];
                table.set_header(header);
                table.load_preset(UTF8_BORDERS_ONLY);
                if let Some(transactions) = transactions {
                    for transaction in transactions.0 {
                        table.add_row(vec![
                            comfy_table::Cell::new(transaction.inner.id.to_string()),
                            comfy_table::Cell::new(transaction.inner.product_id.to_string()),
                            comfy_table::Cell::new(transaction.inner.transaction_type.to_string()),
                            comfy_table::Cell::new(
                                transaction.inner.transaction_type_id.to_string(),
                            ),
                            comfy_table::Cell::new(
                                transaction
                                    .inner
                                    .order_type_id
                                    .map_or(String::new(), |id| id.to_string()),
                            ),
                            comfy_table::Cell::new(transaction.inner.price.to_string())
                                .set_alignment(comfy_table::CellAlignment::Right),
                            comfy_table::Cell::new(transaction.inner.total.to_string())
                                .set_alignment(comfy_table::CellAlignment::Right),
                        ]);
                    }
                }
                res_tx
                    .send(Some(Response::SendTransactions {
                        table: Some(table.to_string()),
                    }))
                    .unwrap();
            }
            Self::GetOrders => {
                let msg = GetOrders;
                let orders = puppeter.ask::<Degiro, _>(msg).await.ok();
                let mut table = comfy_table::Table::new();
                let header = vec![
                    comfy_table::Cell::new("product id"),
                    comfy_table::Cell::new("product"),
                    comfy_table::Cell::new("type"),
                    comfy_table::Cell::new("qty").set_alignment(comfy_table::CellAlignment::Right),
                    comfy_table::Cell::new("price")
                        .set_alignment(comfy_table::CellAlignment::Right),
                    comfy_table::Cell::new("value")
                        .set_alignment(comfy_table::CellAlignment::Right),
                ];
                table.set_header(header);
                table.load_preset(UTF8_BORDERS_ONLY);
                if let Some(orders) = orders {
                    for order in orders.iter() {
                        table.add_row(vec![
                            comfy_table::Cell::new(order.product_id.to_string()),
                            comfy_table::Cell::new(order.product.to_string()),
                            comfy_table::Cell::new(order.transaction_type.to_string()),
                            comfy_table::Cell::new(order.quantity.to_string())
                                .set_alignment(comfy_table::CellAlignment::Right),
                            comfy_table::Cell::new(order.stop_price.to_string())
                                .set_alignment(comfy_table::CellAlignment::Right),
                            comfy_table::Cell::new(order.total_order_value.to_string())
                                .set_alignment(comfy_table::CellAlignment::Right),
                        ]);
                    }
                }
                res_tx
                    .send(Some(Response::SendOrders {
                        table: Some(table.to_string()),
                    }))
                    .unwrap();
            }
            Self::CleanUp => {
                let msg = CleanUp;
                puppeter.send::<Db, _>(msg).await.ok();
                res_tx.send(Some(Response::SendCleanUp)).unwrap();
            }
        }
    }
}
