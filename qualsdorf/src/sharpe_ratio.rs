use std::collections::VecDeque;

use erfurt::candle::Candles;
use statrs::statistics::Statistics;

use crate::{Indicator, Return, Value};

#[derive(Debug)]
pub struct SharpeRatio {
    pub freq: u32,
    pub risk_free: f64,
    pub input: VecDeque<f64>,
    pub value: Option<f64>,
}

impl SharpeRatio {
    pub fn new(freq: u32, risk_free: f64) -> Self {
        Self {
            freq,
            risk_free,
            input: VecDeque::with_capacity(freq as usize),
            value: None,
        }
    }
}

impl<'a> Value<'a> for SharpeRatio {
    type Output = Option<&'a f64>;

    fn value(&'a self) -> Self::Output {
        self.value.as_ref()
    }
}

impl Indicator for SharpeRatio {
    type Input = f64;
    fn feed(&mut self, ret: Self::Input) {
        self.input.push_back(ret);
        if self.input.len() > self.freq as usize {
            self.input.pop_front();
        }
        if self.input.len() == self.freq as usize {
            self.value =
                Some((self.input.iter().mean() - self.risk_free) / self.input.iter().std_dev());
        }
    }
}

pub trait SharpeRatioExt: Return {
    fn sharpe_ratio(&self, freq: u32, risk_free: f64) -> Option<SharpeRatio> {
        let mut indicator = SharpeRatio::new(freq, risk_free);
        if let Some(ret) = self.ret() {
            ret.into_iter().for_each(|v| indicator.feed(v));
            Some(indicator)
        } else {
            None
        }
    }
}

impl SharpeRatioExt for Candles {}

#[cfg(test)]
mod test {
    use float_cmp::assert_approx_eq;

    use crate::{sharpe_ratio::SharpeRatio, Indicator};

    static XS: [f64; 10] = [
        0.003, 0.026, 0.015, -0.009, 0.014, 0.024, 0.015, 0.066, -0.014, 0.039,
    ];
    #[test]
    fn sharpe_ratio() {
        let mut indicator = SharpeRatio::new(10, 0.0);
        XS.iter().for_each(|x| indicator.feed(*x));
        assert_approx_eq!(
            f64,
            0.7705391,
            indicator.value.unwrap(),
            epsilon = 0.0000001
        );
    }
}
