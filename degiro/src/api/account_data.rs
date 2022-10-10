use std::collections::HashMap;

use async_recursion::async_recursion;
use color_eyre::{eyre::eyre, Result};
use reqwest::{header, Url};

use crate::{account::Account, client::SharedClient};

impl SharedClient {
    #[async_recursion]
    pub async fn fetch_account_data(&self) -> Result<&Self> {
        let mut inner = self.inner.try_lock().unwrap();
        match (&inner.session_id, &inner.paths.pa_url) {
            (Some(session_id), Some(pa_url)) => {
                let url = Url::parse(pa_url)?.join("client")?;
                let req = inner
                    .http_client
                    .get(url)
                    .query(&[("sessionId", &session_id)])
                    .header(header::REFERER, &inner.paths.referer);
                let res = req.send().await?;
                match res.error_for_status() {
                    Ok(res) => {
                        let mut body = res.json::<HashMap<String, Account>>().await?;
                        let account = body.remove("data").ok_or(eyre!("data key not found"))?;
                        inner.account = Some(account);
                        Ok(self)
                    }
                    Err(err) => match err.status().unwrap().as_u16() {
                        401 => {
                            drop(inner);
                            self.login().await?.fetch_account_config().await
                        }
                        _ => Err(eyre!(err)),
                    },
                }
            }
            (None, _) => {
                drop(inner);
                self.login().await?.fetch_account_data().await
            }
            (Some(_), None) => {
                drop(inner);
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
        dbg!(&client.inner.lock().await.account);
    }
}
