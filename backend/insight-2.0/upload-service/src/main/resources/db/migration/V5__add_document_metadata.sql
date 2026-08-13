ALTER TABLE documents
    ADD COLUMN doc_type    VARCHAR(128),
    ADD COLUMN description TEXT,
    ADD COLUMN remarks     TEXT;
