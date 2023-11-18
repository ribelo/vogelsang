use anyhow::{anyhow, Result};
use async_trait::async_trait;
use degiro_rs::{
    api::{product::Product, quotes::Quotes},
    util::Period,
};
use erfurt::candle::Candles;
use erfurt::prelude::*;
use nalgebra as na;
use qualsdorf::{
    rolling_economic_drawdown::RollingEconomicDrawdownExt, sharpe_ratio::SharpeRatioExt, Indicator,
    ReturnExt,
};
use statrs::statistics::Statistics;
use strum::EnumString;
use tap::Tap;

#[derive(Debug)]
pub struct LSV {
    pub freq: usize,
    pub input: Vec<f64>,
    pub values: Vec<Option<f64>>,
}

impl LSV {
    pub fn new(freq: usize) -> Self {
        Self {
            freq,
            input: Vec::with_capacity(freq),
            values: Vec::with_capacity(freq),
        }
    }
}

impl Indicator for LSV {
    type Input = f64;
    type Output = f64;

    fn feed(&mut self, ret: Self::Input) {
        // Add the raw return value to the input list
        self.input.push(ret);

        // If we have enough data, calculate the average of the last `self.freq` squared min elements
        if self.input.len() >= self.freq {
            let last_elements: Vec<f64> = self.input[self.input.len() - self.freq..].to_vec();
            let sum: f64 = last_elements
                .iter()
                .map(|&x| f64::min(x, 0.0).powf(2.0))
                .sum();
            let count = last_elements.len() as f64;
            let avg = sum / count;

            // Calculate E[min(rt, 0)]^2
            self.values.push(Some(avg));
        } else {
            self.values.push(None);
        }
    }

    fn last(&self) -> Option<&Self::Output> {
        self.values.last().and_then(|v| v.as_ref())
    }

    fn iter(&self) -> Box<dyn Iterator<Item = Option<&Self::Output>> + '_> {
        Box::new(self.values.iter().map(Option::as_ref))
    }
}

pub trait LsvExt: ReturnExt {
    fn lsv(&self, freq: usize) -> Option<LSV> {
        let mut indicator = LSV::new(freq);
        if let Some(ret) = self.ret() {
            ret.into_iter().for_each(|v| indicator.feed(v));
            Some(indicator)
        } else {
            None
        }
    }
}

impl<T> LsvExt for T where T: CandlesExt {}

#[derive(Debug, Clone, EnumString)]
pub enum RiskMode {
    STD,
    LSV,
}

#[async_trait]
pub trait SingleAllocation {
    async fn single_allocation(
        &self,
        mode: RiskMode,
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
        mode: RiskMode,
        risk: f64,
        risk_free: f64,
        period: &Period,
        interval: &Period,
    ) -> Result<f64> {
        let candles: Candles = self.quotes(period, interval).await?.into();
        candles
            .single_allocation(mode, risk, risk_free, period, interval)
            .await
    }
}

#[async_trait]
impl SingleAllocation for Quotes {
    async fn single_allocation(
        &self,
        mode: RiskMode,
        risk: f64,
        risk_free: f64,
        period: &Period,
        interval: &Period,
    ) -> Result<f64> {
        Into::<Candles>::into(self)
            .single_allocation(mode, risk, risk_free, period, interval)
            .await
    }
}

#[async_trait]
impl SingleAllocation for Candles {
    async fn single_allocation(
        &self,
        mode: RiskMode,
        risk: f64,
        risk_free: f64,
        period: &Period,
        interval: &Period,
    ) -> Result<f64> {
        let freq = period.div(interval);
        let risk_metric = match mode {
            RiskMode::STD => {
                let ret = self
                    .ret()
                    .ok_or_else(|| anyhow!("can't calculate return"))?;
                ret.iter().std_dev()
            }
            RiskMode::LSV => self
                .lsv(freq)
                .ok_or_else(|| anyhow!("can't calculate lsv"))?
                .last()
                .ok_or_else(|| anyhow!("can't get value"))?
                .to_owned(),
        };
        let sr = self
            .sharpe_ratio(freq, risk_free)
            .ok_or_else(|| anyhow!("can't calculate sharpe ratio"))?
            .last()
            .ok_or_else(|| anyhow!("can't get value"))?
            .to_owned();
        let redp = self
            .rolling_economic_drawndown(freq)
            .ok_or_else(|| anyhow!("can't calculate rolling economic drawdown price"))?
            .last()
            .ok_or_else(|| anyhow!("can't get value"))?
            .to_owned();
        let allocation = 1.0_f64.min(0.0_f64.max(
            dbg!(((sr / risk_metric) + 0.5) / (1.0 - risk.powf(2.0)))
                * dbg!(dbg!(risk - redp) / (1.0 - redp)),
        ));
        Ok(allocation)
    }
}

pub struct AssetsSeq(pub Vec<(Product, Candles)>);

impl From<Vec<(Product, Candles)>> for AssetsSeq {
    fn from(xs: Vec<(Product, Candles)>) -> Self {
        AssetsSeq(xs)
    }
}

fn na_covariance(matrix: &na::DMatrix<f64>) -> na::DMatrix<f64> {
    println!("Input matrix shape: {:?}", matrix.shape());
    let nrows = matrix.nrows(); // Number of instruments
    let ncols = matrix.ncols(); // Number of observations

    // The covariance matrix should be a square matrix with dimensions equal to the number of instruments
    let mut covariance_matrix = na::DMatrix::zeros(nrows, nrows);
    println!(
        "Covariance matrix dimensions: {:?}",
        covariance_matrix.shape()
    );

    let means = matrix.row_mean(); // Using row_mean for mean of each feature

    // Compute the covariance matrix
    for i in 0..nrows {
        for j in i..nrows {
            let mut sum = 0.0;

            // Compute the sum of products of deviations from the mean
            for k in 0..ncols {
                sum += (matrix[(i, k)] - means[k]) * (matrix[(j, k)] - means[k]);
            }

            let cov = sum / (ncols as f64 - 1.0); // Use ncols here as it's the number of observations
            covariance_matrix[(i, j)] = cov;
            if i != j {
                covariance_matrix[(j, i)] = cov; // Covariance is symmetric
            }
        }
    }

    covariance_matrix
}

impl AssetsSeq {
    pub async fn redp_multiple_allocation(
        &self,
        mode: RiskMode,
        risk: f64,
        risk_free: f64,
        period: &Period,
        interval: &Period,
        short_sales_constraint: bool,
    ) -> Result<Vec<(Product, f64)>> {
        let freq = period.div(interval);
        let mut rets_rows = Vec::new();

        let mut ys = Vec::new();
        let mut mu = Vec::new();
        for (_p, candles) in self.0.clone() {
            println!("id: {}", &_p.inner.id);
            let ret = candles
                .ret()
                .ok_or_else(|| anyhow!("can't calculate return"))?;
            let row = na::RowDVector::from_vec(ret.clone());
            rets_rows.push(row);
            let risk_metric = match mode {
                RiskMode::STD => ret.clone().std_dev(),
                RiskMode::LSV => candles
                    .lsv(freq)
                    .ok_or_else(|| anyhow!("can't calculate lsv"))?
                    .last()
                    .ok_or_else(|| anyhow!("can't get value"))?
                    .to_owned(),
            };
            let mean_ret = ret.mean();
            let redp = candles
                .rolling_economic_drawndown(freq)
                .ok_or_else(|| anyhow!("can't calculate redp"))?
                .last()
                .ok_or_else(|| anyhow!("can't get value"))?
                .to_owned();
            let y = (1.0 / (1.0 - risk.powf(2.0))) * ((risk - redp) / (1.0 - redp));
            let mut drift = mean_ret - risk_free + risk_metric.powf(2.0) / 2.0;
            if short_sales_constraint {
                drift = drift.max(0.0);
            };
            ys.push(y);
            mu.push(drift);
        }
        let rets = na::DMatrix::from_rows(&rets_rows)
            .tap(|x| println!("rets dimensions: {:?} {}", x.shape(), x.as_slice().len()));
        let ys = na::DVector::<f64>::from_vec(ys);
        let mu =
            na::DVector::<f64>::from_vec(mu).tap(|x| println!("mu dimensions: {:?}", x.shape()));
        let sigma = na_covariance(&rets).tap(|x| println!("sigma dimensions: {:?}", x.shape()));
        if !sigma.is_invertible() {
            return Err(anyhow!("Covariance matrix is not invertible"));
        };
        let Some(sigma_inv) = sigma.try_inverse() else {
            return Err(anyhow!("Can't invert covariance matrix"));
        };
        let diag_y = na::DMatrix::<f64>::from_diagonal(&ys)
            .tap(|x| println!("diag-Y dimensions: {:?}", x.shape()));

        let mut x_redp = ((&sigma_inv * &mu).transpose() * &sigma_inv * &diag_y)
            .as_slice()
            .to_vec();
        if short_sales_constraint {
            x_redp = x_redp.iter().map(|&x| x.max(0.0)).collect();
        };

        let x_redp_sum_abs = x_redp.iter().map(|x| x.abs()).sum::<f64>();
        let x_redp_normalized = x_redp.iter().map(|x| x / x_redp_sum_abs);
        let mut r: Vec<(Product, f64)> = Vec::new();
        for ((p, _), allocation) in self.0.iter().zip(x_redp_normalized) {
            if short_sales_constraint {
                if allocation > 0.0 {
                    // println!("{} allocation {}", &p.inner.name, allocation,);
                    r.push((p.clone(), allocation));
                }
            } else {
                r.push((p.clone(), allocation));
            }
        }
        Ok(r)
    }
}

#[cfg(test)]
mod test {

    use degiro_rs::{
        api::{account::AccountConfigExt, login::Authorize, product::ProductExt},
        client::Client,
        util::Period,
    };

    use super::*;

    #[tokio::test]
    async fn single_allocation() {
        let client = Client::new_from_env()
            .login()
            .await
            .unwrap()
            .account_config()
            .await
            .unwrap();
        let product = client.product("1089390").await.unwrap();
        let allocation = product
            .single_allocation(RiskMode::STD, 0.3, 0.0, &Period::P1Y, &Period::P1M)
            .await
            .unwrap();
        dbg!(product, allocation);
    }
    // TODO:
    // #[tokio::test]
    // async fn multiple_allocation() {
    //     let username = std::env::args().nth(2).expect("no username given");
    //     let password = std::env::args().nth(3).expect("no password given");
    //     let mut builder = ClientBuilder::default();
    //     let client = builder
    //         .username(&username)
    //         .password(&password)
    //         .build()
    //         .unwrap()
    //         .login()
    //         .await
    //         .unwrap()
    //         .account_config()
    //         .await
    //         .unwrap();
    //     let p1 = client.product("1089390").await.unwrap();
    //     let p2 = client.product("332111").await.unwrap();
    //     let pxs = ValorSeq(vec![p1, p2]);
    //     let x = pxs
    //         .redp_multiple_allocation(0.3, 0.0, &Period::P1Y, &Period::P1M)
    //         .await
    //         .unwrap();
    //     dbg!(x);
    // }
}
