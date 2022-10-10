use std::collections::VecDeque;

use erfurt::candle::Candles;

use crate::{Indicator, Return, Value};

#[derive(Debug)]
pub struct UpsidePotential {
    pub freq: u32,
    pub mar: f64,
    input: VecDeque<f64>,
    pub value: Option<f64>,
}

impl UpsidePotential {
    pub fn new(freq: u32, mar: f64) -> Self {
        Self {
            freq,
            mar,
            input: VecDeque::with_capacity(freq as usize),
            value: None,
        }
    }
}

impl<'a> Value<'a> for UpsidePotential {
    type Output = Option<&'a f64>;

    fn value(&'a self) -> Self::Output {
        self.value.as_ref()
    }
}

impl Indicator for UpsidePotential {
    type Input = f64;
    fn feed(&mut self, ret: Self::Input) {
        self.input.push_back(ret);
        if self.input.len() > self.freq as usize {
            self.input.pop_front();
        }
        if self.input.len() == self.freq as usize {
            self.value = Some(self.input.iter().fold(0.0, |acc, x| {
                acc + (x - self.mar).max(0.0) / self.input.len() as f64
            }));
        }
    }
}

pub trait UpsidePotentialExt: Return {
    fn upside_potential(&self, freq: u32, mar: f64) -> Option<UpsidePotential> {
        let mut indicator = UpsidePotential::new(freq, mar);
        if let Some(ret) = self.ret() {
            ret.into_iter().for_each(|v| indicator.feed(v));
            Some(indicator)
        } else {
            None
        }
    }
}

impl UpsidePotentialExt for Candles {}

#[cfg(test)]
mod test {
    use float_cmp::assert_approx_eq;

    use crate::{upside_potential::UpsidePotential, Indicator};

    static XS: [f64; 10] = [
        0.003, 0.026, 0.015, -0.009, 0.014, 0.024, 0.015, 0.066, -0.014, 0.039,
    ];
    #[test]
    fn downside_potential() {
        let mut indicator = UpsidePotential::new(10, 0.1 / 100.0);
        XS.iter().for_each(|x| indicator.feed(*x));
        assert_approx_eq!(f64, 0.0194, indicator.value.unwrap(), epsilon = 0.0000001);
    }
}
