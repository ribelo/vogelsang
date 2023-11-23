use atomic_take::AtomicTake;
use degiro_rs::util::ProductCategory;
use eventual::{eve::Eve, event::Event, Event};
use tokio::sync::oneshot;
use tracing::{error, info};

use crate::{events::authorize::Authorize, portfolio::RiskMode, App};

#[derive(Event)]
pub struct CalculatePorftolio {
    pub tx: AtomicTake<oneshot::Sender<Option<String>>>,
    pub mode: RiskMode,
    pub risk: f64,
    pub risk_free: f64,
    pub freq: u32,
    pub money: f64,
    pub max_stocks: i32,
    pub min_rsi: Option<f64>,
    pub max_rsi: Option<f64>,
    pub min_class: Option<ProductCategory>,
    pub max_class: Option<ProductCategory>,
    pub short_sales_constraint: bool,
}

impl CalculatePorftolio {
    pub fn respond(&self, response: Option<String>) {
        if let Some(tx) = self.tx.take() {
            info!("Sending single allocation...");
            tx.send(response).unwrap_or_else(|_| {
                error!("Failed to send single allocation");
            })
        }
    }
}

pub async fn calculate_portfolio(event: Event<CalculatePorftolio>, eve: Eve<App>) {
    info!("Calculating portfolio...");
    eve.dispatch_sync(Authorize {}).await.unwrap_or_else(|err| {
        error!(error = %err, "Failed to dispatch authorize event");
    });
    let mut calculator = eve
        .state
        .portfolio_calculator(
            event.mode,
            event.risk,
            event.risk_free,
            event.freq,
            event.money,
            event.max_stocks,
            event.min_rsi,
            event.max_rsi,
            event.min_class,
            event.max_class,
            event.short_sales_constraint,
        )
        .await;
    calculator.remove_invalid().calculate().await;
    let table = calculator.as_table();
    event.respond(Some(table.to_string()));
}
