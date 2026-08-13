package com.insight.upload.repository;

import com.insight.upload.entity.Document;
import org.springframework.data.jpa.repository.JpaRepository;

import java.util.List;
import java.util.Optional;
import java.util.UUID;

public interface DocumentRepository extends JpaRepository<Document, UUID> {

    List<Document> findByCaseId(String caseId);

    Optional<Document> findByUploadId(UUID uploadId);
}
