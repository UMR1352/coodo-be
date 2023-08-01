use core::task::Context as Ctx;
use std::{net::TcpListener, pin::Pin, task::Poll, time::Duration};

use anyhow::Context;
use coodo_be::{settings::TodoHandlerSettings, telemetry, user::User};
use futures_util::{
    stream::{SplitSink, SplitStream},
    Future, Stream, StreamExt,
};
use once_cell::sync::Lazy;
use reqwest::{cookie::Jar, Client};
use sqlx::PgPool;
use tokio::{net::TcpStream, task::JoinHandle, time::Interval};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};
use uuid::Uuid;

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

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
        let todo_handler_settings = TodoHandlerSettings {
            store_interval: Duration::from_secs(1),
        };
        let server = coodo_be::startup::make_server(listener, pool.clone(), &todo_handler_settings)
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

    pub async fn create_todo_list(&self, client: &mut Client) -> anyhow::Result<Uuid> {
        client
            .post(format!("{}/todos", &self.address))
            .send()
            .await
            .context("Failed to send POST /todos")?
            .json::<Uuid>()
            .await
            .context("Failed to parse response body")
    }

    pub async fn connect_to_todo_list(
        &self,
        todo_id: Uuid,
        cookie_jar: &Jar,
    ) -> anyhow::Result<(SplitSink<WsStream, Message>, SplitStream<WsStream>)> {
        let sid = get_sid(cookie_jar)?;
        let ws_request = hyper::http::Request::builder()
            .method("GET")
            .uri(format!("ws://127.0.0.1:{}/todos/{}", self.port, todo_id))
            .header("Host", "localhost")
            .header("Upgrade", "websocket")
            .header("Connection", "Upgrade")
            .header("Sec-WebSocket-Key", "x3JJHMbDL1EzLkh9GBhXDw==")
            .header("Sec-WebSocket-Version", "13")
            .header("Cookie", format!("sid={}", sid))
            .body(())
            .context("Failed to build ws connection request")?;

        let (ws_stream, _) = connect_async(ws_request)
            .await
            .context("Failed to establish ws connection")?;
        Ok(ws_stream.split())
    }
}

pub trait StreamExtTimed: StreamExt {
    fn next_with_timeout(&mut self, timeout: Duration) -> TimedNext<'_, Self>
    where
        Self: Unpin,
    {
        TimedNext::new(self, timeout)
    }
}

impl<T: ?Sized> StreamExtTimed for T where T: StreamExt {}

pub struct TimedNext<'a, S: ?Sized> {
    stream: &'a mut S,
    timeout: Interval,
}

impl<S: ?Sized + Unpin> Unpin for TimedNext<'_, S> {}

impl<'a, S: ?Sized + Stream + Unpin> TimedNext<'a, S> {
    pub fn new(stream: &'a mut S, timeout: Duration) -> Self {
        Self {
            stream,
            timeout: tokio::time::interval(timeout),
        }
    }
}

impl<S: ?Sized + Stream + Unpin> Future for TimedNext<'_, S> {
    type Output = Option<S::Item>;

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Ctx<'_>) -> Poll<Self::Output> {
        match self.timeout.poll_tick(ctx) {
            Poll::Pending => self.stream.poll_next_unpin(ctx),
            Poll::Ready(_) => Poll::Ready(None),
        }
    }
}

fn get_sid(cookie_jar: &Jar) -> anyhow::Result<String> {
    use reqwest::cookie::CookieStore;

    let url = "http://127.0.0.1:0/".parse().unwrap();
    cookie_jar
        .cookies(&url)
        .context("No cookies for /")?
        .to_str()
        .context("Failed to convert cookie to string")?
        .split("; ")
        .filter_map(|cookie_str| cookie_str.split_once('='))
        .find_map(|(name, value)| (name == "sid").then_some(value.to_owned()))
        .context("Session cookie not found")
}
