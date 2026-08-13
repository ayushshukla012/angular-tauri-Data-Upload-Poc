package com.insight.upload.service;

import com.insight.common.storage.ObjectStorageClient;
import com.insight.common.storage.ObjectStorageProperties;
import com.insight.common.storage.PartInfo;
import com.insight.upload.dto.InitiateUploadRequest;
import com.insight.upload.dto.InitiateUploadResponse;
import com.insight.upload.dto.PresignPartResponse;
import com.insight.upload.dto.UploadPartsResponse;
import com.insight.upload.dto.UploadResponse;
import com.insight.upload.dto.UploadedPartSummary;
import com.insight.upload.entity.Case;
import com.insight.upload.entity.CaseStatus;
import com.insight.upload.entity.Document;
import com.insight.upload.entity.OutboxEvent;
import com.insight.upload.entity.Upload;
import com.insight.upload.entity.UploadStatus;
import com.insight.upload.exception.UnsupportedFileTypeException;
import com.insight.upload.exception.UploadNotCompletedException;
import com.insight.upload.exception.UploadNotFoundException;
import com.insight.upload.exception.UploadNotMultipartException;
import com.insight.upload.mapper.UploadMapper;
import com.insight.upload.repository.DocumentRepository;
import com.insight.upload.repository.OutboxEventRepository;
import com.insight.upload.repository.UploadRepository;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.net.URI;
import java.time.Duration;
import java.time.Instant;
import java.util.List;
import java.util.Optional;
import java.util.UUID;

@Service
public class UploadService {

    private static final Duration PRESIGNED_URL_TTL = Duration.ofMinutes(15);

    /** S3/MinIO's own minimum part size (except the final part of an upload). */
    private static final long MIN_PART_SIZE_BYTES = 5L * 1024 * 1024;

    /** S3/MinIO's hard cap on parts per multipart upload. */
    private static final int MAX_PART_COUNT = 10_000;

    private final UploadRepository uploadRepository;
    private final OutboxEventRepository outboxEventRepository;
    private final DocumentRepository documentRepository;
    private final CaseService caseService;
    private final UploadMapper uploadMapper;
    private final ObjectStorageClient objectStorageClient;
    private final ObjectStorageProperties storageProperties;

    public UploadService(UploadRepository uploadRepository,
                          OutboxEventRepository outboxEventRepository,
                          DocumentRepository documentRepository,
                          CaseService caseService,
                          UploadMapper uploadMapper,
                          ObjectStorageClient objectStorageClient,
                          ObjectStorageProperties storageProperties) {
        this.uploadRepository = uploadRepository;
        this.outboxEventRepository = outboxEventRepository;
        this.documentRepository = documentRepository;
        this.caseService = caseService;
        this.uploadMapper = uploadMapper;
        this.objectStorageClient = objectStorageClient;
        this.storageProperties = storageProperties;
    }

    /**
     * Step 1 of 2. Issues either a single presigned URL (small files) or starts a multipart
     * upload and returns the sizing bounds a client's adaptive chunking algorithm needs
     * (large files) — see docs/resumable-uploads.md. Either way, this service never touches
     * the file bytes themselves.
     *
     * <p>{@code caseId}/{@code docLabel} are optional — set only when this file is a document
     * attached to an ITD case (insight-javafs's Cases tab). When present, the referenced case
     * must already exist (the client always calls {@code POST /api/v1/cases} first) and this
     * upload gets linked to it via a {@link Document} row.
     */
    @Transactional
    public InitiateUploadResponse initiate(InitiateUploadRequest request) {
        String fileName = request.fileName();
        long fileSizeBytes = request.fileSizeBytes();
        String caseId = request.caseId();
        if (caseId != null) {
            caseService.requireCase(caseId);
        }

        UUID uploadId = UUID.randomUUID();
        String fileType = resolveFileType(fileName);
        String bucket = "data-upload";
        String storageKey = "%s/%s".formatted(bucket, fileName);
        String contentType = contentTypeFor(fileType);

        Upload upload = new Upload(uploadId, fileName, fileType, storageKey, fileSizeBytes);
        Instant expiresAt = Instant.now().plus(PRESIGNED_URL_TTL);

        InitiateUploadResponse response;
        if (fileSizeBytes >= storageProperties.getMultipartThresholdBytes()) {
            String multipartUploadId = objectStorageClient.createMultipartUpload(storageKey, contentType);
            upload.assignMultipartUploadId(multipartUploadId);
            uploadRepository.save(upload);

            long minPartSizeBytes = Math.max(MIN_PART_SIZE_BYTES, ceilDiv(fileSizeBytes, MAX_PART_COUNT));
            long recommendedPartSizeBytes = Math.max(minPartSizeBytes, storageProperties.getDefaultPartSizeBytes());

            response = new InitiateUploadResponse(uploadId.toString(), true, null, null,
                    minPartSizeBytes, recommendedPartSizeBytes, expiresAt);
        } else {
            URI uploadUrl = objectStorageClient.presignPut(storageKey, contentType, PRESIGNED_URL_TTL);
            uploadRepository.save(upload);
            // contentType is echoed back verbatim — it's exactly what was bound into the presigned
            // URL's signature above, so the client's PUT must send this and not derive its own.
            response = new InitiateUploadResponse(uploadId.toString(), false, uploadUrl.toString(), contentType, null, null, expiresAt);
        }

        if (caseId != null) {
            documentRepository.save(new Document(UUID.randomUUID(), caseId, uploadId, request.docLabel(),
                    request.docType(), request.description(), request.remarks()));
        }
        return response;
    }

    /** Returns a presigned URL for exactly one part of a multipart upload, generated on demand. */
    @Transactional(readOnly = true)
    public PresignPartResponse presignPart(String uploadId, int partNumber) {
        Upload upload = requireMultipartUpload(uploadId);
        URI uploadUrl = objectStorageClient.presignUploadPart(
                upload.getStorageReference(), upload.getMultipartUploadId(), partNumber, PRESIGNED_URL_TTL);
        return new PresignPartResponse(partNumber, uploadUrl.toString(), Instant.now().plus(PRESIGNED_URL_TTL));
    }

    /** Resume entry point — reports which parts the object store already has, so a client only re-uploads the rest. */
    @Transactional(readOnly = true)
    public UploadPartsResponse listUploadedParts(String uploadId) {
        Upload upload = requireMultipartUpload(uploadId);
        List<UploadedPartSummary> parts = objectStorageClient
                .listParts(upload.getStorageReference(), upload.getMultipartUploadId())
                .stream()
                .map(part -> new UploadedPartSummary(part.partNumber(), part.eTag(), part.sizeBytes()))
                .toList();
        return new UploadPartsResponse(parts);
    }

    /**
     * Step 2 of 2. The client calls this once its upload succeeds (a single PUT, or every part
     * of a multipart upload). Only now do we touch the database for anything beyond the initial
     * row, and only now does the saga actually start (via the outbox event).
     */
    @Transactional
    public UploadResponse complete(String uploadId) {
        Upload upload = uploadRepository.findById(UUID.fromString(uploadId))
                .orElseThrow(() -> new UploadNotFoundException(uploadId));

        if (upload.getStatus() != UploadStatus.PENDING) {
            return uploadMapper.toResponse(upload); // already completed — idempotent no-op
        }

        if (upload.isMultipart()) {
            completeMultipart(uploadId, upload);
        } else if (!objectStorageClient.exists(upload.getStorageReference())) {
            throw new UploadNotCompletedException(uploadId);
        }

        upload.markStatus(UploadStatus.RECEIVED);

        String payload = """
                {"uploadId":"%s","fileReference":"%s","fileType":"%s"}
                """.formatted(uploadId, upload.getStorageReference(), upload.getFileType());
        outboxEventRepository.save(new OutboxEvent(UUID.randomUUID(), "upload.events.received", uploadId, payload));

        progressCaseIfLinked(uploadId);

        return uploadMapper.toResponse(upload);
    }

    /**
     * If this upload is a case document, advances the case's status: RECEIVED → PROCESSING once
     * its first document lands, PROCESSING → COMPLETED once every document currently attached to
     * the case has reached {@code UploadStatus.RECEIVED} (see {@link CaseStatus#COMPLETED} for
     * exactly what that claim does and doesn't mean).
     */
    private void progressCaseIfLinked(String uploadId) {
        Optional<Document> document = documentRepository.findByUploadId(UUID.fromString(uploadId));
        if (document.isEmpty()) {
            return;
        }

        Case caseEntity = caseService.requireCase(document.get().getCaseId());
        if (caseEntity.getStatus() == CaseStatus.RECEIVED) {
            caseEntity.markStatus(CaseStatus.PROCESSING);
        }

        List<UUID> caseUploadIds = documentRepository.findByCaseId(caseEntity.getId()).stream()
                .map(Document::getUploadId)
                .toList();
        boolean allDocumentsReceived = uploadRepository.findAllById(caseUploadIds).stream()
                .allMatch(u -> u.getStatus() == UploadStatus.RECEIVED);
        if (allDocumentsReceived) {
            caseEntity.markStatus(CaseStatus.COMPLETED);
        }
        // No explicit save() — caseEntity is managed within this transaction (fetched via
        // caseService.requireCase()), so dirty checking persists the status change on commit,
        // same as how `upload.markStatus(...)` above relies on the same mechanism.
    }

    private void completeMultipart(String uploadId, Upload upload) {
        List<PartInfo> parts = objectStorageClient.listParts(upload.getStorageReference(), upload.getMultipartUploadId());
        long uploadedBytes = parts.stream().mapToLong(PartInfo::sizeBytes).sum();

        // Verified against the object store's own record of what's there — never the client's word alone.
        if (parts.isEmpty() || uploadedBytes != upload.getFileSizeBytes()) {
            throw new UploadNotCompletedException(uploadId,
                    "uploaded %d of %d declared bytes across %d part(s)"
                            .formatted(uploadedBytes, upload.getFileSizeBytes(), parts.size()));
        }

        objectStorageClient.completeMultipartUpload(upload.getStorageReference(), upload.getMultipartUploadId(), parts);
    }

    @Transactional(readOnly = true)
    public UploadResponse getStatus(String uploadId) {
        Upload upload = uploadRepository.findById(UUID.fromString(uploadId))
                .orElseThrow(() -> new UploadNotFoundException(uploadId));
        return uploadMapper.toResponse(upload);
    }

    private Upload requireMultipartUpload(String uploadId) {
        Upload upload = uploadRepository.findById(UUID.fromString(uploadId))
                .orElseThrow(() -> new UploadNotFoundException(uploadId));
        if (!upload.isMultipart()) {
            throw new UploadNotMultipartException(uploadId);
        }
        return upload;
    }

    private static long ceilDiv(long total, int divisor) {
        return (total + divisor - 1) / divisor;
    }

    /**
     * CSV/EXCEL/JSON are the original data-file types (transformation-service's input); everything
     * else was added for case documents (scanned forms, ID proofs, reports) attached via
     * insight-javafs's Cases tab — those never went through this service before that feature
     * existed.
     */
    /**
     * The full legacy "Verification Report Upload Utility" allow-list (its Instructions screen,
     * §11): Image / Document / Audio-Video / Others. CSV/EXCEL/JSON predate that list (the
     * original data-file types, for transformation-service) and stay first for priority.
     */
    private String resolveFileType(String fileName) {
        String name = fileName == null ? "" : fileName.toLowerCase();
        if (name.endsWith(".csv")) return "CSV";
        if (name.endsWith(".xlsx") || name.endsWith(".xls")) return "EXCEL";
        if (name.endsWith(".json")) return "JSON";
        // Image
        if (name.endsWith(".gif")) return "GIF";
        if (name.endsWith(".png")) return "PNG";
        if (name.endsWith(".jpg") || name.endsWith(".jpeg")) return "JPEG";
        if (name.endsWith(".tif") || name.endsWith(".tiff")) return "TIFF";
        if (name.endsWith(".bmp")) return "BMP";
        // Document
        if (name.endsWith(".pdf")) return "PDF";
        if (name.endsWith(".doc")) return "DOC";
        if (name.endsWith(".docx")) return "DOCX";
        if (name.endsWith(".ppt")) return "PPT";
        if (name.endsWith(".pptx")) return "PPTX";
        if (name.endsWith(".txt")) return "TEXT";
        if (name.endsWith(".odt")) return "ODT";
        // Audio/Video
        if (name.endsWith(".mp3")) return "MP3";
        if (name.endsWith(".m4a")) return "M4A";
        if (name.endsWith(".wav")) return "WAV";
        if (name.endsWith(".mpeg")) return "MPEG";
        if (name.endsWith(".mpg")) return "MPG";
        if (name.endsWith(".wmv")) return "WMV";
        if (name.endsWith(".mp4")) return "MP4";
        if (name.endsWith(".m4v")) return "M4V";
        if (name.endsWith(".mov")) return "MOV";
        if (name.endsWith(".avi")) return "AVI";
        if (name.endsWith(".swf")) return "SWF";
        // Others
        if (name.endsWith(".xml")) return "XML";
        if (name.endsWith(".xslt")) return "XSLT";
        if (name.endsWith(".tar.gz")) return "TARGZ";
        if (name.endsWith(".tgz")) return "TGZ";
        if (name.endsWith(".gz")) return "GZ";
        if (name.endsWith(".zip")) return "ZIP";
        if (name.endsWith(".epub")) return "EPUB";
        if (name.endsWith(".rar")) return "RAR";
        if (name.endsWith(".7z")) return "SEVENZ";
        throw new UnsupportedFileTypeException(fileName);
    }

    private String contentTypeFor(String fileType) {
        return switch (fileType) {
            case "CSV" -> "text/csv";
            case "EXCEL" -> "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";
            case "JSON" -> "application/json";
            case "GIF" -> "image/gif";
            case "PNG" -> "image/png";
            case "JPEG" -> "image/jpeg";
            case "TIFF" -> "image/tiff";
            case "BMP" -> "image/bmp";
            case "PDF" -> "application/pdf";
            case "DOC" -> "application/msword";
            case "DOCX" -> "application/vnd.openxmlformats-officedocument.wordprocessingml.document";
            case "PPT" -> "application/vnd.ms-powerpoint";
            case "PPTX" -> "application/vnd.openxmlformats-officedocument.presentationml.presentation";
            case "TEXT" -> "text/plain";
            case "ODT" -> "application/vnd.oasis.opendocument.text";
            case "MP3" -> "audio/mpeg";
            case "M4A" -> "audio/mp4";
            case "WAV" -> "audio/wav";
            case "MPEG" -> "video/mpeg";
            case "MPG" -> "video/mpeg";
            case "WMV" -> "video/x-ms-wmv";
            case "MP4" -> "video/mp4";
            case "M4V" -> "video/x-m4v";
            case "MOV" -> "video/quicktime";
            case "AVI" -> "video/x-msvideo";
            case "SWF" -> "application/x-shockwave-flash";
            case "XML" -> "application/xml";
            case "XSLT" -> "application/xslt+xml";
            case "TARGZ", "TGZ" -> "application/gzip";
            case "GZ" -> "application/gzip";
            case "ZIP" -> "application/zip";
            case "EPUB" -> "application/epub+zip";
            case "RAR" -> "application/vnd.rar";
            case "SEVENZ" -> "application/x-7z-compressed";
            default -> "application/octet-stream";
        };
    }
}
