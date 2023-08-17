use anyhow::Context;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use axum_sessions::extractors::{ReadableSession, WritableSession};
use futures_util::{stream::SplitSink, FutureExt, SinkExt, StreamExt};
use hyper::StatusCode;
use redis::JsonAsyncCommands;
use tokio::sync::{broadcast, oneshot};
use uuid::Uuid;

use crate::{
    session::TodoSessionExt,
    state::AppState,
    todo::{Applicable, Command, TodoList, TodoListInfo},
    user::User,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/todos/:id", get(join_todo_list))
        .route("/todos", post(create_todo_list))
        .route("/todos", get(get_users_todo_lists))
        .route("/todos/:id", delete(leave_todo_list))
}

async fn create_todo_list(
    mut session: WritableSession,
    State(state): State<AppState>,
) -> Result<Json<Uuid>, StatusCode> {
    let _user = session
        .get::<User>("user")
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let todo_list = TodoList::default();
    todo_list
        .store(state.redis_pool())
        .await
        .expect("Failed to create todo list");

    session.join_todo_list(todo_list.id());

    Ok(Json(todo_list.id()))
}

async fn get_users_todo_lists(
    session: ReadableSession,
    State(state): State<AppState>,
) -> Json<Vec<TodoListInfo<'static>>> {
    let mut redis_conn = state
        .redis_pool()
        .get()
        .await
        .expect("Failed to acquire redis connection");
    let mut joined_lists = vec![];
    for todo_id in session.get::<Vec<Uuid>>("user_lists").unwrap_or_default() {
        let Ok(todo_list) = redis_conn.json_get::<_, _, TodoList>(todo_id.to_string(), "$").await else {
            continue;
        };
        joined_lists.push(TodoListInfo::new_owned(
            todo_id,
            todo_list.name().to_owned(),
        ));
    }

    Json(joined_lists)
}

async fn leave_todo_list(mut session: WritableSession, Path(todo_id): Path<Uuid>) {
    session.leave_todo_list(todo_id);
}

#[tracing::instrument(
    name = "TodoList connect"
    skip_all,
    fields(todo = %todo_id)
)]
async fn join_todo_list(
    mut session: WritableSession,
    Path(todo_id): Path<Uuid>,
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    let Some(user) = session.get::<User>("user") else {
        return (StatusCode::UNAUTHORIZED, "Establish a session first").into_response();
    };
    let Ok((todo_tx, todo_rx, abort_rx)) = state.join_todo_list(todo_id, user.clone()).await else {
        return (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong while joining todo list").into_response();
    };
    session.join_todo_list(todo_id);

    ws.on_upgrade(move |socket| {
        ws_handler(socket, state, todo_id, todo_tx, todo_rx, abort_rx, user).map(|_| ())
    })
}

#[tracing::instrument(
    name = "WS handler",
    skip_all,
    err,
    fields(
        todo_list = %todo_list_id,
        user = %user.id()
    )
)]
async fn ws_handler(
    ws: WebSocket,
    state: AppState,
    todo_list_id: Uuid,
    todo_tx: broadcast::Sender<TodoList>,
    mut todo_rx: broadcast::Receiver<TodoList>,
    mut abort_rx: oneshot::Receiver<()>,
    user: User,
) -> anyhow::Result<()> {
    tracing::info!("WS connected!");
    let (mut ws_tx, mut ws_rx) = ws.split();

    loop {
        tokio::select! {
            Some(Ok(msg)) = ws_rx.next() => {
                if matches!(msg, Message::Close(_)) {
                    break;
                }
                let mut redis_conn = state.redis_pool().get().await?;
                let cmd = serde_json::from_slice::<Command>(msg.into_data().as_slice())?;
                tracing::debug!("Got {:?}", &cmd);
                let todo_list = cmd
                    .apply(&mut redis_conn, todo_list_id, user.clone())
                    .await?;
                let _ = todo_tx.send(todo_list);
            },
            Ok(todo) = todo_rx.recv() => {
                let _ = send_todo_list(&todo, &mut ws_tx).await;
            },
            _ = &mut abort_rx => {
                tracing::debug!("Abort received. Closing previous WS connection");
                ws_tx.close().await?;
                return Ok(());
            },
            else => break,
        }
    }

    let _ = ws_tx.close().await;
    let _ = state.leave_todo_list(todo_list_id, user, todo_tx).await;
    tracing::debug!("WS connection closed");

    Ok(())
}

async fn send_todo_list(
    todo_list: &TodoList,
    ws_sink: &mut SplitSink<WebSocket, Message>,
) -> anyhow::Result<()> {
    let json_msg = serde_json::to_string(&todo_list).context("Failed to serialize todo list")?;
    ws_sink
        .send(Message::Text(json_msg))
        .await
        .context("Failed to send ws message")?;

    Ok(())
}
