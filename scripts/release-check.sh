#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
echo "release check at $(git rev-parse --short HEAD) $(date -u +%Y-%m-%dT%H:%M:%SZ)"
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run typecheck --prefix apps/web
node --test services/northwind/read-model.test.mjs
node scripts/pdf-roundtrip.mjs
if command -v trivy >/dev/null 2>&1; then
  trivy fs --severity HIGH,CRITICAL --exit-code 1 .
else
  echo "trivy is not installed locally. .github/workflows/release.yml runs the image scan."
fi
echo "release check passed"
