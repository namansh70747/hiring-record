# Implementation status

The application is a local synthetic demonstration. The public interactive deployment described in the rebuild plan is not yet safe or complete. Do not expose the current API to the public internet: browser session IDs are caller-supplied, the MCP interface accepts session IDs, and there is no public authentication, abuse limiting, expiry, or tenant isolation.

## Implemented in this iteration

- Larger default text, stronger workspace text colors, visible focus, reduced-motion behavior, responsive stacking, and a reset confirmation.
- Print-friendly report with a browser print/PDF action and a separate downloadable signed JSON bundle. The PDF is not itself a signed or independently verifiable document.
- Versioned SQL migration compatible with the existing demo tables and append-only trigger. Existing events remain in place.

## Outstanding from the approved plan

- Full accessibility and contrast audit, keyboard/screen-reader testing, automated browser tests and visual regression checks; typography and CSS component refactoring are incomplete.
- Domain commands and entities beyond the fixed demo, complete source/fact/quote/override workflows, versioned events and generalized IDs.
- Authenticated, expiring, isolated visitor sessions; idempotency keys, rate limits, structured errors, generated OpenAPI and client, streaming updates, and broader API hardening.
- Durable Restate workflows, transactional outbox, lagging simulated Northwind service, failure/retry handling and corresponding integration tests.
- Formal Quint model and trace validation, distributed fault simulation, expanded CLI and MCP verification.
- Signed, portable PDF format, production key management and deployed public demo.

The existing JSON verification checks signatures and the event chain, not the factual truth of source evidence or whether the report was recomputed correctly. The current signing key changes when the server restarts.
