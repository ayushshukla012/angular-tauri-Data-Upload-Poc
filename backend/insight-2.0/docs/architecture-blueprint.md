# Reusable Architecture Blueprint: Orchestrated-Saga Platform for Asynchronous, Multi-Stage Processing

**Status:** Proposal, for architecture review
**Scope:** A domain-agnostic Maven multi-module repository pattern for building asynchronous
processing pipelines — independently deployable Spring Boot services, coordinated by an explicit
saga, communicating over gRPC and Kafka. A concrete reference implementation of this exact
pattern is cited at the end.

---

## 1. What problem this pattern solves

Any platform with this shape is a candidate:

- An external actor submits a unit of work (a file, an order, a claim, a document — anything).
- That unit of work needs to pass through **multiple independent processing stages**, potentially
  owned by different teams or bounded contexts.
- Some stages are **conditional** — not every submission needs every stage.
- The submitter needs **status visibility** through a defined lifecycle, not just a final
  success/failure.
- Partial failure at the record/item level should be captured and reported, not treated as a
  whole-submission failure.
- Every stage must **scale independently** and tolerate retries/redelivery without corrupting
  state or double-processing.
- A downstream system needs to query **aggregated results** once processing completes.

If a system needs most of these properties, this blueprint is directly applicable regardless of
what the "unit of work" actually is.

## 2. The generic shape: N bounded contexts, one saga

```
                          ┌────────────────────┐
   client ───REST──────►  │  Ingestion Service   │  owns: the submitted unit of work,
                          └─────────┬───────────┘  its own database
                                    │ Kafka (submission.events.received)
                                    ▼
                          ┌────────────────────┐
                          │ Orchestrator Service │  owns: SagaInstance — the saga's
                          │   (saga coordinator) │  single source of truth
                          └───┬────────────┬────┘
                        gRPC  │            │ gRPC
                              ▼            ▼
                  ┌───────────────┐   ┌───────────────┐
                  │ Processing     │   │ Processing     │
                  │ Service A      │   │ Service B      │
                  │ (always runs)  │   │ (conditional)  │
                  └───────┬───────┘   └───────┬───────┘
                          │  Kafka (events)    │
                          └─────────┬──────────┘
                                    ▼
                          ┌────────────────────┐
                          │ Aggregation Service  │  owns: the final report/result,
                          └────────────────────┘  queryable by downstream consumers
```

Each box is an **independent Maven module, an independent deployable, and an independent
database** — no shared schema, no cross-service transactions. That constraint is exactly why the
Orchestrator Service exists: cross-service consistency is achieved through an explicit saga, not
a distributed transaction. The number and identity of "Processing Service" boxes is
domain-specific — the pattern holds for one, two, or many, run in sequence or in parallel,
conditionally or unconditionally.

---

## 3. Repository structure

```
platform/
├── pom.xml                      ← parent: module list + shared dependency versions
├── docker-compose.yml           ← local infra (message broker, databases, object storage, etc.)
│
├── common-library/              ← shared jar — only for things genuinely reused by every service
├── protos/                      ← shared gRPC contracts — one .proto package per bounded context
│
├── ingestion-service/            ← 1 module = 1 deployable = 1 database, each
├── processing-service-a/
├── processing-service-b/
├── orchestrator-service/
├── aggregation-service/
│
├── architecture-tests/          ← ArchUnit — enforces layering across every service
│
├── helm/<service>/               ← one chart per service (independent scaling/deploy)
├── scripts/                      ← build/run/stop/deploy helpers
└── docs/
```

**Shared-module policy:** exactly two modules are shared, and both earn that status the same
way — genuine reuse across multiple independent deployables. `common-library` holds cross-cutting
concerns (error-response shapes, a shared exception hierarchy, any cross-service infrastructure
client such as an object-storage or notification client). `protos` holds gRPC contracts, one
package per bounded context, each independently versioned (`v1`, `v2`, ...) so a breaking change
to one service's contract doesn't force a version bump on the others.

Everything else is deliberately **not** shared — no shared "core domain" module, no shared entity
definitions. Centralizing those would quietly recreate a distributed monolith: a change to a
shared domain class would ripple across services that are supposed to deploy independently.

### 3.1 Internal shape of every service module

```
<service>/src/main/java/com/<org>/<service>/
├── controller/    inbound adapters (REST, or none if the service is internal-only)
├── dto/           request/response + event payload shapes
├── entity/        JPA entities — this service's own aggregate(s)
├── exception/      + a GlobalExceptionHandler
├── mapper/         entity ↔ DTO translation
├── repository/     Spring Data JPA interfaces
└── service/        use cases, gRPC server/client classes, message listeners/producers
```

Flat, not hexagonal-by-folder — no `port/`/`adapter/`/`domain/` subdivision. See the trade-off
in §4.1.

---

## 4. Key decisions and the reasoning behind each

### 4.1 Single Maven module per service, layering enforced by static analysis

**Decision:** each service is one Maven module with a flat package layout, not split into
`domain`/`application`/`infrastructure` sub-modules.

**Reasoning:** a full hexagonal multi-module split buys compile-time proof that, say, a
controller can never directly touch a repository — but it costs a `pom.xml` per layer and slows
every cross-layer refactor. Where a service's domain logic is narrow (most pipeline stages are:
accept work, validate/transform it, report a result), that cost isn't justified. A static
analysis tool (ArchUnit in the Java/Spring ecosystem) recovers most of the same guarantee as a
build-time test instead of a module boundary.

**When to revisit:** if any single service's domain logic grows substantially more complex than
its siblings — most often the Orchestrator, once its compensation/rollback logic matures — split
*that one service*, not the whole platform, into a proper hexagonal module set.

### 4.2 Orchestration saga, not choreography

**Decision:** a dedicated Orchestrator Service holds the saga state machine and explicitly
dispatches each step, rather than every service reacting to every other service's events.

**Reasoning:** orchestration gives one place that can answer "where is this submission right
now" — essential whenever the platform has a status-tracking requirement — and centralizes
compensation/rollback logic instead of scattering failure-handling logic across every
participant. The accepted trade-off: the orchestrator sits on the critical path and becomes a
scaling/availability-sensitive component. Mitigation: keep it stateless-per-instance, scale it
like every other service, and partition message consumption by the submission's own ID so
ordering per-submission is preserved across orchestrator replicas.

**When choreography fits better instead:** highly linear pipelines with few steps and no need for
centralized status visibility, where the coupling cost of a coordinator outweighs its benefit.

### 4.3 Direct-to-resource handoff for large payloads, not proxying through the app tier

**Decision:** when a submission includes a large binary payload (a file, an image, a large
document), the Ingestion Service never touches the bytes. It issues a short-lived, signed URL
(e.g. an S3/MinIO presigned URL) that the client uses to write directly to the storage backend;
the Ingestion Service is then notified out-of-band (a confirmation call, a storage-side event
notification, or a webhook) that the write completed.

**Reasoning:** proxying large payloads through an application server costs I/O twice (client→app,
then app→storage) for no benefit, ties up an application-tier connection for the full transfer
duration, and tempts slow, network-bound calls into being wrapped inside database transactions
(exactly the mistake this pattern avoids — see the next decision). This generalizes beyond file
uploads: any large-payload handoff in a distributed system benefits from letting the two
endpoints that actually need the bytes talk directly, with the application tier only brokering
*permission*, not *data*.

**Applicability caveat:** requires the client to be able to reach the storage backend directly
(network topology, firewalls, or CORS policies permitting). Where that's not guaranteed, a
proxy-upload fallback path may still be needed for a subset of clients.

### 4.4 Transactional outbox for anything that must be atomic with a message publish

**Decision:** whenever a database write must be atomic with a message-broker publish (e.g. "mark
this record accepted and notify the saga"), write an outbox row in the *same* database
transaction, and relay it to the broker via a separate scheduled poller — never publish directly
from inside the transactional method.

**Reasoning:** a database and a message broker are two different systems; nothing makes a write
to one and a publish to the other atomic for free. Publishing directly risks either a phantom
message (broker send succeeds, then the DB transaction rolls back) or a silently lost one (DB
commits, then the broker send fails). The outbox pattern trades a small delay (the relay polls on
an interval) for a durability guarantee: if the row committed, the message *will* eventually be
published — a foundational pattern for any event-driven system with this correctness
requirement, independent of domain.

### 4.5 Idempotency via natural keys, not a dedicated idempotency-key table

**Decision:** every message listener and inter-service command handler checks "does a record for
this identifier already exist" before doing any work, using the domain's own natural key (a
submission ID, a job ID) rather than maintaining a separate `processed_commands`-style ledger.

**Reasoning:** applicable whenever every handler's "has this already happened" question can be
answered by a simple existence check against a table the domain already needs. Simpler than
maintaining a parallel idempotency ledger, with one caveat: if a future handler's idempotency
condition can't be expressed as a plain existence lookup (e.g., it depends on partial progress
within a single unit of work), a dedicated idempotency-key mechanism becomes the correct choice
for that handler specifically — this isn't an all-or-nothing platform rule.

### 4.6 Async worker hand-off inside each processing service

**Decision:** a processing service's synchronous entry point (its gRPC command handler) does the
minimum necessary — idempotency check, persist a job record — then hands the actual work to an
**internal-only** message topic (not part of the saga's public contract), consumed by a separate
listener that does the real, potentially slow work.

**Reasoning:** keeps the synchronous call fast and non-blocking regardless of how long the actual
processing takes, and lets processing scale independently of inbound call volume — running more
replicas of a processing service scales both its ability to *accept* work and its ability to
*perform* it, without one bottlenecking the other. On the JVM specifically, lightweight
concurrency primitives (Java 21 virtual threads, or the equivalent in another stack) let the
worker overlap slow I/O (e.g. persisting results) with continued processing of the next unit of
work, without needing a hand-tuned thread pool.

---

## 5. A framework for evaluating "is this stage actually done"

Useful as a standing checklist during any implementation of this pattern, regardless of domain:

| Question | Why it matters |
|---|---|
| Does the synchronous entry point (REST or gRPC) return before the real work finishes? | Confirms the async hand-off (§4.6) is real, not just structured to look async |
| Does a redelivered message/retried call produce the same end state, not a duplicate? | Confirms idempotency (§4.5) actually holds under retry |
| Does a database-write-plus-publish operation survive a crash between the two? | Confirms the outbox pattern (§4.4) is wired correctly, not just present in one service |
| Does the saga actually advance past the first dispatched step when that step succeeds? | The most common half-finished state in an orchestrated saga — easy to build the first dispatch and defer the rest |
| Can a downstream aggregation/reporting stage answer a query with zero real data, cleanly (a proper "not found," not a crash)? | Confirms the read side isn't silently coupled to the write side being fully built |

---

## 6. Reference implementation

This pattern was prototyped end-to-end as the **Insight Data Upload Platform** — a file-ingestion
pipeline where the "unit of work" is an uploaded CSV/Excel/JSON file, "Processing Service A" is
mandatory transformation/validation, "Processing Service B" is conditional OCR extraction, and
the Aggregation Service produces a final upload report with an error summary.

| Generic role | Concrete service |
|---|---|
| Ingestion Service | `upload-service` |
| Orchestrator Service | `orchestrator-service` |
| Processing Service A (mandatory) | `transformation-service` |
| Processing Service B (conditional) | `ocr-service` |
| Aggregation Service | `reporting-service` |

The presigned-URL handoff (§4.3), the outbox pattern (§4.4), the internal-worker hand-off (§4.6),
and the ArchUnit-enforced layering (§4.1) are all implemented and verified end-to-end in that
repository. Full detail, including exact code walkthroughs, current build status, and an honest
accounting of what's still scaffolded versus fully wired: see
[`docs/proposed_plan.md`](proposed_plan.md) and its linked per-service docs
(`docs/upload-service.md`, `docs/transformation-service.md`, `docs/ocr-service.md`,
`docs/orchestrator-service.md`, `docs/reporting-service.md`).
