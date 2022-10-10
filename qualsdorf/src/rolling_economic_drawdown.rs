use std::collections::VecDeque;

use erfurt::candle::Candles;
use statrs::statistics::Statistics;

use crate::{Indicator, Value};

#[derive(Debug)]
pub struct RollingEconomicDrawdown {
    pub freq: u32,
    input: VecDeque<f64>,
    pub value: Option<f64>,
}

impl RollingEconomicDrawdown {
    pub fn new(freq: u32) -> Self {
        Self {
            freq,
            input: VecDeque::with_capacity(freq as usize),
            value: None,
        }
    }
}

impl<'a> Value<'a> for RollingEconomicDrawdown {
    type Output = Option<&'a f64>;

    fn value(&'a self) -> Self::Output {
        self.value.as_ref()
    }
}

impl Indicator for RollingEconomicDrawdown {
    type Input = f64;
    fn feed(&mut self, close: Self::Input) {
        self.input.push_back(close);
        if self.input.len() > self.freq as usize {
            self.input.pop_front();
        }
        if self.input.len() == self.freq as usize {
            let mx = Statistics::max(self.input.iter());
            if let (Some(x), true) = (self.input.back(), !mx.is_nan()) {
                self.value = Some(1.0 - (dbg!(x) / dbg!(mx)))
            }
        }
    }
}

pub trait RollingEconomicDrawdownExt {
    fn rolling_economic_drawndown(&self, freq: u32) -> Option<RollingEconomicDrawdown>;
}

impl RollingEconomicDrawdownExt for Candles {
    fn rolling_economic_drawndown(&self, freq: u32) -> Option<RollingEconomicDrawdown> {
        if !self.close.is_empty() {
            let mut indicator = RollingEconomicDrawdown::new(freq);
            self.close.iter().for_each(|v| indicator.feed(*v));
            Some(indicator)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod test {
    use float_cmp::assert_approx_eq;

    use crate::Indicator;

    use super::RollingEconomicDrawdown;
    static XS: [f64; 10] = [
        0.003, 0.026, 0.015, -0.009, 0.014, 0.024, 0.015, 0.066, -0.014, 0.039,
    ];

    #[test]
    fn rolling_economic_drawndown() {
        let mut indicator = RollingEconomicDrawdown::new(10);
        XS.iter().for_each(|x| indicator.feed(*x));
        assert_approx_eq!(f64, 0.40909090909090917, indicator.value.unwrap(), epsilon = 0.0000001)
    }
}
