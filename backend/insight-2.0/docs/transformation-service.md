# Code walkthrough: `transformation-service`

Assumes you've read `docs/upload-service.md` first — this doc skips re-explaining Maven basics,
`@Entity`/`@Table`, Spring Data repositories, Flyway, and `@RestController`, since they work
identically here. This doc focuses on what's *new*: a gRPC **server** that hands work off to an
**internal Kafka topic**, a **strategy pattern** for pluggable file parsers, and **Java 21
virtual threads**.

---

## 1. The role this service plays

`upload-service` never looks at file contents — it just proves a file exists in storage. This
service is where the real work happens: read the file back out of MinIO, parse it row by row
(CSV/Excel/JSON), validate each row, and report a total/valid/error count back to the saga.

## 2. Why there's a gRPC **server** here, but a **client** in `orchestrator-service`

In `upload-service`, `UploadGrpcQueryService` was a gRPC server answering queries. Here,
`TransformationGrpcService` is also a server — but it's the *target* of a command, not a query:
`orchestrator-service` holds a generated `TransformationServiceBlockingStub` (a gRPC **client**)
and calls `startTransformation(...)` on it, which travels over the network to this class:

```java
@GrpcService
public class TransformationGrpcService extends TransformationServiceGrpc.TransformationServiceImplBase {

    @Override
    public void startTransformation(StartTransformationRequest request,
                                     StreamObserver<StartTransformationResponse> responseObserver) {
        UUID uploadId = UUID.fromString(request.getUploadId());

        if (repository.findByUploadId(uploadId).isPresent()) {
            responseObserver.onNext(StartTransformationResponse.newBuilder()
                    .setStatus(DispatchStatus.ALREADY_PROCESSED).build());
            responseObserver.onCompleted();
            return;
        }

        repository.save(new TransformationJob(UUID.randomUUID(), uploadId, request.getSagaId(),
                request.getFileReference(), request.getFileType()));

        kafkaTemplate.send(KafkaTopics.INTERNAL_START, uploadId.toString(), uploadId.toString());

        responseObserver.onNext(StartTransformationResponse.newBuilder()
                .setStatus(DispatchStatus.ACCEPTED).build());
        responseObserver.onCompleted();
    }
    ...
}
```

Two things worth noticing:

**The idempotency check.** `if (repository.findByUploadId(uploadId).isPresent())` — if the
orchestrator retries this call (say, its first attempt timed out but actually succeeded), this
returns `ALREADY_PROCESSED` instead of creating a second job or re-processing the file. This is
the idempotency strategy from the architecture spec, implemented as "check if we've already seen
this upload ID" rather than a separate idempotency-key table — simpler, and sufficient here
because `uploadId` is already the natural dedupe key for this table.

**It returns almost immediately.** This method does *not* parse the file — it just records that
a job exists and fires off a Kafka message, then returns. That's deliberate, and is the subject
of the next section.

## 3. The internal Kafka hand-off — why the gRPC call doesn't do the real work

Parsing a multi-million-row file can take a long time. If `startTransformation` did that work
directly, the gRPC call itself would block for that entire duration — orchestrator-service would
be sitting there waiting, tying up a client connection, with no ability to scale the actual
parsing work independently of how many gRPC calls are coming in.

Instead, `startTransformation` publishes a message to a topic **this service defined for its own
internal use** — `KafkaTopics.INTERNAL_START` (`"transformation.internal.start"`) — not one of
the saga-facing topics like `transformation.events.completed`. A separate `@KafkaListener`
(`TransformationWorker`, §4) picks it up and does the real work, on its own schedule, on
whichever pod happens to be free.

```java
public final class KafkaTopics {
    public static final String INTERNAL_START = "transformation.internal.start";
    public static final String COMPLETED = "transformation.events.completed";
    public static final String FAILED = "transformation.events.failed";
}
```

Why this actually matters for scaling: if you run 5 replicas of `transformation-service`, all 5
run a gRPC server (any of them can *accept* a job) and all 5 run a Kafka consumer in the same
consumer group (`transformation-service-worker`) — Kafka automatically spreads partitions across
them, so the 5 replicas share the *processing* load too, independent of which replica happened
to receive the original gRPC call.

## 4. `TransformationWorker.java` — the actual file-processing loop

```java
@KafkaListener(topics = KafkaTopics.INTERNAL_START, groupId = "transformation-service-worker")
public void onStart(String uploadId) {
    UUID id = UUID.fromString(uploadId);
    TransformationJob job = jobRepository.findByUploadId(id).orElseThrow();

    if (job.getStatus() != TransformationStatus.IN_PROGRESS) {
        return; // redelivered message for a job already finalized — idempotent no-op
    }
    ...
```

Same idempotency idea as §2, at a different layer: Kafka can redeliver a message (e.g. if the
consumer crashed after processing but before committing its offset) — checking the job's current
status before doing anything makes reprocessing harmless.

The core loop:
```java
FileProcessorStrategy processor = processorRegistry.resolve(job.getFileType());

try (InputStream inputStream = objectStorageClient.get(job.getFileReference())) {
    processor.process(inputStream, (rowNumber, fields) -> {
        totalRows.incrementAndGet();
        Optional<String> violation = rowValidator.validate(fields);
        if (violation.isEmpty()) {
            validRows.incrementAndGet();
        } else {
            errorRows.incrementAndGet();
            pendingErrors.add(new RowValidationError(...));
            if (pendingErrors.size() >= BATCH_SIZE) {
                flushes.add(flushAsync(new ArrayList<>(pendingErrors)));
                pendingErrors.clear();
            }
        }
    });
}
```

`objectStorageClient.get(...)` returns a plain `InputStream` — the *same* `ObjectStorageClient`
interface from `common-library` that `upload-service` used to `put`/`presignPut`. This is the
"read" side of the same abstraction.

**Why `AtomicLong`/`AtomicBoolean` instead of plain `long`/`boolean`?** The row-handling logic is
a lambda (`(rowNumber, fields) -> {...}`), and Java lambdas can only capture variables from their
enclosing method if those variables are effectively final — you can't reassign a plain `long`
from inside one. `AtomicLong`/`AtomicBoolean` are mutable *objects* whose reference never changes
(so the lambda capture rule is satisfied) even though the value inside them does
(`.incrementAndGet()`, `.set(...)`). There's no actual multi-threading race here — it's a
single-threaded workaround for a Java language rule, not a concurrency mechanism in this case.

**Batching:** row errors are collected into a list and only written to the database every 500
rows (`BATCH_SIZE`), instead of one `INSERT` per invalid row — this is what makes "millions of
records" tractable; a few thousand batched writes instead of millions of individual ones.

## 5. Virtual threads — where Java 21 actually shows up

```java
private final ExecutorService persistenceExecutor = Executors.newVirtualThreadPerTaskExecutor();
...
private Future<?> flushAsync(List<RowValidationError> batch) {
    return persistenceExecutor.submit(() -> errorRepository.saveAll(batch));
}
```

A **virtual thread** is a JVM-managed thread that's extremely cheap to create — you can spin up
thousands without the memory/scheduling overhead of traditional ("platform") threads, which is
why `Executors.newVirtualThreadPerTaskExecutor()` doesn't need a pool size configured at all
(unlike `Executors.newFixedThreadPool(20)`, say). Here it lets each batch of row-errors get
written to Postgres *while the main loop keeps reading and parsing the next chunk of the file* —
the slow I/O (writing to the DB) overlaps with the slow I/O (reading/parsing the file) instead of
happening one after the other. `awaitAll(flushes)` at the end makes sure every batch actually
finished writing before the job gets marked complete — otherwise you could report "done" while a
batch of errors was still in flight.

`application.yml` also sets `spring.threads.virtual.enabled: true`, which switches Tomcat's own
request-handling threads (and `@Async`/scheduling infra generally) to virtual threads too — a
one-line, framework-level opt-in, separate from the explicit `persistenceExecutor` above which is
this class's own, deliberate use of them.

## 6. The strategy pattern — `FileProcessorStrategy` and its three implementations

```java
public interface FileProcessorStrategy {
    String fileType();
    void process(InputStream inputStream, FileRowHandler rowHandler) throws IOException;
}
```
Three `@Component` classes implement this: `CsvFileProcessor` (Apache Commons CSV),
`JsonFileProcessor` (Jackson's streaming `JsonParser`, reading one array element at a time —
never loading the whole array into memory), and `ExcelFileProcessor` (Apache POI's SAX-based
event API, `XSSFReader`/`XSSFSheetXMLHandler` — the standard `XSSFWorkbook` API loads an entire
spreadsheet into memory, which defeats the purpose for a huge file, so this uses POI's
lower-level streaming reader instead).

`FileProcessorRegistry` is how the right one gets picked at runtime:
```java
@Component
public class FileProcessorRegistry {
    private final Map<String, FileProcessorStrategy> strategiesByFileType;

    public FileProcessorRegistry(List<FileProcessorStrategy> strategies) {
        this.strategiesByFileType = strategies.stream()
                .collect(Collectors.toUnmodifiableMap(FileProcessorStrategy::fileType, Function.identity()));
    }

    public FileProcessorStrategy resolve(String fileType) {
        return strategiesByFileType.get(fileType); // throws IllegalArgumentException if absent
    }
}
```
Spring automatically collects *every* bean implementing `FileProcessorStrategy` into that
`List<FileProcessorStrategy> strategies` constructor parameter — you never register them by hand.
This is the extensibility point called out in the original architecture doc: **onboarding a new
file type is "add one new `@Component` class," not "modify a switch statement somewhere."**

## 7. `RowValidator.java` — deliberately minimal
```java
public Optional<String> validate(Map<String, String> fields) {
    for (Map.Entry<String, String> field : fields.entrySet()) {
        if (field.getValue() == null || field.getValue().isBlank()) {
            return Optional.of("Field '%s' must not be blank".formatted(field.getKey()));
        }
    }
    return Optional.empty();
}
```
Right now the only rule is "no blank fields." This is a placeholder — real per-file-type schemas
(required columns, type coercion, business rules) would replace the body of this one method
without the worker or any file processor needing to change at all.

## 8. Reporting results back — `transformation_jobs` and `row_validation_errors`

`TransformationJob` (entity) tracks one row per upload: `status`
(`IN_PROGRESS`/`COMPLETED`/`COMPLETED_WITH_ERRORS`/`FAILED`), plus `totalRows`/`validRows`/`errorRows`
and `requiresOcr`. `RowValidationError` is one row per invalid record, referencing the job by
foreign key (`db/migration/V1__init.sql`):
```sql
CREATE TABLE row_validation_errors (
    id                    UUID PRIMARY KEY,
    transformation_job_id UUID NOT NULL REFERENCES transformation_jobs (id),
    row_number            BIGINT NOT NULL,
    reason                VARCHAR(1024) NOT NULL
);
```
Note `COMPLETED_WITH_ERRORS` is treated as success, not failure, by `markCompleted`:
```java
this.status = errorRows == 0 ? TransformationStatus.COMPLETED : TransformationStatus.COMPLETED_WITH_ERRORS;
```
A file with 999,000 valid rows and 1,000 malformed ones is a successful transformation carrying
an error report — not a failed saga step. Only an actual exception (file unreadable, storage
unreachable, etc., caught in `onStart`'s `catch` block) calls `job.markFailed()` and publishes to
`transformation.events.failed` instead.

## 9. The two query paths — REST and gRPC, side by side

- **REST**, for humans/external tools: `GET /api/v1/transformation-jobs/{uploadId}` →
  `TransformationJobController` → `TransformationJobQueryService`.
- **gRPC**, for other services: `TransformationGrpcService.getTransformationResult` — the same
  data, but callable as a typed method from `reporting-service` without either side touching
  JSON or HTTP status codes. Same underlying repository, two different transports for two
  different kinds of caller.
