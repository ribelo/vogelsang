use erfurt::candle::Candles;
use statrs::statistics::Statistics;

use crate::{Indicator, continuous_drawdown::ContinousDrawdown, Value, Return};

#[derive(Debug)]
pub struct AverageDrawdown {
    pub freq: u32,
    continuous_drawdown: ContinousDrawdown,
    pub value: Option<f64>,
}

impl AverageDrawdown {
    pub fn new(freq: u32) -> Self {
        Self {
            freq,
            continuous_drawdown: ContinousDrawdown::new(freq),
            value: None,
        }
    }
}

impl<'a> Value<'a> for AverageDrawdown {
    type Output = Option<&'a f64>;

    fn value(&'a self) -> Self::Output {
        self.value.as_ref()
    }
}

impl Indicator for AverageDrawdown {
    type Input = f64;
    fn feed(&mut self, ret: Self::Input) {
        self.continuous_drawdown.feed(ret);
        if let Some(xs) = &self.continuous_drawdown.value {
            self.value = Some(xs.iter().mean());
        }

    }
}

pub trait AverageDrawdownExt: Return {
    fn average_drawdown(&self, freq: u32) -> Option<AverageDrawdown> {
        let mut indicator = AverageDrawdown::new(freq);
        if let Some(ret) = self.ret() {
            ret.into_iter().for_each(|v| indicator.feed(v));
            Some(indicator)
        } else {
            None
        }
    }
}

impl AverageDrawdownExt for Candles {}

#[cfg(test)]
mod test {
    use float_cmp::assert_approx_eq;
    use crate::Indicator;
    use super::AverageDrawdown;

    static XS: [f64; 10] = [
        0.003, 0.026, 0.015, -0.009, 0.014, 0.024, 0.015, 0.066, -0.014, 0.039,
    ];

    #[test]
    fn average_drawdown() {
        let mut indicator = AverageDrawdown::new(10);
        XS.iter().for_each(|x| indicator.feed(*x));
        assert_approx_eq!(f64, 0.0115, indicator.value.unwrap(), epsilon = 0.0000001)
    }
}
