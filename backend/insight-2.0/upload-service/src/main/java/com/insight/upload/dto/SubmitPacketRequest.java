package com.insight.upload.dto;

import jakarta.validation.constraints.NotBlank;

public record SubmitPacketRequest(
        @NotBlank String batchNumber,
        String description,
        @NotBlank String submittingPersonName,
        String submittingPersonAddress,
        @NotBlank String submittingPersonMobile,
        String submittingPersonEmail
) {}
