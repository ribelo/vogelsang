use google_sheets4::{
    self,
    hyper_rustls::{self, HttpsConnector},
};
use hyper::client::HttpConnector;

pub struct Hub {
    pub sheets: google_sheets4::Sheets<HttpsConnector<HttpConnector>>,
}

impl Hub {
    pub async fn default() -> Self {
        let key = google_sheets4::oauth2::read_service_account_key("service-key.json")
            .await
            .expect("unable to read service account key");
        let auth = google_sheets4::oauth2::ServiceAccountAuthenticator::builder(key)
            .build()
            .await
            .expect("unable to auth using service account");
        let sheets = google_sheets4::Sheets::new(
            hyper::Client::builder().build(
                hyper_rustls::HttpsConnectorBuilder::new()
                    .with_native_roots()
                    .https_only()
                    .enable_http1()
                    .build(),
            ),
            auth,
        );

        Hub { sheets }
    }
    pub async fn read_sheet(&self, id: &str, sheet: &str) -> Vec<Vec<String>> {
        self.sheets
            .spreadsheets()
            .values_get(id, sheet)
            .doit()
            .await
            .unwrap()
            .1
            .values
            .unwrap()
    }
}

#[cfg(test)]
mod test {
    use super::Hub;

    #[tokio::test]
    async fn read_sheet() {
        let hub = Hub::default().await;
        let sheet = hub
            .read_sheet("1WhlxGPOXgjK7xzdAznB-Ag-yPU54CdH-S8gnytv5Pac", "stocks")
            .await;
        dbg!(sheet);
    }
}
