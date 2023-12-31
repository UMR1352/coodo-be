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
use futures_util::{stream::SplitSink, SinkExt, StreamExt};
use hyper::StatusCode;
use uuid::Uuid;

use crate::{
    session::TodoSessionExt,
    state::AppState,
    todo::{Command, TodoCommandSender, TodoList, TodoListInfo, TodoListWatcher},
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
    session.join_todo_list(&todo_list);
    todo_list
        .store(state.redis_pool())
        .await
        .expect("Failed to create todo list");

    Ok(Json(todo_list.id()))
}

async fn get_users_todo_lists(
    session: ReadableSession,
    State(state): State<AppState>,
) -> Json<Vec<TodoListInfo<'static>>> {
    let mut joined_lists = session
        .get::<Vec<TodoListInfo>>("user_lists")
        .unwrap_or_default();

    state.fill_todo_lists_info(&mut joined_lists).await;

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
    let (todo_watch, command_tx, abort_rx) = state.join_todo_list(todo_id, *user.id()).await;
    session.join_todo_list(&todo_watch.borrow());

    ws.on_upgrade(move |socket| {
        ws_handler(
            socket, state, todo_id, todo_watch, command_tx, abort_rx, user,
        )
    })
}

#[tracing::instrument(
    name = "WS handler",
    skip_all,
    fields(
        todo_list = %todo_list_id,
        user = %user.id()
    )
)]
async fn ws_handler(
    ws: WebSocket,
    state: AppState,
    todo_list_id: Uuid,
    mut todo: TodoListWatcher,
    command_tx: TodoCommandSender,
    mut abort_rx: tokio::sync::oneshot::Receiver<()>,
    user: User,
) {
    let (mut ws_tx, mut ws_rx) = ws.split();
    if command_tx
        .send(Command::UserJoin(user.clone()).with_issuer(user.clone()))
        .await
        .is_err()
    {
        tracing::error!(
            "User {} failed to join todo list {}",
            user.id(),
            todo_list_id
        );
        return;
    }

    loop {
        tokio::select! {
            Some(Ok(msg)) = ws_rx.next() => {
                if matches!(msg, Message::Close(_)) {
                    break;
                }
                if let Ok(command) = serde_json::from_slice::<Command>(msg.into_data().as_slice()) {
                    let _ = command_tx.send(command.with_issuer(user.clone())).await;
                }
            },
            Ok(()) = todo.changed() => {
                let _ = send_todo_list(&mut todo, &mut ws_tx).await;
            },
            _ = &mut abort_rx => {
                tracing::debug!("Abort received. Closing previous WS connection");
                let _ = ws_tx.close().await;
                return;
            },
            else => break,
        }
    }

    let _ = ws_tx.close().await;
    let _ = command_tx
        .send(Command::UserLeave(user.clone()).with_issuer(user.clone()))
        .await;
    state.leave_todo_list(todo_list_id, *user.id()).await;
    tracing::debug!("WS connection closed");
}

async fn send_todo_list(
    todo_list: &mut TodoListWatcher,
    ws_sink: &mut SplitSink<WebSocket, Message>,
) -> anyhow::Result<()> {
    let json_msg = {
        let todo_list = &*todo_list.borrow_and_update();
        serde_json::to_string(&todo_list).context("Failed to serialize todo list")?
    };
    ws_sink
        .send(Message::Text(json_msg))
        .await
        .context("Failed to send ws message")?;

    Ok(())
}
