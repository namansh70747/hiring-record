# Implementation status

The repository provides a runnable synthetic local demo with signed browser sessions, isolated synthetic role creation and persisted event streams and signing key, optimistic concurrency and idempotent commands, SSE snapshots, outbox-backed simulated Restate receipt and hold workflows, signed JSON verification, a print-friendly report, and a bounded formal abstraction with supported trace replay.

The implementation is not a production recruiting system. Remaining scope from the approved blueprint includes complete generalized role workflows and multi-role live streaming; fully generalized report/support entities and event timestamps; durable quote workflows beyond acceptance receipt delivery; a full distributed fault simulator and exhaustive model conformance; a cryptographically signed portable PDF; and production-grade retention, operator authentication, multi-instance rate limiting and deployment controls. The public demo must not be exposed until those controls have been completed and reviewed.

The current signing key is persisted locally. Its rotation and disaster-recovery process is not automated. JSON verification checks signature and event-chain integrity; it does not verify source truth or independently reconstruct the report. The Quint check covers the model and selected bound, not every implementation behavior.
