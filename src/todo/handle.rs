use std::collections::HashMap;

use super::TodoList;

use tokio::sync::{broadcast, oneshot};
use uuid::Uuid;

#[derive(Debug)]
pub struct TodoListHandle {
    todo_updates: broadcast::Sender<TodoList>,
    connected_users: HashMap<Uuid, oneshot::Sender<()>>,
}

impl Default for TodoListHandle {
    fn default() -> Self {
        Self {
            todo_updates: broadcast::channel(16).0,
            connected_users: HashMap::default(),
        }
    }
}

impl TodoListHandle {
    pub fn join(&mut self, user: Uuid) -> (broadcast::Sender<TodoList>, oneshot::Receiver<()>) {
        let (abort_tx, abort_rx) = oneshot::channel();

        if let Some(prev_session_abort_tx) = self.connected_users.insert(user, abort_tx) {
            let _ = prev_session_abort_tx.send(());
        }

        (self.todo_updates.clone(), abort_rx)
    }

    pub fn disconnect_user(&mut self, user: Uuid) {
        self.connected_users.remove(&user);
    }

    pub fn is_empty(&self) -> bool {
        self.connected_users.is_empty()
    }
}
