package com.insight.upload.service;

import com.insight.protos.upload.v1.GetUploadStatusRequest;
import com.insight.protos.upload.v1.GetUploadStatusResponse;
import com.insight.protos.upload.v1.UploadQueryServiceGrpc;
import com.insight.upload.entity.Upload;
import com.insight.upload.exception.UploadNotFoundException;
import com.insight.upload.repository.UploadRepository;
import io.grpc.stub.StreamObserver;
import net.devh.boot.grpc.server.service.GrpcService;

import java.util.UUID;

@GrpcService
public class UploadGrpcQueryService extends UploadQueryServiceGrpc.UploadQueryServiceImplBase {

    private final UploadRepository repository;

    public UploadGrpcQueryService(UploadRepository repository) {
        this.repository = repository;
    }

    @Override
    public void getUploadStatus(GetUploadStatusRequest request,
                                 StreamObserver<GetUploadStatusResponse> responseObserver) {
        Upload upload = repository.findById(UUID.fromString(request.getUploadId()))
                .orElseThrow(() -> new UploadNotFoundException(request.getUploadId()));

        responseObserver.onNext(GetUploadStatusResponse.newBuilder()
                .setUploadId(upload.getId().toString())
                .setStatus(upload.getStatus().name())
                .build());
        responseObserver.onCompleted();
    }
}
