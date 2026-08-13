ALTER TABLE uploads
    ADD COLUMN file_size_bytes    BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN multipart_upload_id VARCHAR(255);
