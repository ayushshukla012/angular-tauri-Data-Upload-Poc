package com.insight.upload.dto;

import jakarta.validation.constraints.NotBlank;

import java.util.Map;

/**
 * Mirrors {@code CaseRecord}/{@code CaseInput} on the insight-javafs client, field-for-field —
 * the full Verification Result (VSN) shape from the legacy "Verification Report Upload Utility".
 * Only the identity fields are {@code @NotBlank} here; the rest are enforced by the categorized
 * (Mandatory/Defect/Exception) validator, not bean validation.
 */
public record SubmitCaseRequest(
        @NotBlank String caseId,
        String referenceNumber,
        @NotBlank String sourcePan,
        @NotBlank String name,
        String dateOfBirth,
        String address,
        String stateUtCode,
        String pincode,
        @NotBlank String mobileNumber,
        String email,
        @NotBlank String designation,
        String informationFy,
        String informationSourceType,
        String informationSourceDescription,
        String informationType,
        String informationDescription,
        String informationValue,
        String natureOfVerification,
        String actionableAy,
        String verificationResultType1,
        String verificationResultDescription1,
        String verificationResultValue1,
        String verificationResultType2,
        String verificationResultDescription2,
        String verificationResultValue2,
        String verificationResultType3,
        String verificationResultDescription3,
        String verificationResultValue3,
        String remarks,
        String batchNumber,
        Map<String, String> extraFields
) {}
