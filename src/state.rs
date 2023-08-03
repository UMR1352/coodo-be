use std::{collections::HashMap, sync::Arc, time::Duration};

use anyhow::Context;
use sqlx::{pool::PoolConnection, PgPool, Postgres};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{
    settings::TodoHandlerSettings,
    todo::{TodoCommandSender, TodoListHandle, TodoListWatcher},
};

#[derive(Debug, Clone)]
pub struct AppState {
    db_pool: PgPool,
    todo_lists: Arc<RwLock<HashMap<Uuid, TodoListHandle>>>,
    store_interval: Duration,
}

impl AppState {
    pub fn new(db_pool: PgPool, config: &TodoHandlerSettings) -> Self {
        Self {
            db_pool,
            todo_lists: Arc::new(RwLock::new(HashMap::default())),
            store_interval: config.store_interval,
        }
    }

    pub async fn db_connection(&self) -> sqlx::Result<PoolConnection<Postgres>> {
        self.db_pool.acquire().await
    }

    pub fn db_pool(&self) -> PgPool {
        self.db_pool.clone()
    }

    pub async fn join_todo_list(
        &self,
        todo: Uuid,
        user_id: Uuid,
    ) -> anyhow::Result<(TodoListWatcher, TodoCommandSender)> {
        self.todo_lists
            .write()
            .await
            .entry(todo)
            .or_insert(
                TodoListHandle::spawn(todo, self.db_pool.clone(), self.store_interval).await?,
            )
            .get_connection(user_id)
            .context("Failed to connect to given todo list")
    }

    pub async fn leave_todo_list(&self, todo: Uuid, user_id: Uuid) {
        let mut todo_lists = self.todo_lists.write().await;
        let mut empty = false;
        if let Some(handle) = todo_lists.get_mut(&todo) {
            handle.disconnect_user(user_id);
            empty = handle.is_empty();
        }
        if empty {
            todo_lists.remove(&todo);
        }
    }
}
