use std::borrow::Cow;

use anyhow::Context;
use chrono::{DateTime, Utc};
use deadpool_redis::Pool;
use redis::JsonAsyncCommands;
use redis_macros::{FromRedisValue, ToRedisArgs};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::user::User;

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash, Clone)]
pub struct TodoListInfo<'t> {
    name: Cow<'t, str>,
    id: Uuid,
}

impl<'t> TodoListInfo<'t> {
    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn id_mut(&mut self) -> &mut Uuid {
        &mut self.id
    }

    pub fn set_name<S: ToOwned<Owned = String>>(&mut self, name: S) {
        self.name = Cow::Owned(name.to_owned())
    }
}

impl TodoListInfo<'static> {
    pub fn new_owned(id: Uuid, name: String) -> Self {
        Self {
            id,
            name: Cow::Owned(name),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRedisValue, ToRedisArgs)]
#[serde(rename_all = "camelCase")]
pub struct TodoList {
    id: Uuid,
    name: String,
    tasks: Vec<TodoTask>,
    created_at: DateTime<Utc>,
    last_updated_at: DateTime<Utc>,
    connected_users: Vec<User>,
}

impl Default for TodoList {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            name: String::new(),
            tasks: vec![],
            created_at: Utc::now(),
            last_updated_at: Utc::now(),
            connected_users: vec![],
        }
    }
}

impl TodoList {
    pub fn new(tasks: Vec<TodoTask>) -> Self {
        Self {
            tasks,
            ..Default::default()
        }
    }

    pub async fn from_redis(id: Uuid, pool: Pool) -> anyhow::Result<Self> {
        let mut redis = pool.get().await?;
        redis
            .json_get(id.to_string(), "$")
            .await
            .context("Failed to retrieve todo list")
    }

    pub const fn id(&self) -> Uuid {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn rename(&mut self, name: String) {
        self.name = name;
    }

    fn update_time(&mut self) {
        self.last_updated_at = Utc::now();
    }

    pub fn add_task(&mut self, task: TodoTask) {
        self.tasks.push(task);
        self.update_time();
    }

    pub fn tasks(&self) -> &[TodoTask] {
        &self.tasks[..]
    }

    pub fn task(&self, id: Uuid) -> Option<&TodoTask> {
        self.tasks.iter().find(|task| task.id() == id)
    }

    pub fn task_mut(&mut self, id: Uuid) -> Option<&mut TodoTask> {
        self.update_time(); // Enhancement: only call update_time if the mut ref gets used
        self.tasks.iter_mut().find(|task| task.id() == id)
    }

    pub async fn store(&self, pool: Pool) -> anyhow::Result<()> {
        let mut redis = pool.get().await?;
        redis.json_set(self.id.to_string(), "$", self).await?;
        Ok(())
    }

    pub fn connected_users(&self) -> &[User] {
        &self.connected_users[..]
    }

    pub fn add_user(&mut self, user: User) {
        self.connected_users.push(user);
    }

    pub fn remove_user(&mut self, id: Uuid) {
        self.connected_users.retain(|user| user.id() != &id);
    }

    pub fn as_info(&self) -> TodoListInfo<'_> {
        TodoListInfo {
            id: self.id,
            name: Cow::Borrowed(self.name()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToRedisArgs, FromRedisValue)]
pub struct TodoTask {
    id: Uuid,
    name: String,
    assignee: User,
    done: bool,
}

impl TodoTask {
    pub fn new(assignee: User) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: String::new(),
            assignee,
            done: false,
        }
    }

    pub const fn id(&self) -> Uuid {
        self.id
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub const fn assignee(&self) -> &User {
        &self.assignee
    }

    pub const fn is_done(&self) -> bool {
        self.done
    }

    pub fn assign_to(&mut self, assignee: User) {
        self.assignee = assignee;
    }

    pub fn set_done(&mut self, done: bool) {
        self.done = done;
    }

    pub fn rename(&mut self, name: String) {
        self.name = name;
    }
}
