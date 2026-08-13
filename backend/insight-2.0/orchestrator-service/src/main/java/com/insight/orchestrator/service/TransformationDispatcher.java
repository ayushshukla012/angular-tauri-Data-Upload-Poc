package com.insight.orchestrator.service;

import com.insight.orchestrator.entity.SagaInstance;
import com.insight.orchestrator.entity.SagaState;
import com.insight.protos.transformation.v1.StartTransformationRequest;
import com.insight.protos.transformation.v1.TransformationServiceGrpc;
import net.devh.boot.grpc.client.inject.GrpcClient;
import org.springframework.stereotype.Component;

@Component
public class TransformationDispatcher {

    @GrpcClient("transformation-service")
    private TransformationServiceGrpc.TransformationServiceBlockingStub transformationStub;

    public void dispatch(SagaInstance saga, String fileReference, String fileType) {
        transformationStub.startTransformation(StartTransformationRequest.newBuilder()
                .setSagaId(saga.getSagaId().toString())
                .setUploadId(saga.getUploadId().toString())
                .setFileReference(fileReference)
                .setFileType(fileType)
                .build());
        saga.transitionTo(SagaState.VALIDATING, "TRANSFORMATION_DISPATCHED");
    }
}
