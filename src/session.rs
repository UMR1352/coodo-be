use axum::async_trait;
use axum_sessions::{
    async_session::{self, Session, SessionStore},
    SessionLayer,
};
use sqlx::{pool::PoolConnection, PgPool, Postgres};

pub fn get_session_layer(db_pool: PgPool) -> SessionLayer<UserSessionStore> {
    use axum_sessions::{PersistencePolicy, SameSite};

    let (session_store, secret) = UserSessionStore::new(db_pool);
    SessionLayer::new(session_store, &secret)
        .with_persistence_policy(PersistencePolicy::ChangedOnly)
        .with_http_only(false)
        .with_same_site_policy(SameSite::Strict)
}

#[derive(Debug, Clone)]
pub struct UserSessionStore(PgPool);

impl UserSessionStore {
    pub fn new(pool: PgPool) -> (Self, [u8; 128]) {
        (Self(pool), get_or_create_secret())
    }
    pub async fn acquire_connection(&self) -> sqlx::Result<PoolConnection<Postgres>> {
        self.0.acquire().await
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

        let mut db = self.acquire_connection().await?;
        let session_value = sqlx::query!(
            r#"
SELECT value
FROM user_sessions
WHERE id = $1
            "#,
            id.as_str()
        )
        .fetch_optional(&mut db)
        .await?
        .map(|row| row.value);

        Ok(session_value
            .and_then(|value| serde_json::from_str(&value).ok())
            .and_then(Session::validate))
    }

    #[tracing::instrument(skip_all, level = "debug")]
    async fn store_session(&self, session: Session) -> async_session::Result<Option<String>> {
        tracing::trace!("Storing session with id {}", session.id());
        let mut db = self.acquire_connection().await?;
        sqlx::query!(
            r#"
INSERT INTO user_sessions (id, value)
VALUES($1, $2)
ON CONFLICT (id)
DO
    UPDATE SET value = $2
            "#,
            session.id(),
            serde_json::to_string(&session)?
        )
        .execute(&mut db)
        .await?;

        session.reset_data_changed();
        Ok(session.into_cookie_value())
    }

    #[tracing::instrument(skip_all, level = "debug")]
    async fn destroy_session(&self, session: Session) -> async_session::Result {
        tracing::trace!("Destroying session with id {}", &session.id());
        let mut db = self.acquire_connection().await?;
        sqlx::query!(
            r#"
DELETE FROM user_sessions
WHERE id = $1
            "#,
            session.id()
        )
        .execute(&mut db)
        .await?;

        Ok(())
    }

    #[tracing::instrument(skip_all, level = "debug")]
    async fn clear_store(&self) -> async_session::Result {
        tracing::trace!("Clearing session store");
        let mut db = self.acquire_connection().await?;
        sqlx::query!(
            r#"
DELETE FROM user_sessions
            "#,
        )
        .execute(&mut db)
        .await?;

        Ok(())
    }
}
