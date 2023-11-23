use atomic_take::AtomicTake;
use degiro_rs::util::Period;
use eventual::{eve::Eve, event::Event, Event};
use tokio::sync::oneshot;
use tracing::{error, info, warn};

use crate::{
    data::products::ProductQuery,
    portfolio::{RiskMode, SingleAllocation},
    App,
};

use super::get_candles::GetCandles;

#[derive(Event)]
pub struct GetSingleAllocation {
    pub query: ProductQuery,
    pub mode: RiskMode,
    pub risk: f64,
    pub risk_free: f64,
    pub tx: AtomicTake<oneshot::Sender<Option<f64>>>,
}

impl GetSingleAllocation {
    pub fn respond(&self, response: Option<f64>) {
        if let Some(tx) = self.tx.take() {
            info!("Sending single allocation...");
            tx.send(response).unwrap_or_else(|_| {
                error!("Failed to send single allocation");
            })
        }
    }
}

pub async fn get_single_allocation(event: Event<GetSingleAllocation>, eve: Eve<App>) {
    info!("Calculating single allocation...");
    let (tx, rx) = oneshot::channel();
    let get_candles_event = GetCandles {
        query: event.query.clone(),
        tx: AtomicTake::new(tx),
    };
    tokio::spawn(async move {
        eve.dispatch(get_candles_event).await.unwrap_or_else(|err| {
            error!(error = %err, "Failed to dispatch get candles event");
        });
    });
    match rx.await {
        Ok(None) => event.respond(None),
        Ok(Some(candles)) => {
            info!("Received candles");
            if let Ok(single_allocation) = candles
                .single_allocation(
                    event.mode,
                    event.risk,
                    event.risk_free,
                    Period::P1Y,
                    Period::P1M,
                )
                .await
            {
                event.respond(Some(single_allocation));
            } else {
                warn!("Failed to calculate single allocation");
                event.respond(None);
            }
        }
        Err(err) => error!(error = %err, "Failed to receive candles"),
    }
}
