use color_eyre::{eyre::eyre, Result};
use mime;
use reqwest::{header, Url};
use serde::Deserialize;
use serde_json::json;

use crate::client::Client;

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoginResponse {
    locale: Option<String>,
    session_id: Option<String>,
    status: i32,
    status_text: String,
}

impl Client {
    pub async fn login(&mut self) -> Result<&Self> {
        let paths = &self.paths.read().await;
        let base_url = &paths.base_api_url;
        let path_url = &paths.login_url_path;
        let url = Url::parse(&base_url)?.join(&path_url)?;
        let body = json!({
            "isPassCodeReset": false,
            "isRedirectToMobile": false,
            "password": &self.password,
            "username": &self.username,
        });
        let req = self
            .http_client
            .post(url)
            .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.to_string())
            .header(header::REFERER, &paths.referer)
            .json(&body)
            .query(&[("reason", "session_expired")]);

        let res = req.send().await.unwrap();
        match res.error_for_status() {
            Ok(res) => {
                let body = res.json::<LoginResponse>().await?;
                let mut session_id = self.session_id.write().await;
                *session_id = body.session_id;
                Ok(self)
            }
            Err(err) => Err(eyre!(err)),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::client::ClientBuilder;

    #[tokio::test]
    async fn login() {
        let username = std::env::args().nth(2).expect("no username given");
        let password = std::env::args().nth(3).expect("no password given");
        let mut builder = ClientBuilder::default();
        let _client = builder
            .username(&username)
            .password(&password)
            .build()
            .unwrap()
            .login()
            .await
            .unwrap();
    }
}
