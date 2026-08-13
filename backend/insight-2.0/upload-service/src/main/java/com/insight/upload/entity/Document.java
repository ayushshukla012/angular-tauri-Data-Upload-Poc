package com.insight.upload.entity;

import jakarta.persistence.*;
import java.time.Instant;
import java.util.UUID;

/**
 * The case-domain wrapper around one {@link Upload} row: a document is "this file belongs to this
 * case." Deliberately doesn't touch the {@code uploads} table itself — every other consumer of an
 * upload (the outbox relay, ocr-service, transformation-service) stays agnostic of "case" entirely.
 *
 * <p>Field set mirrors the legacy app's Documents grid: VSN (caseId), Document Type, Description,
 * Remarks, Document Name (docLabel).
 */
@Entity
@Table(name = "documents")
public class Document {

    @Id
    private UUID id;

    @Column(nullable = false)
    private String caseId;

    @Column(nullable = false)
    private UUID uploadId;

    private String docLabel;
    private String docType;
    @Column(columnDefinition = "TEXT")
    private String description;
    @Column(columnDefinition = "TEXT")
    private String remarks;

    /** Approval-workflow state for this document. */
    @Enumerated(EnumType.STRING)
    @Column(nullable = false)
    private ApprovalStatus approvalStatus = ApprovalStatus.PENDING;

    @Column(nullable = false)
    private Instant createdAt;

    protected Document() {
    }

    public Document(UUID id, String caseId, UUID uploadId, String docLabel, String docType, String description, String remarks) {
        this.id = id;
        this.caseId = caseId;
        this.uploadId = uploadId;
        this.docLabel = docLabel;
        this.docType = docType;
        this.description = description;
        this.remarks = remarks;
        this.createdAt = Instant.now();
    }

    public UUID getId() { return id; }
    public String getCaseId() { return caseId; }
    public UUID getUploadId() { return uploadId; }
    public String getDocLabel() { return docLabel; }
    public String getDocType() { return docType; }
    public String getDescription() { return description; }
    public String getRemarks() { return remarks; }
    public ApprovalStatus getApprovalStatus() { return approvalStatus; }
    public void setApprovalStatus(ApprovalStatus approvalStatus) { this.approvalStatus = approvalStatus; }
    public Instant getCreatedAt() { return createdAt; }
}
