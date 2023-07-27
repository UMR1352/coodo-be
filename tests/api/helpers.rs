use std::net::TcpListener;

use anyhow::Context;
use coodo_be::{telemetry, user::User};
use once_cell::sync::Lazy;
use reqwest::Client;
use sqlx::PgPool;
use tokio::task::JoinHandle;

static TRACING: Lazy<()> = Lazy::new(|| {
    if let Ok(level) = std::env::var("TEST_LOG") {
        telemetry::init_with_filter(&level);
    } else {
        telemetry::init_with_filter("error");
    }
});

pub struct TestApp {
    pub address: String,
    pub port: u16,
    _pool: PgPool,
    _server_handle: JoinHandle<hyper::Result<()>>,
}

impl TestApp {
    #[tracing::instrument(skip_all)]
    pub async fn spawn(pool: PgPool) -> Self {
        Lazy::force(&TRACING);

        let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind random port");
        let port = listener.local_addr().unwrap().port();
        let address = format!("http://127.0.0.1:{}", port);
        let server = coodo_be::startup::make_server(listener, pool.clone())
            .expect("Failed to create server");
        let server_handle = tokio::spawn(server);

        Self {
            address,
            port,
            _pool: pool,
            _server_handle: server_handle,
        }
    }

    pub async fn get_user(&self, client: &mut Client) -> anyhow::Result<User> {
        client
            .get(format!("{}/session", &self.address))
            .send()
            .await
            .context("Failed to send GET /session")?
            .json::<User>()
            .await
            .context("Failed to parse response body")
    }
}
