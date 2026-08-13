package com.insight.ocr.service;

import com.insight.ocr.entity.OcrJob;
import com.insight.ocr.exception.OcrJobNotFoundException;
import com.insight.ocr.repository.OcrJobRepository;
import com.insight.protos.ocr.v1.GetExtractionResultRequest;
import com.insight.protos.ocr.v1.GetExtractionResultResponse;
import com.insight.protos.ocr.v1.OcrServiceGrpc;
import com.insight.protos.ocr.v1.StartExtractionRequest;
import com.insight.protos.ocr.v1.StartExtractionResponse;
import com.insight.protos.transformation.v1.DispatchStatus;
import io.grpc.stub.StreamObserver;
import net.devh.boot.grpc.server.service.GrpcService;

import java.util.UUID;

@GrpcService
public class OcrGrpcService extends OcrServiceGrpc.OcrServiceImplBase {

    private final OcrJobRepository repository;

    public OcrGrpcService(OcrJobRepository repository) {
        this.repository = repository;
    }

    @Override
    public void startExtraction(StartExtractionRequest request,
                                 StreamObserver<StartExtractionResponse> responseObserver) {
        UUID uploadId = UUID.fromString(request.getUploadId());

        if (repository.findByUploadId(uploadId).isPresent()) {
            responseObserver.onNext(StartExtractionResponse.newBuilder()
                    .setStatus(DispatchStatus.ALREADY_PROCESSED)
                    .build());
            responseObserver.onCompleted();
            return;
        }

        repository.save(new OcrJob(UUID.randomUUID(), uploadId, request.getSagaId(),
                request.getDocumentReferencesCount()));
        // Actual text extraction runs asynchronously (same internal-topic hand-off pattern
        // as transformation-service's worker); this call only accepts the job.

        responseObserver.onNext(StartExtractionResponse.newBuilder()
                .setStatus(DispatchStatus.ACCEPTED)
                .build());
        responseObserver.onCompleted();
    }

    @Override
    public void getExtractionResult(GetExtractionResultRequest request,
                                     StreamObserver<GetExtractionResultResponse> responseObserver) {
        UUID uploadId = UUID.fromString(request.getUploadId());
        OcrJob job = repository.findByUploadId(uploadId)
                .orElseThrow(() -> new OcrJobNotFoundException(request.getUploadId()));

        responseObserver.onNext(GetExtractionResultResponse.newBuilder()
                .setStatus(job.getStatus().name())
                .setDocumentCount(job.getDocumentCount())
                .setExtractedCount(job.getExtractedCount())
                .build());
        responseObserver.onCompleted();
    }
}
