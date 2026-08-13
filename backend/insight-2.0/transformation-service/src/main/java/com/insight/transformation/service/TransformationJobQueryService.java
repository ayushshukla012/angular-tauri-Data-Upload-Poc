package com.insight.transformation.service;

import com.insight.transformation.dto.TransformationJobResponse;
import com.insight.transformation.exception.TransformationJobNotFoundException;
import com.insight.transformation.mapper.TransformationJobMapper;
import com.insight.transformation.repository.TransformationJobRepository;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.util.UUID;

@Service
public class TransformationJobQueryService {

    private final TransformationJobRepository repository;
    private final TransformationJobMapper mapper;

    public TransformationJobQueryService(TransformationJobRepository repository, TransformationJobMapper mapper) {
        this.repository = repository;
        this.mapper = mapper;
    }

    @Transactional(readOnly = true)
    public TransformationJobResponse getByUploadId(String uploadId) {
        return repository.findByUploadId(UUID.fromString(uploadId))
                .map(mapper::toResponse)
                .orElseThrow(() -> new TransformationJobNotFoundException(uploadId));
    }
}
