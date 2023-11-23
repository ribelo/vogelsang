use anyhow::Result;
use degiro_rs::prelude::Portfolio;

use crate::App;

pub struct OrderHandler {
    pub id: String,
}

impl App {
    pub async fn portfolio(&self) -> Result<Portfolio> {
        let portfolio = self.degiro.portfolio().await?;

        Ok(portfolio)
    }
}
