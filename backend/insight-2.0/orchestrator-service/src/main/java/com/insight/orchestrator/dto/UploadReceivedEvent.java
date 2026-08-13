package com.insight.orchestrator.dto;

public record UploadReceivedEvent(
        String uploadId,
        String fileReference,
        String fileType
) {}
