package com.insight.upload.exception;

import com.insight.common.exception.BaseException;

/** Thrown when a part-upload endpoint is called for an upload that initiated in single-shot mode. */
public class UploadNotMultipartException extends BaseException {

    public UploadNotMultipartException(String uploadId) {
        super("UPLOAD_NOT_MULTIPART", "Upload %s did not initiate as a multipart upload".formatted(uploadId));
    }
}
