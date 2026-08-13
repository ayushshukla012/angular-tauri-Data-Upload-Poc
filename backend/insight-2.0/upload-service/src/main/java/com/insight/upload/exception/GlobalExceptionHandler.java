package com.insight.upload.exception;

import com.insight.common.dto.ApiError;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.ExceptionHandler;
import org.springframework.web.bind.annotation.RestControllerAdvice;

import java.time.Instant;
import java.util.List;

@RestControllerAdvice
public class GlobalExceptionHandler {

    @ExceptionHandler(UploadNotFoundException.class)
    public ResponseEntity<ApiError> handleNotFound(UploadNotFoundException ex) {
        ApiError error = new ApiError("RESOURCE_NOT_FOUND", ex.getMessage(), Instant.now(), List.of());
        return ResponseEntity.status(HttpStatus.NOT_FOUND).body(error);
    }

    @ExceptionHandler(CaseNotFoundException.class)
    public ResponseEntity<ApiError> handleCaseNotFound(CaseNotFoundException ex) {
        ApiError error = new ApiError("RESOURCE_NOT_FOUND", ex.getMessage(), Instant.now(), List.of());
        return ResponseEntity.status(HttpStatus.NOT_FOUND).body(error);
    }

    @ExceptionHandler(PacketNotFoundException.class)
    public ResponseEntity<ApiError> handlePacketNotFound(PacketNotFoundException ex) {
        ApiError error = new ApiError("RESOURCE_NOT_FOUND", ex.getMessage(), Instant.now(), List.of());
        return ResponseEntity.status(HttpStatus.NOT_FOUND).body(error);
    }

    @ExceptionHandler(CaseValidationException.class)
    public ResponseEntity<ApiError> handleCaseValidation(CaseValidationException ex) {
        ApiError error = new ApiError(ex.getCode(), ex.getMessage(), Instant.now(), ex.getFieldErrors());
        return ResponseEntity.status(HttpStatus.BAD_REQUEST).body(error);
    }

    @ExceptionHandler(UploadNotCompletedException.class)
    public ResponseEntity<ApiError> handleNotCompleted(UploadNotCompletedException ex) {
        ApiError error = new ApiError(ex.getCode(), ex.getMessage(), Instant.now(), List.of());
        return ResponseEntity.status(HttpStatus.CONFLICT).body(error);
    }

    @ExceptionHandler(UploadNotMultipartException.class)
    public ResponseEntity<ApiError> handleNotMultipart(UploadNotMultipartException ex) {
        ApiError error = new ApiError(ex.getCode(), ex.getMessage(), Instant.now(), List.of());
        return ResponseEntity.status(HttpStatus.CONFLICT).body(error);
    }

    /** A deterministic client-input rejection — 400, not an unhandled 500 (see the exception's javadoc). */
    @ExceptionHandler(UnsupportedFileTypeException.class)
    public ResponseEntity<ApiError> handleUnsupportedFileType(UnsupportedFileTypeException ex) {
        ApiError error = new ApiError(ex.getCode(), ex.getMessage(), Instant.now(), List.of());
        return ResponseEntity.status(HttpStatus.BAD_REQUEST).body(error);
    }
}
