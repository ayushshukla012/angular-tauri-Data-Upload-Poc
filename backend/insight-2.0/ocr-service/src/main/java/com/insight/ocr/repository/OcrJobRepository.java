package com.insight.ocr.repository;

import com.insight.ocr.entity.OcrJob;
import org.springframework.data.jpa.repository.JpaRepository;

import java.util.Optional;
import java.util.UUID;

public interface OcrJobRepository extends JpaRepository<OcrJob, UUID> {

    Optional<OcrJob> findByUploadId(UUID uploadId);
}
