package com.insight.orchestrator.entity;

import jakarta.persistence.*;
import java.time.Instant;
import java.util.UUID;

@Entity
@Table(name = "saga_instances")
public class SagaInstance {

    @Id
    private UUID sagaId;

    @Column(nullable = false)
    private UUID uploadId;

    @Enumerated(EnumType.STRING)
    @Column(nullable = false)
    private SagaState state;

    @Column(nullable = false)
    private String currentStep;

    @Column(nullable = false)
    private int retryCount;

    @Column(nullable = false)
    private Instant createdAt;

    private Instant updatedAt;

    protected SagaInstance() {
    }

    public SagaInstance(UUID sagaId, UUID uploadId) {
        this.sagaId = sagaId;
        this.uploadId = uploadId;
        this.state = SagaState.STARTED;
        this.currentStep = "TRANSFORMATION_DISPATCHED";
        this.retryCount = 0;
        this.createdAt = Instant.now();
    }

    public void transitionTo(SagaState state, String step) {
        this.state = state;
        this.currentStep = step;
        this.updatedAt = Instant.now();
    }

    public void incrementRetry() {
        this.retryCount++;
    }

    public UUID getSagaId() { return sagaId; }
    public UUID getUploadId() { return uploadId; }
    public SagaState getState() { return state; }
    public String getCurrentStep() { return currentStep; }
    public int getRetryCount() { return retryCount; }
}
