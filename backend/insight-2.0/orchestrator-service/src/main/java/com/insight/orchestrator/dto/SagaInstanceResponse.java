package com.insight.orchestrator.dto;

public record SagaInstanceResponse(
        String sagaId,
        String uploadId,
        String state,
        String currentStep,
        int retryCount
) {}
