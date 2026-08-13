package com.insight.ocr.controller;

import com.insight.ocr.dto.OcrJobResponse;
import com.insight.ocr.service.OcrJobQueryService;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

@RestController
@RequestMapping("/api/v1/ocr-jobs")
public class OcrJobController {

    private final OcrJobQueryService queryService;

    public OcrJobController(OcrJobQueryService queryService) {
        this.queryService = queryService;
    }

    @GetMapping("/{uploadId}")
    public OcrJobResponse getByUploadId(@PathVariable String uploadId) {
        return queryService.getByUploadId(uploadId);
    }
}
