#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

PID_DIR=".run"

if [ ! -d "$PID_DIR" ] || [ -z "$(ls -A "$PID_DIR" 2>/dev/null)" ]; then
  echo "No services tracked in ${PID_DIR}/ — nothing to stop."
  exit 0
fi

for pidfile in "${PID_DIR}"/*.pid; do
  svc="$(basename "$pidfile" .pid)"
  pid="$(cat "$pidfile")"
  if kill -0 "$pid" 2>/dev/null; then
    echo "Stopping ${svc} (pid ${pid})..."
    kill "$pid"
  else
    echo "${svc} (pid ${pid}) already stopped."
  fi
  rm -f "$pidfile"
done
