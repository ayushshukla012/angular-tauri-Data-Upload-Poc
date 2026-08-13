package com.insight.transformation.entity;

import jakarta.persistence.*;
import java.util.UUID;

@Entity
@Table(name = "row_validation_errors")
public class RowValidationError {

    @Id
    private UUID id;

    @Column(nullable = false)
    private UUID transformationJobId;

    @Column(nullable = false)
    private long rowNumber;

    @Column(nullable = false)
    private String reason;

    protected RowValidationError() {
    }

    public RowValidationError(UUID id, UUID transformationJobId, long rowNumber, String reason) {
        this.id = id;
        this.transformationJobId = transformationJobId;
        this.rowNumber = rowNumber;
        this.reason = reason;
    }

    public UUID getId() { return id; }
    public UUID getTransformationJobId() { return transformationJobId; }
    public long getRowNumber() { return rowNumber; }
    public String getReason() { return reason; }
}
