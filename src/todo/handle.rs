use std::{collections::HashSet, time::Duration};

use super::{list::TodoList, task::todo_list_task, TodoCommandSender, TodoListWatcher};

use anyhow::Context;
use sqlx::PgPool;
use tokio::task::JoinHandle;
use uuid::Uuid;

#[derive(Debug)]
pub struct TodoListHandle {
    command_tx: TodoCommandSender,
    todo_watcher: TodoListWatcher,
    task_handle: JoinHandle<()>,
    connected_users: HashSet<Uuid>,
}

impl TodoListHandle {
    pub async fn spawn(
        list_id: Uuid,
        pool: PgPool,
        store_interval: Duration,
    ) -> anyhow::Result<Self> {
        use tokio::sync::{mpsc, watch};

        let todo_list = TodoList::from_db(list_id, pool.clone())
            .await
            .context("Failed to retrieve todo list from db")?;

        let todo_id = todo_list.id();
        let (watch_tx, watch_rx) = watch::channel(todo_list);
        let (command_tx, command_rx) = mpsc::channel(16);
        let task_handle = tokio::spawn(todo_list_task(
            todo_id,
            watch_tx,
            command_rx,
            pool,
            store_interval,
        ));

        Ok(Self {
            command_tx,
            todo_watcher: watch_rx,
            task_handle,
            connected_users: HashSet::default(),
        })
    }

    pub fn get_connection(&mut self, user: Uuid) -> Option<(TodoListWatcher, TodoCommandSender)> {
        self.connected_users
            .insert(user)
            .then(|| (self.todo_watcher.clone(), self.command_tx.clone()))
    }

    pub fn disconnect_user(&mut self, user: Uuid) {
        self.connected_users.remove(&user);
    }

    pub fn is_empty(&self) -> bool {
        self.connected_users.is_empty()
    }
}

impl Drop for TodoListHandle {
    fn drop(&mut self) {
        self.task_handle.abort();
    }
}
