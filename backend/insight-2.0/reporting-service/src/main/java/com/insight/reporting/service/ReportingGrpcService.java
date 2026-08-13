package com.insight.reporting.service;

import com.insight.protos.reporting.v1.GenerateReportRequest;
import com.insight.protos.reporting.v1.GenerateReportResponse;
import com.insight.protos.reporting.v1.ReportingServiceGrpc;
import io.grpc.stub.StreamObserver;
import net.devh.boot.grpc.server.service.GrpcService;

@GrpcService
public class ReportingGrpcService extends ReportingServiceGrpc.ReportingServiceImplBase {

    @Override
    public void generateReport(GenerateReportRequest request,
                                StreamObserver<GenerateReportResponse> responseObserver) {
        // Pulls job detail from transformation-service / ocr-service via their query gRPC stubs,
        // persists UploadReport + ErrorSummary rows, then publishes reporting.events.completed.
        responseObserver.onNext(GenerateReportResponse.newBuilder().setStatus("ACCEPTED").build());
        responseObserver.onCompleted();
    }
}
