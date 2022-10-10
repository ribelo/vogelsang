use std::collections::HashMap;

use color_eyre::{eyre::eyre, Result};
use serde::Deserialize;
use strum::{EnumString, ParseError};

#[derive(Debug, Default, Deserialize, Clone, Eq, PartialEq, EnumString, Hash)]
pub enum Currency {
    USD,
    #[default]
    EUR,
    CHF,
    JPY,
    PLN,
    GBP,
}

#[derive(Debug, Default, Deserialize, Clone, PartialEq)]
pub struct Money(pub Currency, pub f64);

impl Money {
    pub fn currency(&self) -> Currency {
        self.0.clone()
    }
}

impl std::ops::Add for Money {
    type Output = Result<Self>;

    fn add(self, rhs: Self) -> Self::Output {
        match (&self, &rhs) {
            (Money(a, x), Money(b, y)) if a == b => Ok(Money(a.clone(), x + y)),
            _ => Err(eyre!(
                "can't sub diffrent currency {:#?} + {:#?}",
                self,
                rhs
            )),
        }
    }
}

impl std::ops::Sub for Money {
    type Output = Result<Self>;

    fn sub(self, rhs: Self) -> Self::Output {
        match (&self, &rhs) {
            (Money(a, x), Money(b, y)) if a == b => Ok(Money(a.clone(), x - y)),
            _ => Err(eyre!(
                "can't sub diffrent currency {:#?} + {:#?}",
                self,
                rhs
            )),
        }
    }
}

impl std::ops::Neg for Money {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Money(self.0, -self.1)
    }
}

impl std::ops::Mul for Money {
    type Output = Result<Self>;

    fn mul(self, rhs: Self) -> Self::Output {
        match (&self, &rhs) {
            (Money(a, x), Money(b, y)) if a == b => Ok(Money(a.clone(), x * y)),
            _ => Err(eyre!(
                "can't sub diffrent currency {:#?} + {:#?}",
                self,
                rhs
            )),
        }
    }
}

impl std::ops::Div for Money {
    type Output = Result<Self>;

    fn div(self, rhs: Self) -> Self::Output {
        match (&self, &rhs) {
            (Money(a, x), Money(b, y)) if a == b => Ok(Money(a.clone(), x / y)),
            _ => Err(eyre!(
                "can't sub diffrent currency {:#?} + {:#?}",
                self,
                rhs
            )),
        }
    }
}

impl TryFrom<HashMap<String, f64>> for Money {
    type Error = ParseError;

    fn try_from(m: HashMap<String, f64>) -> Result<Self, Self::Error> {
        if !m.is_empty() {
            let mut money = Money(Currency::USD, 0.0);
            if let Some((k, &v)) = m.iter().next() {
                let curr: Currency = k.parse()?;
                money.0 = curr;
                money.1 = v;
            }
            Ok(money)
        } else {
            Err(Self::Error::VariantNotFound)
        }
    }
}
