use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::user::User;

use super::list::{TodoList, TodoTask};

pub trait Applicable {
    fn apply(self, todo: &mut TodoList, issuer: User);
}

#[derive(Debug)]
pub struct TodoCommand {
    pub issuer: User,
    pub command: Command,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum Command {
    TaskCommand(TaskCommandMeta),
    CreateTask,
    UserJoin(User),
    UserLeave(User),
    SetListName(String),
}

impl Command {
    pub const fn with_issuer(self, issuer: User) -> TodoCommand {
        TodoCommand {
            issuer,
            command: self,
        }
    }
}

impl Applicable for Command {
    fn apply(self, todo: &mut TodoList, issuer: User) {
        match self {
            Command::TaskCommand(task_command) => task_command.apply(todo, issuer),
            Command::CreateTask => todo.add_task(TodoTask::new(issuer)),
            Command::UserJoin(user) => todo.add_user(user),
            Command::UserLeave(user) => todo.remove_user(*user.id()),
            Command::SetListName(name) => todo.rename(name),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TaskCommandMeta {
    pub task: Uuid,
    #[serde(flatten)]
    pub command: TaskCommand,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "action", content = "data", rename_all = "snake_case")]
pub enum TaskCommand {
    SetDone(bool),
    Rename(String),
    SetAssignee(User),
}

impl Applicable for TaskCommandMeta {
    fn apply(self, todo: &mut TodoList, issuer: User) {
        if let Some(task) = todo.task_mut(self.task) {
            match self.command {
                TaskCommand::SetDone(is_done) => {
                    task.set_done(is_done);
                    task.assign_to(issuer)
                }
                TaskCommand::Rename(name) => {
                    task.rename(name);
                }
                TaskCommand::SetAssignee(assignee) => task.assign_to(assignee),
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use super::{Command, TaskCommand, TaskCommandMeta};

    #[test]
    fn create_task_command_deserialization_works() {
        let command_json = r#"
{
    "type": "create_task"
}
        "#;
        let command = serde_json::from_str::<Command>(command_json);
        assert!(command.is_ok_and(|cmd| matches!(cmd, Command::CreateTask)))
    }

    #[test]
    fn task_command_deserialization_works() {
        let command_json = r#"
{
    "type": "task_command",
    "data": {
        "task": "a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8",
        "action": "set_done",
        "data": true
    }
}
        "#;
        let command = serde_json::from_str::<Command>(command_json);
        assert!(command.is_ok_and(|cmd| matches!(
            cmd,
            Command::TaskCommand(TaskCommandMeta {
                command: TaskCommand::SetDone(true),
                ..
            })
        )));
    }
}
