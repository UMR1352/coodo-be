use std::{collections::HashMap, sync::Arc, time::Duration};

use deadpool_redis::Pool;
use redis::JsonAsyncCommands;
use tokio::sync::{oneshot, RwLock};
use uuid::Uuid;

use crate::{
    settings::TodoHandlerSettings,
    todo::{TodoCommandSender, TodoListHandle, TodoListInfo, TodoListWatcher},
};

#[derive(Clone)]
pub struct AppState {
    redis_pool: Pool,
    todo_lists: Arc<RwLock<HashMap<Uuid, TodoListHandle>>>,
    store_interval: Duration,
}

impl AppState {
    pub fn new(redis_pool: Pool, config: &TodoHandlerSettings) -> Self {
        Self {
            redis_pool,
            todo_lists: Arc::new(RwLock::new(HashMap::default())),
            store_interval: config.store_interval,
        }
    }

    pub fn redis_pool(&self) -> Pool {
        self.redis_pool.clone()
    }

    pub async fn join_todo_list(
        &self,
        todo: Uuid,
        user_id: Uuid,
    ) -> (TodoListWatcher, TodoCommandSender, oneshot::Receiver<()>) {
        let mut todo_lists = self.todo_lists.write().await;
        let mut todo_list_handle = match todo_lists.remove(&todo) {
            Some(handle) => handle,
            None => TodoListHandle::spawn(todo, self.redis_pool(), self.store_interval)
                .await
                .expect("Failed to spawn TodoListHandle"),
        };
        let connection_data = todo_list_handle.get_connection(user_id);
        todo_lists.insert(todo, todo_list_handle);

        connection_data
    }

    pub async fn leave_todo_list(&self, todo: Uuid, user_id: Uuid) {
        let mut todo_lists = self.todo_lists.write().await;
        let mut empty = false;
        if let Some(handle) = todo_lists.get_mut(&todo) {
            handle.disconnect_user(user_id);
            tracing::debug!("User {user_id} has left TodoList {todo}");
            empty = handle.is_empty();
        }
        if empty {
            todo_lists.remove(&todo);
            tracing::debug!("TodoList {todo} has no user connected and has been docked");
        }
    }

    pub async fn fill_todo_lists_info(&self, lists: &mut [TodoListInfo<'_>]) {
        let mut redis = self
            .redis_pool
            .get()
            .await
            .expect("Failed to acquire redis connection");
        let todo_lists = self.todo_lists.read().await;
        for list in lists.iter_mut() {
            if let Some(todo) = todo_lists.get(&list.id()) {
                *list = todo.peek();
            } else {
                let list_name = redis
                    .json_get::<_, _, String>(list.id().to_string(), "name")
                    .await
                    .unwrap()
                    .trim_matches('"')
                    .to_owned();
                list.set_name(list_name);
            }
        }
    }
}
