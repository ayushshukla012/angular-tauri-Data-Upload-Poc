CREATE TABLE uploads (
    id                UUID PRIMARY KEY,
    file_name         VARCHAR(512) NOT NULL,
    file_type         VARCHAR(32)  NOT NULL,
    storage_reference VARCHAR(1024) NOT NULL,
    status            VARCHAR(32)  NOT NULL,
    created_at        TIMESTAMPTZ  NOT NULL,
    updated_at        TIMESTAMPTZ
);

CREATE TABLE outbox_events (
    id           UUID PRIMARY KEY,
    topic        VARCHAR(256) NOT NULL,
    aggregate_id VARCHAR(128) NOT NULL,
    payload      TEXT NOT NULL,
    published    BOOLEAN NOT NULL DEFAULT FALSE,
    created_at   TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_outbox_unpublished ON outbox_events (published, created_at);
