use std::net::TcpListener;

use anyhow::Context;
use coodo_be::{startup::get_redis_pool, telemetry, todo::TodoListInfo, user::User};
use deadpool_redis::Pool;
use futures_util::{
    stream::{SplitSink, SplitStream},
    StreamExt,
};
use once_cell::sync::Lazy;
use rand::Rng;
use reqwest::{cookie::Jar, Client};
use tokio::{net::TcpStream, task::JoinHandle};
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
    pool: Pool,
    _server_handle: JoinHandle<hyper::Result<()>>,
}

impl TestApp {
    #[tracing::instrument(skip_all)]
    pub async fn spawn() -> Self {
        Lazy::force(&TRACING);

        let settings = coodo_be::settings::get_settings().expect("Failed to read config file");
        let mut redis_settings = settings.redis;
        redis_settings.db = rand::thread_rng().gen_range(1..15);
        let pool = get_redis_pool(redis_settings);

        let listener = TcpListener::bind(format!("{}:0", settings.app.host))
            .expect("Failed to bind random port");
        let port = listener.local_addr().unwrap().port();
        let address = format!("http://127.0.0.1:{}", port);

        let server = coodo_be::startup::make_server(listener, pool.clone(), &settings.todo_handler)
            .expect("Failed to create server");
        let server_handle = tokio::spawn(server);

        Self {
            address,
            port,
            pool,
            _server_handle: server_handle,
        }
    }

    pub fn redis_pool(&self) -> Pool {
        self.pool.clone()
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

    pub async fn get_joined_todo_lists(
        &self,
        client: &Client,
    ) -> anyhow::Result<Vec<TodoListInfo>> {
        client
            .get(format!("{}/todos", self.address))
            .send()
            .await
            .context("Failed to send GET /todos")?
            .json::<Vec<TodoListInfo>>()
            .await
            .context("Failed to parse response")
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
