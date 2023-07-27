use sqlx::{pool::PoolConnection, PgPool, Postgres};

#[derive(Debug, Clone)]
pub struct AppState {
    db_pool: PgPool,
}

impl AppState {
    pub const fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }

    pub async fn db_connection(&self) -> sqlx::Result<PoolConnection<Postgres>> {
        self.db_pool.acquire().await
    }
}
