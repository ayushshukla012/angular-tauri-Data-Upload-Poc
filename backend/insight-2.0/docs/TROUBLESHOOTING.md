# Troubleshooting log

Real bugs hit while first bringing the stack up locally, recorded here so a future run (or a
future contributor) doesn't have to rediscover them. Newest entries at the bottom.

- **Bitnami Kafka image tag unavailable.** `bitnami/kafka:3.7` stopped resolving (Bitnami
  deprecated most versioned tags for free use). Switched `docker-compose.yml` to the official
  `apache/kafka:3.7.0` image.
- **Postgres port collision with a native install.** A locally-installed Postgres was already
  bound to `127.0.0.1:5432`, which intercepted connections meant for the Docker container
  (Docker's proxy binds `0.0.0.0`/`::`, but `localhost` resolves to the loopback address the
  native process owns first). Remapped the container to host port `5433` in
  `docker-compose.yml` and updated all 5 `application.yml` datasource URLs to match. If you
  don't have a conflicting local Postgres, you can safely move this back to `5432:5432`.
- **`ocr-service` gRPC port collided with Kafka.** Both were assigned port `9092`. Moved
  `ocr-service`'s gRPC server to `9096` and updated its two callers
  (`orchestrator-service`, `reporting-service`) to match.
- **Flyway 10 + Postgres needs a separate module.** Flyway 10 split database-specific support
  out of `flyway-core`. Without `flyway-database-postgresql` on the classpath, every service
  failed at startup with `FlywayException: Unsupported Database: PostgreSQL 16.x`. Added the
  dependency to all 5 service `pom.xml` files.
- **`@Lob String` maps to the wrong Postgres column type.** Hibernate maps `@Lob` on a
  `String` field to `oid` (Postgres large object) by default, but the Flyway migration created
  a plain `TEXT` column for `outbox_events.payload`. Dropped `@Lob` in favor of
  `columnDefinition = "TEXT"`, which matches a `TEXT` column without the large-object indirection.
- **Shared `ObjectStorageAutoConfiguration` broke services that don't use it.**
  `common-library`'s S3/MinIO client auto-configured unconditionally, so `ocr-service`,
  `orchestrator-service`, and `reporting-service` — which pull in `common-library` for its
  DTOs/exceptions but never touch file storage — crashed on startup with a `NullPointerException`
  building an `S3Client` from unset properties. Added
  `@ConditionalOnProperty(prefix = "insight.storage", name = "endpoint")` so it only activates
  where `insight.storage.*` is actually configured.
- **GET-by-id endpoints returned `500` instead of `200`/`404`.** Spring couldn't resolve
  `@PathVariable String uploadId` (and similar) to the literal name `uploadId` via reflection —
  `IllegalArgumentException: ... Ensure that the compiler uses the '-parameters' flag.` Added
  `<parameters>true</parameters>` to the `maven-compiler-plugin` in the parent `pom.xml` so
  `javac` keeps parameter names in the bytecode for every module, instead of annotating every
  `@PathVariable`/`@RequestParam` by hand.
- **`OutboxRelayPublisher` never ran.** Its `relay()` method is `@Scheduled`, but
  `UploadServiceApplication` was missing `@EnableScheduling` — Spring silently ignores
  `@Scheduled` methods without it, so `upload.events.received` rows sat in the `outbox_events`
  table forever with `published = false`. Added `@EnableScheduling` to the application class.
- **Orchestrator crashed parsing its own event.** `UploadReceivedListener` treated the entire
  Kafka message value (a JSON payload) as a raw UUID string —
  `IllegalArgumentException: UUID string too large`. After 9 retries the message was dropped and
  no saga was ever created. Fixed by deserializing the payload into an `UploadReceivedEvent`
  record with Jackson before pulling `uploadId` out of it.

## A recurring one worth knowing about: flaky `protos` module rebuilds

Several times during setup, a full `mvn clean install` failed with errors like
`cannot find symbol: class SomeServiceGrpc` or `error while writing ...: Invalid argument`,
always pointing at generated code in the `protos` module — even though nothing about the
`.proto` files had changed. This looks like an environment-specific staleness issue in how
`protobuf-maven-plugin`'s generated sources interact with `maven-compiler-plugin`'s output,
not a real code defect (the same source always compiled fine on a second attempt).

**If you hit this:** build `protos` alone first, clean, before building anything else:
```bash
rm -rf protos/target
mvn -pl protos clean install
mvn install -DskipTests -pl common-library,upload-service,transformation-service,ocr-service,orchestrator-service,reporting-service,architecture-tests
```
