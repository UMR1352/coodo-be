use axum::Router;

use crate::state::AppState;

mod session;

pub fn router() -> Router<AppState> {
    Router::new().merge(session::routes())
}
