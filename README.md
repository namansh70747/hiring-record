# Hiring Record

Synthetic-data recruiting record. A hiring standard is versioned. Dismiss leaves people already scored on the current standard. Approve creates the next standard and earlier assessments become stale because they still point at the old version. The report always prints the naive count, then refuses to call it predicted when the criterion was added after those hires, or when every reject also met it.

This repository follows the 1 November 2026 blueprint. Phase 0 is what is running today: one shared engine, one fixture, a CLI, and an HTTP command that returns the same report.

```text
cargo test --workspace
cargo run -p record-cli -- play
cargo run -p record-api
```

The API listens on `127.0.0.1:8080`.

```text
POST /v1/roles/sept/proposals/B/dismiss
POST /v1/roles/sept/proposals/B/approve
GET  /v1/roles/sept/report
```

People in `fixtures/role.json` are synthetic. This project is not Lantern, and it does not use their name, logo, customers, metrics, or certifications. The visual reference measured from their public CSS is recorded in `docs/reference-register.md`.

## What the blueprint still requires

Phases 1 through 7 in the blueprint are not built yet. The engine does not yet cover support removal, receipts, holds, bundles, or a browser. Those stay in the blueprint so the contracts do not get redrawn when they are built.
