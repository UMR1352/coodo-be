use std::{collections::HashMap, time::Duration};

use super::{
    list::TodoList, task::todo_list_task, TodoCommandSender, TodoListInfo, TodoListWatcher,
};

use anyhow::Context;
use deadpool_redis::Pool;
use tokio::{sync::oneshot, task::JoinHandle};
use uuid::Uuid;

#[derive(Debug)]
pub struct TodoListHandle {
    command_tx: TodoCommandSender,
    todo_watcher: TodoListWatcher,
    _task_handle: JoinHandle<()>,
    connected_users: HashMap<Uuid, oneshot::Sender<()>>,
}

impl TodoListHandle {
    pub async fn spawn(
        list_id: Uuid,
        pool: Pool,
        store_interval: Duration,
    ) -> anyhow::Result<Self> {
        use tokio::sync::{mpsc, watch};

        let todo_list = TodoList::from_redis(list_id, pool.clone())
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
            _task_handle: task_handle,
            connected_users: HashMap::default(),
        })
    }

    pub fn get_connection(
        &mut self,
        user: Uuid,
    ) -> (TodoListWatcher, TodoCommandSender, oneshot::Receiver<()>) {
        let (abort_tx, abort_rx) = oneshot::channel();

        if let Some(prev_session_abort_tx) = self.connected_users.insert(user, abort_tx) {
            let _ = prev_session_abort_tx.send(());
        }

        (self.todo_watcher.clone(), self.command_tx.clone(), abort_rx)
    }

    pub fn peek(&self) -> TodoListInfo<'static> {
        let list = self.todo_watcher.borrow();
        TodoListInfo::new_owned(list.id(), list.name().to_owned())
    }

    pub fn disconnect_user(&mut self, user: Uuid) {
        self.connected_users.remove(&user);
    }

    pub fn is_empty(&self) -> bool {
        self.connected_users.is_empty()
    }
}
