use deadpool_redis::Pool;
use std::time::Duration;
use uuid::Uuid;

use super::{
    command::{Applicable, TodoCommand},
    TodoCommandReceiver, TodoList, TodoListUpdater,
};

#[tracing::instrument(
    name = "Todo list handler",
    skip_all,
    fields(list = %todo_id)
)]
pub async fn todo_list_task(
    todo_id: Uuid,
    updater: TodoListUpdater,
    mut commands: TodoCommandReceiver,
    pool: Pool,
    _store_interval: Duration,
) {
    tracing::info!("Spawned successfully!");

    while let Some(command) = commands.recv().await {
        tracing::debug!("Got command {:?}", &command);
        updater.send_modify(|todo| {
            let TodoCommand { issuer, command } = command;
            command.apply(todo, issuer);
        });

        let current_state = updater.borrow().clone();
        store_todo_list(current_state, pool.clone()).await;
    }
}

async fn store_todo_list(todo_list: TodoList, pool: Pool) {
    if todo_list.store(pool).await.is_err() {
        tracing::warn!("Failed to store todo list");
    }
}
