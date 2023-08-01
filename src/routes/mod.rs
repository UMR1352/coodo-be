use axum::Router;

use crate::state::AppState;

mod session;
mod todo;

pub fn router() -> Router<AppState> {
    Router::new().merge(session::routes()).merge(todo::routes())
}
