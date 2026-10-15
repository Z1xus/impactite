use std::sync::Arc;

use axum::Router;

use crate::accounts::Web;

mod sites;
mod stats;

pub fn routes() -> Router<Arc<Web>> {
	stats::routes().merge(sites::routes())
}
