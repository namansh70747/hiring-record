#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p .local/backups
stamp="$(date -u +%Y%m%dT%H%M%SZ)"
out=".local/backups/hiring-record-${stamp}.sql"
if docker compose -p hiring-record ps postgres >/dev/null 2>&1; then
  docker compose -p hiring-record exec -T postgres pg_dump -U hiring_record hiring_record > "$out"
  echo "wrote $out"
else
  echo "PostgreSQL container is not running. Start it with 'just db' before backing up."
  exit 1
fi
