# Insight Data Upload Platform — Proposed Architecture

**Status:** Proposal, for architecture review
**Scope:** Repository/module structure, service boundaries, transport design, and current
implementation status of the file-upload processing platform.

This document walks through *why* the repository is shaped the way it is, module by module, and
is honest about what's fully built versus scaffolded for future work — the intent is to get
sign-off on the structure and the key decisions below, not to present a finished system.

---

## 1. Problem statement (recap)

Users upload CSV/Excel/JSON files, some containing millions of rows, some containing references
to scanned documents. The platform must: accept the upload without blocking on processing, track
status through a defined lifecycle, capture row-level validation errors without failing the
whole file, scale horizontally, tolerate retries safely, and let a downstream reporting system
pull results. New file types must be addable without reshaping the core pipeline.

## 2. High-level shape: five bounded contexts, one saga

```
                          ┌────────────────────┐
   client ───REST──────►  │   upload-service    │  owns: Upload, its own Postgres DB
                          └─────────┬───────────┘
                                    │ Kafka (upload.events.received)
                                    ▼
                          ┌────────────────────┐
                          │ orchestrator-service│  owns: SagaInstance — the saga's
                          │   (saga coordinator) │  single source of truth
                          └───┬────────────┬────┘
                        gRPC  │            │ gRPC
                              ▼            ▼
                  ┌───────────────┐   ┌───────────────┐
                  │transformation-│   │  ocr-service   │
                  │   service      │   │ (conditional)  │
                  └───────┬───────┘   └───────┬───────┘
                          │  Kafka (events)    │
                          └─────────┬──────────┘
                                    ▼
                          ┌────────────────────┐
                          │ reporting-service   │  owns: UploadReport, ErrorSummary
                          └────────────────────┘
```

Each box is an **independent Maven module, an independent Spring Boot deployable, and an
independent Postgres database** — no shared schema, no cross-service transactions. That
constraint is exactly why `orchestrator-service` exists: cross-service consistency is achieved
through an explicit saga, not a distributed transaction.

---

## 3. Repository walkthrough

```
insight-data-upload-platform/
├── pom.xml                      ← parent: module list + shared dependency versions
├── docker-compose.yml           ← local infra: Postgres, Kafka, Kafka UI, MinIO
├── .gitignore
│
├── common-library/              ← shared jar — real reuse across all 5 services
├── protos/                      ← shared gRPC contracts — real reuse across all 5 services
│
├── upload-service/              ← 1 module = 1 deployable = 1 database, each
├── transformation-service/
├── ocr-service/
├── orchestrator-service/
├── reporting-service/
│
├── architecture-tests/          ← ArchUnit — enforces layering across all 5 services
│
├── helm/<service>/               ← one Helm chart per service (independent scaling/deploy)
├── infra/postgres-init/          ← local-only: creates the 5 per-service databases
├── scripts/                      ← build/run/stop/deploy/images helpers
└── docs/                         ← this file and its siblings
```

**Why only two shared modules.** `common-library` and `protos` are the *only* code shared across
service boundaries, and both earn that status the same way: they're genuinely consumed by
multiple independent deployables. Nothing else is shared — no shared "core domain" module, no
shared "utils" grab-bag beyond what's in `common-library`. Every other cross-cutting concern
(entities, DTOs, exceptions) is deliberately duplicated per service rather than centralized,
because centralizing them would quietly recreate a distributed monolith — a change to a "shared"
`Upload`-like class would ripple across services that are supposed to be independently
deployable.

### 3.1 `common-library/` — what's actually in it

```
common-library/src/main/java/com/insight/common/
├── dto/        ApiError, PageResponse           — shared response shapes
├── exception/  BaseException, ResourceNotFoundException  — shared exception hierarchy
├── security/   RequestContext                  — placeholder for auth/correlation propagation
├── util/       IdempotencyKeys
└── storage/    ObjectStorageClient (interface) + S3ObjectStorageClient (impl)
              + ObjectStorageProperties + ObjectStorageAutoConfiguration
```
The `storage` package is the one with real design weight: it's a Spring Boot **auto-configuration**
(`@AutoConfiguration`, registered via
`META-INF/spring/org.springframework.boot.autoconfigure.AutoConfiguration.imports`), gated by
`@ConditionalOnProperty(prefix = "insight.storage", name = "endpoint")` so it only activates for
the services that actually configure it (`upload-service`, `transformation-service`) — `ocr-service`,
`orchestrator-service`, and `reporting-service` also depend on this jar (for `ApiError`/exceptions)
but don't get an unwanted S3 client forced on them. Full walkthrough:
[`docs/upload-service.md`](upload-service.md) §2.

### 3.2 `protos/` — the shared contract layer

```
protos/src/main/proto/
├── common/v1/common.proto
├── upload/v1/upload_service.proto
├── transformation/v1/transformation_service.proto
├── ocr/v1/ocr_service.proto
├── orchestrator/v1/orchestrator_service.proto
└── reporting/v1/reporting_service.proto
```
One `.proto` package per bounded context, each versioned independently (`v1`) so a breaking
change to one service's contract doesn't force a version bump on the others. Compiled once
(`mvn -pl protos install`), consumed by every service as a normal Maven dependency — no service
maintains its own copy of another service's contract. Full explanation of the codegen mechanics:
[`docs/upload-service.md`](upload-service.md) §3, [`docs/runbook.md`](runbook.md) Step 3.

### 3.3 Each service module — one consistent internal shape

```
<service>/src/main/java/com/insight/<pkg>/
├── controller/    REST inbound adapters
├── dto/           request/response + Kafka event shapes
├── entity/        JPA entities (this service's aggregate)
├── exception/      + GlobalExceptionHandler (@RestControllerAdvice)
├── mapper/         entity ↔ DTO translation
├── repository/     Spring Data JPA interfaces
└── service/        use cases, gRPC server classes, Kafka listeners/producers
```
Flat, not hexagonal-by-folder (no `port/`, `adapter/`, `domain/` subfolders). This was a
deliberate simplification, made explicit in review: see §5.1.

### 3.4 `architecture-tests/`

One ArchUnit test class, run against the compiled classes of all 5 services, enforcing e.g.
"controllers must not access repositories directly." This is what stands in for the compile-time
layering guarantees a full hexagonal multi-module split would otherwise provide — see §5.1.

### 3.5 Deployment-facing folders

`helm/<service>/` — one chart per service, each with its own `replicaCount`/autoscaling, because
each service scales independently. `scripts/` — thin wrappers, nothing they do isn't a plain
`mvn`/`docker`/`helm` command underneath (see [`docs/runbook.md`](runbook.md) for the exact
commands). `infra/postgres-init/` is local-dev-only — it doesn't exist in any deployed
environment, where each service's database already exists.

---

## 4. The saga, transport by transport

| Interaction | Transport | Why |
|---|---|---|
| Client → `upload-service` | REST | External-facing, needs to be broadly callable |
| `upload-service` → object storage | Presigned URL (client PUTs directly) | See §5.3 |
| `upload-service` → `orchestrator-service` | Kafka (`upload.events.received`) | Async, durable, decouples producer from consumer uptime |
| `orchestrator-service` → participants (dispatch) | gRPC | Synchronous accept/reject before committing to a saga step; typed contract |
| Participants → `orchestrator-service` (completion) | Kafka (`*.events.completed`/`.failed`) | Same durability/decoupling rationale as above |
| `reporting-service` → `transformation-service`/`ocr-service` (query) | gRPC | Point-to-point read, no durability needed |

Every participant service (`transformation-service`, `ocr-service`) follows the same internal
pattern once a gRPC command lands: accept + idempotency-check + persist immediately, hand the
actual work off to an **internal** Kafka topic (`transformation.internal.start`, not saga-facing),
and let a separate `@KafkaListener` do the slow work — so the gRPC call itself never blocks on
file processing, and processing scales with however many pods are running, independent of
inbound call volume. Full mechanics: [`docs/transformation-service.md`](transformation-service.md) §2–3.

---

## 5. Key decisions, and the reasoning behind each

### 5.1 Single Maven module per service, not a hexagonal multi-module split

**Decision:** each service is one Maven module with flat packages, not split into
`domain`/`application`/`infrastructure` sub-modules.

**Why:** the domain complexity per pipeline stage is narrow enough that multi-module ceremony
(a `pom.xml` per layer, cross-module refactors for every change) isn't worth its cost here. The
compile-time layering guarantee a hexagonal split buys you is instead recovered with ArchUnit
(§3.4) — most of the safety, none of the ceremony.

**Where this could be revisited:** if any one service's domain logic grows substantially more
complex than the others (a strong candidate: `orchestrator-service`, once the saga's
compensation logic is fully built out), splitting *that one* service — not all five — would be
the natural next step, not a wholesale change to the convention.

### 5.2 Orchestration saga, not choreography

**Decision:** `orchestrator-service` is a dedicated coordinator holding the saga state machine,
rather than each service reacting to each other's events directly.

**Why:** the platform has an explicit status-tracking requirement
(`RECEIVED → VALIDATING → PROCESSING → COMPLETED/FAILED`) — orchestration gives one place that
answers "where is this upload right now," and centralizes compensation logic instead of
scattering failure-handling across every participant. Trade-off accepted: the orchestrator sits
on the critical path; mitigated by keeping it stateless-per-instance, scaling it like every other
service, and partitioning Kafka consumption by `uploadId`.

### 5.3 Presigned URLs, not proxying file bytes through the app tier

**Decision:** `upload-service` never touches file bytes. It hands the client a presigned MinIO/S3
URL; the client PUTs directly to object storage; the client then calls back to confirm.

**Why:** the original design proxied the multipart upload through `upload-service` into MinIO.
Two concrete problems surfaced under review: (a) a slow, network-bound object-storage write was
happening *inside* a `@Transactional` method, holding a database connection for the entire
transfer duration; (b) every byte of a multi-gigabyte file was traversing the app server twice
(client→app, then app→storage) for no benefit. The presigned-URL flow removes both — full
before/after and the request-flow diagram: [`docs/PRESIGNED_UPLOADS.md`](PRESIGNED_UPLOADS.md).

### 5.4 Transactional outbox, not "just publish to Kafka"

**Decision:** writes that need to be atomic with a Kafka publish (e.g. "mark upload RECEIVED and
notify the saga") go through an `outbox_events` table in the same DB transaction, relayed to
Kafka by a separate `@Scheduled` poller, rather than calling `kafkaTemplate.send(...)` directly
inside the transactional method.

**Why:** a database write and a Kafka publish are two different systems — nothing makes them
atomic for free. Publishing directly risks either a phantom event (DB rollback after Kafka
already sent) or a silently lost one (Kafka send fails after DB commit). The outbox pattern
accepts a small delay (the relay polls every second) in exchange for a durability guarantee: if
the row committed, the event *will* eventually be published, exactly once from this service's
perspective.

### 5.5 Idempotency strategy: natural keys, not a separate idempotency-key table

**Decision:** every Kafka listener and gRPC command handler checks "does this identifier already
have a record" (by `uploadId`, or `uploadId + sagaId`) before doing any work, rather than
maintaining a dedicated `processed_commands` table with generated idempotency keys.

**Why:** every entity in this system is already keyed by something that's a natural, stable
dedupe key (`uploadId` per saga/job) — a separate idempotency table would be tracking information
the domain tables already contain. Revisit if a future participant's "has this happened" check
can't be expressed as a simple existence lookup.

### 5.6 Java 21 virtual threads for I/O overlap, not for request-handling alone

**Decision:** `transformation-service`'s row-processing worker explicitly submits batched
database writes to `Executors.newVirtualThreadPerTaskExecutor()`, so batch persistence overlaps
with continued file parsing, in addition to the platform-wide
`spring.threads.virtual.enabled: true` opt-in.

**Why:** virtual threads are cheap enough to use per-batch without a tuned pool size, and this is
the actual mechanism behind the "horizontally scalable, large-file-safe" requirement at the
single-instance level — full detail: [`docs/transformation-service.md`](transformation-service.md) §5.

---

## 6. Current implementation status — what's real vs. scaffolded

Presented directly because an architecture review should see this, not discover it later.

| Capability | Status |
|---|---|
| Upload via presigned URL (initiate → PUT → complete) | **Done**, verified end-to-end |
| Outbox → Kafka → saga creation | **Done**, verified end-to-end |
| `transformation-service`: CSV/JSON/Excel streaming parse, row validation, batching | **Done**, verified end-to-end (confirmed against a real file, including a caught validation error) |
| gRPC dispatch: orchestrator → transformation-service | **Done** |
| Saga advancing past "transformation dispatched" (consuming `transformation.events.completed`, conditionally dispatching OCR, then reporting) | **Not built.** Saga currently stalls at `VALIDATING` after real transformation success |
| `ocr-service` accept-and-persist path | **Done** (mirrors transformation-service's gRPC accept pattern) |
| `ocr-service` actual extraction worker | **Not built** — no internal-topic worker exists yet; jobs stay `IN_PROGRESS` forever |
| `reporting-service` gRPC server/client scaffolding | **Done** (both directions configured) |
| `reporting-service.generateReport` actual logic | **Not built** — currently a stub returning `"ACCEPTED"` with no persistence |
| ArchUnit layering enforcement | **Done**, 2 passing rules |
| Local dev environment (docker-compose: Postgres/Kafka/MinIO) | **Done** |
| Helm charts | Scaffolded, not deployed/tested against a real cluster |

Full detail per service, including exact code pointers for what's missing: the "honesty" sections
in [`docs/ocr-service.md`](ocr-service.md) §3, [`docs/orchestrator-service.md`](orchestrator-service.md) §6,
and [`docs/reporting-service.md`](reporting-service.md) §3.

---

## 7. Open questions for this review

1. **Does the single-module-per-service decision (§5.1) hold**, or does the architecture board
   want hexagonal module boundaries enforced physically (separate Maven modules) rather than via
   ArchUnit, for one or more of these services?
2. **Is orchestration the right saga style long-term** (§5.2), given the orchestrator becomes a
   scaling/availability-critical path — or should choreography be reconsidered for the
   OCR/reporting legs specifically, since they're already event-driven at the edges?
3. **Presigned URLs (§5.3) require clients to talk to MinIO/S3 directly** — does this hold up
   against network topology constraints (e.g., clients on a network that can reach
   `upload-service` but not object storage directly)? If not, a fallback proxy-upload path may be
   needed for those cases.
4. **Priority order for the three "not built" gaps** in §6 — orchestrator's remaining saga steps,
   the OCR worker, and `reporting-service.generateReport` — which unblocks the most value first?
5. **`common-library`'s scope** — is `storage` (S3/MinIO) the right thing to keep centralizing
   here, or should it move to a dedicated `storage-client` module now that it's grown beyond
   simple DTOs/exceptions?
