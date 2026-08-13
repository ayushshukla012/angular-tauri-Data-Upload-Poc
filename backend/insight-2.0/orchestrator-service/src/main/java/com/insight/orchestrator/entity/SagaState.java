package com.insight.orchestrator.entity;

public enum SagaState {
    STARTED,
    VALIDATING,
    PROCESSING,
    EXTRACTED,
    REPORTING,
    COMPLETED,
    FAILED,
    COMPENSATING,
    COMPENSATED
}
