use std::path::Path;

use anyhow::Result;
use bincode;

use degiro_rs::{client::Client, util::Period};
use erfurt::candle::Candles;
use thiserror::Error;
use tokio::io::AsyncWriteExt;

use crate::App;

use super::{DataHandler, DataHandlerError};

#[derive(Debug, Default)]
pub struct CandlesHandlerBuilder {
    id: Option<String>,
    interval: Option<Period>,
    degiro: Option<Client>,
    data_path: Option<String>,
}

#[derive(Error, Debug)]
pub enum CandlesHandlerBuilderError {
    #[error("The ID is missing")]
    NoId,
    #[error("Interval is missing")]
    NoInterval,
    #[error("Degiro client is missing")]
    NoDegiroClient,
    #[error("Data path is missing")]
    NoDataPath,
}

impl CandlesHandlerBuilder {
    pub fn id(mut self, id: String) -> CandlesHandlerBuilder {
        self.id = Some(id);
        self
    }

    pub fn interval(mut self, interval: Period) -> CandlesHandlerBuilder {
        self.interval = Some(interval);
        self
    }

    pub fn degiro(mut self, degiro: Client) -> CandlesHandlerBuilder {
        self.degiro = Some(degiro);
        self
    }

    pub fn data_path(mut self, data_path: String) -> CandlesHandlerBuilder {
        self.data_path = Some(data_path);
        self
    }

    pub fn build(self) -> Result<CandlesHandler, CandlesHandlerBuilderError> {
        if self.id.is_none() {
            Err(CandlesHandlerBuilderError::NoId)
        } else if self.interval.is_none() {
            Err(CandlesHandlerBuilderError::NoInterval)
        } else if self.degiro.is_none() {
            Err(CandlesHandlerBuilderError::NoDegiroClient)
        } else if self.data_path.is_none() {
            Err(CandlesHandlerBuilderError::NoDataPath)
        } else {
            Ok(CandlesHandler::new(
                self.id.unwrap(),
                self.interval.unwrap(),
                self.degiro.unwrap(),
                self.data_path.unwrap(),
            ))
        }
    }
}

pub struct CandlesHandler {
    pub id: String,
    pub interval: Period,
    degiro: Client,
    candles: Option<Candles>,
    path: String,
}

impl<'a> CandlesHandler {
    pub fn new(id: String, interval: Period, degiro: Client, data_path: String) -> CandlesHandler {
        CandlesHandler {
            path: format!("{}/candles_{}_{}.json", &data_path, &id, &interval),
            id,
            interval,
            degiro,
            candles: None,
        }
    }
}

#[async_trait::async_trait]
impl DataHandler for CandlesHandler {
    type Output = Candles;

    async fn fetch(&mut self) -> Result<&Self, DataHandlerError> {
        let candles: Candles = self
            .degiro
            .quotes(&self.id, Period::P50Y, self.interval)
            .await?
            .into();
        self.candles.replace(candles);
        Ok(self)
    }

    async fn save(&mut self) -> Result<(), DataHandlerError> {
        let path = Path::new(&self.path);
        let parent = path.parent().unwrap();
        if !tokio::fs::try_exists(&parent).await.unwrap() {
            tokio::fs::create_dir_all(&parent).await?;
        }
        let data = self.candles.as_ref().unwrap();
        let mut file = tokio::fs::File::create(&self.path).await?;
        match serde_json::to_string(data) {
            Ok(bytes) => Ok(file.write_all(&bytes.into_bytes()).await?),
            Err(err) => Err(DataHandlerError::SerializeError(err.to_string())),
        }
        // match bincode::serialize(data) {
        //     Ok(bytes) => Ok(file.write_all(&bytes).await?),
        //     Err(err) => Err(DataHandlerError::SerializeError(err.to_string())),
        // }
    }

    async fn download(&mut self) -> Result<&mut Self, DataHandlerError> {
        self.fetch().await?;
        self.save().await?;

        Ok(self)
    }

    async fn read(&mut self) -> Result<&mut Self, DataHandlerError> {
        let bytes = tokio::fs::read(&self.path).await?;
        match serde_json::from_slice::<Candles>(&bytes) {
            Ok(candles) => {
                self.candles = Some(candles);

                Ok(self)
            }
            Err(err) => {
                println!("{:#?}", err);
                Err(DataHandlerError::DeserializeError(err.to_string()))
            }
        }
    }

    async fn get(&mut self) -> Result<&Candles, DataHandlerError> {
        if self.candles.is_none() && self.read().await.is_err() {
            self.download().await?;
        }
        Ok(self.candles.as_ref().unwrap())
    }

    async fn take(mut self) -> Result<Candles, DataHandlerError> {
        if self.candles.is_none() && self.read().await.is_err() {
            self.download().await?;
        }
        Ok(self.candles.take().unwrap())
    }
}

impl App {
    pub fn candles_handler(&self, id: impl ToString) -> CandlesHandler {
        CandlesHandler::new(
            id.to_string(),
            Period::P1M,
            self.degiro.clone(),
            self.settings.data_path.clone(),
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::data::DataHandler;

    #[tokio::test]
    async fn candles_handler_test() {
        let app = crate::App::new();
        let mut handler = app.candles_handler("1157690");
        handler.read().await.unwrap();
        let product = handler.get().await.unwrap();
        println!("{:#?}", product);
    }
}
