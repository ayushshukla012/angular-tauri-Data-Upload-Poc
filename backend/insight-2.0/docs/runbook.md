# Runbook: how this repository was actually built

A step-by-step record of what was typed/written by hand versus what a tool generated, and the
exact commands used. Written because "how do I build a project like this myself" is a fair
question, and the honest answer is "almost everything was handwritten — only the gRPC Java
classes are machine-generated."

**One thing that did *not* happen anywhere in this process:** no `mvn archetype:generate`, no
Spring Initializr (`start.spring.io`), no IDE "New Spring Boot Project" wizard. Every `pom.xml`,
every Java class, every YAML file was written directly. That was a deliberate choice, not a
default — a generator would have imposed its own package layout and module shape, and the whole
point was to match a specific, already-agreed-on structure (the flat
`controller/dto/entity/exception/mapper/repository/service` convention, single-module-per-service,
`protos`/`common-library` as the only shared modules).

---

## Step 1 — the root and the parent POM (manual)

```bash
mkdir -p /path/to/insight-data-upload-platform
```
Then a single `pom.xml` was written by hand at the root with `<packaging>pom</packaging>` and a
`<modules>` list. This file doesn't compile anything itself — it's the thing that tells Maven
"here are 8 sub-projects, and here's shared version/dependency management for all of them"
(`<dependencyManagement>` importing the `spring-boot-dependencies` BOM, later the AWS SDK BOM
too). `.gitignore` (`target/`, `*.class`, `.idea/`, etc.) was written by hand at the same time.

## Step 2 — `common-library` (manual)

```bash
mkdir -p common-library/src/main/java/com/insight/common/{dto,exception,security,util}
```
Then, one file at a time, by hand: `common-library/pom.xml`, then each class
(`ApiError.java`, `BaseException.java`, `ResourceNotFoundException.java`, `RequestContext.java`,
`IdempotencyKeys.java`). The `storage/` package (`ObjectStorageClient`, `S3ObjectStorageClient`,
`ObjectStorageProperties`, `ObjectStorageAutoConfiguration`) and its
`META-INF/spring/org.springframework.boot.autoconfigure.AutoConfiguration.imports` registration
file were added later, same way, when the presigned-upload work started. Nothing in this module
is generated — every line was written directly.

## Step 3 — `protos` (the one module where a tool generates real code)

```bash
mkdir -p protos/src/main/proto/{common,upload,transformation,ocr,orchestrator,reporting}/v1
```
`protos/pom.xml` and every `.proto` file (`upload_service.proto`, `transformation_service.proto`,
etc.) were written by hand — protobuf/gRPC don't have a scaffolding tool for the contract itself,
you just write the `.proto` syntax directly.

**What's generated, and by which command:**
```bash
mvn -pl protos install
```
This one command is what actually produces Java code nobody wrote directly. Maven runs the
`protobuf-maven-plugin` (configured in `protos/pom.xml`), which:
1. downloads `protoc` (Google's protobuf compiler) and `protoc-gen-grpc-java`, as plain Maven
   artifacts matching your OS (via the `os-maven-plugin` extension) — no separate install step
2. runs them against every `.proto` file
3. writes `message` classes to `protos/target/generated-sources/protobuf/java/`
4. writes the gRPC service stub classes (`XxxServiceGrpc.java`) to
   `protos/target/generated-sources/protobuf/grpc-java/`
5. compiles all of that generated Java, packages it into `protos-1.0.0-SNAPSHOT.jar`, and
   `install` copies that jar into `~/.m2/repository/com/insight/protos/...` so every other
   module can depend on it

Nobody wrote `UploadQueryServiceGrpc.java` — running that command is what creates it, every
single time, from the `.proto` source of truth. Delete `protos/target/` and re-run the command
and it comes back identical. (See `docs/upload-service.md` §3 for a line-by-line explanation of
what's inside those generated files, and `docs/TROUBLESHOOTING.md` for a recurring flakiness
issue with this exact step in this environment.)

## Step 4 — each of the 5 services (manual, same pattern repeated 5 times)

For each of `upload-service`, `transformation-service`, `ocr-service`, `orchestrator-service`,
`reporting-service`, the same sequence happened by hand:

```bash
mkdir -p <service>/src/main/java/com/insight/<pkg>/{controller,dto,entity,exception,mapper,repository,service}
mkdir -p <service>/src/main/resources/db/migration
```
Then, one file at a time: `pom.xml`, the `@SpringBootApplication` class, every controller/dto/
entity/exception/mapper/repository/service class, `application.yml`, and the Flyway
`V1__init.sql` migration. All handwritten — Flyway migrations in particular are meant to be
reviewed, exact SQL, never generated.

**Verifying each one**, after writing its files, used ordinary Maven build commands — not
generators, just compiling what was just written:
```bash
mvn -pl <service> -am compile     # compile this module + whatever it depends on
mvn -pl <service> install -DskipTests   # compile, package, and install to ~/.m2
```

## Step 5 — `architecture-tests` (manual)

```bash
mkdir -p architecture-tests/src/test/java/com/insight/archtest
```
`pom.xml` (depending on all 5 services + ArchUnit) and `LayeringRulesTest.java` were both
written by hand. Running it is a normal test run, nothing generated:
```bash
mvn -pl architecture-tests test
```

## Step 6 — `helm/`, `scripts/`, `docker-compose.yml`, `infra/` (manual)

`helm/<service>/Chart.yaml` and `values.yaml` for all 5 services, `scripts/*.sh`,
`docker-compose.yml`, and `infra/postgres-init/01-create-databases.sh` were all written by hand
(the 5 Helm charts were written in a single loop over the 5 service names to avoid retyping the
same template 5 times, but the content itself isn't generated by any Helm/Docker tool).

---

## Full build/verify command reference

These are the actual commands used throughout, and what each one is for:

| Command | What it does |
|---|---|
| `mvn -pl protos install` | Regenerates and rebuilds just the gRPC stubs (the one real codegen step) |
| `mvn -pl <module> -am compile` | Compile one module and everything it depends on, without packaging |
| `mvn clean install -DskipTests` | Full reactor build, every module, skipping tests — the everyday "does it all still compile" check |
| `mvn -pl architecture-tests test` | Run the ArchUnit layering rules |
| `mvn -pl <module> dependency:tree` | Print what a module actually pulls in transitively — used to debug *why* a class was (or wasn't) on the classpath |
| `mvn -pl <service> -Dspring-boot.run.fork=false spring-boot:run` | Run one service in the foreground, in the same JVM as Maven (see `docs/RUNNING_LOCALLY.md`) |
| `docker compose up -d` | Start Postgres/Kafka/Kafka UI/MinIO |
| `./scripts/run.sh` / `./scripts/stop.sh` | Thin wrapper starting/stopping all 5 services in the background |

## Summary: handwritten vs. generated

| | Handwritten | Generated |
|---|---|---|
| `pom.xml` (all 9 modules) | ✅ | |
| `.proto` files | ✅ | |
| gRPC Java classes (`*Grpc.java`, message classes) | | ✅ (`mvn -pl protos install`, from the `.proto` source) |
| Every `@Entity`/`@Service`/`@RestController`/etc. class | ✅ | |
| `application.yml`, Flyway SQL | ✅ | |
| `docker-compose.yml`, Helm charts, shell scripts | ✅ | |
| `target/` directories (all modules) | | ✅ (build output — compiled `.class` files, packaged jars) |

The one-line version: **if it's under `target/`, a tool made it and it's safe to delete; if it's
anywhere else in the repo, it was typed out by hand, one file at a time.**
