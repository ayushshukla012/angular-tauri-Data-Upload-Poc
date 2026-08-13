package com.insight.upload.dto;

import java.time.Instant;

public record InitiateUploadResponse(
        String uploadId,
        boolean multipart,
        /** Present only when {@code multipart} is false — PUT the whole file here directly. */
        String uploadUrl,
        /**
         * Present only when {@code multipart} is false. The exact Content-Type this server signed
         * the presigned URL with — the client's PUT must send this exact header value, or S3/MinIO
         * rejects it with SignatureDoesNotMatch (403), since content-type is bound into the SigV4
         * signature. The client must never derive its own Content-Type independently.
         */
        String contentType,
        /** Present only when {@code multipart} is true — see docs/resumable-uploads.md §6 for how to use these. */
        Long minPartSizeBytes,
        Long recommendedPartSizeBytes,
        Instant expiresAt
) {}
