package com.insight.ocr.dto;

public record OcrJobResponse(
        String uploadId,
        String status,
        int documentCount,
        int extractedCount
) {}
