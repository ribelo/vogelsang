use atomic_take::AtomicTake;
use degiro_rs::api::product::ProductInner;
use erfurt::{candle, prelude::Candles};
use eventual::{eve::Eve, event::Event, reactive::Node, Event};
use tokio::sync::oneshot;
use tracing::{error, info, warn};

use crate::{
    data::{
        products::{ProductHandlers, ProductQuery},
        DataHandler, DataHandlerError,
    },
    App,
};

use super::authorize::Authorize;

#[derive(Debug, Event)]
pub struct GetCandles {
    pub query: ProductQuery,
    pub tx: AtomicTake<oneshot::Sender<Option<Candles>>>,
}

impl GetCandles {
    pub fn respond(&self, candles: Option<Candles>) {
        if let Some(tx) = self.tx.take() {
            info!("Sending candles...");
            tx.send(candles).unwrap_or_else(|_| {
                error!("Failed to send candles");
            })
        }
    }
}

pub async fn get_candles(
    event: Event<GetCandles>,
    product_handlers: Node<ProductHandlers>,
    eve: Eve<App>,
) {
    info!("Fetching candles...");
    match product_handlers
        .as_ref()
        .clone()
        .find(event.query.clone())
        .await
    {
        Ok(None) => event.respond(None),
        Ok(Some(product)) => {
            let mut candles_handler = eve.state.candles_handler(&product.inner.id);
            match candles_handler.get().await {
                Ok(candles) => {
                    event.respond(Some(candles.clone()));
                }
                Err(DataHandlerError::Unauthorized) => {
                    eve.dispatch_sync(Authorize {}).await.unwrap_or_else(|err| {
                        error!(error = %err, "Failed to dispatch authorize event");
                    });
                    tokio::spawn(async move {
                        eve.dispatch(event).await.unwrap_or_else(|err| {
                            error!(error = %err, "Failed to dispatch get data event");
                        });
                    });
                }
                Err(err) => error!(error = %err, "Failed to get candles"),
            }
        }
        Err(err) => match err {
            crate::data::DataHandlerError::Unauthorized => {
                warn!("Handler unauthorized, attempting authorization...");
                eve.dispatch_sync(Authorize {}).await.unwrap_or_else(|err| {
                    error!(error = %err, "Failed to dispatch authorize event");
                });
                tokio::spawn(async move {
                    eve.dispatch(event).await.unwrap_or_else(|err| {
                        error!(error = %err, "Failed to dispatch get data event");
                    });
                });
            }
            _ => error!(error = %err, "Failed to get product"),
        },
    };
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use eventual::eve::EveBuilder;
//
//     #[tokio::test]
//     async fn get_data_test() {
//         let app = App::new();
//         let eve = EveBuilder::new(app).build().unwrap();
//         let (tx, _rx) = oneshot::channel();
//         let event = GetProduct {
//             id: "1157690".to_string(),
//             tx: Arc::new(AtomicTake::new(tx)),
//         };
//         get_product(event, eve).await;
//     }
// }
