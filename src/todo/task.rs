use sqlx::PgPool;
use std::time::Duration;
use tokio::time;
use uuid::Uuid;

use super::{
    command::{Applicable, TodoCommand},
    TodoCommandReceiver, TodoListUpdater,
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
    pool: PgPool,
    store_interval: Duration,
) {
    tracing::info!("Spawned successfully!");
    let mut store_interval = time::interval(store_interval);
    let mut dirty = false;

    loop {
        tokio::select! {
            Some(command) = commands.recv() => {
                tracing::debug!("Got command {:?}", &command);
                updater.send_modify(|todo| {
                    let TodoCommand {
                        issuer,
                        command,
                    } = command;
                    command.apply(todo, issuer);
                });
                dirty = true;
            },
            _ = store_interval.tick() => {
                if dirty {
                    let current_todo = {(*updater.borrow()).clone()};
                    match current_todo.store(pool.clone()).await {
                        Ok(_) => {
                            tracing::debug!("Successfully stored to db");
                            dirty = false;
                        },
                        Err(_) => tracing::warn!("Couldn't store to db"),
                    }
                }
            },
            else => break,
        }
    }
}
