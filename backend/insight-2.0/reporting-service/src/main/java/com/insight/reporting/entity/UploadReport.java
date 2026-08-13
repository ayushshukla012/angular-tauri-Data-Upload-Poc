package com.insight.reporting.entity;

import jakarta.persistence.*;
import java.time.Instant;
import java.util.UUID;

@Entity
@Table(name = "upload_reports")
public class UploadReport {

    @Id
    private UUID id;

    @Column(nullable = false)
    private UUID uploadId;

    @Enumerated(EnumType.STRING)
    @Column(nullable = false)
    private ReportStatus status;

    private long totalRows;
    private long validRows;
    private long errorRows;

    @Column(nullable = false)
    private Instant generatedAt;

    protected UploadReport() {
    }

    public UploadReport(UUID id, UUID uploadId, long totalRows, long validRows, long errorRows) {
        this.id = id;
        this.uploadId = uploadId;
        this.status = ReportStatus.GENERATED;
        this.totalRows = totalRows;
        this.validRows = validRows;
        this.errorRows = errorRows;
        this.generatedAt = Instant.now();
    }

    public UUID getId() { return id; }
    public UUID getUploadId() { return uploadId; }
    public ReportStatus getStatus() { return status; }
    public long getTotalRows() { return totalRows; }
    public long getValidRows() { return validRows; }
    public long getErrorRows() { return errorRows; }
    public Instant getGeneratedAt() { return generatedAt; }
}
