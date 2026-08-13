package com.insight.upload.entity;

import jakarta.persistence.*;
import java.time.Instant;
import java.util.UUID;

@Entity
@Table(name = "uploads")
public class Upload {

    @Id
    private UUID id;

    @Column(nullable = false)
    private String fileName;

    @Column(nullable = false)
    private String fileType;

    @Column(nullable = false)
    private String storageReference;

    @Column(nullable = false)
    private long fileSizeBytes;

    /** Set only when this upload used multipart (fileSizeBytes >= insight.storage.multipart-threshold-bytes). */
    private String multipartUploadId;

    @Enumerated(EnumType.STRING)
    @Column(nullable = false)
    private UploadStatus status;

    @Column(nullable = false)
    private Instant createdAt;

    private Instant updatedAt;

    protected Upload() {
    }

    public Upload(UUID id, String fileName, String fileType, String storageReference, long fileSizeBytes) {
        this.id = id;
        this.fileName = fileName;
        this.fileType = fileType;
        this.storageReference = storageReference;
        this.fileSizeBytes = fileSizeBytes;
        this.status = UploadStatus.PENDING;
        this.createdAt = Instant.now();
    }

    public void markStatus(UploadStatus status) {
        this.status = status;
        this.updatedAt = Instant.now();
    }

    public void assignMultipartUploadId(String multipartUploadId) {
        this.multipartUploadId = multipartUploadId;
    }

    public boolean isMultipart() {
        return multipartUploadId != null;
    }

    public UUID getId() { return id; }
    public String getFileName() { return fileName; }
    public String getFileType() { return fileType; }
    public String getStorageReference() { return storageReference; }
    public long getFileSizeBytes() { return fileSizeBytes; }
    public String getMultipartUploadId() { return multipartUploadId; }
    public UploadStatus getStatus() { return status; }
    public Instant getCreatedAt() { return createdAt; }
}
