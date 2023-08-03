use anyhow::Context;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use axum_sessions::extractors::ReadableSession;
use futures_util::{stream::SplitSink, SinkExt, StreamExt};
use hyper::StatusCode;
use uuid::Uuid;

use crate::{
    state::AppState,
    todo::{Command, TodoCommandSender, TodoList, TodoListWatcher},
    user::User,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/todos/:id", get(join_todo_list))
        .route("/todos", post(create_todo_list))
}

async fn create_todo_list(
    session: ReadableSession,
    State(state): State<AppState>,
) -> Result<Json<Uuid>, StatusCode> {
    let _user = session
        .get::<User>("user")
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let todo_list = TodoList::default();
    todo_list
        .store(state.db_pool())
        .await
        .expect("Failed to create todo list");

    Ok(Json(todo_list.id()))
}

// #[derive(Serialize, Debug)]
// struct TodoListInfo {
//     name: String,
//     id: Uuid,
// }

// async fn get_users_todo_lists(
//     session: ReadableSession,
//     State(state): State<AppState>
// ) -> Json<TodoListInfo> {

// }

async fn join_todo_list(
    session: ReadableSession,
    Path(todo_id): Path<Uuid>,
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    let user = if let Some(user) = session.get::<User>("user") {
        user
    } else {
        return (StatusCode::UNAUTHORIZED, "Establish a session first").into_response();
    };
    if let Ok((todo_watch, command_tx)) = state.join_todo_list(todo_id, *user.id()).await {
        ws.on_upgrade(move |socket| {
            ws_handler(socket, state, todo_id, todo_watch, command_tx, user)
        })
    } else {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to join requeste todo list",
        )
            .into_response()
    }
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
    user: User,
) {
    let (mut ws_tx, mut ws_rx) = ws.split();
    if command_tx
        .send(Command::UserJoin(user.clone()).with_issuer(*user.id()))
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
                if let Ok(command) = serde_json::from_slice::<Command>(msg.into_data().as_slice()) {
                    let _ = command_tx.send(command.with_issuer(*user.id())).await;
                }
            },
            Ok(()) = todo.changed() => {
                let _ = send_todo_list(&mut todo, &mut ws_tx).await;
            },
            else => {
                let _ = ws_tx.close().await;
                let _ = command_tx.send(Command::UserLeave(user.clone()).with_issuer(*user.id())).await;
                state.leave_todo_list(todo_list_id, *user.id()).await;

                break;
            }
        }
    }
}

async fn send_todo_list(
    todo_list: &mut TodoListWatcher,
    ws_sink: &mut SplitSink<WebSocket, Message>,
) -> anyhow::Result<()> {
    let bytes = {
        let todo_list = &*todo_list.borrow_and_update();
        serde_json::to_vec(&todo_list).context("Failed to serialize todo list")?
    };
    ws_sink
        .send(Message::Binary(bytes))
        .await
        .context("Failed to send ws message")?;

    Ok(())
}
