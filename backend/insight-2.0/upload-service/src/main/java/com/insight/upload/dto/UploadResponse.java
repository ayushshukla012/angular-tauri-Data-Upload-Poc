package com.insight.upload.dto;

import java.time.Instant;

public record UploadResponse(
        String uploadId,
        String fileName,
        String fileType,
        String status,
        Instant createdAt
) {}
