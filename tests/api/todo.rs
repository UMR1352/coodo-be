use std::{sync::Arc, time::Duration};

use anyhow::Context;
use coodo_be::todo::{Command, TaskCommandMeta, TodoList};
use futures_util::{SinkExt, StreamExt};
use reqwest::{cookie::Jar, Client};
use sqlx::PgPool;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;

use crate::helpers::TestApp;

#[sqlx::test]
async fn create_todo_without_session_returns_401(pool: PgPool) -> anyhow::Result<()> {
    let app = TestApp::spawn(pool).await;
    let client = Client::new();

    let response = client
        .post(format!("{}/todos", &app.address))
        .send()
        .await?;
    assert_eq!(response.status().as_u16(), 401);

    Ok(())
}

#[sqlx::test]
async fn create_todo_works(pool: PgPool) -> anyhow::Result<()> {
    let app = TestApp::spawn(pool.clone()).await;
    let mut client = Client::builder().cookie_store(true).build()?;

    let _user = app.get_user(&mut client).await?;
    let todo_list_id = app.create_todo_list(&mut client).await?;

    let todo_list = TodoList::from_db(todo_list_id, pool).await?;
    assert_eq!(todo_list_id, todo_list.id());

    Ok(())
}

#[sqlx::test]
async fn todo_list_workflow_works(pool: PgPool) -> anyhow::Result<()> {
    let app = TestApp::spawn(pool.clone()).await;
    let jar = Arc::new(Jar::default());
    let mut client = Client::builder().cookie_provider(jar.clone()).build()?;

    let user = app.get_user(&mut client).await?;
    let todo_list_id = app.create_todo_list(&mut client).await?;
    let (mut ws_sink, mut ws_stream) = app.connect_to_todo_list(todo_list_id, &jar).await?;

    let todo_list = {
        let msg = timeout(Duration::from_secs(1), ws_stream.next())
            .await
            .context("Timeout")?
            .context("No message")?
            .context("Websocket error")?
            .into_data();
        serde_json::from_slice::<TodoList>(&msg[..]).context("Failed to deserialize todo list")?
    };

    assert_eq!(todo_list.id(), todo_list_id);
    assert!(todo_list
        .connected_users()
        .get(0)
        .is_some_and(|u| u.id() == user.id()));

    ws_sink
        .send(Message::Binary(serde_json::to_vec(&Command::CreateTask)?))
        .await?;

    let todo_list_updated = {
        let msg = timeout(Duration::from_secs(1), ws_stream.next())
            .await
            .context("Timeout")?
            .context("Channel closed")?
            .context("Websocket error")?
            .into_data();
        serde_json::from_slice::<TodoList>(&msg[..]).context("Failed to deserialize todo list")?
    };
    assert_eq!(todo_list.id(), todo_list_updated.id());
    assert_eq!(todo_list_updated.tasks().len(), 1);

    let task_id = todo_list_updated.tasks()[0].id();
    let rename_task = Command::TaskCommand(TaskCommandMeta {
        task: task_id,
        command: coodo_be::todo::TaskCommand::Rename(String::from("my task")),
    });
    ws_sink
        .send(Message::Binary(serde_json::to_vec(&rename_task)?))
        .await?;

    let todo_list_updated = {
        let msg = timeout(Duration::from_secs(1), ws_stream.next())
            .await
            .context("Timeout")?
            .context("Channel closed")?
            .context("Websocket error")?
            .into_data();
        serde_json::from_slice::<TodoList>(&msg[..]).context("Failed to deserialize todo list")?
    };
    assert_eq!(todo_list_updated.tasks()[0].name(), "my task");
    Ok(())
}
