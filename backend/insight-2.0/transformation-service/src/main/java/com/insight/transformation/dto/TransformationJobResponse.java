package com.insight.transformation.dto;

public record TransformationJobResponse(
        String uploadId,
        String status,
        long totalRows,
        long validRows,
        long errorRows,
        boolean requiresOcr
) {}
