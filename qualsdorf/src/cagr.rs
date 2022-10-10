use std::collections::VecDeque;

use erfurt::candle::Candles;

use crate::{ror::RoR, Indicator, Return, Value};

#[derive(Debug)]
pub struct CAGR {
    pub freq: u32,
    pub p: f64,
    input: VecDeque<f64>,
    ror: RoR,
    pub value: Option<f64>,
}

impl CAGR {
    pub fn new(freq: u32, p: f64) -> Self {
        CAGR {
            freq,
            p,
            input: VecDeque::with_capacity(freq as usize),
            ror: RoR::new(freq),
            value: None,
        }
    }
}

impl<'a> Value<'a> for CAGR {
    type Output = Option<&'a f64>;

    fn value(&'a self) -> Self::Output {
        self.value.as_ref()
    }
}

impl Indicator for CAGR {
    type Input = f64;
    fn feed(&mut self, ret: Self::Input) {
        self.input.push_back(ret);
        self.ror.feed(ret);
        if self.input.len() > self.freq as usize {
            self.input.pop_front();
        }
        if let Some(ror) = self.ror.value() {
            self.value = Some((1.0 + ror).powf(self.p) - 1.0);
        }
    }
}

pub trait CAGRExt: Return {
    fn cagr(&self, freq: u32, p: f64) -> Option<CAGR> {
        let mut indicator = CAGR::new(freq, p);
        if let Some(ret) = self.ret() {
            ret.into_iter().for_each(|v| indicator.feed(v));
            Some(indicator)
        } else {
            None
        }
    }
}

impl CAGRExt for Candles {}

#[cfg(test)]
mod test {
    use float_cmp::assert_approx_eq;

    use crate::{cagr::CAGR, Indicator};

    static XS: [f64; 10] = [
        0.003, 0.026, 0.015, -0.009, 0.014, 0.024, 0.015, 0.066, -0.014, 0.039,
    ];
    #[test]
    fn cagr() {
        let mut indicator = CAGR::new(10, 12.0 / 10.0);
        XS.into_iter().for_each(|x| indicator.feed(x));
        assert_approx_eq!(f64, 0.229388, indicator.value.unwrap(), epsilon = 0.000001);
    }
}
