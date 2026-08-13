package com.insight.upload.exception;

import com.insight.common.exception.ResourceNotFoundException;

public class CaseNotFoundException extends ResourceNotFoundException {

    public CaseNotFoundException(String caseId) {
        super("Case not found: " + caseId);
    }
}
