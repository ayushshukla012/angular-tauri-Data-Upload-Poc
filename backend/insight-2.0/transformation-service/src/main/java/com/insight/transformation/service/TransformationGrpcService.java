package com.insight.transformation.service;

import com.insight.protos.transformation.v1.DispatchStatus;
import com.insight.protos.transformation.v1.GetTransformationResultRequest;
import com.insight.protos.transformation.v1.GetTransformationResultResponse;
import com.insight.protos.transformation.v1.StartTransformationRequest;
import com.insight.protos.transformation.v1.StartTransformationResponse;
import com.insight.protos.transformation.v1.TransformationServiceGrpc;
import com.insight.transformation.entity.TransformationJob;
import com.insight.transformation.exception.TransformationJobNotFoundException;
import com.insight.transformation.repository.TransformationJobRepository;
import io.grpc.stub.StreamObserver;
import net.devh.boot.grpc.server.service.GrpcService;
import org.springframework.kafka.core.KafkaTemplate;

import java.util.UUID;

@GrpcService
public class TransformationGrpcService extends TransformationServiceGrpc.TransformationServiceImplBase {

    private final TransformationJobRepository repository;
    private final KafkaTemplate<String, String> kafkaTemplate;

    public TransformationGrpcService(TransformationJobRepository repository,
                                      KafkaTemplate<String, String> kafkaTemplate) {
        this.repository = repository;
        this.kafkaTemplate = kafkaTemplate;
    }

    @Override
    public void startTransformation(StartTransformationRequest request,
                                     StreamObserver<StartTransformationResponse> responseObserver) {
        UUID uploadId = UUID.fromString(request.getUploadId());

        if (repository.findByUploadId(uploadId).isPresent()) {
            responseObserver.onNext(StartTransformationResponse.newBuilder()
                    .setStatus(DispatchStatus.ALREADY_PROCESSED)
                    .build());
            responseObserver.onCompleted();
            return;
        }

        repository.save(new TransformationJob(UUID.randomUUID(), uploadId, request.getSagaId(),
                request.getFileReference(), request.getFileType()));

        // The gRPC call only accepts and persists the job — it must return fast. The actual
        // chunked parse/validate happens off this thread, driven by the internal topic below,
        // so it scales with however many transformation-service pods are running.
        kafkaTemplate.send(KafkaTopics.INTERNAL_START, uploadId.toString(), uploadId.toString());

        responseObserver.onNext(StartTransformationResponse.newBuilder()
                .setStatus(DispatchStatus.ACCEPTED)
                .build());
        responseObserver.onCompleted();
    }

    @Override
    public void getTransformationResult(GetTransformationResultRequest request,
                                         StreamObserver<GetTransformationResultResponse> responseObserver) {
        UUID uploadId = UUID.fromString(request.getUploadId());
        TransformationJob job = repository.findByUploadId(uploadId)
                .orElseThrow(() -> new TransformationJobNotFoundException(request.getUploadId()));

        responseObserver.onNext(GetTransformationResultResponse.newBuilder()
                .setStatus(job.getStatus().name())
                .setTotalRows(job.getTotalRows())
                .setValidRows(job.getValidRows())
                .setErrorRows(job.getErrorRows())
                .setRequiresOcr(job.isRequiresOcr())
                .build());
        responseObserver.onCompleted();
    }
}
