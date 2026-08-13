# Code walkthrough: `ocr-service`

Assumes `docs/upload-service.md` and `docs/transformation-service.md` are already read — this
one is short, because `ocr-service` deliberately mirrors `transformation-service`'s shape almost
exactly, and is honestly **incomplete** in one specific way called out below.

## 1. The role this service plays

In the saga, OCR is a *conditional* step — only for uploads where `transformation-service`
detected something needing text extraction (see `TransformationWorker.referencesDocument`, which
sets `requiresOcr` if any column name contains `"document"`). Most CSV/JSON/Excel uploads never
reach this service at all.

## 2. Structurally identical to `transformation-service`, one command "ahead"

- `OcrGrpcService` — the gRPC server, receiving `StartExtraction` from `orchestrator-service`,
  same idempotency check (`if (repository.findByUploadId(uploadId).isPresent())` → `ALREADY_PROCESSED`),
  same "accept and return immediately" shape as `TransformationGrpcService.startTransformation`.
- `OcrJob` / `OcrStatus` — the entity/enum pair, same role as `TransformationJob`/`TransformationStatus`.
- `OcrJobQueryService` + `OcrJobController` — the REST read side, same pattern as
  `TransformationJobQueryService`/`TransformationJobController`.
- `getExtractionResult` — the gRPC query method, same role as `getTransformationResult`.

```java
@Override
public void startExtraction(StartExtractionRequest request, StreamObserver<StartExtractionResponse> responseObserver) {
    ...
    repository.save(new OcrJob(UUID.randomUUID(), uploadId, request.getSagaId(),
            request.getDocumentReferencesCount()));
    // Actual text extraction runs asynchronously (same internal-topic hand-off pattern
    // as transformation-service's worker); this call only accepts the job.

    responseObserver.onNext(StartExtractionResponse.newBuilder().setStatus(DispatchStatus.ACCEPTED).build());
    responseObserver.onCompleted();
}
```

## 3. What's honestly missing: there is no worker

Read that comment in the code above literally: `startExtraction` persists an `OcrJob` row with
`status = IN_PROGRESS` and returns `ACCEPTED` — but **nothing ever flips that status to
`COMPLETED`**. Unlike `transformation-service`, there's no `KafkaTopics.INTERNAL_START`-style
internal topic, no `@KafkaListener` worker consuming it, and no actual call out to an OCR engine
(Tesseract, a cloud OCR API, etc.). If a saga ever reaches this step today, the `OcrJob` sits in
`IN_PROGRESS` forever and no `ocr.events.completed`/`.failed` event is ever published.

This was a deliberate scope decision, not an oversight discovered late: `transformation-service`
was built as the **reference implementation** of the "gRPC accepts, internal Kafka topic hands
off to a worker" pattern (see `docs/transformation-service.md` §3–5). Building a real OCR worker
means picking and integrating an actual OCR engine, which is a separate piece of work — the
scaffolding here (entity, gRPC accept-path, query methods) exists so that work has somewhere to
plug in, following the exact same shape `transformation-service` already demonstrates.

**If/when this gets built for real**, it would follow `TransformationWorker` line for line: an
internal topic (`ocr.internal.start`), a `@KafkaListener` consuming it, virtual threads for
overlapping I/O, and a call to `job.markCompleted(...)`/`markFailed()` before publishing to
`ocr.events.completed`/`.failed`.

## 4. `application.yml` — nothing new here
Same shape as every other service: its own Postgres database (`ocr`), its own gRPC port
(`9096` — see `docs/TROUBLESHOOTING.md` for why not `9092`), Kafka bootstrap servers, Flyway
enabled. No `insight.storage.*` block, because this service never touches file storage directly
— it would only need that once a real OCR worker exists to read documents out of MinIO.
