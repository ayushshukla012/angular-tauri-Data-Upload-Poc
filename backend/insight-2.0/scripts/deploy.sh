#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

NAMESPACE="${NAMESPACE:-insight}"
SERVICES=(upload-service transformation-service ocr-service orchestrator-service reporting-service)

for svc in "${SERVICES[@]}"; do
  helm upgrade --install "$svc" "helm/${svc}" --namespace "$NAMESPACE" --create-namespace
done
