package com.insight.reporting.repository;

import com.insight.reporting.entity.UploadReport;
import org.springframework.data.jpa.repository.JpaRepository;

import java.util.Optional;
import java.util.UUID;

public interface UploadReportRepository extends JpaRepository<UploadReport, UUID> {

    Optional<UploadReport> findByUploadId(UUID uploadId);
}
