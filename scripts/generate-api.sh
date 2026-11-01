#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
tmp_file=$(mktemp)
trap 'rm -f "$tmp_file"' EXIT
cargo run -q -p record-api -- --openapi > "$tmp_file"
cp "$tmp_file" apps/web/public/openapi.json
apps/web/node_modules/.bin/openapi-typescript apps/web/public/openapi.json -o apps/web/lib/api-schema.d.ts
