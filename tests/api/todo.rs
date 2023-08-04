use std::{sync::Arc, time::Duration};

use anyhow::Context;
use coodo_be::todo::{Command, TaskCommandMeta, TodoList};
use futures_util::{SinkExt, StreamExt};
use reqwest::{cookie::Jar, Client};
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;

use crate::helpers::TestApp;

#[tokio::test]
async fn create_todo_without_session_returns_401() -> anyhow::Result<()> {
    let app = TestApp::spawn().await;
    let client = Client::new();

    let response = client
        .post(format!("{}/todos", &app.address))
        .send()
        .await?;
    assert_eq!(response.status().as_u16(), 401);

    Ok(())
}

#[tokio::test]
async fn create_todo_works() -> anyhow::Result<()> {
    let app = TestApp::spawn().await;
    let mut client = Client::builder().cookie_store(true).build()?;

    let _user = app.get_user(&mut client).await?;
    let todo_list_id = app.create_todo_list(&mut client).await?;

    let todo_list = TodoList::from_redis(todo_list_id, app.redis_pool()).await?;
    assert_eq!(todo_list_id, todo_list.id());

    Ok(())
}

#[tokio::test]
async fn todo_list_workflow_works() -> anyhow::Result<()> {
    let app = TestApp::spawn().await;
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

#[tokio::test]
async fn user_get_todos_works() -> anyhow::Result<()> {
    let app = TestApp::spawn().await;
    let jar = Arc::new(Jar::default());
    let mut client = Client::builder().cookie_provider(jar.clone()).build()?;

    let _user = app.get_user(&mut client).await?;
    app.create_todo_list(&mut client).await?;
    app.create_todo_list(&mut client).await?;

    let joined_todos = app.get_joined_todo_lists(&client).await?;
    assert_eq!(joined_todos.len(), 2);

    Ok(())
}

#[tokio::test]
async fn user_delete_todos_works() -> anyhow::Result<()> {
    let app = TestApp::spawn().await;
    let jar = Arc::new(Jar::default());
    let mut client = Client::builder().cookie_provider(jar.clone()).build()?;

    let _user = app.get_user(&mut client).await?;
    let todo_id = app.create_todo_list(&mut client).await?;

    let response = client
        .delete(format!("{}/todos/{}", app.address, todo_id))
        .send()
        .await
        .context("Failed to send delete /todos")?;
    assert!(response.status().is_success());

    let joined_todos = app.get_joined_todo_lists(&client).await?;
    assert!(joined_todos.is_empty());

    Ok(())
}
