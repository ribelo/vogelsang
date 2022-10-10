use std::collections::HashMap;

use color_eyre::{eyre::eyre, Result};
use reqwest::{header, Url};
use serde::Deserialize;

use crate::client::Client;
use async_recursion::async_recursion;

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    pub(crate) allocations_url: String,
    pub(crate) beta_landing_path: String,
    pub(crate) client_id: i32,
    pub(crate) companies_service_url: String,
    pub(crate) dictionary_url: String,
    pub(crate) exante_reporting_url: String,
    pub(crate) favorites_url: String,
    pub(crate) feedback_url: String,
    pub(crate) i18n_url: String,
    pub(crate) landing_path: String,
    pub(crate) latest_searched_products_url: String,
    pub(crate) login_url: String,
    pub(crate) mobile_landing_path: String,
    pub(crate) pa_url: String,
    pub(crate) payment_service_url: String,
    pub(crate) product_notes_url: String,
    pub(crate) product_search_url: String,
    pub(crate) product_search_v2_url: String,
    pub(crate) product_types_url: String,
    pub(crate) refinitiv_agenda_url: String,
    pub(crate) refinitiv_clips_url: String,
    pub(crate) refinitiv_company_profile_url: String,
    pub(crate) refinitiv_company_ratios_url: String,
    pub(crate) refinitiv_esgs_url: String,
    pub(crate) refinitiv_estimates_url: String,
    pub(crate) refinitiv_financial_statements_url: String,
    pub(crate) refinitiv_insider_transactions_url: String,
    pub(crate) refinitiv_insiders_report_url: String,
    pub(crate) refinitiv_investor_url: String,
    pub(crate) refinitiv_news_url: String,
    pub(crate) refinitiv_shareholders_url: String,
    pub(crate) refinitiv_top_news_categories_url: String,
    pub(crate) reporting_url: String,
    pub(crate) session_id: String,
    pub(crate) settings_url: String,
    pub(crate) task_manager_url: String,
    pub(crate) trading_url: String,
    pub(crate) translations_url: String,
    pub(crate) vwd_chart_api_url: String,
    pub(crate) vwd_gossips_url: String,
    pub(crate) vwd_news_url: String,
    pub(crate) vwd_quotecast_service_url: String,
}

impl Client {
    #[async_recursion]
    pub async fn fetch_account_config(&self) -> Result<&Self> {
        let mut paths = self.paths.write().await;
        let url = Url::parse(&paths.base_api_url)?.join(&paths.account_config_path)?;
        let req = self
            .http_client
            .get(url)
            .header(header::REFERER, &paths.referer);
        let res = req.send().await?;
    
        match res.error_for_status() {
            Ok(res) => {
                let body = res.json::<HashMap<String, Response>>().await?;
                let data = body.get("data").ok_or(eyre!("data key not found"))?;
                let mut client_id = self.client_id.write().await;
                *client_id = Some(data.client_id);
                paths.pa_url = Some(data.pa_url.clone());
                paths.products_search_url = Some(data.product_search_url.clone());
                paths.trading_url = Some(data.trading_url.clone());
                paths.reporting_url = Some(data.reporting_url.clone());
                Ok(self)
            }
            Err(err) => match err.status().unwrap().as_u16() {
                401 => {
                    self.login().await?.fetch_account_config().await
                }
                _ => Err(eyre!(err)),
            },
        }
    }
}

#[cfg(test)]
mod test {
    use crate::client::ClientBuilder;

    #[tokio::test]
    async fn fetch_account_config() {
        let username = std::env::args().nth(2).expect("no username given");
        let password = std::env::args().nth(3).expect("no password given");
        let mut builder = ClientBuilder::default();
        let client = builder
            .username(&username)
            .password(&password)
            .build()
            .unwrap();
        client.fetch_account_config().await.unwrap();
    }
}
