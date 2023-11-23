use eventual::eve::Eve;

use crate::{data::products::ProductHandlers, App};

pub async fn products(eve: Eve<App>) -> ProductHandlers {
    eve.state.all_products_handler()
}
