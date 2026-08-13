CREATE TABLE saga_instances (
    saga_id      UUID PRIMARY KEY,
    upload_id    UUID NOT NULL UNIQUE,
    state        VARCHAR(32) NOT NULL,
    current_step VARCHAR(128) NOT NULL,
    retry_count  INT NOT NULL DEFAULT 0,
    created_at   TIMESTAMPTZ NOT NULL,
    updated_at   TIMESTAMPTZ
);

CREATE INDEX idx_saga_state ON saga_instances (state);
