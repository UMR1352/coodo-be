use std::net::TcpListener;

use axum::Router;
use sqlx::PgPool;

use crate::{settings::{Settings, DbSettings}, state::AppState, session::get_session_layer};

pub type Server =
    hyper::Server<hyper::server::conn::AddrIncoming, axum::routing::IntoMakeService<Router>>;

#[allow(dead_code)]
pub struct Application {
    db_pool: PgPool,
    port: u16,
    server: Server,
}

impl Application {
    pub async fn build(settings: Settings) -> anyhow::Result<Self> {
        let db_pool = get_db_pool(&settings.db);
        let listener = {
            let address = format!("{}:{}", &settings.app.host, settings.app.port);
            TcpListener::bind(address)?
        };
        
        let server = make_server(listener, db_pool.clone())?;

        Ok(Self {
            server,
            db_pool,
            port: settings.app.port,
        })
    }

    pub async fn run_until_stopped(self) -> Result<(), hyper::Error> {
        tracing::info!("Server is starting at {}", self.server.local_addr());
        self.server.await
    }
}

pub fn make_server(
    listener: TcpListener,
    db_pool: PgPool,
) -> anyhow::Result<Server> {
    use tower_http::{catch_panic::CatchPanicLayer, trace::{TraceLayer, self}};
    use tracing::Level;
    use anyhow::Context;

    let state = AppState::new(db_pool.clone());
    let router = Router::new()
        .merge(crate::routes::router())
        .layer(CatchPanicLayer::new())
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(trace::DefaultMakeSpan::new().level(Level::INFO))
                .on_response(trace::DefaultOnResponse::new().level(Level::INFO))
        )
        .layer(get_session_layer(db_pool))
        .with_state(state);
    let server = axum::Server::from_tcp(listener)
        .context("Cannot make server with the provided socket")?
        .serve(router.into_make_service());

    Ok(server)
}

pub fn get_db_pool(settings: &DbSettings) -> PgPool {
    use sqlx::postgres::PgPoolOptions;
    use std::time::Duration;

    PgPoolOptions::new()
        .acquire_timeout(Duration::from_secs(1))
        .connect_lazy_with(settings.with_db())
}