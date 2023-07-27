use axum::{Router, Json};
use axum_sessions::extractors::WritableSession;
use chrono::Duration;

use crate::{user::User, state::AppState};

pub fn routes() -> Router<AppState> {
    use axum::routing::get;

    Router::new()
        .route("/session", get(get_session))
}

#[tracing::instrument(skip_all, ret, name = "Get session")]
async fn get_session(mut session: WritableSession) -> Json<User> {
    if session.is_expired() {
        session.destroy();
    }

    let user = session.get::<User>("user").unwrap_or(User::new().await);
    if session.insert("user", &user).is_err() {
        tracing::error!("Failed to serialize user");
    }
    session.expire_in(
        Duration::days(1)
            .to_std()
            .expect("Failed to convert to std::Duration"),
    );

    Json(user)
}
