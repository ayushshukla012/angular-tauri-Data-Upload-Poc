package com.insight.reporting.controller;

import com.insight.reporting.dto.UploadReportResponse;
import com.insight.reporting.service.UploadReportQueryService;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

@RestController
@RequestMapping("/api/v1/reports")
public class UploadReportController {

    private final UploadReportQueryService queryService;

    public UploadReportController(UploadReportQueryService queryService) {
        this.queryService = queryService;
    }

    @GetMapping("/{uploadId}")
    public UploadReportResponse getByUploadId(@PathVariable String uploadId) {
        return queryService.getByUploadId(uploadId);
    }
}
