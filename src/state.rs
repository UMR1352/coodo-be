use std::{collections::HashMap, sync::Arc};

use deadpool_redis::Pool;
use tokio::sync::{broadcast, oneshot, RwLock};
use uuid::Uuid;

use crate::{
    redis_fcall,
    todo::{TodoList, TodoListHandle},
    user::User,
};

#[derive(Clone)]
pub struct AppState {
    redis_pool: Pool,
    todo_lists: Arc<RwLock<HashMap<Uuid, TodoListHandle>>>,
}

impl AppState {
    pub fn new(redis_pool: Pool) -> Self {
        Self {
            redis_pool,
            todo_lists: Arc::new(RwLock::new(HashMap::default())),
        }
    }

    pub fn redis_pool(&self) -> Pool {
        self.redis_pool.clone()
    }

    pub async fn join_todo_list(
        &self,
        todo: Uuid,
        user: User,
    ) -> anyhow::Result<(
        broadcast::Sender<TodoList>,
        broadcast::Receiver<TodoList>,
        oneshot::Receiver<()>,
    )> {
        let mut todo_lists = self.todo_lists.write().await;
        let mut todo_list_handle = todo_lists.remove(&todo).unwrap_or_default();
        let (todo_tx, abort_rx) = todo_list_handle.join(*user.id());
        todo_lists.insert(todo, todo_list_handle);
        drop(todo_lists);

        let todo_rx = todo_tx.subscribe();
        let mut redis_conn = self.redis_pool().get().await?;
        let todo_list = redis_fcall!(user_join_todo, todo.to_string(), user)
            .query_async(&mut redis_conn)
            .await?;
        todo_tx.send(todo_list).unwrap(); // Safety: Just created a receiver

        Ok((todo_tx, todo_rx, abort_rx))
    }

    pub async fn leave_todo_list(
        &self,
        todo: Uuid,
        user: User,
        todo_tx: broadcast::Sender<TodoList>,
    ) -> anyhow::Result<()> {
        let mut todo_lists = self.todo_lists.write().await;
        let mut empty = false;
        if let Some(handle) = todo_lists.get_mut(&todo) {
            handle.disconnect_user(*user.id());
            empty = handle.is_empty();

            let mut redis_conn = self.redis_pool().get().await?;
            let todo_list = redis_fcall!(user_leave_todo, todo.to_string(), user.id().to_string())
                .query_async(&mut redis_conn)
                .await?;
            let _ = todo_tx.send(todo_list);
            tracing::debug!("Disconnected from todo list");
        }
        if empty {
            todo_lists.remove(&todo);
            tracing::debug!("This TodoList has no user connected and has been docked");
        }

        Ok(())
    }
}
