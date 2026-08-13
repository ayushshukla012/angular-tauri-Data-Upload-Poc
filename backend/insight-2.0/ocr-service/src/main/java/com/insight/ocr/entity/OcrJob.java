package com.insight.ocr.entity;

import jakarta.persistence.*;
import java.time.Instant;
import java.util.UUID;

@Entity
@Table(name = "ocr_jobs")
public class OcrJob {

    @Id
    private UUID id;

    @Column(nullable = false)
    private UUID uploadId;

    @Column(nullable = false)
    private String sagaId;

    @Enumerated(EnumType.STRING)
    @Column(nullable = false)
    private OcrStatus status;

    private int documentCount;
    private int extractedCount;

    @Column(nullable = false)
    private Instant createdAt;

    protected OcrJob() {
    }

    public OcrJob(UUID id, UUID uploadId, String sagaId, int documentCount) {
        this.id = id;
        this.uploadId = uploadId;
        this.sagaId = sagaId;
        this.documentCount = documentCount;
        this.status = OcrStatus.IN_PROGRESS;
        this.createdAt = Instant.now();
    }

    public UUID getId() { return id; }
    public UUID getUploadId() { return uploadId; }
    public String getSagaId() { return sagaId; }
    public OcrStatus getStatus() { return status; }
    public int getDocumentCount() { return documentCount; }
    public int getExtractedCount() { return extractedCount; }
}
