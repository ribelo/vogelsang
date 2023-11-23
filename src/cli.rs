use std::net::{Ipv4Addr, SocketAddrV4};

use anyhow::Result;
use async_trait::async_trait;
use clap::{ArgGroup, Parser, Subcommand};
use degiro_rs::util::ProductCategory;
use eventual::eve::Eve;
use tokio::signal;
use tracing::{error, warn};

use crate::{
    data::products::ProductQuery,
    portfolio::RiskMode,
    server::{self, ClientBuilder, Response},
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
    Login {},
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
        freq: u32,
        #[clap(long)]
        money: f64,
        #[clap(long)]
        max_stocks: i32,
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
    },
    RecalculateSl {
        #[clap(short, default_value = "3")]
        n: u32,
    },
}

impl App {
    pub async fn authorize(&self) -> Result<()> {
        self.degiro.login().await?;
        self.degiro.account_config().await?;
        Ok(())
    }
}

#[async_trait]
pub trait CliExt {
    async fn run(self) -> Result<()>;
}

#[async_trait]
impl CliExt for Eve<App> {
    async fn run(self) -> Result<()> {
        let cli = Cli::parse();
        let port = cli.port;
        match cli.command {
            Some(cmd) => {
                let addr = Ipv4Addr::new(127, 0, 0, 1);
                let socket = SocketAddrV4::new(addr, port);
                let mut client = ClientBuilder::new(socket).build().await.unwrap();
                match cmd {
                    Commands::Login {} => {
                        let msg = server::Request::Login {};
                        if let Some(msg) = client.write(msg).await {
                            dbg!(msg);
                        }
                    }
                    Commands::FetchData { id } => {
                        let msg = server::Request::FetchData { id };
                        if let Some(msg) = client.write(msg).await {
                            dbg!(msg);
                        }
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
                        if let Some(msg) = client.write(msg).await {
                            dbg!(msg);
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
                        if let Some(msg) = client.write(msg).await {
                            dbg!(msg);
                        }
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
                        if let Some(msg) = client.write(msg).await {
                            dbg!(msg);
                        }
                    }
                    Commands::GetPortfolio => {
                        // let app = self.state.authorize().await?;
                        // let portfolio = app.portfolio().await?;
                        // dbg!(portfolio.cash().value());
                    }
                    Commands::RecalculateSl { n } => {
                        // let app = self.state.authorize().await?;
                        // let portfolio = app.portfolio().await?.current().products();
                        // let mut table = comfy_table::Table::new();
                        // let header = vec![
                        //     comfy_table::Cell::new("id"),
                        //     comfy_table::Cell::new("name"),
                        //     comfy_table::Cell::new("symbol"),
                        //     comfy_table::Cell::new("date"),
                        //     comfy_table::Cell::new("price"),
                        //     comfy_table::Cell::new("avg dd")
                        //         .set_alignment(comfy_table::CellAlignment::Right),
                        //     comfy_table::Cell::new("stop loss")
                        //         .set_alignment(comfy_table::CellAlignment::Right),
                        // ];
                        // table.set_header(header);
                        // table.load_preset(UTF8_BORDERS_ONLY);
                        // for position in portfolio.0.iter() {
                        //     let product = app.product_handler(&position.inner.id).take().await;
                        //     let candles = app.candles_handler(&position.inner.id).take().await;
                        //     if let (Ok(product), Ok(candles)) = (product, candles) {
                        //         if let Some(avg_dd) = candles.average_drawdown(12) {
                        //             if let Some(Some(avg_dd_value)) = avg_dd.values.last() {
                        //                 let last_price = candles.open.last().unwrap();
                        //                 let stop_loss = last_price * (1.0 - avg_dd_value * n as f64);
                        //                 table.add_row(vec![
                        //                     comfy_table::Cell::new(product.inner.id.clone()),
                        //                     comfy_table::Cell::new(format!(
                        //                         "{:<24}",
                        //                         product.inner.name.chars().take(24).collect::<String>()
                        //                     )),
                        //                     comfy_table::Cell::new(product.inner.symbol.clone()),
                        //                     comfy_table::Cell::new(
                        //                         candles.time.last().unwrap().to_string(),
                        //                     ),
                        //                     comfy_table::Cell::new(last_price)
                        //                         .set_alignment(comfy_table::CellAlignment::Right),
                        //                     comfy_table::Cell::new(format!("{:.2}", avg_dd_value))
                        //                         .set_alignment(comfy_table::CellAlignment::Right),
                        //                     comfy_table::Cell::new(format!("{:.2}", stop_loss))
                        //                         .set_alignment(comfy_table::CellAlignment::Right),
                        //                 ]);
                        //             }
                        //         }
                        //     } else {
                        //         eprintln!("Failed to get data for {}", &position.inner.id);
                        //     };
                        // }
                        // print!("{}", table);
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
                }
            }
            None => {
                let addr = Ipv4Addr::new(127, 0, 0, 1);
                let socket = SocketAddrV4::new(addr, port);
                match server::Server::new(socket, self.clone()).await {
                    Ok(mut server) => server.run().await,
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
