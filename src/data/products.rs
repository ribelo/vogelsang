use std::{path::Path, sync::Arc};

use degiro_rs::{
    api::product::{Product, ProductInner},
    client::Client,
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::io::AsyncWriteExt;

use crate::App;

use super::{DataHandler, DataHandlerError};

#[derive(Debug, Default)]
pub struct ProductHandlerBuilder {
    id: Option<String>,
    client: Option<Client>,
    data_path: Option<String>,
}

#[derive(Error, Debug)]
pub enum ProductHandlerBuilderError {
    #[error("The ID is missing")]
    NoId,
    #[error("Degiro client is missing")]
    NoDegiroClient,
    #[error("Data path is missing")]
    NoDataPath,
}

impl ProductHandlerBuilder {
    pub fn id(mut self, id: String) -> ProductHandlerBuilder {
        self.id = Some(id);
        self
    }

    pub fn degiro(mut self, degiro: Client) -> ProductHandlerBuilder {
        self.client = Some(degiro);
        self
    }

    pub fn data_path(mut self, data_path: String) -> ProductHandlerBuilder {
        self.data_path = Some(data_path);
        self
    }

    pub fn build(self) -> Result<ProductHandler, ProductHandlerBuilderError> {
        if self.id.is_none() {
            Err(ProductHandlerBuilderError::NoId)
        } else if self.client.is_none() {
            Err(ProductHandlerBuilderError::NoDegiroClient)
        } else if self.data_path.is_none() {
            Err(ProductHandlerBuilderError::NoDataPath)
        } else {
            Ok(ProductHandler::new(
                self.id.unwrap(),
                self.client.unwrap(),
                self.data_path.unwrap(),
            ))
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProductHandler {
    id: String,
    degiro: Client,
    path: String,
    product: Option<Product>,
}

impl PartialEq for ProductHandler {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl ProductHandler {
    pub fn new(id: String, degiro: Client, data_path: String) -> Self {
        let path = Path::new(&data_path)
            .join(format!("product_{}.json", &id))
            .to_str()
            .unwrap()
            .to_string();
        ProductHandler {
            path,
            id,
            degiro,
            product: None,
        }
    }
}

#[async_trait::async_trait]
impl DataHandler for ProductHandler {
    type Output = Product;

    async fn fetch(&mut self) -> Result<&Self, DataHandlerError> {
        let product = self.degiro.product(&self.id).await?;
        self.product.replace(product);
        Ok(self)
    }

    async fn save(&mut self) -> Result<(), DataHandlerError> {
        let path = Path::new(&self.path);
        let parent = path.parent().unwrap();
        if !tokio::fs::try_exists(&parent).await.unwrap() {
            tokio::fs::create_dir_all(&parent).await?;
        }
        let product = self.degiro.product(&self.id).await?;
        let mut file = tokio::fs::File::create(&self.path).await?;
        match serde_json::to_string(product.inner.as_ref()) {
            Ok(bytes) => Ok(file.write_all(&bytes.into_bytes()).await?),
            Err(err) => Err(DataHandlerError::SerializeError(err.to_string())),
        }
    }

    async fn download(&mut self) -> Result<&mut Self, DataHandlerError> {
        self.fetch().await?;
        self.save().await?;
        Ok(self)
    }

    async fn read(&mut self) -> Result<&mut Self, DataHandlerError> {
        let bytes = tokio::fs::read(&self.path).await?;
        match serde_json::from_slice::<ProductInner>(&bytes) {
            Ok(data) => {
                self.product = Some(Product {
                    inner: Arc::new(data),
                    client: self.degiro.clone(),
                });

                Ok(self)
            }
            Err(err) => Err(DataHandlerError::DeserializeError(err.to_string())),
        }
    }

    async fn get(&mut self) -> Result<&Self::Output, DataHandlerError> {
        if self.product.is_none() && self.read().await.is_err() {
            self.download().await?;
        }
        Ok(self.product.as_ref().unwrap())
    }

    async fn take(mut self) -> Result<Product, DataHandlerError> {
        if self.product.is_none() && self.read().await.is_err() {
            self.download().await?;
        }
        Ok(self.product.take().unwrap())
    }
}

impl App {
    pub fn product_handler(&self, id: impl ToString) -> ProductHandler {
        ProductHandler::new(
            id.to_string(),
            self.degiro.clone(),
            self.settings.data_path.clone(),
        )
    }
    pub fn all_products_handler(&self) -> ProductHandlers {
        self.settings
            .assets
            .iter()
            .map(|(id, _)| {
                ProductHandler::new(
                    id.clone(),
                    self.degiro.clone(),
                    self.settings.data_path.clone(),
                )
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProductHandlers(Vec<ProductHandler>);

impl FromIterator<ProductHandler> for ProductHandlers {
    fn from_iter<T: IntoIterator<Item = ProductHandler>>(iter: T) -> Self {
        ProductHandlers(iter.into_iter().collect())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProductQuery {
    Id(String),
    Symbol(String),
    Name(String),
}

impl ProductHandlers {
    pub async fn find(
        &mut self,
        query: impl Into<ProductQuery>,
    ) -> Result<Option<Product>, DataHandlerError> {
        let query = query.into();
        for handler in &mut self.0 {
            let product = handler.get().await?.clone();
            match &query {
                ProductQuery::Id(id) => {
                    if handler.id == *id {
                        return Ok(Some(product.clone()));
                    }
                }
                ProductQuery::Symbol(symbol) => {
                    if product.inner.symbol == *symbol {
                        return Ok(Some(product.clone()));
                    }
                }
                ProductQuery::Name(name) => {
                    let re = Regex::new(&format!("(?i){}", name)).unwrap();
                    if re.is_match(&product.inner.name) {
                        return Ok(Some(product.clone()));
                    }
                }
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use crate::data::DataHandler;

    #[tokio::test]
    async fn product_handler_test() {
        let app = crate::App::new();
        let mut handler = app.product_handler("1157690");
        handler.read().await.unwrap();
        let product = handler.get().await.unwrap();
        println!("{:#?}", product);
    }
}
