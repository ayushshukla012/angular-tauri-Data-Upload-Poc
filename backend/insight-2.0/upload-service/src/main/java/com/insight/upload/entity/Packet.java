package com.insight.upload.entity;

import jakarta.persistence.*;
import java.time.Instant;

/**
 * A Packet/Batch — the legacy app's "Packet Details" screen (Batch Number, Description, Submitting
 * Person details). Purely a grouping + submitter-metadata concept here: cases (VSNs) are submitted
 * under a packet, but nothing gets zipped or exported — no XML/package generation in this platform.
 */
@Entity
@Table(name = "packets")
public class Packet {

    /** The natural batch identifier (e.g. from a physical file/note sheet) — not a generated UUID. */
    @Id
    private String batchNumber;

    @Column(columnDefinition = "TEXT")
    private String description;

    @Column(nullable = false)
    private String submittingPersonName;

    @Column(columnDefinition = "TEXT")
    private String submittingPersonAddress;

    @Column(nullable = false)
    private String submittingPersonMobile;

    private String submittingPersonEmail;

    /** Approval-workflow state for this packet/batch. */
    @Enumerated(EnumType.STRING)
    @Column(nullable = false)
    private ApprovalStatus approvalStatus = ApprovalStatus.PENDING;

    @Column(nullable = false)
    private Instant createdAt;

    protected Packet() {
    }

    public Packet(String batchNumber, String description, String submittingPersonName,
                  String submittingPersonAddress, String submittingPersonMobile, String submittingPersonEmail) {
        this.batchNumber = batchNumber;
        this.description = description;
        this.submittingPersonName = submittingPersonName;
        this.submittingPersonAddress = submittingPersonAddress;
        this.submittingPersonMobile = submittingPersonMobile;
        this.submittingPersonEmail = submittingPersonEmail;
        this.createdAt = Instant.now();
    }

    public String getBatchNumber() { return batchNumber; }
    public String getDescription() { return description; }
    public String getSubmittingPersonName() { return submittingPersonName; }
    public String getSubmittingPersonAddress() { return submittingPersonAddress; }
    public String getSubmittingPersonMobile() { return submittingPersonMobile; }
    public String getSubmittingPersonEmail() { return submittingPersonEmail; }
    public ApprovalStatus getApprovalStatus() { return approvalStatus; }
    public void setApprovalStatus(ApprovalStatus approvalStatus) { this.approvalStatus = approvalStatus; }
    public Instant getCreatedAt() { return createdAt; }
}
