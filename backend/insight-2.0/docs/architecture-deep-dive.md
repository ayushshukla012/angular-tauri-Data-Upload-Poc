# Architecture Deep-Dive

**Status:** Proposal, for architecture/demo review
**Purpose:** A ground-up explanation of every structural decision in this repository — the
folder layout, the protobuf/gRPC contract layer, the REST entry layer, how a shared library is
used safely across independently-deployed services, how hexagonal architecture is realized
*without* a multi-module split, how the strategy pattern gives us a swappable storage backend,
and how Helm/scripts/docs/infra/config/docker-compose/the outbox pattern/ArchUnit all fit
together. The pattern is domain-agnostic — it would hold for order processing, claims
processing, or document ingestion equally — but every section grounds the explanation in the
concrete repository so it's verifiable, not abstract.

---

## 1. Folder structure, top to bottom

```
platform/
├── pom.xml                     ← Maven parent: module list + shared dependency versions
├── docker-compose.yml          ← one-command local infra (§10)
├── .gitignore
│
├── common-library/             ← shared jar (§4)
├── protos/                     ← shared gRPC contracts (§2)
│
├── <service-a>/                ← one Maven module = one deployable
├── <service-b>/
├── <service-c>/                (repeat per bounded context)
│
├── architecture-tests/         ← ArchUnit: enforces layering at build time (§8)
│
├── helm/<service>/              ← one Helm chart per service (§6)
├── infra/                       ← local-dev-only provisioning (§7)
├── scripts/                     ← thin operational wrappers (§6.1)
└── docs/                        ← this file and its siblings (§6.2)
```

**The organizing principle:** every top-level folder answers exactly one question about the
system — "what's the shared contract" (`protos`), "what's genuinely shared code"
(`common-library`), "what runs as its own process" (each service folder), "does the layering
stay honest" (`architecture-tests`), "how does this get deployed" (`helm`), "how do I run this on
my laptop" (`infra` + `docker-compose.yml` + `scripts`), "how do I understand any of this"
(`docs`). No folder tries to answer two of those questions at once — that's what keeps a
multi-module repository like this navigable as it grows.

### 1.1 Does every service need its own database?

No — "one service, one database" applies to services that own persistent state; it isn't a
mandate that every module must have one. A purely stateless service (a router, an aggregator, a
notification dispatcher, an API gateway fronting others) is still its own Maven module and its
own deployable, but simply has no `entity/`, `repository/`, or `db/migration/` — no JPA/Flyway
dependency in its `pom.xml`, no `datasource` block in `application.yml`. It holds logic only:
gRPC calls out, Kafka listen/publish, REST. ArchUnit's entity/repository rules (§11) don't apply
to it, because those packages don't exist there — nothing needs to be disabled or special-cased.

If a service needs *some* state but not a full relational store — a short-lived idempotency
cache, a rate-limit counter — the right move is still "its own store, never borrowed from another
service's database": something like Redis, scoped to that service alone, preserves the
no-shared-schema principle without forcing a full Postgres instance onto a service that doesn't
need one.

---

## 2. How the protobuf/gRPC contract layer (`protos/`) is laid out

```
protos/src/main/proto/
├── common/v1/
│   └── common.proto             ← shared message types (error envelope, etc.)
├── <context-a>/v1/
│   └── <context_a>_service.proto
├── <context-b>/v1/
│   └── <context_b>_service.proto
└── ...                          ← one folder per bounded context
```

**One package per bounded context, each independently versioned (`v1`, `v2`, ...).** This is
deliberate: if `<context-b>`'s contract needs a breaking change, only `<context-b>/v2/` gets
created — every other service's contract, and every service that *doesn't* depend on
`<context-b>`, is completely unaffected. A single shared `platform.proto` with everything in one
package would force a coordinated version bump across the whole system for any change.

**What's actually inside a `.proto` file** — a contract, nothing else:
```protobuf
syntax = "proto3";
package insight.upload.v1;
option java_package = "com.insight.protos.upload.v1";
option java_multiple_files = true;

service UploadQueryService {
  rpc GetUploadStatus (GetUploadStatusRequest) returns (GetUploadStatusResponse);
}
message GetUploadStatusRequest  { string upload_id = 1; }
message GetUploadStatusResponse { string upload_id = 1; string status = 2; }
```

**Why this lives in its own Maven module, not inside each service:** `protos/pom.xml` runs the
`protobuf-maven-plugin`, which invokes `protoc` and `protoc-gen-grpc-java` to turn every `.proto`
file into real Java classes — message classes and a generated `XxxServiceGrpc` class containing
both the server-side base class (`XxxServiceImplBase`) and the client-side stub
(`XxxServiceBlockingStub`). That generation happens **once**, the result is compiled and
installed as `protos-1.0.0-SNAPSHOT.jar`, and every service that needs it (whether it's
implementing the server side or calling it as a client) adds one `<dependency>` line and gets
byte-for-byte the same generated classes. No service maintains its own copy of another service's
contract, which is what makes the contract actually binding rather than aspirational.

---

## 3. The REST entry layer — what it is and isn't responsible for

Every service that's externally callable has a `controller/` package: thin classes annotated
`@RestController`, whose only job is translating HTTP concepts into a plain method call.

```java
@RestController
@RequestMapping("/api/v1/uploads")
public class UploadController {
    @PostMapping("/initiate")
    public ResponseEntity<InitiateUploadResponse> initiate(@Valid @RequestBody InitiateUploadRequest request) {
        return ResponseEntity.status(HttpStatus.CREATED).body(uploadService.initiate(request.fileName()));
    }
}
```

**What this layer does:** deserializes the request body (`@RequestBody`), validates it before the
method body even runs (`@Valid`), extracts path/query values (`@PathVariable`), and picks the
right HTTP status code for the response. **What this layer explicitly does not do:** no business
logic, no direct repository access, no direct calls to infrastructure (storage clients, Kafka
producers) — all of that is one line away, in the `service/` package. This is enforced, not just
convention — see §8.

Paired with the controller is a `GlobalExceptionHandler` (`@RestControllerAdvice`) per service,
which turns domain exceptions into a consistent JSON error shape (`ApiError`, from
`common-library`) with the right HTTP status — a `ResourceNotFoundException` subclass becomes a
`404`, a conflict-type exception becomes a `409`, and so on. The controller method itself never
has a `try/catch` in it — exception-to-status-code translation is centralized in exactly one
place per service.

---

## 4. How `common-library` is used across services

`common-library` is a plain Maven dependency, not a running service — nothing in it executes on
its own; it's compiled into whichever service jar depends on it. It holds exactly two kinds of
things:

- **Cross-cutting shapes and base types** used identically everywhere: `ApiError` (the shared
  error-response shape referenced in §3), a small exception base hierarchy every service's own
  exceptions extend, and small utilities.
- **A shared infrastructure client with its own auto-wiring**, covered in depth in §5 — an
  object-storage client is the concrete example in this repository, but the mechanism applies to
  any cross-cutting infrastructure concern (a notification client, a feature-flag client, etc.).

**The discipline that keeps this safe:** nothing service-specific ever goes in here. No entity
definitions, no service-specific DTOs, no business rules. The moment a "shared" domain class
appears in `common-library`, changes to it start rippling across services that are supposed to
deploy independently — which quietly recreates a distributed monolith under the appearance of
microservices. `common-library` earns its place by being genuinely identical for every consumer,
not by being "stuff more than one service happens to need right now."

---

## 5. Hexagonal architecture — ports and adapters, without a multi-module split

The repository's packages are flat (`controller/dto/entity/exception/mapper/repository/service`)
rather than split into `domain/application/infrastructure` modules — but the *hexagonal
principle* (business logic depends only on interfaces it defines; infrastructure depends on
business logic, never the reverse) is still fully in force, enforced by package convention plus
ArchUnit (§8) instead of by module boundaries:

```
                     ┌─────────────────────────────┐
   inbound adapters  │                              │  outbound adapters
   ───────────────►  │      service/  (the core)     │  ◄───────────────
                     │                              │
   REST controller    │  business logic here, talking  │   JPA repository
   gRPC server class  │  only to interfaces it owns    │   (Spring Data-generated)
   Kafka @KafkaListener│                              │   ObjectStorageClient impl
                     │                              │   KafkaTemplate (producer)
                     └─────────────────────────────┘
                     entity/  — the domain model itself
```

**Inbound ports** are the entry points *into* the service — a REST controller method, a gRPC
`ImplBase` override, a `@KafkaListener` method. All three are just different transports calling
the same kind of thing: a method on a class in `service/`.

**Outbound ports** are interfaces the business logic depends on, never a concrete implementation
directly: `UploadRepository extends JpaRepository<Upload, UUID>` is an outbound port (the
business logic calls `.save()`/`.findById()` against an interface; Spring Data generates the real
implementation at startup). `ObjectStorageClient` (§6) is the other outbound port in this
repository — the business logic calls `.put()`/`.get()`/`.presignPut()` against an interface it
owns, never against a concrete storage SDK class.

**The dependency rule that matters:** `service/` classes import `repository/` interfaces and
`common-library`'s `ObjectStorageClient` interface — never a concrete Hibernate/JPA class beyond
what Spring Data generates, never a concrete AWS-SDK class. Infrastructure depends inward on the
business logic's interfaces; the business logic never depends outward on a specific
infrastructure choice. That's the entire hexagonal principle, realized here without the
ceremony of separate Maven modules per layer (the trade-off explained in §8).

---

## 6. Strategy pattern + dependency inversion: swappable infrastructure behind an interface

This is the concrete mechanism that makes "change MinIO for a different object store without
touching business logic" actually true, not just an aspiration.

```
              ┌───────────────────────────┐
              │   ObjectStorageClient       │   ← interface, owned by common-library,
              │   (interface)               │      imported by every service's business logic
              ├───────────────────────────┤
              │ + put(key, data, ...)       │
              │ + get(key)                  │
              │ + presignPut(key, ...)      │
              │ + exists(key)               │
              └───────────▲───────────────┘
                          │ implements
              ┌───────────┴───────────────┐
              │  S3ObjectStorageClient      │   ← the ONLY class that knows this is MinIO/S3
              │  (concrete implementation)  │      (uses the AWS SDK's S3Client/S3Presigner)
              └───────────────────────────┘
```

Business logic (`UploadService`) is constructed with the *interface* as a constructor parameter:
```java
public UploadService(UploadRepository uploadRepository, ..., ObjectStorageClient objectStorageClient) {
    this.objectStorageClient = objectStorageClient;
}
```
It calls `objectStorageClient.presignPut(...)` and has **no idea** MinIO is on the other end of
that call — it could be AWS S3, Google Cloud Storage, Azure Blob Storage, or a local filesystem
adapter for tests. Swapping the backend means writing one new class implementing
`ObjectStorageClient` and changing which bean gets constructed — `UploadService` doesn't change
by a single line. This is the **Dependency Inversion Principle** (business logic depends on an
abstraction it owns, not on a concrete infrastructure library) combined with the **strategy
pattern** (the concrete implementation is a swappable strategy selected at configuration time,
not hard-coded).

Spring's dependency injection is *how* the swap happens without a factory or a `new` anywhere:
`common-library` ships an `@AutoConfiguration` class that builds the concrete
`S3ObjectStorageClient` and exposes it as the `ObjectStorageClient` bean, gated by
`@ConditionalOnProperty` so it only activates for services that actually configure
`insight.storage.*`. If a second implementation existed (say, a `GcsObjectStorageClient`), the
choice of which one wins would be a configuration-time decision (a Spring profile, a different
property value), never a code change in any service.

**The same pattern appears a second time, at a different seam:** `transformation-service` defines
a `FileProcessorStrategy` interface with one implementation per file type (CSV/Excel/JSON). Every
implementation is a plain `@Component`; Spring collects all of them into a
`List<FileProcessorStrategy>`, and a small registry class picks the right one at runtime by file
type. Onboarding a new file type is "write one new class implementing the interface" — zero
changes to the worker that uses it. Same principle (interface owned by the business logic,
concrete strategies swappable/extensible), applied to "which parser" instead of "which storage
backend."

---

## 7. How Helm fits in

One Helm chart per service (`helm/<service>/Chart.yaml` + `values.yaml`), because each service
scales, deploys, and gets rolled back **independently** — that's the entire point of the
one-module-one-deployable structure from §1. `values.yaml` per service captures exactly the
things that differ per environment (replica count, resource requests/limits, image tag,
autoscaling thresholds) — the same chart is used for dev/staging/prod, with a different values
file (or `--set` overrides) per environment, rather than separate charts per environment. This is
also where environment-specific configuration (§9) ultimately gets injected as environment
variables or mounted config into each pod, overriding whatever `application.yml` sets as a
default.

### 7.0 Multiple environments — the chart doesn't change, the values do

The chart's templates never fork per environment. What changes is a values *overlay*: keep the
existing `values.yaml` as the set of sane defaults, and add one small file per environment next
to it — `values-dev.yaml`, `values-staging.yaml`, `values-prod.yaml` — each overriding only what
actually differs there (replica count, resource limits, autoscaling min/max, image tag, and
environment-specific config such as datasource URLs or Kafka brokers, which in a real environment
are references into that environment's secret store, not literal values). Deploying becomes
layering the two files together:

```bash
helm upgrade --install upload-service helm/upload-service \
  -f helm/upload-service/values.yaml \
  -f helm/upload-service/values-prod.yaml \
  --namespace platform-prod
```

Paired with one Kubernetes namespace per environment (`platform-dev`, `platform-staging`,
`platform-prod`), so the same release name can exist independently in each without colliding.
`scripts/deploy.sh` already takes `NAMESPACE` as a parameter (§7.1) — supporting multiple
environments is a small extension of that (add the `values-<env>.yaml` files, pass `-f` and
`--namespace` per environment at deploy time), not a restructure of the chart itself.

## 7.1 `scripts/` — operational ergonomics, not logic

Every script here (`build.sh`, `run.sh`, `stop.sh`, `deploy.sh`, `images.sh`) is a thin wrapper —
nothing inside them isn't a plain `mvn`, `docker`, or `helm` command underneath. Their entire
purpose is muscle-memory: `./scripts/run.sh` instead of five separate `mvn -pl ... spring-boot:run`
invocations remembered by hand. They intentionally contain zero business or deployment logic that
isn't equally expressible as the underlying tool's own command.

## 7.2 `docs/` — documentation as a build artifact, not an afterthought

Structured so each concern has exactly one home: how to run the platform locally, how a specific
service's code works, a running troubleshooting log, and this architecture explanation. The
principle mirrors §1's folder-structure discipline — each doc answers one question, and links to
its siblings instead of repeating them.

## 7.3 `infra/` — local-dev-only provisioning

Contains things needed *only* to bootstrap a local environment from nothing — e.g. a script that
creates one database/role per service in a fresh Postgres container. This folder has no
equivalent in a deployed environment, where each service's database already exists, provisioned
by whatever infrastructure-as-code owns real environments. Keeping it clearly separate from
`helm/` (which *does* apply to real environments) avoids confusing "how do I get a laptop
sandbox running" with "how does this actually deploy."

---

## 8. Configuration management

Every service reads configuration the same way, via `application.yml` plus Spring's
`@ConfigurationProperties` binding (the mechanism `ObjectStorageProperties` uses, §6):

```yaml
insight:
  storage:
    endpoint: http://localhost:9000
    bucket: platform-uploads
```
maps directly onto typed Java fields (`getEndpoint()`, `getBucket()`) — no manual parsing
anywhere. Locally, `application.yml` holds real values (pointing at `docker-compose.yml`'s
containers, §10). In a deployed environment, the same key names get overridden by environment
variables or mounted config injected via the Helm chart (§7) — `application.yml`'s values become
defaults, not the source of truth in production. Secrets (the storage access key/secret in this
repo, database passwords) are placeholder plaintext values *only* in the local `application.yml` —
in any real environment these would come from a secrets manager or the platform's native secret
store, injected the same way as any other environment override, never committed as real
credentials.

---

## 9. What `docker-compose.yml` actually buys you

One command (`docker compose up -d`) reproduces every piece of infrastructure every service
needs locally — message broker, per-service databases, object storage — without any engineer
hand-installing or hand-configuring any of it. Concretely:

- **Onboarding speed** — a new engineer runs one command instead of following a multi-page setup
  guide for Kafka/Postgres/object storage individually.
- **Environment parity** — the same broker/database/storage *technology* runs locally as in a
  real environment (Kafka is Kafka, Postgres is Postgres) — the topology is simplified (single
  broker node, one Postgres instance for all 5 databases) but the interfaces the code talks to
  are the real ones, not in-memory fakes, so integration bugs surface locally instead of only in
  a shared environment.
- **Disposability** — `docker compose down -v` throws away all local state and the next
  `docker compose up -d` starts from a guaranteed-clean slate — valuable specifically because the
  init scripts in `infra/` (§7.3) only run once, against an empty volume.

---

## 10. The outbox pattern

**The problem it solves:** a service's own database and a message broker are two different
systems. Nothing makes "commit this database row" and "publish this message" atomic for free. Do
them as two separate calls in either order and you risk either a phantom message (the publish
succeeds, then the database transaction rolls back) or a silently lost one (the database commits,
then the publish fails).

**The mechanism:**
```
┌─────────────────────────── one database transaction ───────────────────────────┐
│  1. INSERT the domain row (e.g. mark a submission accepted)                     │
│  2. INSERT an outbox row: {topic, key, payload, published=false}                │
└──────────────────────────────────────────────────────────────────────────────────┘
                                     │
                                     │  (both committed together, or neither did)
                                     ▼
                  ┌─────────────────────────────────────┐
                  │  a separate @Scheduled poller,        │   runs every ~1s
                  │  independent of any request/transaction│
                  └─────────────────┬─────────────────────┘
                                    │ SELECT ... WHERE published = false
                                    ▼
                     publish to the message broker,
                     then UPDATE the row: published = true
```
The domain write and the "I intend to publish this" write happen in the same transaction — so
they're guaranteed consistent with each other with no special coordination needed. The actual
publish happens moments later, by a completely separate piece of code that only ever asks "which
outbox rows are unpublished" — if that poller crashes mid-run, the next tick simply finds the same
unpublished rows and retries; nothing is lost, and nothing gets double-committed, because the
domain-side transaction already fully committed before the poller ever looks at it.

**Trade-off accepted:** a small publish delay (bounded by the poller's interval) in exchange for
never losing or fabricating an event relative to what's actually in the database.

---

## 11. How `architecture-tests` enforces strict layering at build time

Because the packages are flat (§1, §5) rather than split into separate Maven modules, nothing
stops a careless change from having a controller call a repository directly — *unless* something
checks for it. `architecture-tests` is that check: a small ArchUnit test suite, run against the
**compiled classes** of every service, as a normal part of `mvn test`:

```java
@Test
void controllersMustNotAccessRepositoriesDirectly() {
    ArchRule rule = classes()
            .that().resideInAPackage("..controller..")
            .should().onlyAccessClassesThat().resideOutsideOfPackage("..repository..");
    rule.check(classes);
}

@Test
void entitiesMustNotBeAccessedFromControllers() {
    ArchRule rule = classes()
            .that().resideInAPackage("..entity..")
            .should().onlyBeAccessed().byClassesThat()
            .resideInAnyPackage("..entity..", "..repository..", "..service..", "..mapper..");
    rule.check(classes);
}
```
If someone adds `uploadRepository.findById(...)` directly inside a `@RestController` method
tomorrow, this test suite fails the build — not a code review comment that can be missed, an
actual red build. This is what lets the repository stay single-module-per-service (§5's trade-off)
without that convenience quietly eroding into a tangle over time: the architectural rule is
enforced the same way a unit test enforces correctness — automatically, on every build, for
every service, from one shared test suite.

---

## 12. Putting it together: the orchestrator saga, built on the outbox pattern

Briefly, tying §10 directly to the cross-service coordination problem from the very top of this
document:

1. A service accepts a unit of work, and — in one transaction — persists it *and* writes an
   outbox row (§10). The outbox relay publishes that event moments later.
2. An **Orchestrator** service consumes that event, creates a `SagaInstance` row (its own
   database, its own transaction — no shared transaction with the service that emitted the
   event), and dispatches the first processing step via a synchronous, typed call (gRPC) to the
   relevant processing service.
3. Each processing service, when it finishes (or fails), publishes its own completion event —
   again via its own outbox, for the same atomicity guarantee.
4. The Orchestrator consumes each completion event and advances the saga's state machine,
   dispatching the next step (conditionally skipping stages that don't apply), until the saga
   reaches a terminal state.

**Trade-offs accepted, explicitly:**
- **Latency vs. correctness** — every cross-service step incurs the outbox relay's polling delay
  (§10) on top of the actual processing time. Accepted because the alternative (publish directly,
  no outbox) risks losing or fabricating saga-critical events, which is a worse failure mode than
  a bounded, small delay.
- **A coordinator becomes a critical path** — the Orchestrator's availability and throughput now
  gate the whole pipeline's progress, which a pure choreography (services reacting directly to
  each other's events, no central coordinator) would avoid. Accepted because centralized status
  visibility and centralized compensation/rollback logic were requirements this pattern is
  specifically solving for — the coordinator's cost is the price of that visibility.
- **Eventual, not immediate, consistency across services** — at any given instant, the
  Orchestrator's view of saga state and a processing service's view of its own job may be
  momentarily out of sync (an event published but not yet consumed). Accepted because the
  alternative — a distributed transaction spanning every service's database — isn't available
  once each service owns its own database, which is itself a requirement for independent
  deployability.

Each of these is a deliberate choice with a named alternative and a stated reason it was
rejected — not an oversight to be discovered later.
