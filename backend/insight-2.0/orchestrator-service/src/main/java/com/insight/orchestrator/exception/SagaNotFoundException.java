package com.insight.orchestrator.exception;

import com.insight.common.exception.ResourceNotFoundException;

public class SagaNotFoundException extends ResourceNotFoundException {

    public SagaNotFoundException(String uploadId) {
        super("Saga not found for upload: " + uploadId);
    }
}
