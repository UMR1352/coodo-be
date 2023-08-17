mod command;
mod handle;
mod list;

pub type TodoCommandReceiver = tokio::sync::mpsc::Receiver<TodoCommand>;
pub type TodoCommandSender = tokio::sync::mpsc::Sender<TodoCommand>;
pub type TodoListWatcher = tokio::sync::watch::Receiver<TodoList>;
pub type TodoListUpdater = tokio::sync::watch::Sender<TodoList>;

pub use command::{Applicable, Command, TaskCommand, TaskCommandMeta, TodoCommand};
pub use handle::TodoListHandle;
pub use list::{TodoList, TodoListInfo};
