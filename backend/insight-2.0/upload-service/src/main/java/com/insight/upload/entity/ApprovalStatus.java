package com.insight.upload.entity;

/** Approval-workflow state, tracked independently of {@link CaseStatus}/upload processing state. */
public enum ApprovalStatus {
    PENDING,
    APPROVED,
    REJECTED
}
