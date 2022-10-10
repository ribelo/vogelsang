use std::collections::HashMap;

use async_recursion::async_recursion;
use color_eyre::{eyre::eyre, Result};
use reqwest::{header, Url};

use crate::{account::Account, client::Client};

impl Client {
    #[async_recursion]
    pub async fn fetch_account_data(&self) -> Result<&Self> {
        let paths = self.paths.read().await;
        let session_id = self.session_id.read().await.as_ref();
        match (&session_id, &paths.pa_url) {
            (Some(session_id), Some(pa_url)) => {
                let url = Url::parse(pa_url)?.join("client")?;
                let req = self
                    .http_client
                    .get(url)
                    .query(&[("sessionId", &session_id)])
                    .header(header::REFERER, &paths.referer);
                let res = req.send().await?;
                match res.error_for_status() {
                    Ok(res) => {
                        let mut body = res.json::<HashMap<String, Account>>().await?;
                        let account_data = body.remove("data").ok_or(eyre!("data key not found"))?;
                        let mut account = self.account.write().await;
                        *account = Some(account_data);
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
            (None, _) => {
                self.login().await?.fetch_account_data().await
            }
            (Some(_), None) => {
                self.login()
                    .await?
                    .fetch_account_config()
                    .await?
                    .fetch_account_data()
                    .await
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::client::ClientBuilder;

    #[tokio::test]
    async fn fetch_account_data() {
        let username = std::env::args().nth(2).expect("no username given");
        let password = std::env::args().nth(3).expect("no password given");
        let mut builder = ClientBuilder::default();
        let client = builder
            .username(&username)
            .password(&password)
            .build()
            .unwrap();
        client
            .login()
            .await
            .unwrap()
            .fetch_account_data()
            .await
            .unwrap();
        dbg!(&client.account);
    }
}
