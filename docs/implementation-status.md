# Implementation status

The repository provides a runnable synthetic local demo with signed browser sessions, isolated synthetic role creation and persisted event streams and signing key, optimistic concurrency and idempotent commands, SSE snapshots, an outbox-backed simulated Restate receipt workflow, signed JSON verification, a print-friendly report, and a bounded formal abstraction with supported trace replay.

The implementation is not a production recruiting system. Remaining scope from the approved blueprint includes a multi-role management interface and complete generalized role workflows; fully generalized report/support entities and event timestamps; durable quote and hold workflows beyond the receipt delivery currently implemented; a full distributed fault simulator and exhaustive model conformance; a cryptographically signed portable PDF; and production-grade retention, operator authentication, multi-instance rate limiting and deployment controls. The public demo must not be exposed until those controls have been completed and reviewed.

The current signing key is persisted locally. Its rotation and disaster-recovery process is not automated. JSON verification checks signature and event-chain integrity; it does not verify source truth or independently reconstruct the report. The Quint check covers the model and selected bound, not every implementation behavior.
