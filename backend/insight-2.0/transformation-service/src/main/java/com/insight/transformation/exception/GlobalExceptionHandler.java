package com.insight.transformation.exception;

import com.insight.common.dto.ApiError;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.ExceptionHandler;
import org.springframework.web.bind.annotation.RestControllerAdvice;

import java.time.Instant;
import java.util.List;

@RestControllerAdvice
public class GlobalExceptionHandler {

    @ExceptionHandler(TransformationJobNotFoundException.class)
    public ResponseEntity<ApiError> handleNotFound(TransformationJobNotFoundException ex) {
        ApiError error = new ApiError("RESOURCE_NOT_FOUND", ex.getMessage(), Instant.now(), List.of());
        return ResponseEntity.status(HttpStatus.NOT_FOUND).body(error);
    }
}
