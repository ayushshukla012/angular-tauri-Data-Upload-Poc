# Code walkthrough: `common-library` → `protos` → `upload-service`

Written for someone new to Java/Spring Boot. It walks through every file involved in the
upload flow, in the order they'd actually execute, explaining both *what the code does* and
*what the Java/Maven/Spring mechanism is* — since you can't understand one without the other.

---

## 1. The shape of the repo, and what Maven is doing

This is a **Maven multi-module project**. At the root, `pom.xml` (the "parent POM") lists every
module:

```xml
<modules>
  <module>common-library</module>
  <module>protos</module>
  <module>upload-service</module>
  <module>transformation-service</module>
  ...
</modules>
```

Each module is its own folder with its own `pom.xml`, and each one is compiled into its own
`.jar` file. `upload-service` depends on `common-library` and `protos` the same way it'd depend
on any third-party library — by listing them in its `<dependencies>`:

```xml
<dependency><groupId>com.insight</groupId><artifactId>common-library</artifactId><version>${project.version}</version></dependency>
<dependency><groupId>com.insight</groupId><artifactId>protos</artifactId><version>${project.version}</version></dependency>
```

**What `groupId`/`artifactId`/`version` mean:** together they uniquely identify a jar, the same
way a package name does in npm/pip. `com.insight:upload-service:1.0.0-SNAPSHOT` is this
service's own identity; `com.insight:common-library:1.0.0-SNAPSHOT` is the shared library's.

**What the `target/` folder is:** every module has one, and it's 100% generated — never edit
anything inside it, and it's excluded from git (`.gitignore` has `target/`). When you run
`mvn compile`, Maven:
1. Compiles `.java` files into `.class` files (bytecode the JVM runs) → `target/classes/`
2. If a plugin generates source code from something else (more on this in §3) → `target/generated-sources/`
3. Packages `target/classes/` into a single `.jar` → `target/<module>-1.0.0-SNAPSHOT.jar`
4. `mvn install` additionally copies that jar into `~/.m2/repository/...` — your local cache —
   so *other* modules (and other projects on your machine) can depend on it without rebuilding it.

Delete `target/` any time and rebuild — that's the whole point of it being disposable output.

---

## 2. `common-library` — the shared jar

Every service depends on this for things that would otherwise be copy-pasted five times:
error-response shapes, exception base classes, and (relevant to upload-service) the object
storage client.

### `common-library/pom.xml`
```xml
<dependency><groupId>org.springframework.boot</groupId><artifactId>spring-boot-starter</artifactId></dependency>
<dependency><groupId>org.projectlombok</groupId><artifactId>lombok</artifactId><optional>true</optional></dependency>
<dependency><groupId>software.amazon.awssdk</groupId><artifactId>s3</artifactId></dependency>
```
`spring-boot-starter` is the bare-minimum Spring Boot dependency (no web server, no database —
just the core framework), needed here because this module defines Spring-managed beans (§2.3).
`software.amazon.awssdk:s3` is AWS's own Java SDK — MinIO speaks the same S3 API, so the same
client library talks to either.

### 2.1 Plain utility classes
- **`dto/ApiError.java`** — a `record` (see the sidebar below) describing the JSON shape every
  service returns on error: `{code, message, timestamp, fieldErrors}`.
- **`exception/BaseException.java`** / **`ResourceNotFoundException.java`** — a small exception
  hierarchy every service's own exceptions extend, so error handling is consistent.

> **Sidebar — what's a `record`?** Java's shorthand for an immutable data class. Writing
> `public record ApiError(String code, String message, Instant timestamp, List<FieldError> fieldErrors) {}`
> gives you, for free: a constructor, getters named after the fields (`code()`, not
> `getCode()`), and working `equals()`/`hashCode()`/`toString()`. You'll see this used
> everywhere instead of the old-style class-with-getters-and-setters pattern.

### 2.2 `storage/ObjectStorageClient.java` — the interface
```java
public interface ObjectStorageClient {
    void put(String key, InputStream data, long contentLength, String contentType);
    InputStream get(String key);
    URI presignPut(String key, String contentType, Duration ttl);
    boolean exists(String key);
}
```
This is **just a contract** — no logic. `upload-service`'s code depends only on this interface,
never on the class below it. That's deliberate: if MinIO were swapped for something else, only
`S3ObjectStorageClient` would need to change.

### 2.3 `storage/S3ObjectStorageClient.java` — the implementation
Implements the four methods using AWS SDK classes (`S3Client` for `put`/`get`/`exists`,
`S3Presigner` for `presignPut`). This is the class that actually talks to MinIO over HTTP.

### 2.4 How this class gets *into* upload-service without upload-service writing any wiring code

This is the part that looks like magic the first time you see it. Three pieces work together:

**a) `ObjectStorageProperties.java`** — a `@ConfigurationProperties(prefix = "insight.storage")`
class. Spring Boot automatically fills in its fields from `application.yml`:
```yaml
insight:
  storage:
    endpoint: http://localhost:9000
    bucket: insight-uploads
    access-key: minioadmin
    secret-key: minioadmin
```
`insight.storage.endpoint` in YAML → `properties.getEndpoint()` in Java. No manual parsing.

**b) `ObjectStorageAutoConfiguration.java`** — an `@AutoConfiguration` class that declares
`@Bean` methods for `S3Client`, `S3Presigner`, and `ObjectStorageClient`:
```java
@AutoConfiguration
@EnableConfigurationProperties(ObjectStorageProperties.class)
@ConditionalOnProperty(prefix = "insight.storage", name = "endpoint")
public class ObjectStorageAutoConfiguration {
    @Bean
    public ObjectStorageClient objectStorageClient(S3Client s3Client, S3Presigner presigner, ObjectStorageProperties properties) {
        return new S3ObjectStorageClient(s3Client, presigner, properties.getBucket());
    }
    ...
}
```
A **"bean"** is just an object Spring creates and manages for you, so any other class can ask
for it by declaring it as a constructor parameter (you'll see `UploadService`'s constructor do
exactly that) instead of constructing it with `new`.

The `@ConditionalOnProperty` guard is important: `ocr-service` and `orchestrator-service` also
depend on `common-library`, but never set `insight.storage.*` — without this guard, Spring would
try to build an `S3Client` for them too and crash on missing config. This line means "only
switch this on if `insight.storage.endpoint` is actually set somewhere."

**c) `resources/META-INF/spring/org.springframework.boot.autoconfigure.AutoConfiguration.imports`**
— a one-line text file:
```
com.insight.common.storage.ObjectStorageAutoConfiguration
```
This is how Spring Boot finds `ObjectStorageAutoConfiguration` at all. Normally Spring only
scans classes inside your own application's package (`com.insight.upload.*` for
upload-service) — it would never look inside `com.insight.common.*` on its own. This file,
shipped inside the `common-library` jar, is a registration mechanism: "when you start up, also
consider this class from over here." It's the standard way a shared library hands a Spring Boot
application ready-made beans.

---

## 3. `protos` — how gRPC is set up, and how the stub code gets generated

### 3.1 What gRPC actually is, in one paragraph
REST (what `UploadController` exposes) is "call a URL, get JSON back" — flexible, but every
client has to know the URL shape and parse JSON by hand. **gRPC** is for service-to-service
calls: you write the contract once in a `.proto` file, and a code generator produces a real Java
class with real method signatures for both the caller and the callee — no URLs, no manual JSON
parsing, just calling a method that happens to go over the network.

### 3.2 The contract: `protos/src/main/proto/upload/v1/upload_service.proto`
```protobuf
syntax = "proto3";
package insight.upload.v1;

option java_package = "com.insight.protos.upload.v1";
option java_multiple_files = true;

service UploadQueryService {
  rpc GetUploadStatus (GetUploadStatusRequest) returns (GetUploadStatusResponse);
}

message GetUploadStatusRequest {
  string upload_id = 1;
}

message GetUploadStatusResponse {
  string upload_id = 1;
  string status = 2;
}
```
Line by line:
- `syntax = "proto3"` — the protobuf language version.
- `package insight.upload.v1` — the protobuf package (separate from the Java package below).
- `option java_package = "..."` — what Java package the *generated* classes will live in.
- `option java_multiple_files = true` — generate one `.java` file per message, instead of one
  giant file with everything nested inside it.
- `service ... { rpc MethodName (RequestType) returns (ResponseType); }` — declares one callable
  method. This is what gets turned into an actual Java interface you implement.
- `message` — a data structure. Each field gets a number (`= 1`, `= 2`) that's part of the
  binary wire format — this is why you never renumber existing fields once they're in use.

### 3.3 The plugin that turns this into Java: `protos/pom.xml`
```xml
<extensions>
  <extension><groupId>kr.motd.maven</groupId><artifactId>os-maven-plugin</artifactId>.../>
</extensions>
<plugin>
  <groupId>org.xolstice.maven.plugins</groupId>
  <artifactId>protobuf-maven-plugin</artifactId>
  <configuration>
    <protocArtifact>com.google.protobuf:protoc:${protobuf.version}:exe:${os.detected.classifier}</protocArtifact>
    <pluginId>grpc-java</pluginId>
    <pluginArtifact>io.grpc:protoc-gen-grpc-java:${grpc.version}:exe:${os.detected.classifier}</pluginArtifact>
  </configuration>
  <executions>
    <execution><goals><goal>compile</goal><goal>compile-custom</goal></goals></execution>
  </executions>
</plugin>
```
What's actually happening when you run `mvn compile` on this module:
1. **`os-maven-plugin`** detects your OS/architecture (e.g. `osx-aarch_64`), so Maven can
   download the *correct* prebuilt binary in the next step.
2. **`protoc`** (Google's protobuf compiler — downloaded automatically as a Maven artifact, not
   something you install yourself) reads every `.proto` file and generates the `message` classes
   — `GetUploadStatusRequest.java`, `GetUploadStatusResponse.java`, etc. These land in
   `target/generated-sources/protobuf/java/`.
3. **`protoc-gen-grpc-java`** (the `compile-custom` goal) is a second code generator, specific to
   gRPC, that reads the same `.proto` and generates the `service` part —
   `UploadQueryServiceGrpc.java` — into `target/generated-sources/protobuf/grpc-java/`. This one
   file contains, nested inside it:
   - `UploadQueryServiceGrpc.UploadQueryServiceImplBase` — an abstract class the *server side*
     extends and overrides (see §5.7).
   - `UploadQueryServiceGrpc.UploadQueryServiceBlockingStub` — a ready-to-use client the
     *caller side* gets injected and just calls methods on (used by `orchestrator-service` and
     `reporting-service`, not by upload-service itself).
4. Maven's normal compiler then compiles *both* the hand-written nothing (there's no
   hand-written Java in this module — it's 100% generated) and these generated `.java` files
   into `.class` files, and packages the result into `protos-1.0.0-SNAPSHOT.jar`.

This is **why `protos` is its own Maven module** rather than each service defining its own
copies: the generated classes are compiled once, installed to `~/.m2`, and every service that
needs them (`upload-service`, `orchestrator-service`, `reporting-service`, ...) just adds one
`<dependency>` line and gets the same, guaranteed-identical classes.

---

## 4. `upload-service` — the full walkthrough

### 4.1 `pom.xml` — what each dependency buys you

| Dependency | What it's for |
|---|---|
| `common-library`, `protos` | the two modules above |
| `spring-boot-starter-web` | embedded Tomcat + Spring MVC (`@RestController` and friends) |
| `spring-boot-starter-data-jpa` | Hibernate + Spring Data repositories (§4.5) |
| `spring-boot-starter-validation` | powers `@Valid`/`@NotBlank` on request DTOs |
| `spring-boot-starter-actuator` | `/actuator/health` etc. |
| `grpc-server-spring-boot-starter` | runs a gRPC server alongside the HTTP one (§4.8) |
| `spring-kafka` | `KafkaTemplate` (producing) and `@KafkaListener` (consuming) |
| `flyway-core` + `flyway-database-postgresql` | runs the SQL in `db/migration/` on startup (§4.9) |
| `postgresql` | the JDBC driver — how Java actually talks to Postgres |
| `lombok` | (currently unused directly, but available) generates boilerplate getters/setters via annotations |

### 4.2 `UploadServiceApplication.java` — the entry point
```java
@SpringBootApplication
@EnableScheduling
public class UploadServiceApplication {
    public static void main(String[] args) {
        SpringApplication.run(UploadServiceApplication.class, args);
    }
}
```
`@SpringBootApplication` is three annotations in one: it turns on auto-configuration (which is
what discovers `ObjectStorageAutoConfiguration` from §2.4), turns on component scanning (finds
every `@Service`/`@RestController`/etc. under `com.insight.upload`), and marks this as a
Spring-managed class itself. `@EnableScheduling` turns on `@Scheduled` methods — without it,
`OutboxRelayPublisher.relay()` (§4.7) would be silently ignored, which was a real bug we hit
(see `docs/TROUBLESHOOTING.md`).

### 4.3 `application.yml` — configuration, not code
```yaml
server:
  port: 8081
grpc:
  server:
    port: 9095
spring:
  datasource:
    url: jdbc:postgresql://localhost:5433/upload
  flyway:
    enabled: true
  kafka:
    bootstrap-servers: localhost:9092
insight:
  storage:
    endpoint: http://localhost:9000
    ...
```
This is the file that gets read into `ObjectStorageProperties` (§2.4) and into a dozen other
Spring-internal configuration classes you never see. Nothing here is Java — it's all
key → field mapping done by Spring at startup.

### 4.4 `entity/Upload.java` and `entity/UploadStatus.java` — the domain object

```java
@Entity
@Table(name = "uploads")
public class Upload {
    @Id
    private UUID id;
    @Column(nullable = false)
    private String fileName;
    ...
    @Enumerated(EnumType.STRING)
    private UploadStatus status;

    protected Upload() {}   // JPA needs this — see below

    public Upload(UUID id, String fileName, String fileType, String storageReference) {
        ...
        this.status = UploadStatus.PENDING;
    }
}
```
`@Entity` + `@Table` tells Hibernate "rows in the `uploads` table map to instances of this
class." `@Id` marks the primary key. `@Enumerated(EnumType.STRING)` means the `status` column
stores the literal text `"PENDING"`/`"RECEIVED"`/etc., not a number — much easier to read when
you query the database by hand.

The empty `protected Upload()` constructor looks pointless but isn't: Hibernate needs *some*
no-argument constructor to build an instance before it populates the fields via reflection.
It's `protected` (not `public`) so your own code is forced to use the real constructor below it,
which enforces "every `Upload` starts life with a valid file name, type, and `PENDING` status" —
you can't accidentally construct a half-built one.

`UploadStatus` is a plain Java `enum` — `PENDING, RECEIVED, VALIDATING, PROCESSING, COMPLETED, FAILED`.
`PENDING` is an implementation detail (§ see `docs/PRESIGNED_UPLOADS.md`) — not one of the 5
client-facing statuses from the original spec, but needed internally to represent "we handed out
a URL, the file hasn't arrived yet."

### 4.5 `repository/UploadRepository.java` — no implementation, on purpose
```java
public interface UploadRepository extends JpaRepository<Upload, UUID> {
}
```
That's the entire file. `JpaRepository<Upload, UUID>` already gives you `save()`, `findById()`,
`findAll()`, `deleteById()`, etc. — Spring Data JPA generates a real implementation of this
interface at startup, on the fly. `OutboxEventRepository` adds one custom method:
```java
List<OutboxEvent> findTop100ByPublishedFalseOrderByCreatedAtAsc();
```
Spring Data parses the *method name itself* and derives the SQL from it — "find top 100 where
published = false, ordered by createdAt ascending." No query is written anywhere for this;
the name **is** the query.

### 4.6 `dto/` — the shapes that cross the HTTP boundary

`InitiateUploadRequest`, `InitiateUploadResponse`, `UploadResponse` — all `record`s (§2.1
sidebar). These are deliberately *separate* types from `Upload` the entity. `UploadMapper`
converts between them:
```java
public UploadResponse toResponse(Upload upload) {
    return new UploadResponse(upload.getId().toString(), upload.getFileName(), ...);
}
```
Why not just return the entity directly from the controller? Mainly so the database shape and
the API shape can evolve independently — e.g. `Upload` could grow an internal-only field
tomorrow without it leaking into the JSON response, since the mapper controls exactly what
crosses that boundary.

### 4.7 `service/OutboxRelayPublisher.java` — the outbox pattern
```java
@Scheduled(fixedDelay = 1000)
@Transactional
public void relay() {
    List<OutboxEvent> pending = outboxEventRepository.findTop100ByPublishedFalseOrderByCreatedAtAsc();
    for (OutboxEvent event : pending) {
        kafkaTemplate.send(event.getTopic(), event.getAggregateId(), event.getPayload());
        event.markPublished();
    }
}
```
`@Scheduled(fixedDelay = 1000)` runs this method once a second, forever (requires
`@EnableScheduling` on the application class, §4.2). The reason this exists instead of just
calling `kafkaTemplate.send(...)` directly from `UploadService.complete()`: writing to Postgres
and publishing to Kafka are two *different systems* — there's no way to make both happen
atomically in one transaction. If you published to Kafka first and the DB write then failed,
downstream services would react to an upload that, as far as the database is concerned, doesn't
exist. The **outbox pattern** sidesteps this: write the "I want to publish this" intent as a
normal row in the *same* database transaction as everything else (so it's guaranteed
consistent), then have this separate poller pick up unpublished rows and actually send them to
Kafka moments later. A brief delay, but no lost or phantom events.

### 4.8 `service/UploadGrpcQueryService.java` — the gRPC server side
```java
@GrpcService
public class UploadGrpcQueryService extends UploadQueryServiceGrpc.UploadQueryServiceImplBase {
    @Override
    public void getUploadStatus(GetUploadStatusRequest request, StreamObserver<GetUploadStatusResponse> responseObserver) {
        Upload upload = repository.findById(UUID.fromString(request.getUploadId()))
                .orElseThrow(() -> new UploadNotFoundException(request.getUploadId()));
        responseObserver.onNext(GetUploadStatusResponse.newBuilder()
                .setUploadId(upload.getId().toString())
                .setStatus(upload.getStatus().name())
                .build());
        responseObserver.onCompleted();
    }
}
```
`UploadQueryServiceGrpc.UploadQueryServiceImplBase` is the class generated in §3.3 — this is
where the contract from the `.proto` file actually gets a real implementation. `@GrpcService` is
this project's equivalent of `@RestController`, but for gRPC: it tells `grpc-server-spring-boot-starter`
to register this bean and start listening on the port from `application.yml`
(`grpc.server.port: 9095`). Other services (`reporting-service`, potentially
`orchestrator-service`) can call `getUploadStatus` as a real method call via a generated
`UploadQueryServiceBlockingStub`, instead of hitting a REST URL.

### 4.9 `db/migration/V1__init.sql` — how the tables get created

Flyway looks at every file in `db/migration/`, reads the version number out of the filename
(`V1__init.sql` = version 1), and runs any it hasn't run yet, in order, tracking progress in a
`flyway_schema_history` table it manages itself. `spring.jpa.hibernate.ddl-auto: validate` in
`application.yml` means Hibernate is *not* allowed to create/alter tables itself — it only
checks the tables Flyway already created actually match what the `@Entity` classes expect, and
fails fast at startup if they don't. This is the standard production pairing: migrations are
explicit, reviewable SQL files, not something an ORM guesses at.

### 4.10 `controller/UploadController.java` — where an HTTP request enters the code
```java
@RestController
@RequestMapping("/api/v1/uploads")
public class UploadController {

    @PostMapping("/initiate")
    public ResponseEntity<InitiateUploadResponse> initiate(@Valid @RequestBody InitiateUploadRequest request) {
        return ResponseEntity.status(HttpStatus.CREATED).body(uploadService.initiate(request.fileName()));
    }

    @PostMapping("/{uploadId}/complete")
    public ResponseEntity<UploadResponse> complete(@PathVariable String uploadId) {
        return ResponseEntity.ok(uploadService.complete(uploadId));
    }

    @GetMapping("/{uploadId}")
    public ResponseEntity<UploadResponse> getStatus(@PathVariable String uploadId) {
        return ResponseEntity.ok(uploadService.getStatus(uploadId));
    }
}
```
- `@RestController` = "every method's return value gets serialized to JSON and written to the
  HTTP response body" (as opposed to `@Controller`, used for server-rendered HTML pages).
- `@RequestMapping("/api/v1/uploads")` — a shared URL prefix for every method below.
- `@PostMapping`/`@GetMapping` — HTTP method + the rest of the URL path.
- `@RequestBody` — deserialize the JSON request body into `InitiateUploadRequest`. This is also
  exactly why you must send `Content-Type: application/json` — without it, Spring has no idea
  how to do that deserialization and rejects the request with `415 Unsupported Media Type`.
- `@Valid` — before the method body even runs, check the annotations on `InitiateUploadRequest`
  (`@NotBlank String fileName`) and reject the request with `400` automatically if they fail.
- `@PathVariable` — pull a value out of the URL itself (the `{uploadId}` part).

This class deliberately has almost no logic — it just translates HTTP concepts (status codes,
path variables, request bodies) into a plain method call on `UploadService`, which is where the
actual behavior lives (walked through in full in `docs/PRESIGNED_UPLOADS.md`).

### 4.11 `exception/` and `GlobalExceptionHandler.java` — turning failures into clean responses
```java
@RestControllerAdvice
public class GlobalExceptionHandler {
    @ExceptionHandler(UploadNotFoundException.class)
    public ResponseEntity<ApiError> handleNotFound(UploadNotFoundException ex) {
        return ResponseEntity.status(HttpStatus.NOT_FOUND).body(new ApiError(...));
    }
}
```
`@RestControllerAdvice` + `@ExceptionHandler` means: no matter which controller method throws a
`UploadNotFoundException`, intercept it here and turn it into a proper `404` with a JSON body
matching `ApiError` (from `common-library`, §2.1) — instead of Spring's generic, unhelpful `500`.
This is also exactly the gap identified earlier for unrecognized file extensions: there's no
handler here for the plain `IllegalArgumentException` that `resolveFileType` throws, so it falls
through to a `500` instead of a clean `400`.

---

## 5. Tying it all together — one request, start to finish

```
1. POST /api/v1/uploads/initiate  {"fileName": "sample.csv"}
       │
       ▼
   UploadController.initiate()          — deserializes JSON, validates it
       │
       ▼
   UploadService.initiate()             — @Transactional
       ├─ resolveFileType("sample.csv") → "CSV"
       ├─ objectStorageClient.presignPut(...)   ← common-library bean, built via
       │                                          ObjectStorageAutoConfiguration
       ├─ new Upload(..., status=PENDING)
       └─ uploadRepository.save(upload)          ← Hibernate INSERT, via the JPA repo
       │
       ▼
   UploadMapper / InitiateUploadResponse → JSON back to caller

2. (client PUTs file bytes straight to MinIO — upload-service isn't involved)

3. POST /api/v1/uploads/{id}/complete
       │
       ▼
   UploadController.complete()
       │
       ▼
   UploadService.complete()             — @Transactional
       ├─ uploadRepository.findById(id)
       ├─ objectStorageClient.exists(key)         ← confirms the file really landed
       ├─ upload.markStatus(RECEIVED)
       └─ outboxEventRepository.save(new OutboxEvent(...))   ← NOT published to Kafka yet

   (up to ~1 second later, on a separate scheduled thread)
       │
       ▼
   OutboxRelayPublisher.relay()          — @Scheduled, runs every 1s
       └─ kafkaTemplate.send("upload.events.received", ...)  ← NOW it hits Kafka,
                                                                 picked up by orchestrator-service
```

Meanwhile, `UploadGrpcQueryService` sits alongside all of this on a separate port (`9095`),
answering `GetUploadStatus` calls from other services over gRPC at any time — completely
independent of the HTTP flow above.

---

## 6. Other folders worth knowing about

- **`src/main/resources/`** — anything here gets copied as-is onto the classpath at build time.
  `application.yml` and `db/migration/*.sql` both live here for exactly that reason — Flyway and
  Spring both look for them on the classpath at runtime, not on disk relative to the source code.
- **`src/test/`** — where test code would go (currently empty scaffolding — `spring-boot-starter-test`
  is on the classpath, ready for when tests get written).
- **`~/.m2/repository/`** — your machine's local Maven cache, not part of this repo at all. Every
  dependency you've ever built or downloaded lives here, keyed by `groupId/artifactId/version`.
  This is *why* `mvn install` (not just `mvn compile`) matters for `common-library` and `protos`
  — `install` is the step that copies the jar here so sibling modules can find it.
- **`scripts/`** — `build.sh`/`run.sh`/`stop.sh`/`deploy.sh`/`images.sh`: thin wrappers around the
  Maven/Docker commands from `docs/RUNNING_LOCALLY.md`, nothing more.
- **`infra/postgres-init/`** — shell scripts Postgres's Docker image runs once, on first startup
  of an empty volume, to create the per-service databases/roles (see `docker-compose.yml`).
