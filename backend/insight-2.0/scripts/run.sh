#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

LOG_DIR="logs"
PID_DIR=".run"
mkdir -p "$LOG_DIR" "$PID_DIR"

# Order matters only to avoid noisy connection-refused logs from gRPC clients on startup —
# each client stub reconnects lazily, so it's not fatal to start out of order.
SERVICES=(transformation-service ocr-service reporting-service orchestrator-service upload-service)

echo "Assuming infra is already up (docker compose up -d) and the reactor is already built (./scripts/build.sh)."
echo ""

for svc in "${SERVICES[@]}"; do
  echo "Starting ${svc} (log: ${LOG_DIR}/${svc}.log)..."
  nohup mvn -pl "${svc}" -Dspring-boot.run.fork=false spring-boot:run \
    > "${LOG_DIR}/${svc}.log" 2>&1 &
  echo $! > "${PID_DIR}/${svc}.pid"
  sleep 3
done

cat <<'EOF'

All services launching in the background.

  Tail a service's logs:  tail -f logs/<service>.log
  Stop everything:        ./scripts/stop.sh

Ports:
  upload-service         http://localhost:8081  (grpc 9095)
  transformation-service http://localhost:8082  (grpc 9091)
  ocr-service             http://localhost:8083  (grpc 9096)
  orchestrator-service    http://localhost:8084  (grpc 9094)
  reporting-service       http://localhost:8085  (grpc 9093)

Try it once upload-service is up (see docs/PRESIGNED_UPLOADS.md for the full flow):
  curl -X POST http://localhost:8081/api/v1/uploads/initiate \
    -H "Content-Type: application/json" -d '{"fileName":"sample.csv"}'
EOF
