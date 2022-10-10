use async_recursion::async_recursion;
use color_eyre::{eyre::eyre, Result};
use reqwest::{header, Url};
use std::collections::HashMap;

use crate::{account::AccountInfo, client::SharedClient};

impl SharedClient {
    #[async_recursion]
    pub async fn fetch_account_info(&self) -> Result<&Self> {
        let mut inner = self.inner.try_lock().unwrap();
        match (&inner.session_id, &inner.account, &inner.paths.trading_url) {
            (Some(session_id), Some(account), Some(trading_url)) => {
                let url = Url::parse(trading_url)?
                    .join(&inner.paths.account_info_path)?
                    .join(&format!(
                        "{};jsessionid={}",
                        account.int_account, session_id
                    ))?;
                let req = inner
                    .http_client
                    .get(url)
                    .query(&[("sessionId", &session_id)])
                    .header(header::REFERER, &inner.paths.referer);
                let res = req.send().await?;
                match res.error_for_status() {
                    Ok(res) => {
                        let mut body = res.json::<HashMap<String, AccountInfo>>().await?;
                        let info = body.remove("data").ok_or(eyre!("data key not found"))?;
                        inner.account.as_mut().unwrap().info = Some(info);
                        Ok(self)
                    }
                    Err(err) => match err.status().unwrap().as_u16() {
                        401 => {
                            drop(inner);
                            self.login().await?.fetch_account_info().await
                        }
                        _ => Err(eyre!(err)),
                    },
                }
            }
            (None, _, _) => {
                drop(inner);
                self.login().await?.fetch_account_info().await
            }
            (Some(_), _, _) => {
                drop(inner);
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
        dbg!(&client.inner.lock().await.account);
    }
}
