use std::collections::VecDeque;

use erfurt::candle::Candles;

use crate::{Indicator, Return, Value};

#[derive(Debug)]
pub struct Drawdown {
    pub freq: u32,
    input: VecDeque<f64>,
    pub value: Option<f64>,
}

impl Drawdown {
    pub fn new(freq: u32) -> Self {
        Self {
            freq,
            input: VecDeque::with_capacity(freq as usize),
            value: None,
        }
    }
}

impl<'a> Value<'a> for Drawdown {
    type Output = Option<&'a f64>;

    fn value(&'a self) -> Self::Output {
        self.value.as_ref()
    }
}

impl Indicator for Drawdown {
    type Input = f64;
    fn feed(&mut self, ret: Self::Input) {
        self.input.push_back(ret);
        if self.input.len() > self.freq as usize {
            self.input.pop_front();
        }
        if self.input.len() == self.freq as usize {
            let mut s = 1.0;
            let mut mx = 1.0;
            let mut r = Vec::with_capacity(self.input.len());
            for x in self.input.iter() {
                let v = (1.0 + x) * s;
                mx = v.max(mx);
                s = v;
                let dr = (mx - v) / mx;
                r.push(dr);
            }
            self.value = r.last().copied();
        }
    }
}

pub trait DrawdownExt: Return {
    fn drawdown(&self, freq: u32) -> Option<Drawdown> {
        let mut indicator = Drawdown::new(freq);
        if let Some(ret) = self.ret() {
            ret.into_iter().for_each(|v| indicator.feed(v));
            Some(indicator)
        } else {
            None
        }
    }
}

impl DrawdownExt for Candles {}

#[cfg(test)]
mod test {
    use float_cmp::assert_approx_eq;

    use crate::Indicator;

    use super::Drawdown;
    static XS: [f64; 10] = [
        0.003, 0.026, 0.015, -0.009, 0.014, 0.024, 0.015, 0.066, -0.014, 0.039,
    ];

    #[test]
    fn drawdown() {
        let mut indicator = Drawdown::new(10);
        XS.iter().for_each(|x| indicator.feed(*x));
        assert_approx_eq!(f64, 0.0, indicator.value.unwrap(), epsilon = 0.0000001)
    }
}
