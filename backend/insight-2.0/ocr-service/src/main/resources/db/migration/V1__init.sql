CREATE TABLE ocr_jobs (
    id               UUID PRIMARY KEY,
    upload_id        UUID NOT NULL UNIQUE,
    saga_id          VARCHAR(64) NOT NULL,
    status           VARCHAR(32) NOT NULL,
    document_count   INT NOT NULL DEFAULT 0,
    extracted_count  INT NOT NULL DEFAULT 0,
    created_at       TIMESTAMPTZ NOT NULL
);
