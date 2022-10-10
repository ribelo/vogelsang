use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use crate::{portfolio::ProductsSeq, settings::Settings};
use chrono::NaiveDate;
use clap::{Parser, Subcommand};
use color_eyre::{eyre::eyre, Result};
use degiro::{account::Account, api::product::Product, client::ClientBuilder};

#[derive(Debug, Parser)]
pub struct Cli {
    #[clap(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    AccountInfo,
    AccountData,
    AccountState {
        #[clap(short, long)]
        from_date: NaiveDate,
        #[clap(short, long)]
        to_date: NaiveDate,
    },
    AccountPortfolio,
    Config,
    CalculatePortfolio,
}

pub async fn run(settings: Settings) -> Result<()> {
    let cli = Cli::parse();
    match &cli.command {
        Some(command) => match &command {
            Commands::AccountInfo => {
                let client = ClientBuilder::default()
                    .username(&settings.username)
                    .password(&settings.password)
                    .build()?;
                client.fetch_account_info().await?;
                print!(
                    "{:#?}",
                    &client.inner.lock().await.account.as_ref().unwrap()
                );

                Ok(())
            }
            Commands::AccountData => {
                let client = ClientBuilder::default()
                    .username(&settings.username)
                    .password(&settings.password)
                    .build()?;
                client.fetch_account_data().await?;
                print!(
                    "{:#?}",
                    &client.inner.lock().await.account.as_ref().unwrap()
                );

                Ok(())
            }
            Commands::AccountState { from_date, to_date } => {
                let client = ClientBuilder::default()
                    .username(&settings.username)
                    .password(&settings.password)
                    .build()?;
                let state = client.account_state(from_date, to_date).await?;
                print!("{:#?}", &state);

                Ok(())
            }
            Commands::AccountPortfolio => {
                let client = ClientBuilder::default()
                    .username(&settings.username)
                    .password(&settings.password)
                    .build()?;
                let portfolio = client.portfolio().await?;
                print!("{:#?}", &portfolio);

                Ok(())
            }
            Commands::Config => {
                print!("{:#?}", &settings);

                Ok(())
            }
            Commands::CalculatePortfolio => {
                let client = ClientBuilder::default()
                    .username(&settings.username)
                    .password(&settings.password)
                    .build()?;
                let mut blacklisted: HashSet<String> = HashSet::new();

                'outer: loop {
                    dbg!("loop");
                    let mut stocks: Vec<Arc<Product>> = Vec::new();
                    for (id, _, _) in &settings.stocks {
                        dbg!(&id);
                        if !blacklisted.contains(id) {
                            let product = client.product_by_id(id).await?;
                            stocks.push(product.clone());
                        }
                    }
                    let seq = ProductsSeq(stocks);
                    let redp_allocation: Vec<_> = seq
                        .redp_multiple_allocation(
                            settings.risk,
                            settings.risk_free,
                            &settings.period,
                            &settings.interval,
                        )
                        .await?;
                    let mut actual_allocation: HashMap<(String, String), _> =
                        HashMap::new();
                    for (p, x) in redp_allocation {
                        let cash = settings.money.1 * x;
                        if cash >= p.close_price {
                            let n = (cash / p.close_price).round() as i64;
                            actual_allocation
                                .insert((p.id.clone(), p.symbol.clone()), (x, cash, n, p.close_price));
                        } else {
                            blacklisted.insert(p.id.clone());
                            continue 'outer;
                        }
                    }
                    dbg!(&actual_allocation);
                    return Ok(());
                }
            }
        },
        None => Err(eyre!("no command")),
    }
}
