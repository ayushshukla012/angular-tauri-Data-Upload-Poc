CREATE TABLE upload_reports (
    id           UUID PRIMARY KEY,
    upload_id    UUID NOT NULL UNIQUE,
    status       VARCHAR(32) NOT NULL,
    total_rows   BIGINT NOT NULL DEFAULT 0,
    valid_rows   BIGINT NOT NULL DEFAULT 0,
    error_rows   BIGINT NOT NULL DEFAULT 0,
    generated_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE error_summaries (
    id               UUID PRIMARY KEY,
    upload_report_id UUID NOT NULL REFERENCES upload_reports (id),
    reason           VARCHAR(1024) NOT NULL,
    occurrences      BIGINT NOT NULL
);

CREATE INDEX idx_error_summary_report ON error_summaries (upload_report_id);
