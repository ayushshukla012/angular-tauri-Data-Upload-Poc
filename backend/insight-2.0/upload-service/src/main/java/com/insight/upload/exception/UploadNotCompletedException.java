package com.insight.upload.exception;

import com.insight.common.exception.BaseException;

/** Thrown when the client calls "complete" but the file was never actually written to storage. */
public class UploadNotCompletedException extends BaseException {

    public UploadNotCompletedException(String uploadId) {
        super("UPLOAD_NOT_COMPLETED", "No file found in storage for upload: " + uploadId);
    }

    public UploadNotCompletedException(String uploadId, String reason) {
        super("UPLOAD_NOT_COMPLETED", "Upload %s is not ready to complete: %s".formatted(uploadId, reason));
    }
}
