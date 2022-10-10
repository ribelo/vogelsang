use std::collections::VecDeque;

use crate::{mode, Indicator, Return, Value};
use erfurt::candle::Candles;
use statrs::{self, statistics::Statistics};

#[derive(Debug)]
pub struct AnnualizedReturn<T> {
    pub mode: T,
    pub freq: u32,
    input: VecDeque<f64>,
    pub value: Option<f64>,
}

impl<T> AnnualizedReturn<T> {
    pub fn new(mode: T, freq: u32) -> Self {
        AnnualizedReturn {
            mode,
            freq,
            input: VecDeque::with_capacity(freq as usize),
            value: None,
        }
    }
}

impl<'a, T> Value<'a> for AnnualizedReturn<T> {
    type Output = Option<&'a f64>;

    fn value(&'a self) -> Self::Output {
        self.value.as_ref()
    }
}

pub trait AnnualizedReturnExt<T>: Return {
    fn annualized_return(&self, mode: T, freq: u32) -> Option<AnnualizedReturn<T>>;
}

impl Indicator for AnnualizedReturn<mode::Geometric> {
    type Input = f64;
    fn feed(&mut self, ret: Self::Input) {
        self.input.push_back(ret);
        if self.input.len() > self.freq as usize {
            self.input.pop_front();
        }
        if self.input.len() == self.freq as usize {
            let n = self.input.len();
            let ret = self
                .input
                .iter()
                .map(|x| x + 1.0)
                .fold(1.0, |acc, x| acc * x);
            let annret = ret.powf(self.freq as f64 / n as f64) - 1.0;
            self.value = Some(annret);
        }
    }
}

impl AnnualizedReturnExt<mode::Geometric> for Candles {
    fn annualized_return(
        &self,
        mode: mode::Geometric,
        freq: u32,
    ) -> Option<AnnualizedReturn<mode::Geometric>> {
        let mut indicator = AnnualizedReturn::new(mode, freq);
        if let Some(ret) = self.ret() {
            ret.into_iter().for_each(|v| indicator.feed(v));
            Some(indicator)
        } else {
            None
        }
    }
}

impl Indicator for AnnualizedReturn<mode::Simple> {
    type Input = f64;
    fn feed(&mut self, ret: Self::Input) {
        self.input.push_back(ret);
        if self.input.len() > self.freq as usize {
            self.input.pop_front();
        }
        if self.input.len() == self.freq as usize {
            let mean = self.input.iter().mean();
            self.value = Some(mean * self.freq as f64);
        }
    }
}

impl AnnualizedReturnExt<mode::Simple> for Candles {
    fn annualized_return(
        &self,
        mode: mode::Simple,
        freq: u32,
    ) -> Option<AnnualizedReturn<mode::Simple>> {
        let mut indicator = AnnualizedReturn::new(mode, freq);
        if let Some(ret) = self.ret() {
            ret.into_iter().for_each(|v| indicator.feed(v));
            Some(indicator)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{
        annualized_return::{mode, AnnualizedReturn},
        Indicator,
    };
    use float_cmp::assert_approx_eq;

    static XS: [f64; 10] = [
        0.003, 0.026, 0.015, -0.009, 0.014, 0.024, 0.015, 0.066, -0.014, 0.039,
    ];
    #[test]
    fn geometric() {
        let mut indicator = AnnualizedReturn::new(mode::Geometric, 10);
        XS.iter().for_each(|x| indicator.feed(*x));
        assert_approx_eq!(
            f64,
            0.19135615147149543,
            indicator.value.unwrap(),
            epsilon = 0.0000001
        );
    }
    #[test]
    fn simple() {
        let mut indicator = AnnualizedReturn::new(mode::Simple, 10);
        XS.iter().for_each(|x| indicator.feed(*x));
        assert_approx_eq!(f64, 0.179, indicator.value.unwrap(), epsilon = 0.0000001);
    }
}
