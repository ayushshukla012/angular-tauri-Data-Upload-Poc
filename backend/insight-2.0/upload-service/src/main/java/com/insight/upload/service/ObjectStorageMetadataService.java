package com.insight.upload.service;

import com.fasterxml.jackson.databind.ObjectMapper;
import com.insight.common.storage.ObjectStorageClient;
import com.insight.upload.entity.Case;
import com.insight.upload.entity.Document;
import com.insight.upload.entity.Packet;
import com.insight.upload.entity.Upload;
import org.springframework.stereotype.Service;

import java.io.ByteArrayInputStream;
import java.nio.charset.StandardCharsets;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.UUID;

/**
 * Writes searchable, human-readable JSON metadata next to uploaded files in the S3/MinIO bucket.
 *
 * Storage layout:
 *   data-upload/packets/{batchNumber}/metadata/packet.json
 *   data-upload/packets/{batchNumber}/cases/{caseId}/metadata/case.json
 *   data-upload/packets/{batchNumber}/cases/{caseId}/documents/{uploadId}/metadata.json
 *   data-upload/packets/{batchNumber}/cases/{caseId}/documents/{uploadId}/content/{fileName}
 *
 * Generic uploads that are not linked to a case use:
 *   data-upload/uploads/{uploadId}/metadata.json
 *   data-upload/uploads/{uploadId}/content/{fileName}
 */
@Service
public class ObjectStorageMetadataService {

    private static final String JSON_CONTENT_TYPE = "application/json";

    private final ObjectStorageClient objectStorageClient;
    private final ObjectMapper objectMapper;

    public ObjectStorageMetadataService(ObjectStorageClient objectStorageClient,
                                        ObjectMapper objectMapper) {
        this.objectStorageClient = objectStorageClient;
        this.objectMapper = objectMapper;
    }

    public String packetMetadataKey(String batchNumber) {
        return "data-upload/packets/%s/metadata/packet.json".formatted(segment(batchNumber));
    }

    public String caseMetadataKey(Case caseEntity) {
        if (caseEntity.getBatchNumber() == null || caseEntity.getBatchNumber().isBlank()) {
            return "data-upload/cases/%s/metadata/case.json".formatted(segment(caseEntity.getId()));
        }
        return "data-upload/packets/%s/cases/%s/metadata/case.json".formatted(
                segment(caseEntity.getBatchNumber()),
                segment(caseEntity.getId()));
    }

    public String documentMetadataKey(Case caseEntity, UUID uploadId) {
        if (caseEntity.getBatchNumber() == null || caseEntity.getBatchNumber().isBlank()) {
            return "data-upload/cases/%s/documents/%s/metadata.json".formatted(
                    segment(caseEntity.getId()),
                    segment(uploadId.toString()));
        }
        return "data-upload/packets/%s/cases/%s/documents/%s/metadata.json".formatted(
                segment(caseEntity.getBatchNumber()),
                segment(caseEntity.getId()),
                segment(uploadId.toString()));
    }

    public String documentContentKey(Case caseEntity, UUID uploadId, String fileName) {
        if (caseEntity.getBatchNumber() == null || caseEntity.getBatchNumber().isBlank()) {
            return "data-upload/cases/%s/documents/%s/content/%s".formatted(
                    segment(caseEntity.getId()),
                    segment(uploadId.toString()),
                    segment(fileName));
        }
        return "data-upload/packets/%s/cases/%s/documents/%s/content/%s".formatted(
                segment(caseEntity.getBatchNumber()),
                segment(caseEntity.getId()),
                segment(uploadId.toString()),
                segment(fileName));
    }

    public String genericMetadataKey(UUID uploadId) {
        return "data-upload/uploads/%s/metadata.json".formatted(segment(uploadId.toString()));
    }

    public String genericContentKey(UUID uploadId, String fileName) {
        return "data-upload/uploads/%s/content/%s".formatted(
                segment(uploadId.toString()),
                segment(fileName));
    }

    public void writePacketMetadata(Packet packet) {
        Map<String, Object> metadata = new LinkedHashMap<>();
        metadata.put("schemaVersion", 1);
        metadata.put("resourceType", "packet");
        metadata.put("batchNumber", packet.getBatchNumber());
        metadata.put("description", packet.getDescription());
        metadata.put("submittingPersonName", packet.getSubmittingPersonName());
        metadata.put("submittingPersonAddress", packet.getSubmittingPersonAddress());
        metadata.put("submittingPersonMobile", packet.getSubmittingPersonMobile());
        metadata.put("submittingPersonEmail", packet.getSubmittingPersonEmail());
        metadata.put("approvalStatus", packet.getApprovalStatus().name());
        metadata.put("createdAt", packet.getCreatedAt());
        writeJson(packetMetadataKey(packet.getBatchNumber()), metadata);
    }

    public void writeCaseMetadata(Case caseEntity) {
        Map<String, Object> metadata = new LinkedHashMap<>();
        metadata.put("schemaVersion", 1);
        metadata.put("resourceType", "case");
        metadata.put("caseId", caseEntity.getId());
        metadata.put("batchNumber", caseEntity.getBatchNumber());
        metadata.put("referenceNumber", caseEntity.getReferenceNumber());
        metadata.put("sourcePan", caseEntity.getSourcePan());
        metadata.put("name", caseEntity.getName());
        metadata.put("dateOfBirth", caseEntity.getDateOfBirth());
        metadata.put("address", caseEntity.getAddress());
        metadata.put("stateUtCode", caseEntity.getStateUtCode());
        metadata.put("pincode", caseEntity.getPincode());
        metadata.put("mobileNumber", caseEntity.getMobileNumber());
        metadata.put("email", caseEntity.getEmail());
        metadata.put("designation", caseEntity.getDesignation());
        metadata.put("informationFy", caseEntity.getInformationFy());
        metadata.put("informationSourceType", caseEntity.getInformationSourceType());
        metadata.put("informationSourceDescription", caseEntity.getInformationSourceDescription());
        metadata.put("informationType", caseEntity.getInformationType());
        metadata.put("informationDescription", caseEntity.getInformationDescription());
        metadata.put("informationValue", caseEntity.getInformationValue());
        metadata.put("natureOfVerification", caseEntity.getNatureOfVerification());
        metadata.put("actionableAy", caseEntity.getActionableAy());
        metadata.put("verificationResultType1", caseEntity.getVerificationResultType1());
        metadata.put("verificationResultDescription1", caseEntity.getVerificationResultDescription1());
        metadata.put("verificationResultValue1", caseEntity.getVerificationResultValue1());
        metadata.put("verificationResultType2", caseEntity.getVerificationResultType2());
        metadata.put("verificationResultDescription2", caseEntity.getVerificationResultDescription2());
        metadata.put("verificationResultValue2", caseEntity.getVerificationResultValue2());
        metadata.put("verificationResultType3", caseEntity.getVerificationResultType3());
        metadata.put("verificationResultDescription3", caseEntity.getVerificationResultDescription3());
        metadata.put("verificationResultValue3", caseEntity.getVerificationResultValue3());
        metadata.put("remarks", caseEntity.getRemarks());
        metadata.put("status", caseEntity.getStatus().name());
        metadata.put("approvalStatus", caseEntity.getApprovalStatus().name());
        metadata.put("errorMessage", caseEntity.getErrorMessage());
        metadata.put("extraFields", caseEntity.getExtraFields());
        metadata.put("createdAt", caseEntity.getCreatedAt());
        metadata.put("updatedAt", caseEntity.getUpdatedAt());
        writeJson(caseMetadataKey(caseEntity), metadata);
    }

    public void writeDocumentMetadata(Case caseEntity, Document document, Upload upload) {
        Map<String, Object> metadata = new LinkedHashMap<>();
        metadata.put("schemaVersion", 1);
        metadata.put("resourceType", "document");
        metadata.put("uploadId", upload.getId());
        metadata.put("caseId", caseEntity.getId());
        metadata.put("batchNumber", caseEntity.getBatchNumber());
        metadata.put("fileName", upload.getFileName());
        metadata.put("fileType", upload.getFileType());
        metadata.put("fileSizeBytes", upload.getFileSizeBytes());
        metadata.put("storageReference", documentContentKey(caseEntity, upload.getId(), upload.getFileName()));
        metadata.put("docLabel", document.getDocLabel());
        metadata.put("docType", document.getDocType());
        metadata.put("description", document.getDescription());
        metadata.put("remarks", document.getRemarks());
        metadata.put("uploadStatus", upload.getStatus().name());
        metadata.put("approvalStatus", document.getApprovalStatus().name());
        metadata.put("createdAt", upload.getCreatedAt());
        writeJson(documentMetadataKey(caseEntity, upload.getId()), metadata);
    }

    public void writeGenericUploadMetadata(Upload upload) {
        Map<String, Object> metadata = new LinkedHashMap<>();
        metadata.put("schemaVersion", 1);
        metadata.put("resourceType", "upload");
        metadata.put("uploadId", upload.getId());
        metadata.put("fileName", upload.getFileName());
        metadata.put("fileType", upload.getFileType());
        metadata.put("fileSizeBytes", upload.getFileSizeBytes());
        metadata.put("storageReference", genericContentKey(upload.getId(), upload.getFileName()));
        metadata.put("status", upload.getStatus().name());
        metadata.put("createdAt", upload.getCreatedAt());
        writeJson(genericMetadataKey(upload.getId()), metadata);
    }

    private void writeJson(String key, Map<String, Object> metadata) {
        try {
            byte[] json = objectMapper.writerWithDefaultPrettyPrinter()
                    .writeValueAsString(metadata)
                    .getBytes(StandardCharsets.UTF_8);
            objectStorageClient.put(
                    key,
                    new ByteArrayInputStream(json),
                    json.length,
                    JSON_CONTENT_TYPE);
        } catch (Exception e) {
            throw new IllegalStateException("Unable to write object-storage metadata: " + key, e);
        }
    }

    private String segment(String value) {
        if (value == null || value.isBlank()) {
            throw new IllegalArgumentException("Object-storage path segment must not be blank");
        }

        String normalized = value.trim().replace('\\', '_').replace('/', '_');
        if (normalized.equals(".") || normalized.equals("..")) {
            throw new IllegalArgumentException("Invalid object-storage path segment");
        }
        return normalized;
    }
}
