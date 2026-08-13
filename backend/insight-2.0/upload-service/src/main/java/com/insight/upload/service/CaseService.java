package com.insight.upload.service;

import com.fasterxml.jackson.databind.ObjectMapper;
import com.insight.common.dto.ApiError;
import com.insight.upload.dto.CaseDocumentResponse;
import com.insight.upload.dto.CaseResponse;
import com.insight.upload.dto.SubmitCaseRequest;
import com.insight.upload.entity.Case;
import com.insight.upload.entity.Document;
import com.insight.upload.entity.Upload;
import com.insight.upload.exception.CaseNotFoundException;
import com.insight.upload.exception.CaseValidationException;
import com.insight.upload.mapper.CaseMapper;
import com.insight.upload.repository.CaseRepository;
import com.insight.upload.repository.DocumentRepository;
import com.insight.upload.repository.UploadRepository;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;
import java.util.stream.Collectors;

@Service
public class CaseService {

    private final CaseRepository caseRepository;
    private final DocumentRepository documentRepository;
    private final UploadRepository uploadRepository;
    private final CaseValidator caseValidator;
    private final CaseMapper caseMapper;
    private final ObjectMapper objectMapper;
    private final PacketService packetService;

    public CaseService(CaseRepository caseRepository,
                        DocumentRepository documentRepository,
                        UploadRepository uploadRepository,
                        CaseValidator caseValidator,
                        CaseMapper caseMapper,
                        ObjectMapper objectMapper,
                        PacketService packetService) {
        this.caseRepository = caseRepository;
        this.documentRepository = documentRepository;
        this.uploadRepository = uploadRepository;
        this.caseValidator = caseValidator;
        this.caseMapper = caseMapper;
        this.objectMapper = objectMapper;
        this.packetService = packetService;
    }

    /**
     * Idempotent by design: the desktop client retries this exact call on every reconnect for a
     * case it queued offline (insight-javafs's UploadQueue.maybeAutoResubmitCases), so a case id
     * that already exists here is a replay, not a conflict — it returns the existing record
     * unchanged rather than re-validating or erroring.
     */
    @Transactional
    public CaseResponse submitCase(SubmitCaseRequest request) {
        Optional<Case> existing = caseRepository.findById(request.caseId());
        if (existing.isPresent()) {
            return caseMapper.toResponse(existing.get());
        }

        List<ApiError.FieldError> errors = caseValidator.validate(request);
        if (!errors.isEmpty()) {
            throw new CaseValidationException(errors);
        }

        if (request.batchNumber() != null) {
            packetService.requirePacket(request.batchNumber());
        }

        Case caseEntity = new Case(request.caseId(), request.sourcePan(), request.name(),
                request.mobileNumber(), request.designation(), writeExtraFields(request.extraFields()));
        applyOptionalFields(caseEntity, request);
        caseRepository.save(caseEntity);
        return caseMapper.toResponse(caseEntity);
    }

    private void applyOptionalFields(Case caseEntity, SubmitCaseRequest request) {
        caseEntity.setReferenceNumber(request.referenceNumber());
        caseEntity.setDateOfBirth(request.dateOfBirth());
        caseEntity.setAddress(request.address());
        caseEntity.setStateUtCode(request.stateUtCode());
        caseEntity.setPincode(request.pincode());
        caseEntity.setEmail(request.email());
        caseEntity.setInformationFy(request.informationFy());
        caseEntity.setInformationSourceType(request.informationSourceType());
        caseEntity.setInformationSourceDescription(request.informationSourceDescription());
        caseEntity.setInformationType(request.informationType());
        caseEntity.setInformationDescription(request.informationDescription());
        caseEntity.setInformationValue(request.informationValue());
        caseEntity.setNatureOfVerification(request.natureOfVerification());
        caseEntity.setActionableAy(request.actionableAy());
        caseEntity.setVerificationResultType1(request.verificationResultType1());
        caseEntity.setVerificationResultDescription1(request.verificationResultDescription1());
        caseEntity.setVerificationResultValue1(request.verificationResultValue1());
        caseEntity.setVerificationResultType2(request.verificationResultType2());
        caseEntity.setVerificationResultDescription2(request.verificationResultDescription2());
        caseEntity.setVerificationResultValue2(request.verificationResultValue2());
        caseEntity.setVerificationResultType3(request.verificationResultType3());
        caseEntity.setVerificationResultDescription3(request.verificationResultDescription3());
        caseEntity.setVerificationResultValue3(request.verificationResultValue3());
        caseEntity.setRemarks(request.remarks());
        caseEntity.setBatchNumber(request.batchNumber());
    }

    @Transactional(readOnly = true)
    public CaseResponse getCase(String caseId) {
        return caseMapper.toResponse(requireCase(caseId));
    }

    @Transactional(readOnly = true)
    public List<CaseDocumentResponse> listDocuments(String caseId) {
        requireCase(caseId);
        List<Document> documents = documentRepository.findByCaseId(caseId);
        Map<UUID, Upload> uploadsById = uploadRepository
                .findAllById(documents.stream().map(Document::getUploadId).toList())
                .stream()
                .collect(Collectors.toMap(Upload::getId, u -> u));

        return documents.stream()
                .map(doc -> {
                    Upload upload = uploadsById.get(doc.getUploadId());
                    return new CaseDocumentResponse(
                            doc.getUploadId().toString(),
                            doc.getDocLabel(),
                            doc.getDocType(),
                            doc.getDescription(),
                            doc.getRemarks(),
                            upload != null ? upload.getFileName() : null,
                            upload != null ? upload.getStatus().name() : null,
                            doc.getApprovalStatus().name());
                })
                .toList();
    }

    Case requireCase(String caseId) {
        return caseRepository.findById(caseId).orElseThrow(() -> new CaseNotFoundException(caseId));
    }

    private String writeExtraFields(Map<String, String> extraFields) {
        if (extraFields == null || extraFields.isEmpty()) {
            return null;
        }
        try {
            return objectMapper.writeValueAsString(extraFields);
        } catch (Exception e) {
            throw new IllegalStateException("Failed to serialize extra case fields", e);
        }
    }
}
