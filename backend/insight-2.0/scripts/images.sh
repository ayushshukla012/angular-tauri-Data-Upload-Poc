#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

REGISTRY="${REGISTRY:-registry.internal/insight}"
SERVICES=(upload-service transformation-service ocr-service orchestrator-service reporting-service)

for svc in "${SERVICES[@]}"; do
  # Build context is the repo root, not the service directory — Maven needs the parent
  # pom and sibling modules (common-library, protos) present to resolve dependencies.
  docker build -t "${REGISTRY}/${svc}:latest" -f "${svc}/Dockerfile" .
done
