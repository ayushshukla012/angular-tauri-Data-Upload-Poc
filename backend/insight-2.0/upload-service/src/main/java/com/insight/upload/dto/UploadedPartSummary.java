package com.insight.upload.dto;

public record UploadedPartSummary(
        int partNumber,
        String eTag,
        long sizeBytes
) {}
