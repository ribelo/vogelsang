use anyhow::Result;

use crate::App;
use degiro_rs::{client::client_status::Authorized, prelude::*};

pub struct OrderHandler {
    pub id: String,
}

impl App<Authorized> {
    pub async fn portfolio(&self) -> Result<Portfolio> {
        let portfolio = self.degiro.portfolio().await?;

        Ok(portfolio)
    }
}
