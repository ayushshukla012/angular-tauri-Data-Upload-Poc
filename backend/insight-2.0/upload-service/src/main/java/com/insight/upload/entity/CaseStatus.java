package com.insight.upload.entity;

public enum CaseStatus {
    /** Case metadata accepted; no documents received yet. */
    RECEIVED,
    /** At least one document received; others may still be in flight. */
    PROCESSING,
    /**
     * Every document currently attached to this case has reached {@code Upload.RECEIVED} in
     * upload-service. Deliberately does NOT mean the cross-service saga (transformation/OCR/
     * reporting) has finished — orchestrator-service doesn't track cases at all yet, only
     * individual uploads, so no service in this platform can make that broader claim today.
     */
    COMPLETED,
    FAILED
}
