#![allow(dead_code)]

use erfurt::candle::Candles;
use itertools::Itertools;
pub mod active_return;
pub mod annualized_return;
pub mod annualized_risk;
pub mod average_drawdown;
pub mod cagr;
pub mod continuous_drawdown;
pub mod downside_potential;
pub mod downside_risk;
pub mod drawndown;
pub mod maximum_drawdown;
pub mod sharpe_ratio;
pub mod sortino_ratio;
pub mod upside_potential;
pub mod ror;
pub mod rolling_economic_drawdown;

pub trait Indicator {
    type Input;
    fn feed(&mut self, first: Self::Input);
}

pub mod mode {
    #[derive(Clone, Debug)]
    pub struct Geometric;

    #[derive(Clone, Debug)]
    pub struct Simple;
}

pub trait Return {
    fn ret(&self) -> Option<Vec<f64>>;
}

pub trait Value<'a> {
    type Output;
    fn value(&'a self) -> Self::Output;
}

impl Return for Candles {
    fn ret(&self) -> Option<Vec<f64>> {
        if !self.is_empty() {
            let mut ret = vec![0.0];
            for (x, y) in self.close.iter().tuple_windows() {
                ret.push(y / x - 1.0)
            }
            Some(ret)
        } else {
            None
        }
    }
}
