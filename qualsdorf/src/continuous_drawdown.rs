use std::collections::VecDeque;

use erfurt::candle::Candles;

use crate::{Indicator, Return, Value};

#[derive(Debug)]
pub struct ContinousDrawdown {
    pub freq: u32,
    input: VecDeque<f64>,
    pub value: Option<Vec<f64>>,
}

impl ContinousDrawdown {
    pub fn new(freq: u32) -> Self {
        Self {
            freq,
            input: VecDeque::with_capacity(freq as usize),
            value: None,
        }
    }
}

impl<'a> Value<'a> for ContinousDrawdown {
    type Output = Option<&'a Vec<f64>>;

    fn value(&'a self) -> Self::Output {
        self.value.as_ref()
    }
}

impl Indicator for ContinousDrawdown {
    type Input = f64;
    fn feed(&mut self, ret: Self::Input) {
        self.input.push_back(ret);
        if self.input.len() > self.freq as usize {
            self.input.pop_front();
        }
        if self.input.len() == self.freq as usize {
            let mut xs = Vec::with_capacity(self.input.len());
            let mut s = 1.0;
            for (i, x) in self.input.iter().enumerate() {
                if i == 0 && x < &0.0 {
                    s = x + 1.0;
                    continue;
                } else if i == 0 && x > &0.0 {
                    s = 1.0;
                    continue;
                } else if i > 0 && x < &0.0 {
                    s *= x + 1.0;
                    continue;
                } else if i > 0 && x > &0.0 {
                    let dd = 1.0 - s;
                    if dd != 0.0 {
                        xs.push(dd);
                        s = 1.0;
                    }
                };
            }
            if s < 1.0 {
                let dd = 1.0 - s;
                if dd != 0.0 {
                    xs.push(dd)
                }
            }
            self.value = Some(xs);
        }
    }
}

pub trait ContinousDrawdownExt: Return {
    fn continuous_drawdown(&self, freq: u32) -> Option<ContinousDrawdown> {
        let mut indicator = ContinousDrawdown::new(freq);
        if let Some(ret) = self.ret() {
            ret.into_iter().for_each(|v| indicator.feed(v));
            Some(indicator)
        } else {
            None
        }
    }
}

impl ContinousDrawdownExt for Candles {}

#[cfg(test)]
mod test {
    use float_cmp::assert_approx_eq;

    use crate::Indicator;

    use super::ContinousDrawdown;
    static XS: [f64; 10] = [
        0.003, 0.026, 0.015, -0.009, 0.014, 0.024, 0.015, 0.066, -0.014, 0.039,
    ];

    #[test]
    fn drawdown() {
        let mut indicator = ContinousDrawdown::new(10);
        XS.iter().for_each(|x| indicator.feed(*x));
        let valid = vec![0.009, 0.014];
        valid
            .iter()
            .zip(indicator.value.unwrap().iter())
            .for_each(|(x, y)| assert_approx_eq!(f64, *x, *y, epsilon = 0.0000001))
    }
}
