#!/usr/bin/env bash
set -euo pipefail

for db in upload transformation ocr orchestrator reporting; do
  psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" <<-EOSQL
    CREATE USER ${db} WITH PASSWORD '${db}';
    CREATE DATABASE ${db} OWNER ${db};
EOSQL
done
