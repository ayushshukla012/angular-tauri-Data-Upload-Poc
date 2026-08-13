package com.insight.transformation.service;

import com.insight.common.storage.ObjectStorageClient;
import com.insight.transformation.entity.RowValidationError;
import com.insight.transformation.entity.TransformationJob;
import com.insight.transformation.entity.TransformationStatus;
import com.insight.transformation.repository.RowValidationErrorRepository;
import com.insight.transformation.repository.TransformationJobRepository;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.kafka.annotation.KafkaListener;
import org.springframework.kafka.core.KafkaTemplate;
import org.springframework.stereotype.Service;

import java.io.InputStream;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicLong;

/**
 * Does the actual chunked parse/validate for a transformation job. Decoupled from the gRPC
 * entry point via {@link KafkaTopics#INTERNAL_START} so it scales independently — every
 * transformation-service pod runs one of these consumers, partitioned by uploadId.
 */
@Service
public class TransformationWorker {

    private static final Logger log = LoggerFactory.getLogger(TransformationWorker.class);
    private static final int BATCH_SIZE = 500;

    private final TransformationJobRepository jobRepository;
    private final RowValidationErrorRepository errorRepository;
    private final ObjectStorageClient objectStorageClient;
    private final FileProcessorRegistry processorRegistry;
    private final RowValidator rowValidator;
    private final KafkaTemplate<String, String> kafkaTemplate;

    // Row-level persistence is I/O-bound and independent per batch, so it's handed off to
    // virtual threads — the file-reading thread keeps streaming while batches flush in the
    // background, without tuning a fixed platform-thread pool size.
    private final ExecutorService persistenceExecutor = Executors.newVirtualThreadPerTaskExecutor();

    public TransformationWorker(TransformationJobRepository jobRepository,
                                 RowValidationErrorRepository errorRepository,
                                 ObjectStorageClient objectStorageClient,
                                 FileProcessorRegistry processorRegistry,
                                 RowValidator rowValidator,
                                 KafkaTemplate<String, String> kafkaTemplate) {
        this.jobRepository = jobRepository;
        this.errorRepository = errorRepository;
        this.objectStorageClient = objectStorageClient;
        this.processorRegistry = processorRegistry;
        this.rowValidator = rowValidator;
        this.kafkaTemplate = kafkaTemplate;
    }

    @KafkaListener(topics = KafkaTopics.INTERNAL_START, groupId = "transformation-service-worker")
    public void onStart(String uploadId) {
        UUID id = UUID.fromString(uploadId);
        TransformationJob job = jobRepository.findByUploadId(id).orElseThrow();

        if (job.getStatus() != TransformationStatus.IN_PROGRESS) {
            return; // redelivered message for a job already finalized — idempotent no-op
        }

        AtomicLong totalRows = new AtomicLong();
        AtomicLong validRows = new AtomicLong();
        AtomicLong errorRows = new AtomicLong();
        AtomicBoolean requiresOcr = new AtomicBoolean(false);
        List<RowValidationError> pendingErrors = new ArrayList<>(BATCH_SIZE);
        List<Future<?>> flushes = new ArrayList<>();

        try {
            FileProcessorStrategy processor = processorRegistry.resolve(job.getFileType());

            try (InputStream inputStream = objectStorageClient.get(job.getFileReference())) {
                processor.process(inputStream, (rowNumber, fields) -> {
                    totalRows.incrementAndGet();
                    if (referencesDocument(fields)) {
                        requiresOcr.set(true);
                    }

                    Optional<String> violation = rowValidator.validate(fields);
                    if (violation.isEmpty()) {
                        validRows.incrementAndGet();
                        return;
                    }

                    errorRows.incrementAndGet();
                    pendingErrors.add(new RowValidationError(UUID.randomUUID(), job.getId(), rowNumber, violation.get()));
                    if (pendingErrors.size() >= BATCH_SIZE) {
                        flushes.add(flushAsync(new ArrayList<>(pendingErrors)));
                        pendingErrors.clear();
                    }
                });
            }

            if (!pendingErrors.isEmpty()) {
                flushes.add(flushAsync(pendingErrors));
            }
            awaitAll(flushes);

            job.markCompleted(totalRows.get(), validRows.get(), errorRows.get(), requiresOcr.get());
            jobRepository.save(job);
            kafkaTemplate.send(KafkaTopics.COMPLETED, uploadId, uploadId);
        } catch (Exception e) {
            log.error("Transformation failed for upload {}", uploadId, e);
            job.markFailed();
            jobRepository.save(job);
            kafkaTemplate.send(KafkaTopics.FAILED, uploadId, uploadId);
        }
    }

    private Future<?> flushAsync(List<RowValidationError> batch) {
        return persistenceExecutor.submit(() -> errorRepository.saveAll(batch));
    }

    private void awaitAll(List<Future<?>> flushes) throws Exception {
        for (Future<?> flush : flushes) {
            flush.get();
        }
    }

    private boolean referencesDocument(Map<String, String> fields) {
        return fields.keySet().stream().anyMatch(key -> key.toLowerCase().contains("document"));
    }
}
