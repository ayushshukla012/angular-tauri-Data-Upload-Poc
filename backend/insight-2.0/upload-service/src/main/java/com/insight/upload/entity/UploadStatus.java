package com.insight.upload.entity;

public enum UploadStatus {
    /** Presigned URL issued, waiting for the client to finish the direct upload to storage. */
    PENDING,
    RECEIVED,
    VALIDATING,
    PROCESSING,
    COMPLETED,
    FAILED
}
