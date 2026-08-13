package com.insight.orchestrator.service;

import com.insight.orchestrator.dto.SagaInstanceResponse;
import com.insight.orchestrator.exception.SagaNotFoundException;
import com.insight.orchestrator.mapper.SagaInstanceMapper;
import com.insight.orchestrator.repository.SagaInstanceRepository;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.util.UUID;

@Service
public class SagaQueryService {

    private final SagaInstanceRepository repository;
    private final SagaInstanceMapper mapper;

    public SagaQueryService(SagaInstanceRepository repository, SagaInstanceMapper mapper) {
        this.repository = repository;
        this.mapper = mapper;
    }

    @Transactional(readOnly = true)
    public SagaInstanceResponse getByUploadId(String uploadId) {
        return repository.findByUploadId(UUID.fromString(uploadId))
                .map(mapper::toResponse)
                .orElseThrow(() -> new SagaNotFoundException(uploadId));
    }
}
