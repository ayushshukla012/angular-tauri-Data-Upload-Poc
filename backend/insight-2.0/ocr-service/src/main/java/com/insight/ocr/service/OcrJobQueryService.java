package com.insight.ocr.service;

import com.insight.ocr.dto.OcrJobResponse;
import com.insight.ocr.exception.OcrJobNotFoundException;
import com.insight.ocr.mapper.OcrJobMapper;
import com.insight.ocr.repository.OcrJobRepository;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.util.UUID;

@Service
public class OcrJobQueryService {

    private final OcrJobRepository repository;
    private final OcrJobMapper mapper;

    public OcrJobQueryService(OcrJobRepository repository, OcrJobMapper mapper) {
        this.repository = repository;
        this.mapper = mapper;
    }

    @Transactional(readOnly = true)
    public OcrJobResponse getByUploadId(String uploadId) {
        return repository.findByUploadId(UUID.fromString(uploadId))
                .map(mapper::toResponse)
                .orElseThrow(() -> new OcrJobNotFoundException(uploadId));
    }
}
