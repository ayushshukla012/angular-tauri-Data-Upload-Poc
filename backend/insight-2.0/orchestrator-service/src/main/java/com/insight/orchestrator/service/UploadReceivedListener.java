package com.insight.orchestrator.service;

import com.fasterxml.jackson.databind.ObjectMapper;
import com.insight.orchestrator.dto.UploadReceivedEvent;
import com.insight.orchestrator.entity.SagaInstance;
import com.insight.orchestrator.repository.SagaInstanceRepository;
import org.springframework.kafka.annotation.KafkaListener;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.util.UUID;

@Service
public class UploadReceivedListener {

    private final SagaInstanceRepository sagaInstanceRepository;
    private final TransformationDispatcher transformationDispatcher;
    private final ObjectMapper objectMapper;

    public UploadReceivedListener(SagaInstanceRepository sagaInstanceRepository,
                                   TransformationDispatcher transformationDispatcher,
                                   ObjectMapper objectMapper) {
        this.sagaInstanceRepository = sagaInstanceRepository;
        this.transformationDispatcher = transformationDispatcher;
        this.objectMapper = objectMapper;
    }

    @KafkaListener(topics = "upload.events.received", groupId = "orchestrator-service")
    @Transactional
    public void onUploadReceived(String payload) throws Exception {
        UploadReceivedEvent event = objectMapper.readValue(payload, UploadReceivedEvent.class);
        UUID uploadId = UUID.fromString(event.uploadId());

        if (sagaInstanceRepository.findByUploadId(uploadId).isPresent()) {
            return; // idempotent — saga already created for this upload
        }

        SagaInstance saga = new SagaInstance(UUID.randomUUID(), uploadId);
        sagaInstanceRepository.save(saga);
        transformationDispatcher.dispatch(saga, event.fileReference(), event.fileType());
    }
}
