use std::fmt::Debug;

use axum::async_trait;
use axum_sessions::{
    async_session::{self, Session, SessionStore},
    SessionLayer,
};
use deadpool_redis::{Connection, Pool, PoolError};

pub fn get_session_layer(redis_pool: Pool) -> SessionLayer<UserSessionStore> {
    use axum_sessions::{PersistencePolicy, SameSite};

    let (session_store, secret) = UserSessionStore::new(redis_pool);
    SessionLayer::new(session_store, &secret)
        .with_persistence_policy(PersistencePolicy::ChangedOnly)
        .with_http_only(false)
        .with_same_site_policy(SameSite::Strict)
}

#[derive(Clone)]
pub struct UserSessionStore(Pool);

impl Debug for UserSessionStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "UserSessionStore {{}}")
    }
}

impl UserSessionStore {
    pub fn new(pool: Pool) -> (Self, [u8; 128]) {
        (Self(pool), get_or_create_secret())
    }
    pub async fn acquire_connection(&self) -> Result<Connection, PoolError> {
        self.0.get().await
    }
}

#[tracing::instrument(name = "Get session secret")]
fn get_or_create_secret() -> [u8; 128] {
    use rand::RngCore;
    use std::{
        fs::{self, File},
        io::Read,
    };

    let mut secret_buffer = [0_u8; 128];
    if File::open(".session_secret")
        .and_then(|mut file| file.read_exact(&mut secret_buffer))
        .is_err()
    {
        tracing::trace!("Previous session's secret not detected. Generating a new one..");
        let mut rng = rand::thread_rng();
        rng.fill_bytes(&mut secret_buffer);
        let _ = fs::write(".session_secret", &secret_buffer[..]);
    }

    secret_buffer
}

#[async_trait]
impl SessionStore for UserSessionStore {
    #[tracing::instrument(skip_all, level = "debug")]
    async fn load_session(&self, cookie_value: String) -> async_session::Result<Option<Session>> {
        tracing::trace!("Received cookie {}", &cookie_value);
        let id = Session::id_from_cookie_value(&cookie_value)?;
        tracing::trace!("Loading session with id {}", &id);

        let mut redis = self.acquire_connection().await?;
        let session = redis::cmd("FCALL")
            .arg("session_load")
            .arg(1)
            .arg(&id)
            .query_async::<_, String>(&mut redis)
            .await
            .map(|session| serde_json::from_str(&session).unwrap())?;

        Ok(session)
    }

    #[tracing::instrument(skip_all, level = "debug")]
    async fn store_session(&self, session: Session) -> async_session::Result<Option<String>> {
        tracing::trace!("Storing session with id {}", session.id());
        let mut redis = self.acquire_connection().await?;
        redis::cmd("FCALL")
            .arg("session_store")
            .arg(1)
            .arg(session.id())
            .arg(&serde_json::to_string(&session)?)
            .query_async(&mut redis)
            .await?;

        session.reset_data_changed();
        Ok(session.into_cookie_value())
    }

    #[tracing::instrument(skip_all, level = "debug")]
    async fn destroy_session(&self, session: Session) -> async_session::Result {
        tracing::trace!("Destroying session with id {}", &session.id());
        let mut redis = self.acquire_connection().await?;
        redis::cmd("FCALL")
            .arg("session_destroy")
            .arg(1)
            .arg(session.id())
            .query_async(&mut redis)
            .await?;

        Ok(())
    }

    #[tracing::instrument(skip_all, level = "debug")]
    async fn clear_store(&self) -> async_session::Result {
        tracing::trace!("Clearing session store");
        let mut redis = self.acquire_connection().await?;
        redis::cmd("FCALL")
            .arg("session_clear_all")
            .arg(0)
            .query_async(&mut redis)
            .await?;

        Ok(())
    }
}
