package com.insight.ocr.exception;

import com.insight.common.exception.ResourceNotFoundException;

public class OcrJobNotFoundException extends ResourceNotFoundException {

    public OcrJobNotFoundException(String uploadId) {
        super("OCR job not found for upload: " + uploadId);
    }
}
