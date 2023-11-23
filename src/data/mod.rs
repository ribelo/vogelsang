use thiserror::Error;

pub mod candles;
pub mod orders;
pub mod products;

#[derive(Debug, Error)]
pub enum DataHandlerError {
    #[error("Unauthorized")]
    Unauthorized,
    #[error("Failed to fetch the required data: {0}")]
    FetchError(String),
    #[error("Failed to save the data: {0}")]
    SaveError(String),
    #[error("Failed to download the required data: {0}")]
    DownloadError(String),
    #[error("Failed to read the data: {0}")]
    ReadError(String),
    #[error("Failed to get the requested output: {0}")]
    GetError(String),
    #[error("Failed to deserialize the data: {0}")]
    DeserializeError(String),
    #[error("Failed to serialize the data: {0}")]
    SerializeError(String),
}

impl From<std::io::Error> for DataHandlerError {
    fn from(e: std::io::Error) -> Self {
        DataHandlerError::SaveError(e.to_string())
    }
}

impl From<degiro_rs::client::ClientError> for DataHandlerError {
    fn from(e: degiro_rs::client::ClientError) -> Self {
        match e {
            degiro_rs::client::ClientError::Unauthorized => Self::Unauthorized,
            _ => Self::FetchError(e.to_string()),
        }
    }
}

impl From<serde_json::Error> for DataHandlerError {
    fn from(e: serde_json::Error) -> Self {
        DataHandlerError::FetchError(e.to_string())
    }
}

// impl From<reqwest::Error> for DataHandlerError {
//     fn from(e: reqwest::Error) -> Self {
//         DataHandlerError::FetchError(e.to_string())
//     }
// }

#[async_trait::async_trait]
pub trait DataHandler: Sized {
    type Output;
    async fn fetch(&mut self) -> Result<&Self, DataHandlerError>;

    async fn save(&mut self) -> Result<(), DataHandlerError>;

    async fn download(&mut self) -> Result<&mut Self, DataHandlerError>;

    async fn read(&mut self) -> Result<&mut Self, DataHandlerError>;

    async fn get(&mut self) -> Result<&Self::Output, DataHandlerError>;

    async fn take(mut self) -> Result<Self::Output, DataHandlerError>;
}
