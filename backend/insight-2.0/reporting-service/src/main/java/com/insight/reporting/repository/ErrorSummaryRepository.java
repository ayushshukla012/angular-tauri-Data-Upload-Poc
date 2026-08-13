package com.insight.reporting.repository;

import com.insight.reporting.entity.ErrorSummary;
import org.springframework.data.jpa.repository.JpaRepository;

import java.util.List;
import java.util.UUID;

public interface ErrorSummaryRepository extends JpaRepository<ErrorSummary, UUID> {

    List<ErrorSummary> findByUploadReportId(UUID uploadReportId);
}
