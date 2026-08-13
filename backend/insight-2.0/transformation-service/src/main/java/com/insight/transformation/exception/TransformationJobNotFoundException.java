package com.insight.transformation.exception;

import com.insight.common.exception.ResourceNotFoundException;

public class TransformationJobNotFoundException extends ResourceNotFoundException {

    public TransformationJobNotFoundException(String uploadId) {
        super("Transformation job not found for upload: " + uploadId);
    }
}
