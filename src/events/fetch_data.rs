use std::sync::Arc;

use eventual::{eve::Eve, event::Event, Event};
use tracing::{error, info, warn};

use crate::{data::DataHandler, events::authorize::Authorize, App};

#[derive(Debug, Clone, Event)]
pub struct FetchData {
    pub id: Option<String>,
    pub name: Option<String>,
}

pub async fn fetch_data(event: Event<FetchData>, eve: Eve<App>) {
    info!("Fetching data...");
    if let Some(id) = &event.id {
        let asset_name = event.name.as_deref().unwrap_or("Unknown");
        info!(asset_id = %id, %asset_name, "Fetching data for asset.");

        if let Err(err) = eve.state.candles_handler(id).download().await {
            match err {
                crate::data::DataHandlerError::Unauthorized => {
                    handle_download_error(event.clone(), eve).await;
                }
                _ => error!(asset_id = %id, error = %err, "Failed to fetch candles data"),
            }
        } else if let Err(err) = eve.state.product_handler(id).download().await {
            match err {
                crate::data::DataHandlerError::Unauthorized => {
                    handle_download_error(event.clone(), eve).await;
                }
                _ => error!(asset_id = %id, error = %err, "Failed to fetch product data"),
            }
        }
        info!(asset_id = %id, asset_name, "Successfully fetched data for asset.");
    } else {
        info!("Fetching data for all assets");
        for (id, name) in eve.clone().state.settings.assets.into_iter() {
            let eve = eve.clone();
            tokio::spawn(async move {
                eve.dispatch(FetchData {
                    id: Some(id.to_string()),
                    name: Some(name.clone()),
                })
                .await
                .unwrap_or_else(|err| {
                    error!(error = %err, "Failed to dispatch fetch data event");
                });
            });
        }
    }
}

async fn handle_download_error(event: Arc<FetchData>, eve: Eve<App>) {
    warn!(asset_id = %event.id.as_ref().unwrap(), "Handler unauthorized, attempting authorization...");
    eve.dispatch_sync(Authorize {}).await.unwrap_or_else(|err| {
        error!(error = %err, "Failed to dispatch authorize event");
    });
    tokio::spawn(async move {
        eve.dispatch(event).await.unwrap_or_else(|err| {
            error!(error = %err, "Failed to dispatch fetch data event");
        })
    });
}
