CREATE TABLE tasks (
    id            SERIAL PRIMARY KEY,
    title         VARCHAR(255) NOT NULL,
    description   TEXT,
    state         TEXT NOT NULL DEFAULT 'todo'
                  CHECK (state IN ('todo', 'in_progress', 'review', 'done')),
    priority      TEXT NOT NULL DEFAULT 'medium'
                  CHECK (priority IN ('low', 'medium', 'high', 'urgent')),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_tasks_state ON tasks(state);
CREATE INDEX idx_tasks_priority ON tasks(priority);
CREATE INDEX idx_tasks_created_at ON tasks(created_at DESC);

CREATE TABLE task_media (
    id          SERIAL PRIMARY KEY,
    task_id     INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    media_uuid  UUID NOT NULL UNIQUE DEFAULT gen_random_uuid(),
    media_type  TEXT NOT NULL
                CHECK (media_type IN ('image', 'video', 'audio')),
    extension   VARCHAR(10) NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_task_media_task_id ON task_media(task_id);
