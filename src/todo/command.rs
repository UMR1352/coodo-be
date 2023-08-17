use anyhow::Context;
use axum::async_trait;
use deadpool_redis::Connection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{redis_fcall, user::User};

use super::list::{TodoList, TodoTask};

#[async_trait]
pub trait Applicable {
    async fn apply(
        self,
        redis: &mut Connection,
        todo_id: Uuid,
        issuer: User,
    ) -> anyhow::Result<TodoList>;
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

#[async_trait]
impl Applicable for Command {
    async fn apply(
        self,
        redis: &mut Connection,
        todo_id: Uuid,
        issuer: User,
    ) -> anyhow::Result<TodoList> {
        let todo_id_str = todo_id.to_string();

        if let Command::TaskCommand(task_cmd) = self {
            return task_cmd.apply(redis, todo_id, issuer).await;
        }

        let redis_cmd = match self {
            Command::CreateTask => redis_fcall!(add_task, todo_id_str, TodoTask::new(issuer)),
            Command::SetListName(name) => redis_fcall!(set_todo_name, todo_id_str, name),
            Command::TaskCommand(_) => unreachable!(),
        };

        redis_cmd
            .query_async(redis)
            .await
            .context("Failed to apply TodoCommand")
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

#[async_trait]
impl Applicable for TaskCommandMeta {
    async fn apply(
        self,
        redis: &mut Connection,
        todo_id: Uuid,
        issuer: User,
    ) -> anyhow::Result<TodoList> {
        let todo_id = todo_id.to_string();
        let task = self.task.to_string();
        let redis_cmd = match self.command {
            TaskCommand::SetDone(is_done) => {
                redis_fcall!(set_task_done, todo_id, task, is_done, issuer)
            }
            TaskCommand::Rename(name) => redis_fcall!(set_task_name, todo_id, task, name),
            TaskCommand::SetAssignee(assignee) => {
                redis_fcall!(set_task_assignee, todo_id, task, assignee)
            }
        };
        redis_cmd
            .query_async(redis)
            .await
            .context("Failed to apply TodoCommand")
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
