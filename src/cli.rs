use std::net::{Ipv4Addr, SocketAddrV4};

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use comfy_table::{presets::UTF8_BORDERS_ONLY, Table};
use degiro_rs::{
    api::{account::AccountConfigExt, login::Authorize},
    client::client_status::{Authorized, Unauthorized},
    prelude::*,
    util::{Period, ProductCategory},
};
use qualsdorf::{
    average_drawdown::{self, AverageDrawdownExt},
    std::StdExt,
};
use strum::EnumString;

use crate::{
    data::candles::CandlesHandler,
    portfolio::{RiskMode, SingleAllocation},
    prelude::*,
    tcp::{self, ClientBuilder},
    App,
};

#[derive(Debug, Parser)]
pub struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Test,
    Config,
    FetchData {
        id: Option<String>,
    },
    GetData {
        id: String,
    },
    GetPortfolio,
    SingleAllocation {
        id: String,
        #[clap(long)]
        mode: RiskMode,
        #[clap(long)]
        risk: f64,
        #[clap(long)]
        risk_free: f64,
    },
    CalculatePortfolio {
        #[clap(long)]
        mode: RiskMode,
        #[clap(long)]
        risk: f64,
        #[clap(long)]
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
    Server {
        #[clap(short, long, default_value = "9123")]
        port: u16,
    },
    Send {
        #[clap(short, long, default_value = "9123")]
        port: u16,
    },
}

impl App<Unauthorized> {
    pub async fn authorize(self) -> Result<App<Authorized>> {
        let client = self.degiro.login().await?.account_config().await?;
        let app = App {
            settings: self.settings,
            degiro: client,
        };
        Ok(app)
    }
}

impl App<Unauthorized> {
    pub async fn run(self) -> Result<()> {
        let cli = Cli::parse();
        match cli.command {
            Commands::Test => {
                // 332111 - msft
                let app = self.authorize().await?;
                let quotes = app
                    .degiro
                    .quotes("332111", &Period::P50Y, &Period::P1M)
                    .await
                    .unwrap();
                dbg!(quotes);
            }
            Commands::Config => todo!(),
            Commands::FetchData { id } => {
                let app = self.authorize().await?;
                if let Some(id) = id {
                    println!("Fetching data for {}", &id);
                    app.candles_handler(&id).download().await?;
                    app.product_handler(&id).download().await?;
                } else {
                    for (id, name) in app.settings.assets.iter() {
                        // Align ID to 10 characters to the right. rust
                        println!("Fetching data for {:>10} - {}", id, name);
                        if app.candles_handler(id).download().await.is_err() {
                            println!("Failed to fetch candles data for {:>10} - {}", &id, &name);
                        };
                        if app.product_handler(id).download().await.is_err() {
                            println!("Failed to fetch product data for {:>10} - {}", &id, &name);
                        };
                    }
                }
            }
            Commands::GetData { id } => {
                let app = self.authorize().await?;
                let product = app.product_handler(&id).take().await?;
                dbg!(product);
            }
            Commands::SingleAllocation {
                id,
                mode,
                risk,
                risk_free,
            } => {
                let app = self.authorize().await?;
                let product = app.product_handler(&id).take().await?;
                let candles = app.candles_handler(&id).take().await?;
                let allocation = candles
                    .single_allocation(mode, risk, risk_free, &Period::P1Y, &Period::P1M)
                    .await?;
                println!("{} - {} : {}", id, product.inner.name, allocation)
            }
            Commands::GetPortfolio => {
                let app = self.authorize().await?;
                let portfolio = app.portfolio().await?;
                dbg!(portfolio.cash().value());
            }
            Commands::RecalculateSl { n } => {
                let app = self.authorize().await?;
                let portfolio = app.portfolio().await?.current().products();
                let mut table = comfy_table::Table::new();
                let header = vec![
                    comfy_table::Cell::new("id"),
                    comfy_table::Cell::new("name"),
                    comfy_table::Cell::new("symbol"),
                    comfy_table::Cell::new("date"),
                    comfy_table::Cell::new("price"),
                    comfy_table::Cell::new("avg dd")
                        .set_alignment(comfy_table::CellAlignment::Right),
                    comfy_table::Cell::new("stop loss")
                        .set_alignment(comfy_table::CellAlignment::Right),
                ];
                table.set_header(header);
                table.load_preset(UTF8_BORDERS_ONLY);
                for position in portfolio.0.iter() {
                    let product = app.product_handler(&position.inner.id).take().await;
                    let candles = app.candles_handler(&position.inner.id).take().await;
                    if let (Ok(product), Ok(candles)) = (product, candles) {
                        if let Some(avg_dd) = candles.average_drawdown(12) {
                            if let Some(Some(avg_dd_value)) = avg_dd.values.last() {
                                let last_price = candles.open.last().unwrap();
                                let stop_loss = last_price * (1.0 - avg_dd_value * n as f64);
                                table.add_row(vec![
                                    comfy_table::Cell::new(product.inner.id.clone()),
                                    comfy_table::Cell::new(format!(
                                        "{:<24}",
                                        product.inner.name.chars().take(24).collect::<String>()
                                    )),
                                    comfy_table::Cell::new(product.inner.symbol.clone()),
                                    comfy_table::Cell::new(
                                        candles.time.last().unwrap().to_string(),
                                    ),
                                    comfy_table::Cell::new(last_price)
                                        .set_alignment(comfy_table::CellAlignment::Right),
                                    comfy_table::Cell::new(format!("{:.2}", avg_dd_value))
                                        .set_alignment(comfy_table::CellAlignment::Right),
                                    comfy_table::Cell::new(format!("{:.2}", stop_loss))
                                        .set_alignment(comfy_table::CellAlignment::Right),
                                ]);
                            }
                        }
                    } else {
                        eprintln!("Failed to get data for {}", &position.inner.id);
                    };
                }
                print!("{}", table);
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
                let app = self.authorize().await?;
                let mut calculator = app
                    .portfolio_calculator(
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
                    )
                    .await;
                calculator.remove_invalid().calculate().await;
                print!("{}", calculator.as_table());
            }
            Commands::Server { port } => {
                let addr = Ipv4Addr::new(127, 0, 0, 1);
                let socket = SocketAddrV4::new(addr, port);
                let app = self.clone().authorize().await?;
                match tcp::Server::new(socket, app).await {
                    Ok(mut server) => server.run().await,
                    Err(err) => println!("{err}"),
                }
            }
            Commands::Send { port } => {
                let addr = Ipv4Addr::new(127, 0, 0, 1);
                let socket = SocketAddrV4::new(addr, port);
                let mut client = ClientBuilder::new(socket).build().await.unwrap();
                if let Some(msg) = client.write(tcp::Msg::Ping).await {
                    dbg!(msg);
                }
            }
        };
        Ok(())
    }
}
