package com.insight.upload.dto;

import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.Positive;

public record InitiateUploadRequest(
        @NotBlank String fileName,
        @Positive long fileSizeBytes,
        /** Optional — set only when this file is a document attached to an ITD case. */
        String caseId,
        /** Optional, only meaningful alongside {@link #caseId}. Mirrors the legacy app's Documents
         * grid (Document Type/Description/Remarks columns, "Document Name" = docLabel). */
        String docLabel,
        String docType,
        String description,
        String remarks
) {}
