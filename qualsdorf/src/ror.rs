use crate::{Indicator, Return, Value};
use erfurt::candle::Candles;
use std::collections::VecDeque;

#[derive(Debug)]
pub struct RoR {
    pub freq: u32,
    pub input: VecDeque<f64>,
    pub value: Option<f64>,
}

impl RoR {
    pub fn new(freq: u32) -> Self {
        RoR {
            freq,
            input: VecDeque::with_capacity(freq as usize),
            value: None,
        }
    }
}

impl<'a> Value<'a> for RoR {
    type Output = Option<&'a f64>;

    fn value(&'a self) -> Self::Output {
        self.value.as_ref()
    }
}

impl Indicator for RoR {
    type Input = f64;
    fn feed(&mut self, ret: Self::Input) {
        self.input.push_back(ret);
        if self.input.len() > self.freq as usize {
            self.input.pop_front();
        }
        if self.input.len() == self.freq as usize {
            let arr: Vec<f64> = self
                .input
                .iter()
                .map(|x| x + 1.0)
                .scan(1.0, |acc, x| {
                    *acc *= x;
                    Some(*acc)
                })
                .collect();
            let (x, y) = (arr.first().unwrap(), arr.last().unwrap());
            self.value = Some(y / x - 1.0);
        }
    }
}

pub trait RoRExt: Return {
    fn ror(&self, freq: u32) -> Option<RoR> {
        let mut indicator = RoR::new(freq);
        if let Some(ret) = self.ret() {
            ret.into_iter().for_each(|v| indicator.feed(v));
            Some(indicator)
        } else {
            None
        }
    }
}

impl RoRExt for Candles {}

#[cfg(test)]
mod test {
    use float_cmp::assert_approx_eq;

    use crate::{ror::RoR, Indicator};

    static XS: [f64; 10] = [
        0.003, 0.026, 0.015, -0.009, 0.014, 0.024, 0.015, 0.066, -0.014, 0.039,
    ];
    #[test]
    fn ror() {
        let mut indicator = RoR::new(10);
        XS.iter().for_each(|x| indicator.feed(*x));
        assert_approx_eq!(f64, 0.187793, indicator.value.unwrap(), epsilon = 0.000001);
    }
}
