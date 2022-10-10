use async_recursion::async_recursion;
use color_eyre::{eyre::eyre, Result};
use reqwest::{header, Url};
use std::collections::HashMap;

use crate::{account::AccountInfo, client::Client};

impl Client {
    #[async_recursion]
    pub async fn fetch_account_info(&self) -> Result<&Self> {
        let paths = self.paths.read().await;
        let account = self.account.read().await.as_ref();
        let session_id = self.session_id.read().await.as_ref();
        match (&self.session_id, &account, &paths.trading_url) {
            (Some(session_id), Some(account), Some(trading_url)) => {
                let url = Url::parse(trading_url)?
                    .join(&self.paths.account_info_path)?
                    .join(&format!(
                        "{};jsessionid={}",
                        account.int_account, session_id
                    ))?;
                let req = &self 
                    .http_client
                    .get(url)
                    .query(&[("sessionId", &session_id)])
                    .header(header::REFERER, &self.paths.referer);
                let res = req.send().await?;
                match res.error_for_status() {
                    Ok(res) => {
                        let mut body = res.json::<HashMap<String, AccountInfo>>().await?;
                        let info = body.remove("data").ok_or(eyre!("data key not found"))?;
                        account.info = Some(info);
                        Ok(self)
                    }
                    Err(err) => match err.status().unwrap().as_u16() {
                        401 => {
                            self.login().await?.fetch_account_info().await
                        }
                        _ => Err(eyre!(err)),
                    },
                }
            }
            (None, _, _) => {
                self.login().await?.fetch_account_info().await
            }
            (Some(_), _, _) => {
                self.fetch_account_data().await?.fetch_account_info().await
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::client::ClientBuilder;

    #[tokio::test]
    async fn fetch_account_info() {
        let username = std::env::args().nth(2).expect("no username given");
        let password = std::env::args().nth(3).expect("no password given");
        let mut builder = ClientBuilder::default();
        let client = builder
            .username(&username)
            .password(&password)
            .build()
            .unwrap();
        client.fetch_account_info().await.unwrap();
        dbg!(&client.account);
    }
}
