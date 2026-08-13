package com.insight.upload.dto;

import java.time.Instant;
import java.util.Map;

public record CaseResponse(
        String caseId,
        String referenceNumber,
        String sourcePan,
        String name,
        String dateOfBirth,
        String address,
        String stateUtCode,
        String pincode,
        String mobileNumber,
        String email,
        String designation,
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
        String status,
        String approvalStatus,
        String errorMessage,
        Map<String, String> extraFields,
        Instant createdAt,
        Instant updatedAt
) {}
