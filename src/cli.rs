use std::net::{Ipv4Addr, SocketAddrV4};

use anyhow::Result;
use async_trait::async_trait;
use chrono::NaiveDate;
use clap::{ArgGroup, Parser, Subcommand};
use degiro_rs::util::ProductCategory;
use pptr::{puppet::PuppetBuilder, puppeter::Puppeter};
use tokio::signal;
use tracing::{error, info, warn};

use crate::{
    portfolio::RiskMode,
    puppet::{
        db::{Db, ProductQuery},
        degiro::Degiro,
        portfolio::Calculator,
        settings::Settings,
    },
    server::{self, ClientBuilder, Response},
    ui, App,
};

#[derive(Debug, Parser)]
pub struct Cli {
    #[clap(short, long, default_value = "9123")]
    port: u16,
    #[clap(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Server,
    FetchData {
        id: Option<String>,
    },
    #[clap(group(ArgGroup::new("product_query").required(true).args(&["id", "symbol", "name"])))]
    GetProduct {
        #[clap(long, group = "product_query")]
        id: Option<String>,
        #[clap(long, group = "product_query")]
        symbol: Option<String>,
        #[clap(long, group = "product_query")]
        name: Option<String>,
    },
    #[clap(group(ArgGroup::new("product_query").required(true).args(&["id", "symbol", "name"])))]
    GetCandles {
        #[clap(long, group = "product_query")]
        id: Option<String>,
        #[clap(long, group = "product_query")]
        symbol: Option<String>,
        #[clap(long, group = "product_query")]
        name: Option<String>,
    },
    #[clap(group(ArgGroup::new("product_query").required(true).args(&["id", "symbol", "name"])))]
    GetFinancials {
        #[clap(long, group = "product_query")]
        id: Option<String>,
        #[clap(long, group = "product_query")]
        symbol: Option<String>,
        #[clap(long, group = "product_query")]
        name: Option<String>,
    },
    GetPortfolio,
    #[clap(group(ArgGroup::new("product_query").required(true).args(&["id", "symbol", "name"])))]
    GetSingleAllocation {
        #[clap(long, group = "product_query")]
        id: Option<String>,
        #[clap(long, group = "product_query")]
        symbol: Option<String>,
        #[clap(long, group = "product_query")]
        name: Option<String>,
        #[clap(long, default_value = "STD")]
        mode: RiskMode,
        #[clap(long)]
        risk: f64,
        #[clap(long, default_value = "0.0")]
        risk_free: f64,
    },
    CalculatePortfolio {
        #[clap(long)]
        mode: RiskMode,
        #[clap(long)]
        risk: f64,
        #[clap(long, default_value = "0.0")]
        risk_free: f64,
        #[clap(long)]
        freq: usize,
        #[clap(long)]
        money: f64,
        #[clap(long)]
        max_stocks: usize,
        #[clap(long)]
        min_rsi: Option<f64>,
        #[clap(long)]
        max_rsi: Option<f64>,
        #[clap(long)]
        min_dd: Option<f64>,
        #[clap(long)]
        max_dd: Option<f64>,
        #[clap(long)]
        min_class: Option<ProductCategory>,
        #[clap(long)]
        max_class: Option<ProductCategory>,
        #[clap(long)]
        short_sales_constraint: bool,
        #[clap(long)]
        min_roic: Option<f64>,
        #[clap(long)]
        roic_wacc_delta: Option<f64>,
    },
    RecalculateSl {
        #[clap(short, long, default_value = "3")]
        nstd: usize,
        #[clap(short, long)]
        max_percent: Option<f64>,
    },
    GetTransactions {
        #[clap(short, long)]
        from_date: NaiveDate,
        #[clap(short, long)]
        to_date: NaiveDate,
    },
    GetOrders,
    CleanUp,
}

impl App {
    pub async fn run(self) -> Result<()> {
        let cli = Cli::parse();
        let port = cli.port;
        if let Some(cmd) = cli.command {
            let addr = Ipv4Addr::new(127, 0, 0, 1);
            let socket = SocketAddrV4::new(addr, port);
            let mut client = ClientBuilder::new(socket).build().await.unwrap();
            match cmd {
                Commands::FetchData { id } => {
                    let msg = server::Request::FetchData { id };
                    client.write(msg).await.or_else(|| {
                        warn!("No response");
                        None
                    });
                }
                Commands::GetProduct { id, symbol, name } => {
                    let query = if let Some(id) = id {
                        ProductQuery::Id(id.clone())
                    } else if let Some(symbol) = symbol {
                        ProductQuery::Symbol(symbol.clone())
                    } else if let Some(name) = name {
                        ProductQuery::Name(name.clone())
                    } else {
                        panic!("No valid argument provided for GetProduct");
                    };
                    let msg = server::Request::GetProduct { query };
                    match client.write(msg).await {
                        Some(Response::SendProduct { product }) => {
                            if let Some(product) = product {
                                println!("{product}");
                            } else {
                                println!("No product found");
                            }
                        }
                        Some(res) => error!(res = ?res, "Unexpected response"),
                        None => warn!("No response"),
                    };
                }
                Commands::GetFinancials { id, symbol, name } => {
                    let query = if let Some(id) = id {
                        ProductQuery::Id(id.clone())
                    } else if let Some(symbol) = symbol {
                        ProductQuery::Symbol(symbol.clone())
                    } else if let Some(name) = name {
                        ProductQuery::Name(name.clone())
                    } else {
                        panic!("No valid argument provided for GetProduct");
                    };
                    let msg = server::Request::GetFinancials { query };
                    if let Some(Response::SendFinancials { financials }) = client.write(msg).await {
                        if let Some(financials) = financials {
                            println!("{financials:#?}");
                        } else {
                            println!("No financials found");
                        }
                    } else {
                        warn!("Unexpected response");
                    }
                }
                Commands::GetCandles { id, symbol, name } => {
                    let query = if let Some(id) = id {
                        ProductQuery::Id(id.clone())
                    } else if let Some(symbol) = symbol {
                        ProductQuery::Symbol(symbol.clone())
                    } else if let Some(name) = name {
                        ProductQuery::Name(name.clone())
                    } else {
                        panic!("No valid argument provided for GetProduct");
                    };
                    let msg = server::Request::GetCandles { query };
                    client.write(msg).await.or_else(|| {
                        warn!("No response");
                        None
                    });
                }
                Commands::GetSingleAllocation {
                    id,
                    mode,
                    risk,
                    risk_free,
                    symbol,
                    name,
                } => {
                    let query = id.map_or_else(
                        || {
                            symbol.map_or_else(
                                || {
                                    name.map_or_else(
                                        || {
                                            panic!("No valid argument provided for GetProduct");
                                        },
                                        ProductQuery::Name,
                                    )
                                },
                                ProductQuery::Symbol,
                            )
                        },
                        ProductQuery::Id,
                    );
                    let msg = server::Request::GetSingleAllocation {
                        query,
                        mode,
                        risk,
                        risk_free,
                    };
                    client.write(msg).await.or_else(|| {
                        warn!("No response");
                        None
                    });
                }
                Commands::GetPortfolio => {
                    let msg = server::Request::GetPortfolio {};
                    match client.write(msg).await {
                        Some(Response::SendPortfolio { portfolio }) => {
                            if let Some(portfolio) = portfolio {
                                println!("{portfolio}");
                            } else {
                                println!("No portfolio calculated");
                            }
                        }
                        Some(_) => error!("Unexpected response"),
                        None => warn!("No response"),
                    }
                }
                Commands::RecalculateSl { nstd, max_percent } => {
                    let msg = server::Request::RecalculateSl { nstd, max_percent };
                    match client.write(msg).await {
                        Some(Response::SendRecalcucatetSl { table }) => {
                            if let Some(table) = table {
                                println!("{table}");
                            }
                        }
                        Some(_) => error!("Unexpected response"),
                        None => warn!("No response"),
                    }
                }
                Commands::CalculatePortfolio {
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
                    let req = server::Request::CalculatePortfolio {
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
                    match client.write(req).await {
                        Some(Response::SendPortfolio { portfolio }) => {
                            if let Some(portfolio) = portfolio {
                                println!("{portfolio}");
                            } else {
                                println!("No portfolio calculated");
                            }
                        }
                        Some(_) => error!("Unexpected response"),
                        None => warn!("No response"),
                    }
                }
                Commands::CleanUp => {
                    let msg = server::Request::CleanUp;
                    client.write(msg).await.or_else(|| {
                        warn!("No response");
                        None
                    });
                }
                Commands::GetTransactions { from_date, to_date } => {
                    dbg!(from_date, to_date);
                    // let msg = server::Request::GetTransactions { from_date, to_date };
                    // match client.write(msg).await {
                    //     Some(Response::SendTransactions { table }) => {
                    //         if let Some(table) = table {
                    //             println!("{}", table);
                    //         } else {
                    //             println!("No transactions found");
                    //         }
                    //     }
                    //     Some(_) => error!("Unexpected response"),
                    //     None => warn!("No response"),
                    // }
                }
                Commands::GetOrders => {
                    let msg = server::Request::GetOrders;
                    match client.write(msg).await {
                        Some(Response::SendOrders { table }) => {
                            if let Some(table) = table {
                                println!("{table}");
                            } else {
                                println!("No orders found");
                            }
                        }
                        Some(_) => error!("Unexpected response"),
                        None => warn!("No response"),
                    }
                }
                Commands::Server => {
                    let addr = Ipv4Addr::new(127, 0, 0, 1);
                    let socket = SocketAddrV4::new(addr, port);
                    match server::Server::new(socket).await {
                        Ok(server) => {
                            let pptr = Puppeter::default();
                            let settings = Settings::new().await;
                            let _settings_address = PuppetBuilder::new(settings.clone())
                                .spawn(&pptr)
                                .await
                                .unwrap();
                            let server_address =
                                PuppetBuilder::new(server).spawn(&pptr).await.unwrap();
                            server_address.send(server::RunServer).await.unwrap();
                            let _db_address =
                                PuppetBuilder::new(Db::new()).spawn(&pptr).await.unwrap();
                            let degiro =
                                Degiro::new(&settings.username, &settings.password).unwrap();
                            let _degiro_address =
                                PuppetBuilder::new(degiro).spawn(&pptr).await.unwrap();
                            let _calculator_address =
                                PuppetBuilder::new(Calculator::new(settings.clone()))
                                    .spawn(&pptr)
                                    .await
                                    .unwrap();
                        }
                        Err(err) => println!("{err}"),
                    }

                    tokio::select! {
                        _ = signal::ctrl_c() => {
                            println!("Ctrl-C received, shutting down");
                        },
                    }
                }
            }
        } else {
            let addr = Ipv4Addr::new(127, 0, 0, 1);
            let socket = SocketAddrV4::new(addr, port);
            match server::Server::new(socket).await {
                Ok(server) => {
                    let pptr = Puppeter::default();
                    let settings = Settings::new().await;
                    PuppetBuilder::new(settings.clone())
                        .spawn(&pptr)
                        .await
                        .unwrap();
                    let server_address = PuppetBuilder::new(server).spawn(&pptr).await.unwrap();
                    server_address.send(server::RunServer).await.unwrap();
                    PuppetBuilder::new(Db::new()).spawn(&pptr).await.unwrap();
                    let degiro = Degiro::new(&settings.username, &settings.password).unwrap();
                    PuppetBuilder::new(degiro).spawn(&pptr).await.unwrap();
                    PuppetBuilder::new(Calculator::new(settings.clone()))
                        .spawn(&pptr)
                        .await
                        .unwrap();
                    let _r = ui::show(pptr, settings);
                }
                Err(_err) => todo!(),
            }
        };
        Ok(())
    }
}
