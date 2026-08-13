package com.insight.upload.dto;

import java.time.Instant;

public record PresignPartResponse(
        int partNumber,
        String uploadUrl,
        Instant expiresAt
) {}
