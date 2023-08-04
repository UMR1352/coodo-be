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
    }

    tracing::info!("Closing todo list");
    let todo_list = updater.send_replace(TodoList::default());
    if todo_list.store(pool).await.is_err() {
        tracing::error!("Failed to store list!")
    }
}
