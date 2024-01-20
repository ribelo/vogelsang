use std::net::{Ipv4Addr, SocketAddrV4};

use anyhow::Result;
use async_trait::async_trait;
use clap::{ArgGroup, Parser, Subcommand};
use degiro_rs::util::ProductCategory;
use master_of_puppets::{master_of_puppets::MasterOfPuppets, puppet::PuppetBuilder};
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
    server::{self, ClientBuilder, Response, Server},
    App,
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
    Authorize {},
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
        min_class: Option<ProductCategory>,
        #[clap(long)]
        max_class: Option<ProductCategory>,
        #[clap(long)]
        short_sales_constraint: bool,
        #[clap(long)]
        roic_wacc_delta: Option<f64>,
    },
    RecalculateSl {
        #[clap(short, default_value = "3")]
        n: usize,
    },
    CleanUp,
}

#[async_trait]
pub trait CliExt {
    async fn run(self) -> Result<()>;
}

#[async_trait]
impl CliExt for App {
    async fn run(self) -> Result<()> {
        let cli = Cli::parse();
        let port = cli.port;
        match cli.command {
            Some(cmd) => {
                let addr = Ipv4Addr::new(127, 0, 0, 1);
                let socket = SocketAddrV4::new(addr, port);
                let mut client = ClientBuilder::new(socket).build().await.unwrap();
                match cmd {
                    Commands::Authorize {} => {
                        info!("Authorizing...");
                        let msg = server::Request::Authorize {};
                        client.write(msg).await.or_else(|| {
                            warn!("No response");
                            None
                        });
                    }
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
                                    println!("{}", product);
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
                        match client.write(msg).await {
                            Some(Response::SendFinancials { financials }) => {
                                if let Some(financials) = financials {
                                    println!("{:#?}", financials);
                                } else {
                                    println!("No financials found");
                                }
                            }
                            _ => warn!("Unexpected response"),
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
                        let query = if let Some(id) = id {
                            ProductQuery::Id(id.clone())
                        } else if let Some(symbol) = symbol {
                            ProductQuery::Symbol(symbol.clone())
                        } else if let Some(name) = name {
                            ProductQuery::Name(name.clone())
                        } else {
                            panic!("No valid argument provided for GetProduct");
                        };
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
                                    println!("{}", portfolio);
                                } else {
                                    println!("No portfolio calculated");
                                }
                            }
                            Some(_) => error!("Unexpected response"),
                            None => warn!("No response"),
                        }
                    }
                    Commands::RecalculateSl { n } => {
                        let msg = server::Request::RecalculateSl { n };
                        match client.write(msg).await {
                            Some(Response::SendRecalcucatetSl { table }) => {
                                if let Some(table) = table {
                                    println!("{}", table);
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
                        min_class,
                        max_class,
                        short_sales_constraint,
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
                            min_class,
                            max_class,
                            short_sales_constraint,
                            roic_wacc_delta,
                        };
                        match client.write(req).await {
                            Some(Response::SendPortfolio { portfolio }) => {
                                if let Some(portfolio) = portfolio {
                                    println!("{}", portfolio);
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
                }
            }
            None => {
                let addr = Ipv4Addr::new(127, 0, 0, 1);
                let socket = SocketAddrV4::new(addr, port);
                match server::Server::new(socket).await {
                    Ok(server) => {
                        let mop = MasterOfPuppets::default();
                        let settings = Settings::new(None);
                        let _settings_address = PuppetBuilder::new(settings.clone())
                            .spawn(&mop)
                            .await
                            .unwrap();
                        let _server_address = PuppetBuilder::new(server).spawn(&mop).await.unwrap();
                        let _db_address = PuppetBuilder::new(Db::new()).spawn(&mop).await.unwrap();
                        let degiro = Degiro::new(&settings.username, &settings.password);
                        let _degiro_address = PuppetBuilder::new(degiro).spawn(&mop).await.unwrap();
                        let _calculator_address =
                            PuppetBuilder::new(Calculator::new(settings.clone()))
                                .spawn(&mop)
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
        };
        Ok(())
    }
}
