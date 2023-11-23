use atomic_take::AtomicTake;
use degiro_rs::api::product::ProductInner;
use eventual::{eve::Eve, event::Event, reactive::Node, Event};
use tokio::sync::oneshot;
use tracing::{error, info, warn};

use crate::{
    data::products::{ProductHandlers, ProductQuery},
    App,
};

use super::authorize::Authorize;

#[derive(Debug, Event)]
pub struct GetProduct {
    pub query: ProductQuery,
    pub tx: AtomicTake<oneshot::Sender<Option<ProductInner>>>,
}

impl GetProduct {
    pub fn new(query: ProductQuery, tx: oneshot::Sender<Option<ProductInner>>) -> Self {
        Self {
            query,
            tx: AtomicTake::new(tx),
        }
    }

    pub fn respond(&self, product: Option<ProductInner>) {
        if let Some(tx) = self.tx.take() {
            info!("Sending product...");
            tx.send(product).unwrap_or_else(|_| {
                error!("Failed to send product");
            })
        }
    }
}

pub async fn get_product(
    event: Event<GetProduct>,
    product_handlers: Node<ProductHandlers>,
    eve: Eve<App>,
) {
    info!("Fetching product...");
    match product_handlers
        .as_ref()
        .clone()
        .find(event.query.clone())
        .await
    {
        Ok(maybe_product) => event.respond(maybe_product.map(|p| p.inner.as_ref().clone())),
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
