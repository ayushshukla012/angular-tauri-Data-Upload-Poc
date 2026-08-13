# Running the platform locally

Manual, step-by-step instructions for building, starting, and testing all 5 services on a
dev machine — no Kubernetes/Helm involved.

## 1. Start infra (Postgres, Kafka, Kafka UI, MinIO)

```bash
cd insight-data-upload-platform
docker compose up -d
```

Verify everything is actually healthy before moving on:

```bash
docker compose ps
```

`postgres` and `minio` should show `(healthy)`; `kafka` and `kafka-ui` should show `Up`. If
anything is `Restarting` or missing, check its log: `docker compose logs <service-name>`.

Postgres init (`infra/postgres-init/01-create-databases.sh`) creates one role + database per
service (`upload`, `transformation`, `ocr`, `orchestrator`, `reporting`) — this only runs on
the **first** startup of a fresh volume. If you need to re-run it, remove the volume first:
`docker compose down -v`.

MinIO init (`minio-init` container) creates the `insight-uploads` bucket automatically and
exits — seeing it listed as exited/stopped in `docker compose ps` is expected, not a failure.

## 2. Build the code

```bash
mvn clean install -DskipTests
```

Watch for `BUILD SUCCESS` / `BUILD FAILURE` at the end. If it fails, scroll to the **first**
`[ERROR]` block — that's the actual cause; everything after it is usually just Maven's summary.

## 3. Run each service, one terminal per service

```bash
mvn -pl transformation-service -Dspring-boot.run.fork=false spring-boot:run
mvn -pl ocr-service -Dspring-boot.run.fork=false spring-boot:run
mvn -pl reporting-service -Dspring-boot.run.fork=false spring-boot:run
mvn -pl orchestrator-service -Dspring-boot.run.fork=false spring-boot:run
mvn -pl upload-service -Dspring-boot.run.fork=false spring-boot:run
```

Each terminal streams that service's logs live. `Ctrl+C` stops that one service.

**Reading the output:**
- `Started UploadServiceApplication in 1.8 seconds` → up and healthy.
- A stack trace ending in `APPLICATION FAILED TO START` → read the trace **bottom-up**: the
  bottom-most `Caused by:` is the real root cause, everything above it is Spring's wrapper.

**Or run everything in the background** with the equivalent helper scripts:

```bash
./scripts/run.sh     # starts all 5, logs to logs/<service>.log
tail -f logs/upload-service.log
./scripts/stop.sh    # stops all 5
```

## 4. Ports reference

| Service | HTTP | gRPC |
|---|---|---|
| upload-service | 8081 | 9095 |
| transformation-service | 8082 | 9091 |
| ocr-service | 8083 | 9096 |
| orchestrator-service | 8084 | 9094 |
| reporting-service | 8085 | 9093 |

Infra: Postgres `5433` (see [docs/TROUBLESHOOTING.md](TROUBLESHOOTING.md) for why not `5432`),
Kafka `9092`, Kafka UI `8090`, MinIO API `9000` / console `9001`.

## 5. Exercise the API

Uploads are a **3-step presigned-URL flow** — the file bytes go straight to MinIO, never
through `upload-service`. See [docs/PRESIGNED_UPLOADS.md](PRESIGNED_UPLOADS.md) if you want the
full explanation of why. The short version:

```bash
# 1. Ask upload-service for a presigned URL
INIT=$(curl -s -X POST http://localhost:8081/api/v1/uploads/initiate \
  -H "Content-Type: application/json" \
  -d '{"fileName":"sample.csv"}')
echo "$INIT"

UPLOAD_ID=$(echo "$INIT" | python3 -c "import json,sys; print(json.load(sys.stdin)['uploadId'])")
UPLOAD_URL=$(echo "$INIT" | python3 -c "import json,sys; print(json.load(sys.stdin)['uploadUrl'])")

# 2. PUT the file bytes directly to MinIO (not to upload-service)
curl -X PUT "$UPLOAD_URL" -H "Content-Type: text/csv" --data-binary @sample.csv

# 3. Tell upload-service the upload finished — this is what starts the saga
curl -X POST http://localhost:8081/api/v1/uploads/${UPLOAD_ID}/complete
```

Then check progress:

```bash
curl http://localhost:8081/api/v1/uploads/$UPLOAD_ID
curl http://localhost:8084/api/v1/sagas/$UPLOAD_ID
curl http://localhost:8082/api/v1/transformation-jobs/$UPLOAD_ID
```

Watch it happen in Kafka UI (`http://localhost:8090`) and the MinIO console
(`http://localhost:9001`, minioadmin/minioadmin — the file should land in `insight-uploads`).

## Something not working?

Check [docs/TROUBLESHOOTING.md](TROUBLESHOOTING.md) first — it's a running log of every real
bug hit while bringing this stack up locally (port collisions, Flyway/Postgres version quirks,
missing Spring annotations, etc.), kept up to date as new ones turn up.
