package com.insight.reporting.mapper;

import com.insight.reporting.dto.UploadReportResponse;
import com.insight.reporting.entity.ErrorSummary;
import com.insight.reporting.entity.UploadReport;
import org.springframework.stereotype.Component;

import java.util.List;

@Component
public class UploadReportMapper {

    public UploadReportResponse toResponse(UploadReport report, List<ErrorSummary> errorSummaries) {
        return new UploadReportResponse(
                report.getUploadId().toString(),
                report.getStatus().name(),
                report.getTotalRows(),
                report.getValidRows(),
                report.getErrorRows(),
                report.getGeneratedAt(),
                errorSummaries.stream().map(ErrorSummary::getReason).toList()
        );
    }
}
