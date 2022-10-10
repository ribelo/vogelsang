use crate::{annualized_return::AnnualizedReturn, mode, Indicator, Value};

#[derive(Debug)]
struct ActiveReturn<T> {
    pub mode: T,
    pub freq: u32,
    first_annualized_return: AnnualizedReturn<T>,
    second_annualized_return: AnnualizedReturn<T>,
    pub value: Option<f64>,
}

impl<T: Clone> ActiveReturn<T> {
    pub fn new(mode: T, freq: u32) -> Self {
        ActiveReturn {
            mode: mode.clone(),
            freq,
            first_annualized_return: AnnualizedReturn::new(mode.clone(), freq),
            second_annualized_return: AnnualizedReturn::new(mode, freq),
            value: None,
        }
    }
}

impl<'a, T> Value<'a> for ActiveReturn<T> {
    type Output = Option<&'a f64>;

    fn value(&'a self) -> Self::Output {
        self.value.as_ref()
    }
}

impl Indicator for ActiveReturn<mode::Geometric> {
    type Input = (f64, f64);
    fn feed(&mut self, (first_input, second_input): Self::Input) {
        self.first_annualized_return.feed(first_input);
        self.second_annualized_return.feed(second_input);
        if let (Some(v1), Some(v2)) = (
            self.first_annualized_return.value,
            self.second_annualized_return.value,
        ) {
            self.value = Some(v1 - v2);
        };
    }
}

impl Indicator for ActiveReturn<mode::Simple> {
    type Input = (f64, f64);
    fn feed(&mut self, (first_input, second_input): Self::Input) {
        self.first_annualized_return.feed(first_input);
        self.second_annualized_return.feed(second_input);
        if let (Some(v1), Some(v2)) = (
            self.first_annualized_return.value,
            self.second_annualized_return.value,
        ) {
            self.value = Some(v1 - v2);
        };
    }
}

#[cfg(test)]
mod test {
    use float_cmp::assert_approx_eq;

    use crate::{active_return::ActiveReturn, mode, Indicator};

    static XS: [f64; 10] = [
        0.003, 0.026, 0.015, -0.009, 0.014, 0.024, 0.015, 0.066, -0.014, 0.039,
    ];
    static YS: [f64; 10] = [
        -0.005, 0.081, 0.04, -0.037, -0.061, 0.058, -0.049, -0.021, 0.062, 0.058,
    ];
    #[test]
    fn geometric() {
        let mut indicator = ActiveReturn::new(mode::Geometric, 10);
        XS.iter()
            .zip(YS.iter())
            .for_each(|(x, y)| indicator.feed((*x, *y)));
        assert_approx_eq!(
            f64,
            0.07183306403588108,
            indicator.value.unwrap(),
            epsilon = 0.0000001
        );
    }
}
