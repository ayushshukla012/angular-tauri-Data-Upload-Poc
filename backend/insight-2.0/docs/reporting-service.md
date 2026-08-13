# Code walkthrough: `reporting-service`

Assumes the other three docs are already read. This is the last stop in the saga, and the one
most explicitly built as scaffolding for future work — most of its shape exists, but the actual
report-assembly logic doesn't yet.

## 1. The role this service plays

Once transformation (and optionally OCR) finish, something needs to combine their results into
one final artifact — the report a user or `reporting-service`'s own API consumers actually care
about: total/valid/error row counts and a human-readable error summary. This is that service.

## 2. Both a gRPC server *and* client, in one class family

Unlike `transformation-service`/`ocr-service` (pure servers, called *by* the orchestrator) or
`orchestrator-service` (pure client, calling *them*), this service is both:

```yaml
grpc:
  server:
    port: 9093        # orchestrator calls GenerateReport on this
  client:
    transformation-service:
      address: static://localhost:9091   # this service calls GetTransformationResult
    ocr-service:
      address: static://localhost:9096   # this service calls GetExtractionResult
```
The idea: `orchestrator-service` calls `ReportingService.GenerateReport` (server side, port
`9093`), and to actually build that report, this service in turn calls back out to
`transformation-service` and `ocr-service` (client side) to pull their job results — the same
`GetTransformationResult`/`GetExtractionResult` gRPC query methods documented in
`docs/transformation-service.md` §9 and `docs/ocr-service.md` §2.

## 3. `ReportingGrpcService.java` — currently a stub

```java
@GrpcService
public class ReportingGrpcService extends ReportingServiceGrpc.ReportingServiceImplBase {

    @Override
    public void generateReport(GenerateReportRequest request, StreamObserver<GenerateReportResponse> responseObserver) {
        // Pulls job detail from transformation-service / ocr-service via their query gRPC stubs,
        // persists UploadReport + ErrorSummary rows, then publishes reporting.events.completed.
        responseObserver.onNext(GenerateReportResponse.newBuilder().setStatus("ACCEPTED").build());
        responseObserver.onCompleted();
    }
}
```
Read that literally: it accepts the call and immediately says `"ACCEPTED"` — it never actually
calls `transformation-service`/`ocr-service`, never writes an `UploadReport` row, and never
publishes `reporting.events.completed`. The comment describes what it's *supposed* to do; none
of it is implemented yet. This mirrors `ocr-service`'s honesty note in
`docs/ocr-service.md` §3 — both are real gaps, not subtle bugs, and both were explicitly out of
scope for the work done so far (this service isn't even reachable yet in practice, since
`orchestrator-service` doesn't dispatch to it — see `docs/orchestrator-service.md` §6).

**What finishing this would actually look like**, using the two gRPC client stubs already
configured:
```java
@GrpcClient("transformation-service")
private TransformationServiceGrpc.TransformationServiceBlockingStub transformationStub;
@GrpcClient("ocr-service")
private OcrServiceGrpc.OcrServiceBlockingStub ocrStub;

// inside generateReport:
var transformationResult = transformationStub.getTransformationResult(
        GetTransformationResultRequest.newBuilder().setUploadId(request.getUploadId()).build());
// ... conditionally call ocrStub.getExtractionResult(...) if the upload needed OCR ...
UploadReport report = new UploadReport(UUID.randomUUID(), UUID.fromString(request.getUploadId()),
        transformationResult.getTotalRows(), transformationResult.getValidRows(), transformationResult.getErrorRows());
uploadReportRepository.save(report);
// + save one ErrorSummary row per distinct error reason, then publish reporting.events.completed
```

## 4. The entities that already exist, waiting for that logic

`UploadReport` — one row per upload, `status` (`GENERATED`/`FAILED`) plus the same
`totalRows`/`validRows`/`errorRows` shape as `TransformationJob`. `ErrorSummary` — one row per
*distinct* error reason with an `occurrences` count (a rollup, not one row per bad record like
`transformation-service`'s `row_validation_errors` — the report is meant to summarize, not
duplicate, the detail).

## 5. The query side already works today

Even though nothing populates an `UploadReport` yet, the read path is fully wired:
`GET /api/v1/reports/{uploadId}` → `UploadReportController` → `UploadReportQueryService`, which
joins the report with its `ErrorSummary` rows via `UploadReportMapper`. Hitting it before any
report exists correctly returns a `404` (`UploadReportNotFoundException` →
`GlobalExceptionHandler`), same pattern as every other service's not-found handling.
