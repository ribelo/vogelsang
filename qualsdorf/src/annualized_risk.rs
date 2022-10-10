use std::collections::VecDeque;

use erfurt::candle::Candles;
use statrs::statistics::Statistics;

use crate::{Indicator, Return, Value};

#[derive(Debug)]
pub struct AnnualizedRisk {
    pub freq: u32,
    pub input: VecDeque<f64>,
    pub value: Option<f64>,
}

impl AnnualizedRisk {
    pub fn new(freq: u32) -> Self {
        Self {
            freq,
            input: VecDeque::with_capacity(freq as usize),
            value: None,
        }
    }
}

impl<'a> Value<'a> for AnnualizedRisk {
    type Output = Option<&'a f64>;

    fn value(&'a self) -> Self::Output {
        self.value.as_ref()
    }
}

impl Indicator for AnnualizedRisk {
    type Input = f64;
    fn feed(&mut self, ret: Self::Input) {
        self.input.push_back(ret);
        if self.input.len() > self.freq as usize {
            self.input.pop_front();
        }
        if self.input.len() == self.freq as usize {
            self.value = Some(self.input.iter().std_dev() * (self.freq as f64).sqrt())
        }
    }
}

pub trait AnnualizedRiskExt: Return {
    fn annualized_risk(&self, freq: u32) -> Option<AnnualizedRisk> {
        let mut indicator = AnnualizedRisk::new(freq);
        if let Some(ret) = self.ret() {
            ret.into_iter().for_each(|v| indicator.feed(v));
            Some(indicator)
        } else {
            None
        }
    }
}

impl AnnualizedRiskExt for Candles {}

#[cfg(test)]
mod test {
    use crate::{annualized_risk::AnnualizedRisk, Indicator};
    use float_cmp::assert_approx_eq;

    static XS: [f64; 10] = [
        0.003, 0.026, 0.015, -0.009, 0.014, 0.024, 0.015, 0.066, -0.014, 0.039,
    ];
    #[test]
    fn annualized_risk() {
        let mut indicator = AnnualizedRisk::new(10);
        XS.iter().for_each(|x| indicator.feed(*x));
        assert_approx_eq!(
            f64,
            0.07346125206907078,
            indicator.value.unwrap(),
            epsilon = 0.0000001
        );
    }
}
