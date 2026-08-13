package com.insight.upload.exception;

import com.insight.common.dto.ApiError;
import com.insight.common.exception.BaseException;

import java.util.List;

/** Field-level case validation failure — the first real consumer of ApiError.fieldErrors(). */
public class CaseValidationException extends BaseException {

    private final List<ApiError.FieldError> fieldErrors;

    public CaseValidationException(List<ApiError.FieldError> fieldErrors) {
        super("CASE_VALIDATION_FAILED", "Case validation failed: " + summarize(fieldErrors));
        this.fieldErrors = fieldErrors;
    }

    public List<ApiError.FieldError> getFieldErrors() {
        return fieldErrors;
    }

    private static String summarize(List<ApiError.FieldError> fieldErrors) {
        return fieldErrors.stream()
                .map(e -> e.field() + ": " + e.reason())
                .reduce((a, b) -> a + "; " + b)
                .orElse("");
    }
}
