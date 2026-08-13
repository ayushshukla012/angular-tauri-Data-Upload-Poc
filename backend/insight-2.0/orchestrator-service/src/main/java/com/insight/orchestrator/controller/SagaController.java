package com.insight.orchestrator.controller;

import com.insight.orchestrator.dto.SagaInstanceResponse;
import com.insight.orchestrator.service.SagaQueryService;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

@RestController
@RequestMapping("/api/v1/sagas")
public class SagaController {

    private final SagaQueryService queryService;

    public SagaController(SagaQueryService queryService) {
        this.queryService = queryService;
    }

    @GetMapping("/{uploadId}")
    public SagaInstanceResponse getByUploadId(@PathVariable String uploadId) {
        return queryService.getByUploadId(uploadId);
    }
}
