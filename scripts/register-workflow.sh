#!/usr/bin/env bash
set -euo pipefail
for attempt in {1..40}; do
  if curl -fsS -o /dev/null -X POST http://127.0.0.1:19070/deployments -H 'content-type: application/json' -d '{"uri":"http://northwind:9080"}'; then
    echo 'Synthetic Northwind workflow registered.'
    exit 0
  fi
  sleep 1
done
echo 'Restate or Northwind is not ready; inspect docker compose logs.' >&2
exit 1
