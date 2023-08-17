use std::net::TcpListener;

use axum::Router;
use deadpool_redis::Pool;

use crate::{
    session::get_session_layer,
    settings::{RedisSettings, Settings},
    state::AppState,
};

pub type Server =
    hyper::Server<hyper::server::conn::AddrIncoming, axum::routing::IntoMakeService<Router>>;

#[allow(dead_code)]
pub struct Application {
    redis_pool: Pool,
    port: u16,
    server: Server,
}

impl Application {
    pub async fn build(settings: Settings) -> anyhow::Result<Self> {
        let redis_pool = get_redis_pool(settings.redis);
        let listener = {
            let address = format!("{}:{}", &settings.app.host, settings.app.port);
            TcpListener::bind(address)?
        };

        let server = make_server(listener, redis_pool.clone())?;

        Ok(Self {
            server,
            redis_pool,
            port: settings.app.port,
        })
    }

    pub async fn run_until_stopped(self) -> Result<(), hyper::Error> {
        tracing::info!("Server is starting at {}", self.server.local_addr());
        self.server.await
    }
}

pub fn make_server(listener: TcpListener, redis_pool: Pool) -> anyhow::Result<Server> {
    use anyhow::Context;
    use tower_http::{
        catch_panic::CatchPanicLayer,
        trace::{self, TraceLayer},
    };
    use tracing::Level;

    let state = AppState::new(redis_pool.clone());
    let router = Router::new()
        .merge(crate::routes::router())
        .layer(CatchPanicLayer::new())
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(trace::DefaultMakeSpan::new().level(Level::INFO))
                .on_response(trace::DefaultOnResponse::new().level(Level::INFO)),
        )
        .layer(get_session_layer(redis_pool))
        .with_state(state);
    let server = axum::Server::from_tcp(listener)
        .context("Cannot make server with the provided socket")?
        .serve(router.into_make_service());

    Ok(server)
}

pub fn get_redis_pool(settings: RedisSettings) -> deadpool_redis::Pool {
    use deadpool_redis::{Config, Runtime};
    use std::time::Duration;

    Config::from_connection_info(settings)
        .builder()
        .expect("Failed to create redis pool builder")
        .max_size(16)
        .wait_timeout(Some(Duration::from_secs(1)))
        .runtime(Runtime::Tokio1)
        .build()
        .expect("Failed to establish redis connection pool")
}
