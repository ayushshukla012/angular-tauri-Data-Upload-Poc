package com.insight.upload.mapper;

import com.insight.upload.dto.CaseResponse;
import com.insight.upload.entity.Case;
import com.fasterxml.jackson.core.type.TypeReference;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.springframework.stereotype.Component;

import java.util.Map;

@Component
public class CaseMapper {

    private final ObjectMapper objectMapper;

    public CaseMapper(ObjectMapper objectMapper) {
        this.objectMapper = objectMapper;
    }

    public CaseResponse toResponse(Case caseEntity) {
        return new CaseResponse(
                caseEntity.getId(),
                caseEntity.getReferenceNumber(),
                caseEntity.getSourcePan(),
                caseEntity.getName(),
                caseEntity.getDateOfBirth(),
                caseEntity.getAddress(),
                caseEntity.getStateUtCode(),
                caseEntity.getPincode(),
                caseEntity.getMobileNumber(),
                caseEntity.getEmail(),
                caseEntity.getDesignation(),
                caseEntity.getInformationFy(),
                caseEntity.getInformationSourceType(),
                caseEntity.getInformationSourceDescription(),
                caseEntity.getInformationType(),
                caseEntity.getInformationDescription(),
                caseEntity.getInformationValue(),
                caseEntity.getNatureOfVerification(),
                caseEntity.getActionableAy(),
                caseEntity.getVerificationResultType1(),
                caseEntity.getVerificationResultDescription1(),
                caseEntity.getVerificationResultValue1(),
                caseEntity.getVerificationResultType2(),
                caseEntity.getVerificationResultDescription2(),
                caseEntity.getVerificationResultValue2(),
                caseEntity.getVerificationResultType3(),
                caseEntity.getVerificationResultDescription3(),
                caseEntity.getVerificationResultValue3(),
                caseEntity.getRemarks(),
                caseEntity.getBatchNumber(),
                caseEntity.getStatus().name(),
                caseEntity.getApprovalStatus().name(),
                caseEntity.getErrorMessage(),
                readExtraFields(caseEntity.getExtraFields()),
                caseEntity.getCreatedAt(),
                caseEntity.getUpdatedAt()
        );
    }

    private Map<String, String> readExtraFields(String json) {
        if (json == null || json.isBlank()) {
            return Map.of();
        }
        try {
            return objectMapper.readValue(json, new TypeReference<Map<String, String>>() {});
        } catch (Exception e) {
            throw new IllegalStateException("Failed to parse stored extra_fields JSON", e);
        }
    }
}
