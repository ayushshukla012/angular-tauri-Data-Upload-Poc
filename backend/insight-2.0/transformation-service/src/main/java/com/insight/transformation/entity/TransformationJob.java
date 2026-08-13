package com.insight.transformation.entity;

import jakarta.persistence.*;
import java.time.Instant;
import java.util.UUID;

@Entity
@Table(name = "transformation_jobs")
public class TransformationJob {

    @Id
    private UUID id;

    @Column(nullable = false)
    private UUID uploadId;

    @Column(nullable = false)
    private String sagaId;

    @Column(nullable = false)
    private String fileReference;

    @Column(nullable = false)
    private String fileType;

    @Enumerated(EnumType.STRING)
    @Column(nullable = false)
    private TransformationStatus status;

    private long totalRows;
    private long validRows;
    private long errorRows;
    private boolean requiresOcr;

    @Column(nullable = false)
    private Instant createdAt;

    private Instant completedAt;

    protected TransformationJob() {
    }

    public TransformationJob(UUID id, UUID uploadId, String sagaId, String fileReference, String fileType) {
        this.id = id;
        this.uploadId = uploadId;
        this.sagaId = sagaId;
        this.fileReference = fileReference;
        this.fileType = fileType;
        this.status = TransformationStatus.IN_PROGRESS;
        this.createdAt = Instant.now();
    }

    public void markCompleted(long totalRows, long validRows, long errorRows, boolean requiresOcr) {
        this.totalRows = totalRows;
        this.validRows = validRows;
        this.errorRows = errorRows;
        this.requiresOcr = requiresOcr;
        this.status = errorRows == 0 ? TransformationStatus.COMPLETED : TransformationStatus.COMPLETED_WITH_ERRORS;
        this.completedAt = Instant.now();
    }

    public void markFailed() {
        this.status = TransformationStatus.FAILED;
        this.completedAt = Instant.now();
    }

    public UUID getId() { return id; }
    public UUID getUploadId() { return uploadId; }
    public String getSagaId() { return sagaId; }
    public String getFileReference() { return fileReference; }
    public String getFileType() { return fileType; }
    public TransformationStatus getStatus() { return status; }
    public long getTotalRows() { return totalRows; }
    public long getValidRows() { return validRows; }
    public long getErrorRows() { return errorRows; }
    public boolean isRequiresOcr() { return requiresOcr; }
    public Instant getCompletedAt() { return completedAt; }
}
