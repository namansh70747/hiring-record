# Hiring Record

A synthetic, interactive record of a hiring standard, the people assessed against it, and the claims a report can support. The demo is designed for a walkthrough: review a change, see its effect, rewind it, and verify a signed export.

**All people, events and systems are fictional.** This is a local product demonstration, not a production recruiting system. It is independent of Lantern.

## Start the experience

Install Node.js 24+ (26 LTS recommended), Rust with rustup, Docker and [just](https://github.com/casey/just), then from the repository root:

```bash
just up
```

Open [http://127.0.0.1:8080](http://127.0.0.1:8080). The command installs frontend dependencies if needed, builds the Rust WebAssembly engine and Next.js frontend, starts this project's PostgreSQL container and runs the Rust API. Database data is kept in a Docker volume. `just down` stops the database without removing the volume.

Without Docker, run `just memory`; this stores scenarios in memory until the server stops. A browser tab has its own synthetic session ID. PostgreSQL mode persists events across server restarts; resetting a scenario begins a new generation and retains earlier events in the database. The signing key is generated on each server start in **both** modes.

## Walkthrough

1. Open the record. The proposal preview shows eight assessments and one interview affected.
2. Dismiss the proposal: assessments remain current. Reset and approve it instead: the original assessments are now stale and Priya's interview still uses its original rubric.
3. The report retains the two “3 of 3” observations while refusing predictive claims with `R-AFTER-HIRE` and `R-REJECTS-MET`.
4. Inspect a person's synthetic transcript moment, simulate an acknowledgement, confirm the hold, compare the opposite path in browser WebAssembly and scrub the timeline.
5. Select **Seal & export**, then open **Verify** to check the bundle locally. Compare its public key with a trusted key. The demo key is temporary; a valid signature alone does not establish signer identity or factual accuracy.
6. Open **Simulation lab** to run deterministic engine scenarios. The lab does not simulate distributed infrastructure faults.
7. Visit `/developers/` for the API and agent reference. Connect a read-only MCP client to `http://127.0.0.1:8080/mcp`. Without a session ID, tools use a reproducible approved example. The workspace displays its session ID for inspecting a particular synthetic browser scenario.

Keyboard shortcuts: `D` dismiss, `A` approve, `[` previous event, `]` next event. Shortcuts are disabled while editing a field or inspecting a dialog.

## CLI and verification

```bash
just play
just sim 42 100
cargo run -p record-cli -- verify /path/to/hiring-record-sealed-bundle.json
cargo run -p record-cli -- verify /path/to/hiring-record-sealed-bundle.json --trust-key <hex-public-key>
```

The bundle signs the exact UTF-8 payload with Ed25519 and includes BLAKE3-chained events. Verification checks the signature, event chain and optionally a separately supplied public key. It does **not** replay the report rules, include source documents, or establish that the underlying evidence is true. This is a JSON bundle, not a PDF signature.

## Checks

```bash
just test
just test-db
just wasm-parity
cargo clippy --workspace --all-targets -- -D warnings
```

`just test-db` starts the local PostgreSQL container and tests event replay, optimistic concurrency and reset generations. The ordinary Rust test suite works without a database.

## Architecture and boundaries

- Next.js 16 and React 19 produce a static frontend served by the Rust API on the same origin.
- A pure Rust engine owns proposal behavior, assessment currency, report evaluation, event hashing and deterministic simulation.
- axum exposes local synthetic scenario APIs and a read-only MCP Streamable HTTP server built with the official Rust SDK.
- PostgreSQL mode appends scenario events transactionally with an expected sequence and generation. It is a demo event store, not a production multi-tenant event platform.
- The browser runs the same Rust domain engine as WebAssembly for counterfactual comparisons, with an API fallback.
- The browser and CLI independently verify signed bundle bytes and event hashes.
- Northwind is a simulated integration. No ATS, calendar, outbound email, identity provider or AI model is connected.

The larger blueprint also calls for production authentication, multi-tenant authorization, durable external workflows, full source evidence, real integrations, formal modeling, production key management and broader infrastructure. Those are not implemented here and should not be represented as live capabilities.

The visual direction draws from publicly visible editorial and product design patterns; all branding, copy and assets here are original to this project. See `docs/reference-register.md`.
