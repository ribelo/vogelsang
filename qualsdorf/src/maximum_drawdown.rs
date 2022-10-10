use erfurt::candle::Candles;

use crate::{continuous_drawdown::ContinousDrawdown, Indicator, Return, Value};

#[derive(Debug)]
pub struct MaximumDrawdown {
    pub freq: u32,
    continuous_drawdown: ContinousDrawdown,
    pub value: Option<f64>,
}

impl<'a> Value<'a> for MaximumDrawdown {
    type Output = Option<&'a f64>;

    fn value(&'a self) -> Self::Output {
        self.value.as_ref()
    }
}

impl MaximumDrawdown {
    pub fn new(freq: u32) -> Self {
        Self {
            freq,
            continuous_drawdown: ContinousDrawdown::new(freq),
            value: None,
        }
    }
}

impl Indicator for MaximumDrawdown {
    type Input = f64;
    fn feed(&mut self, ret: Self::Input) {
        self.continuous_drawdown.feed(ret);
        if let Some(xs) = &self.continuous_drawdown.value {
            self.value = Some(statrs::statistics::Statistics::max(xs.iter()));
        }
    }
}

pub trait MaximumDrawdownExt: Return {
    fn upside_potential(&self, freq: u32) -> Option<MaximumDrawdown> {
        let mut indicator = MaximumDrawdown::new(freq);
        if let Some(ret) = self.ret() {
            ret.into_iter().for_each(|v| indicator.feed(v));
            Some(indicator)
        } else {
            None
        }
    }
}

impl MaximumDrawdownExt for Candles {}

#[cfg(test)]
mod test {
    use super::MaximumDrawdown;
    use crate::Indicator;
    use float_cmp::assert_approx_eq;

    static XS: [f64; 10] = [
        0.003, 0.026, 0.015, -0.009, 0.014, 0.024, 0.015, 0.066, -0.014, 0.039,
    ];

    #[test]
    fn maximum_drawdown() {
        let mut indicator = MaximumDrawdown::new(10);
        XS.iter().for_each(|x| indicator.feed(*x));
        assert_approx_eq!(f64, 0.0140, indicator.value.unwrap(), epsilon = 0.0000001)
    }
}
