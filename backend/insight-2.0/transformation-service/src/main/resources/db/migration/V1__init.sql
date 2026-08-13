CREATE TABLE transformation_jobs (
    id             UUID PRIMARY KEY,
    upload_id      UUID NOT NULL UNIQUE,
    saga_id        VARCHAR(64) NOT NULL,
    file_reference VARCHAR(1024) NOT NULL,
    file_type      VARCHAR(32) NOT NULL,
    status         VARCHAR(32) NOT NULL,
    total_rows     BIGINT NOT NULL DEFAULT 0,
    valid_rows     BIGINT NOT NULL DEFAULT 0,
    error_rows     BIGINT NOT NULL DEFAULT 0,
    requires_ocr   BOOLEAN NOT NULL DEFAULT FALSE,
    created_at     TIMESTAMPTZ NOT NULL,
    completed_at   TIMESTAMPTZ
);

CREATE TABLE row_validation_errors (
    id                    UUID PRIMARY KEY,
    transformation_job_id UUID NOT NULL REFERENCES transformation_jobs (id),
    row_number            BIGINT NOT NULL,
    reason                VARCHAR(1024) NOT NULL
);

CREATE INDEX idx_row_errors_job ON row_validation_errors (transformation_job_id);
