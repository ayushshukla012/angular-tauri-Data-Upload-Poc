package com.insight.reporting.service;

import com.insight.reporting.dto.UploadReportResponse;
import com.insight.reporting.entity.UploadReport;
import com.insight.reporting.exception.UploadReportNotFoundException;
import com.insight.reporting.mapper.UploadReportMapper;
import com.insight.reporting.repository.ErrorSummaryRepository;
import com.insight.reporting.repository.UploadReportRepository;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.util.UUID;

@Service
public class UploadReportQueryService {

    private final UploadReportRepository uploadReportRepository;
    private final ErrorSummaryRepository errorSummaryRepository;
    private final UploadReportMapper mapper;

    public UploadReportQueryService(UploadReportRepository uploadReportRepository,
                                     ErrorSummaryRepository errorSummaryRepository,
                                     UploadReportMapper mapper) {
        this.uploadReportRepository = uploadReportRepository;
        this.errorSummaryRepository = errorSummaryRepository;
        this.mapper = mapper;
    }

    @Transactional(readOnly = true)
    public UploadReportResponse getByUploadId(String uploadId) {
        UploadReport report = uploadReportRepository.findByUploadId(UUID.fromString(uploadId))
                .orElseThrow(() -> new UploadReportNotFoundException(uploadId));
        return mapper.toResponse(report, errorSummaryRepository.findByUploadReportId(report.getId()));
    }
}
