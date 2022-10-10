use std::collections::VecDeque;

use erfurt::candle::Candles;
use statrs::statistics::Statistics;

use crate::{downside_risk::DownsideRisk, Indicator, Return, Value};

#[derive(Debug)]
pub struct SortinoRatio {
    pub freq: u32,
    pub risk_free: f64,
    pub mar: f64,
    input: VecDeque<f64>,
    downside_risk: DownsideRisk,
    pub value: Option<f64>,
}

impl SortinoRatio {
    pub fn new(freq: u32, risk_free: f64, mar: f64) -> Self {
        Self {
            freq,
            risk_free,
            mar,
            input: VecDeque::with_capacity(freq as usize),
            downside_risk: DownsideRisk::new(freq, mar),
            value: None,
        }
    }
}

impl<'a> Value<'a> for SortinoRatio {
    type Output = Option<&'a f64>;

    fn value(&'a self) -> Self::Output {
        self.value.as_ref()
    }
}

impl Indicator for SortinoRatio {
    type Input = f64;
    fn feed(&mut self, ret: Self::Input) {
        self.downside_risk.feed(ret);
        self.input.push_back(ret);
        if self.input.len() > self.freq as usize {
            self.input.pop_front();
        }
        if self.input.len() == self.freq as usize {
            let downside_risk = self.downside_risk.value.unwrap();
            let mean = self.input.iter().mean();
            self.value = Some((mean - self.risk_free) / downside_risk);
        }
    }
}

pub trait SortinoRatioExt: Return {
    fn sortino_ratio(&self, freq: u32, risk_free: f64, mar: f64) -> Option<SortinoRatio> {
        let mut indicator = SortinoRatio::new(freq, risk_free, mar);
        if let Some(ret) = self.ret() {
            ret.into_iter().for_each(|v| indicator.feed(v));
            Some(indicator)
        } else {
            None
        }
    }
}

impl SortinoRatioExt for Candles {}

#[cfg(test)]
mod test {
    use float_cmp::assert_approx_eq;

    use crate::{sortino_ratio::SortinoRatio, Indicator};

    static XS: [f64; 10] = [
        0.003, 0.026, 0.015, -0.009, 0.014, 0.024, 0.015, 0.066, -0.014, 0.039,
    ];
    #[test]
    fn sortino_ratio() {
        let mut indicator = SortinoRatio::new(10, 0.0, 0.0);
        XS.iter().for_each(|x| indicator.feed(*x));
        assert_approx_eq!(f64, 3.401051, indicator.value.unwrap(), epsilon = 0.0000001);
    }
}
