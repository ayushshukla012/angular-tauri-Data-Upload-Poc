package com.insight.transformation.controller;

import com.insight.transformation.dto.TransformationJobResponse;
import com.insight.transformation.service.TransformationJobQueryService;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

@RestController
@RequestMapping("/api/v1/transformation-jobs")
public class TransformationJobController {

    private final TransformationJobQueryService queryService;

    public TransformationJobController(TransformationJobQueryService queryService) {
        this.queryService = queryService;
    }

    @GetMapping("/{uploadId}")
    public TransformationJobResponse getByUploadId(@PathVariable String uploadId) {
        return queryService.getByUploadId(uploadId);
    }
}
