CREATE TABLE todo_tasks (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    assignee UUID NOT NULL,
    done BOOLEAN NOT NULL,
    list UUID REFERENCES todo_lists(id) NOT NULL
);
