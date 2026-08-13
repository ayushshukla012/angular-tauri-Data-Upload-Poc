package com.insight.orchestrator.service;

import com.insight.orchestrator.entity.SagaInstance;
import com.insight.orchestrator.exception.SagaNotFoundException;
import com.insight.orchestrator.repository.SagaInstanceRepository;
import com.insight.protos.orchestrator.v1.GetSagaStatusRequest;
import com.insight.protos.orchestrator.v1.GetSagaStatusResponse;
import com.insight.protos.orchestrator.v1.OrchestratorQueryServiceGrpc;
import io.grpc.stub.StreamObserver;
import net.devh.boot.grpc.server.service.GrpcService;

import java.util.UUID;

@GrpcService
public class OrchestratorGrpcQueryService extends OrchestratorQueryServiceGrpc.OrchestratorQueryServiceImplBase {

    private final SagaInstanceRepository repository;

    public OrchestratorGrpcQueryService(SagaInstanceRepository repository) {
        this.repository = repository;
    }

    @Override
    public void getSagaStatus(GetSagaStatusRequest request,
                               StreamObserver<GetSagaStatusResponse> responseObserver) {
        UUID uploadId = UUID.fromString(request.getUploadId());
        SagaInstance saga = repository.findByUploadId(uploadId)
                .orElseThrow(() -> new SagaNotFoundException(request.getUploadId()));

        responseObserver.onNext(GetSagaStatusResponse.newBuilder()
                .setSagaId(saga.getSagaId().toString())
                .setState(saga.getState().name())
                .setCurrentStep(saga.getCurrentStep())
                .build());
        responseObserver.onCompleted();
    }
}
