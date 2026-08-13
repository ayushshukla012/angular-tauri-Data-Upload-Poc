package com.insight.upload.dto;

public record CaseDocumentResponse(
        String uploadId,
        String docLabel,
        String docType,
        String description,
        String remarks,
        String fileName,
        String status,
        String approvalStatus
) {}
