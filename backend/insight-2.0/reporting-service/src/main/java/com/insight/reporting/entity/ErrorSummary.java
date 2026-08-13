package com.insight.reporting.entity;

import jakarta.persistence.*;
import java.util.UUID;

@Entity
@Table(name = "error_summaries")
public class ErrorSummary {

    @Id
    private UUID id;

    @Column(nullable = false)
    private UUID uploadReportId;

    @Column(nullable = false)
    private String reason;

    @Column(nullable = false)
    private long occurrences;

    protected ErrorSummary() {
    }

    public ErrorSummary(UUID id, UUID uploadReportId, String reason, long occurrences) {
        this.id = id;
        this.uploadReportId = uploadReportId;
        this.reason = reason;
        this.occurrences = occurrences;
    }

    public UUID getId() { return id; }
    public UUID getUploadReportId() { return uploadReportId; }
    public String getReason() { return reason; }
    public long getOccurrences() { return occurrences; }
}
