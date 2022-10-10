use std::sync::Arc;

use async_trait::async_trait;
use color_eyre::{eyre::eyre, Result};
use degiro::{api::product::Product, Period};
use ndarray::{Array, Array2, Dim, Array1};
use qualsdorf::{
    annualized_return::AnnualizedReturnExt, mode::Geometric,
    rolling_economic_drawdown::RollingEconomicDrawdownExt, sharpe_ratio::SharpeRatioExt, Return,
    Value,
};
use statrs::statistics::Statistics;

#[async_trait]
pub trait SingleAllocation {
    async fn single_allocation(
        &self,
        risk: f64,
        risk_free: f64,
        period: &Period,
        interval: &Period,
    ) -> Result<f64>;
}

#[async_trait]
impl SingleAllocation for Product {
    async fn single_allocation(
        &self,
        risk: f64,
        risk_free: f64,
        period: &Period,
        interval: &Period,
    ) -> Result<f64> {
        let freq = period.to_owned() / interval.to_owned();
        let candles = self.candles(period, interval).await?;
        let ret = candles
            .ret()
            .ok_or_else(|| eyre!("can't calculate return"))?;
        let std = ret.iter().std_dev();
        let sr = candles
            .sharpe_ratio(freq, risk_free)
            .ok_or_else(|| eyre!("can't calculate sharpe ratio"))?
            .value()
            .ok_or_else(|| eyre!("can't get value"))?
            .to_owned();
        let redp = candles
            .rolling_economic_drawndown(freq)
            .ok_or_else(|| eyre!("can't calculate rolling economic drawdown price"))?
            .value()
            .ok_or_else(|| eyre!("can't get value"))?
            .to_owned();
        let allocation = 1.0_f64.min(0.0_f64.max(
            (((dbg!(sr) / dbg!(std)) + 0.5) / (1.0 - risk.powf(2.0)))
                * 0.0_f64.max((dbg!(risk) - dbg!(redp)) / (1.0 - redp)),
        ));
        Ok(allocation)
    }
}

pub struct ProductsSeq(pub Vec<Arc<Product>>);

impl From<Vec<Arc<Product>>> for ProductsSeq {
    fn from(xs: Vec<Arc<Product>>) -> Self {
        ProductsSeq(xs)
    }
}

async fn redp_stats(
    product: Arc<Product>,
    risk: f64,
    risk_free: f64,
    period: &Period,
    interval: &Period,
) -> Result<Array<f64, Dim<[usize; 1]>>> {
    let freq = period.clone() / interval.clone();
    let candles = product.candles(period, interval).await?;
    let ret = candles
        .ret()
        .ok_or_else(|| eyre!("can't calculate return"))?;
    let std = ret.std_dev();
    let ann_ret = candles
        .annualized_return(Geometric, freq)
        .ok_or_else(|| eyre!("can't calculate annualized return"))?
        .value()
        .ok_or_else(|| eyre!("can't get value"))?
        .to_owned();
    let redp = candles
        .rolling_economic_drawndown(freq)
        .ok_or_else(|| eyre!("can't calculate redp"))?
        .value()
        .ok_or_else(|| eyre!("can't get value"))?
        .to_owned();
    let drift = 0.0_f64.max((ann_ret - risk_free) + (std.powf(2.0) / 2.0));
    let y = (1.0 / (1.0 - risk.powf(2.0))) * ((risk - redp) / (1.0 - redp));
    Ok(Array::from_vec(vec![std, drift, y]))
}

impl ProductsSeq {
    pub async fn redp_multiple_allocation(
        &self,
        risk: f64,
        risk_free: f64,
        period: &Period,
        interval: &Period,
        ) -> Result<Vec<(Arc<Product>, f64)>> {
        let mut xs = Array2::<f64>::zeros((0, 3));
        for p in self.0.clone() {
            // TODO:
            dbg!(&p.id, &p.name);
            let stats = redp_stats(p, risk, risk_free, period, interval).await?;
            xs.push_row(stats.view()).unwrap();
        }
        let std = xs.column(0);
        let drift = xs.column(1);
        let y = xs.column(2);
        let inv_std = std.map(|x| x.powf(-1.0));
        let matrix = inv_std * drift * y;
        let sum = matrix.sum();
        let mut r: Vec<(Arc<Product>, f64)> = Vec::new();
        if sum <= 1.0 {
            for (product, allocation) in self.0.iter().zip(matrix.into_iter()) {
                r.push((product.clone(), allocation));
            }
            Ok(r)
        } else {
            let matrix = matrix.map(|x| x / sum);
            for (product, allocation) in self.0.iter().zip(matrix.into_iter()) {
                r.push((product.clone(), allocation));
            }
            Ok(r)
        }
    }
}

#[cfg(test)]
mod test {

    use degiro::client::ClientBuilder;

    use super::{SingleAllocation, ProductsSeq};

    #[tokio::test]
    async fn single_allocation() {
        let username = std::env::args().nth(2).expect("no username given");
        let password = std::env::args().nth(3).expect("no password given");
        let mut builder = ClientBuilder::default();
        let client = builder
            .username(&username)
            .password(&password)
            .build()
            .unwrap();
        let product = client.product_by_id("1089390").await.unwrap();
        let allocation = product
            .single_allocation(0.3, 0.0, &degiro::Period::P1Y, &degiro::Period::P1M)
            .await
            .unwrap();
        dbg!(product, allocation);
    }
    #[tokio::test]
    async fn multiple_allocation() {
        let username = std::env::args().nth(2).expect("no username given");
        let password = std::env::args().nth(3).expect("no password given");
        let mut builder = ClientBuilder::default();
        let client = builder
            .username(&username)
            .password(&password)
            .build()
            .unwrap();
        let p1 = client.product_by_id("1089390").await.unwrap();
        let p2 = client.product_by_id("332111").await.unwrap();
        let pxs = ProductsSeq(vec![p1, p2]);
        let x = pxs.redp_multiple_allocation(0.3, 0.0, &degiro::Period::P1Y, &degiro::Period::P1M).await.unwrap();
        dbg!(x);
    }
}
