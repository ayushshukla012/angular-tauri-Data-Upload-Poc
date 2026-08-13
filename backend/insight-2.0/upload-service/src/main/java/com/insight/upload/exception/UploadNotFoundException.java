package com.insight.upload.exception;

import com.insight.common.exception.ResourceNotFoundException;

public class UploadNotFoundException extends ResourceNotFoundException {

    public UploadNotFoundException(String uploadId) {
        super("Upload not found: " + uploadId);
    }
}
