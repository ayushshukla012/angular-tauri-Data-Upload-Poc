package com.insight.reporting.dto;

import java.time.Instant;
import java.util.List;

public record UploadReportResponse(
        String uploadId,
        String status,
        long totalRows,
        long validRows,
        long errorRows,
        Instant generatedAt,
        List<String> errorSummary
) {}
