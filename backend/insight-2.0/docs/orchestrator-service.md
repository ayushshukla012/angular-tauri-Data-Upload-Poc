# Code walkthrough: `orchestrator-service`

Assumes `docs/upload-service.md` and `docs/transformation-service.md` are already read. This is
the saga coordinator — the one service whose entire job is *knowing where every upload currently
is in the pipeline* and telling the next participant what to do.

## 1. The role this service plays

Every other service answers "what's my job status." This service answers "what's the *whole
upload's* status" — it's the single source of truth for the cross-service state machine
described in `docs/RUNNING_LOCALLY.md`/the architecture spec. It's also the only service that's
**both** a gRPC client (it calls `transformation-service`, `ocr-service`, `reporting-service`)
**and** a gRPC server (`GetSagaStatus`, so others can ask it things) **and** a Kafka consumer
(reacting to `upload.events.received`).

## 2. The state machine — `SagaInstance` / `SagaState`

```java
public enum SagaState {
    STARTED, VALIDATING, PROCESSING, EXTRACTED, REPORTING, COMPLETED, FAILED, COMPENSATING, COMPENSATED
}
```
This is a bigger enum than `UploadStatus` (`RECEIVED`/`VALIDATING`/`PROCESSING`/`COMPLETED`/`FAILED`)
on purpose — it's the *internal* state machine driving the saga, with more granularity than what
gets exposed to an end user checking upload status. `SagaInstance` (the entity) additionally
tracks `currentStep` (a free-text label like `"TRANSFORMATION_DISPATCHED"`, useful for debugging
— "which specific action are we waiting on") and `retryCount`.

```java
public SagaInstance(UUID sagaId, UUID uploadId) {
    this.state = SagaState.STARTED;
    this.currentStep = "TRANSFORMATION_DISPATCHED";
    ...
}

public void transitionTo(SagaState state, String step) {
    this.state = state;
    this.currentStep = step;
    this.updatedAt = Instant.now();
}
```
`transitionTo` is the *only* way this entity's state ever changes — there's no public setter for
`state` directly. Keeping every transition behind one named method is what makes it possible to
answer "what are all the valid state changes for a saga" just by reading this one class.

## 3. Reacting to the upload — `UploadReceivedListener.java`

```java
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
```
The Kafka message value here is a JSON string (written by `upload-service`'s `UploadService.complete()`):
`{"uploadId":"...","fileReference":"...","fileType":"..."}`. `UploadReceivedEvent` is a small
`record` (`dto/UploadReceivedEvent.java`) that mirrors that shape, and `objectMapper.readValue(payload, UploadReceivedEvent.class)`
is Jackson deserializing the JSON string into it — the same `ObjectMapper` Spring Boot
auto-configures for you whenever `spring-boot-starter-web` is on the classpath (no extra setup
needed; it's injected here as a constructor parameter, same as any other bean).

Getting this wrong was a real bug during development: the very first version of this method took
the payload as if it *were* the uploadId directly (`UUID.fromString(uploadId)`, no JSON parsing
at all) — it compiled and ran, but crashed the moment a real message with a JSON body arrived
(`IllegalArgumentException: UUID string too large`). See `docs/TROUBLESHOOTING.md`. The lesson:
the *type* of a Kafka listener parameter (`String payload`) doesn't tell you what's actually
inside that string — that's determined entirely by whatever the producer decided to send.

Same idempotency pattern as every other Kafka listener in this system: check if the work already
exists before doing it, so a redelivered message is a safe no-op.

## 4. Calling another service — `TransformationDispatcher.java`, the gRPC **client** side

Contrast this with `UploadGrpcQueryService`/`TransformationGrpcService` from the other two docs,
which were gRPC *servers*. This class is the other half — a gRPC *client*:

```java
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
```
`@GrpcClient("transformation-service")` tells `grpc-client-spring-boot-starter` to build a
connection to whatever address is configured under that name in `application.yml`:
```yaml
grpc:
  client:
    transformation-service:
      address: static://localhost:9091
      negotiation-type: plaintext
```
— and inject a ready-to-use `TransformationServiceBlockingStub` into that field. "Blocking" means
`startTransformation(...)` behaves like a normal synchronous method call: it sends the request,
waits for the response, and returns — no callbacks, no futures. Note there are *three* such
client blocks in this service's `application.yml` (`transformation-service`, `ocr-service`,
`reporting-service`), even though only the transformation one is wired up to a dispatcher class
today — the OCR/reporting dispatch (the rest of the saga) is a known next step, not yet built
(see §6).

`StartTransformationRequest.newBuilder()...build()` is protobuf's generated **builder pattern**
— every generated message class is immutable once built, and this is how you construct one:
chain `.setXxx(...)` calls, then `.build()`.

Notice the request now actually carries `fileReference`/`fileType` — an earlier version of this
method didn't set those fields at all, silently sending empty strings, which meant
`transformation-service` would try to read an empty storage key. Fixed at the same time as the
JSON-parsing bug in §3, since both were needed to get real data flowing end to end.

## 5. Answering queries — `OrchestratorGrpcQueryService.java` and `SagaController.java`

Same two-transports-for-two-audiences pattern as `transformation-service`:
- **gRPC** — `GetSagaStatus`, for other services to ask directly (not yet called by anything,
  but available).
- **REST** — `GET /api/v1/sagas/{uploadId}` → `SagaController` → `SagaQueryService`, for humans
  and external tools (this is what `docs/RUNNING_LOCALLY.md`'s curl examples hit).

## 6. What's not built yet, honestly

The saga today gets exactly one step further than "started": upload received → saga created →
transformation dispatched. There is no listener consuming `transformation.events.completed` (or
`.failed`), so nothing ever advances `SagaState` past `VALIDATING`, and the OCR/reporting steps
are never dispatched even though their gRPC clients are configured and ready. Verified in
practice: after a real upload completes transformation successfully (see
`docs/PRESIGNED_UPLOADS.md`'s worked example), `GET /api/v1/sagas/{uploadId}` still shows
`"state": "STARTED", "currentStep": "TRANSFORMATION_DISPATCHED"` indefinitely.

Completing this would mean adding, following the exact shape of `UploadReceivedListener`:
- a `TransformationCompletedListener` (`@KafkaListener` on `transformation.events.completed`/`.failed`)
  that reads the saga, decides whether OCR is needed (`requiresOcr` from the event payload), and
  either calls a not-yet-written `OcrDispatcher` or skips straight to `ReportingDispatcher`
- similarly, an `OcrCompletedListener` and a `ReportingCompletedListener` to walk the rest of the
  chain through to `SagaState.COMPLETED`, plus the compensation logic for `FAILED`/`COMPENSATING`
  described in the architecture spec.
