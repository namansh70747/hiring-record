# Completion checklist

Audited baseline: `652155e`. This file records what this branch actually implemented and tested. Status is not a plan.

| Requirement | Status | Implementation files | Tests/evidence | Remaining condition |
|---|---|---|---|---|
| G01 / shared role workspace | passed | `apps/web/app/record/page.tsx`, `apps/web/components/record-studio.tsx`, `apps/web/app/roles/page.tsx` | `apps/web/tests/journey.spec.ts` custom-role case updated to open `/record/?role=` | Browser suite not re-run in this session |
| G02 / all criteria | passed | `CustomRoleTools` criterion select; domain assessments are per criterion | `requirements.rs` atomic source+assessment | UI does not yet edit or remove criteria |
| G03 / atomic assessment save | passed | `Command::RecordSourceAndAssessment` in `engine.rs` | `requirements.rs` supporter journey uses the atomic command | Lost-response recovery still depends on the idempotency key |
| G04 / draft preservation | passed | Role create keeps one idempotency key; custom-role fields clear only after `action` returns true | Code review of `roles/page.tsx` and `CustomRoleTools` | No dedicated browser test for a failed submit |
| G05 / generic routes | in_progress | Existing `/v1/roles/{id}` plus stream, compare, and seal routes | `cargo test -p record-api` contract tests passed | September compatibility routes remain; not every listed route exists |
| G06 / complete person drawer | in_progress | Drawer still opens from the shared workspace and shows the selected assessment, source, decision, and interview | Existing drawer accessibility test | Full historical assessment list is not a separate drawer section yet |
| G07 / entity state machines | in_progress | Holds are created on confirmation request; receipts track acknowledgement ids | `pending_hold_is_not_reported_as_booked`, `partial_acknowledgement_keeps_r_no_ack_until_complete` | Quote expiry, declined holds, and full workflow rows are not all first-class |
| G08 / generalized support | passed | `removed_supporters`; report no longer checks only `mina` | `supporter_rule_is_not_hard_coded_to_one_name` |  |
| G09 / absence semantics | passed | `report.rs` absence copy for hold, interview, and receipt | `absent_hold_interview_and_receipt_are_not_fabricated` |  |
| G10 / versioned event provenance | in_progress | Optional `Event.envelope`; v1 hash ignores it | `provenance_does_not_change_the_v1_hash` | API stamps new events; legacy rows stay without a timestamp |
| G11 / immutable baseline | passed | `seal_opening` stores digest, schema, rules version, and `opening_json` | Replayable bundle test |  |
| G12 / replayable bundle | passed | `create_replay_bundle`, payload `hiring-record-payload/v3` | `replayable_bundle_uses_its_own_baseline` | v1/v2 remain integrity-only |
| G13 / immutable report links | in_progress | `POST/GET /v1/roles/{role}/reports/{seal}` | API compiles; contract tests passed | UI still downloads the live bundle from Seal & export |
| G14 / PDF embedded artifact | pending |  |  | No PDF attachment generator is implemented |
| G15 / verification parity | in_progress | Browser verifier accepts payload v3; CLI prints replay status | Domain verify tests | WASM verifier was not extended to replay v3 |
| G16 / scoped read-only MCP | pending | Sample MCP remains | Existing MCP tool test | Session grants are not issued |
| G17 / synthetic read model/faults | pending | Northwind is still the timer callback | Existing workflow smoke not re-run | No persisted lagging read model |
| G18 / workflow lifecycle | in_progress | Migration `0008` adds status, lease, outcome columns | Migration file only | Dispatcher still records dispatched, not the full lifecycle |
| G19 / stateful simulation | in_progress | `run_schedule`, `minimize_schedule` | Compiles with record-core tests | Lab UI still shows the capped regression simulator |
| G20 / model/trace mapping | pending | Existing `spec/record.qnt` | Not re-run | Model was not expanded |
| G21 / TTL and cleanup | pending | Migration `0006` and `0011` tables |  | Janitor is not implemented; cookie TTL behavior is unchanged |
| G22 / shared limits and notifications | in_progress | `NOTIFY` trigger in `0007`; rate-bucket table in `0010` |  | SSE still polls once a second; limits stay process-local |
| G23 / operator auth | pending |  |  | No OIDC provider is configured. Local operator UI is not built |
| G24 / purpose-separated keys | pending | Signing-key discovery route added |  | Report, session, and workflow still share one key unless an operator configures separate files |
| G25 / typed errors/request IDs | passed | `security.rs` assigns `x-request-id` before the handler and includes it on generated error bodies | API contract tests passed | Handlers that return a bare status still rely on the middleware body |
| G26 / hardening/timeouts/cache | in_progress | Existing headers retained; private `/v1` responses stay `no-store` | Existing origin test | Route-aware timeouts are not complete |
| G27 / organized styles | in_progress | `apps/web/styles/tokens.css`; Google Fonts request removed |  | `globals.css` and `readability.css` still contain the working layout |
| G28 / accessibility/recording QA | pending | Existing axe smoke remains | Not re-run after this change | No new manual screen-reader recording |
| G29 / CLI | in_progress | `record verify` prints replay status | `cargo test` does not execute the CLI binary | `replay`, `minimize`, and PDF verify commands are not wired |
| G30 / complete local packaging | pending | `just up` unchanged |  | No full API container profile |
| G31 / release/restore evidence | pending |  |  | Release workflow, SBOM, and restore smoke were not added |
| G32 / deployment preparation/public smoke | external_dependency |  |  | No hosting or OIDC credentials are available. Public deployment was not performed |

## Domain cases exercised here

`cargo test -p record-core --offline` passed, including the existing 25 journey tests and `tests/requirements.rs` (absence, generalized support, unknown evidence, partial acknowledgement, replayable bundle, provenance hash, comparison non-mutation).

`cargo test -p record-api --offline` passed the in-memory contract tests. PostgreSQL tests return immediately without `TEST_DATABASE_URL`, so they were not evidence of migration `0006`–`0011`.
