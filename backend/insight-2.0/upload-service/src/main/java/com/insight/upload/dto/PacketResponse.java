package com.insight.upload.dto;

import java.time.Instant;

public record PacketResponse(
        String batchNumber,
        String description,
        String submittingPersonName,
        String submittingPersonAddress,
        String submittingPersonMobile,
        String submittingPersonEmail,
        String approvalStatus,
        Instant createdAt
) {}
