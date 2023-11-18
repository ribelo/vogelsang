use std::{path::Path, sync::Arc};

use degiro_rs::{
    api::product::{Product, ProductInner},
    client::{client_status::Authorized, Client},
    prelude::*,
};
use thiserror::Error;
use tokio::{fs::File, io::AsyncWriteExt};

use crate::App;

use super::{DataHandler, DataHandlerError};

#[derive(Debug, Default)]
pub struct ProductHandlerBuilder {
    id: Option<String>,
    degiro: Option<Client<Authorized>>,
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

    pub fn degiro(mut self, degiro: Client<Authorized>) -> ProductHandlerBuilder {
        self.degiro = Some(degiro);
        self
    }

    pub fn data_path(mut self, data_path: String) -> ProductHandlerBuilder {
        self.data_path = Some(data_path);
        self
    }

    pub fn build(self) -> Result<ProductHandler, ProductHandlerBuilderError> {
        if self.id.is_none() {
            Err(ProductHandlerBuilderError::NoId)
        } else if self.degiro.is_none() {
            Err(ProductHandlerBuilderError::NoDegiroClient)
        } else if self.data_path.is_none() {
            Err(ProductHandlerBuilderError::NoDataPath)
        } else {
            Ok(ProductHandler::new(
                self.id.unwrap(),
                self.degiro.unwrap(),
                self.data_path.unwrap(),
            ))
        }
    }
}
pub struct ProductHandler {
    id: String,
    degiro: Client<Authorized>,
    path: String,
    product: Option<Product>,
}

impl ProductHandler {
    pub fn new(id: String, degiro: Client<Authorized>, data_path: String) -> ProductHandler {
        ProductHandler {
            path: format!("{}/product_{}.bin", &data_path, &id),
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
        let bin = bincode::serialize(product.inner.as_ref())?;
        file.write_all(&bin).await?;

        Ok(())
    }

    async fn download(&mut self) -> Result<&mut Self, DataHandlerError> {
        self.fetch().await?;
        self.save().await?;
        Ok(self)
    }

    async fn read(&mut self) -> Result<&mut Self, DataHandlerError> {
        let bytes = tokio::fs::read(&self.path).await?;
        let data: ProductInner = bincode::deserialize(&bytes)?;
        self.product = Some(Product {
            inner: Arc::new(data),
            client: self.degiro.clone(),
        });

        Ok(self)
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

impl App<Authorized> {
    pub fn product_handler(&self, id: impl ToString) -> ProductHandler {
        ProductHandler::new(
            id.to_string(),
            self.degiro.clone(),
            self.settings.data_path.clone(),
        )
    }
}
