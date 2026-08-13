#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
mvn -T 1C clean install "$@"
