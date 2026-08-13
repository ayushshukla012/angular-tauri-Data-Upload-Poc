package com.insight.reporting.exception;

import com.insight.common.exception.ResourceNotFoundException;

public class UploadReportNotFoundException extends ResourceNotFoundException {

    public UploadReportNotFoundException(String uploadId) {
        super("Report not found for upload: " + uploadId);
    }
}
