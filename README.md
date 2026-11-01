# Hiring Record

An interactive synthetic hiring record: follow a changing hiring standard, the people already assessed, and the claims a report can support. All people, roles, evidence and external systems are fictional. This is an independent product exploration, not a real hiring system.

## Run locally

Install Node.js 24+, Rust with rustup, Docker and [just](https://github.com/casey/just). From this repository:

```bash
just up
```

Open <http://127.0.0.1:8080>. The command builds the Next.js static frontend and Rust WebAssembly engine, starts PostgreSQL, Restate and the simulated Northwind service, registers the workflow, runs database migrations, and starts the API. Docker volumes retain PostgreSQL and Restate data. `just down` stops the containers without deleting their volumes. `just memory` runs without Docker; workflow acknowledgement is manual and data lasts only until the server stops.

The API stores its Ed25519 signing key at `.local/signing.key`, with restricted file permissions, and reuses it on restart. Set `SIGNING_KEY_FILE` to a protected persistent file when configuring another environment; do not commit or share the private key. The key also signs the expiring scenario cookie. Backup and rotation need an operator-managed procedure; losing the key invalidates existing browser sessions.

## Explore the experience

1. Open the record and inspect the proposal impact and a person's source moment. Dismiss the proposal: the original assessments stay current. Reset and approve it: eight assessments become stale while Priya's interview remains bound to its original rubric.
2. Inspect the report's descriptive counts and the reasons `R-AFTER-HIRE` and `R-REJECTS-MET` refuse stronger claims. The timeline replays prior states and **Compare paths** uses the shared Rust engine in browser WebAssembly, with a server fallback.
3. Explore synthetic quotes, sourced facts and human overrides. A declined quote produces no receipt; an accepted quote creates a pending receipt. In PostgreSQL/Restate mode, delivery is simulated with a delay and acknowledgment; no real ATS, email, or calendar is contacted. The Northwind link has no sync operation, so the report does not claim a sync.
4. Use **View report** for the print-friendly report or browser print-to-PDF. Use **Seal & export** to download signed JSON; `/verify/` checks that JSON on your device. The printed PDF is a rendering, not itself a signed PDF.
5. Open the simulation lab and developer reference. Read-only MCP is available at `/mcp` and operates on a separate, reproducible approved example. Browser sessions cannot be selected by MCP.

Use **Commands** or Ctrl/Cmd+K to see actions and shortcuts. `D` dismisses, `A` approves, and `[` / `]` navigate history. Shortcuts are disabled while editing or inspecting dialogs.

## API and verification

The browser receives a signed, 24-hour HttpOnly, SameSite=Strict scenario cookie. Requests cannot choose a scenario using an HTTP header. Write requests use `Idempotency-Key` and `If-Match: generation:sequence`; repeated identical requests return the original response and conflicting versions are rejected. SSE sends current snapshots and reconnects using the last event ID. A private `/internal/workflow/complete` callback verifies a signed job before changing a receipt.

`/openapi.json` and `apps/web/lib/api-schema.d.ts` are generated from Rust route and model types. `npm run generate:api --prefix apps/web` regenerates them. The API can create additional synthetic roles with `POST /v1/roles`, list them with `GET /v1/roles`, and read or mutate each through `/v1/roles/{role_id}/…`. New role creation requires an initial standard, `If-Match: 0:0` and an idempotency key. These role streams are isolated by browser session. The visual walkthrough remains focused on the seeded September role. Responses and examples describe only the synthetic demo.

```bash
just play
just sim 42 100
cargo run -p record-cli -- verify /path/to/hiring-record-sealed-bundle.json --trust-key <hex-public-key>
```

The signed JSON preserves the exact UTF-8 payload, an Ed25519 signature, and BLAKE3-chained events. Browser, CLI and MCP verification check signature and chain, and can compare a separately supplied trust key. A valid signature does not establish signer identity, source truth, or independently rerun report rules; the signed bundle includes synthetic source references, but not complete source documents.

## Validation

```bash
just test
just test-db
just smoke
just wasm-parity
just workflow-smoke          # restarts only this project's synthetic Northwind container
just e2e                     # first run: cd apps/web && npx playwright install chromium
just spec                    # bounded Quint/Apalache check; requires Java
just model-traces
```

The formal specification is a bounded abstraction of the original proposal, stale-score, acknowledgement, hold and support path. The trace check replays the simulator's supported approve/dismiss/ack traces through that abstraction; neither proves the entire implementation. The current simulator is deterministic engine-level testing, not a complete distributed fault simulator.

## Deployment boundaries

The implementation is designed for synthetic data only. No real candidates, live ATS, identity provider, outbound messages, or AI hiring decisions are connected. The API defaults to loopback binding. Public deployment additionally needs HTTPS, provisioned and monitored persistent storage, operator controls, a stable private signing key, trusted proxy/IP rate limiting, retention/cleanup and capacity planning. Current per-process rate limits and fixed capacity ceilings are demo safeguards rather than a complete multi-instance abuse defense. Do not expose the local stack publicly until these requirements are addressed and independently tested. No public deployment is included by default.

A non-root API image can be built with `docker build -t hiring-record-api .`. It expects a writable persistent `/data` mount for its signing key, a PostgreSQL URL and a correct `PUBLIC_ORIGIN` behind HTTPS. The image and local Compose files are packaging aids, not a provisioned or reviewed public deployment.
