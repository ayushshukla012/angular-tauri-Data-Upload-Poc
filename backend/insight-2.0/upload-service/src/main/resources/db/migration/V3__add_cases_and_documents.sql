CREATE TABLE cases (
    id            VARCHAR(64)  PRIMARY KEY,
    source_pan    VARCHAR(16)  NOT NULL,
    name          VARCHAR(256) NOT NULL,
    phone_number  VARCHAR(20)  NOT NULL,
    designation   VARCHAR(128) NOT NULL,
    status        VARCHAR(32)  NOT NULL,
    error_message TEXT,
    extra_fields  TEXT,
    created_at    TIMESTAMPTZ  NOT NULL,
    updated_at    TIMESTAMPTZ
);

CREATE TABLE documents (
    id         UUID PRIMARY KEY,
    case_id    VARCHAR(64) NOT NULL REFERENCES cases (id),
    upload_id  UUID NOT NULL REFERENCES uploads (id),
    doc_label  VARCHAR(256),
    created_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT uq_documents_upload_id UNIQUE (upload_id)
);

CREATE INDEX idx_documents_case_id ON documents (case_id);
