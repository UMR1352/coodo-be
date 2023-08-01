use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

#[derive(Debug, serde::Serialize, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoList {
    id: Uuid,
    name: String,
    tasks: Vec<TodoTask>,
    created_at: DateTime<Utc>,
    last_updated_at: DateTime<Utc>,
}

impl Default for TodoList {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            name: String::new(),
            tasks: vec![],
            created_at: Utc::now(),
            last_updated_at: Utc::now(),
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

    pub async fn from_db(id: Uuid, pool: PgPool) -> sqlx::Result<Self> {
        let mut db_conn = pool.acquire().await?;
        let tasks = sqlx::query_as::<_, TodoTask>(
            r#"
SELECT id, name, assignee, done
FROM todo_tasks
WHERE list = $1
            "#,
        )
        .bind(id)
        .fetch_all(&mut db_conn)
        .await?;
        sqlx::query!(
            r#"
SELECT id, name, created_at, last_updated_at 
FROM todo_lists
WHERE id = $1
        "#,
            id
        )
        .fetch_one(&mut db_conn)
        .await
        .map(|record| Self {
            id: record.id,
            name: record.name,
            tasks,
            created_at: record.created_at,
            last_updated_at: record.last_updated_at,
        })
    }

    pub const fn id(&self) -> Uuid {
        self.id
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

    pub async fn store(&self, pool: PgPool) -> sqlx::Result<()> {
        let mut db_conn = pool.acquire().await?;
        sqlx::query!(
            r#"
INSERT INTO todo_lists (id, name, created_at, last_updated_at)
    VALUES ($1, $2, $3, $4)
    ON CONFLICT (id) DO
        UPDATE SET name = $2,
            created_at = $3,
            last_updated_at = $4
            "#,
            self.id,
            &self.name,
            self.created_at,
            self.last_updated_at
        )
        .execute(&mut db_conn)
        .await?;

        if !self.tasks.is_empty() {
            tasks_upsert_query(self.tasks.iter())
                .build()
                .execute(&mut db_conn)
                .await?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, FromRow, serde::Serialize, serde::Deserialize)]
pub struct TodoTask {
    id: Uuid,
    name: String,
    assignee: Uuid,
    done: bool,
}

impl TodoTask {
    pub fn new(assignee: Uuid) -> Self {
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

    pub const fn assignee(&self) -> Uuid {
        self.assignee
    }

    pub const fn is_done(&self) -> bool {
        self.done
    }

    pub fn assign_to(&mut self, assignee: Uuid) {
        self.assignee = assignee;
    }

    pub fn set_done(&mut self, done: bool) {
        self.done = done;
    }

    pub fn rename(&mut self, name: String) {
        self.name = name;
    }
}

fn tasks_upsert_query<'list>(
    tasks: impl Iterator<Item = &'list TodoTask>,
) -> QueryBuilder<'list, Postgres> {
    let mut builder = sqlx::QueryBuilder::new("INSERT INTO todo_tasks (id, name, assignee, done) ");
    builder
        .push_values(tasks, |mut b, task| {
            b.push_bind(task.id)
                .push_bind(task.name())
                .push_bind(task.assignee)
                .push_bind(task.done);
        })
        .push(
            r#"
ON CONFLICT (id) DO
    UPDATE SET name = EXCLUDED.name,
        assignee = EXCLUDED.assignee,
        done = EXCLUDED.done,
    "#,
        );

    builder
}
