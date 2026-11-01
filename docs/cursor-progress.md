# Cursor progress

Updated: 1 November 2026.

## Current branch

- Repository: `/Users/namansharma/Projects/hiring-record`
- Branch: `codex/hiring-record-redesign`
- Starting commit: `652155e` (audited baseline, still the parent)
- Working tree: implementation in progress, not committed yet in this note

## What is running

No API server was left running by this session. PostgreSQL was not required for the tests that passed.

## Commands that passed

```text
cargo test -p record-core --offline
cargo test -p record-api --offline
```

`record-api` PostgreSQL tests skip themselves when `TEST_DATABASE_URL` is unset. That skip is not database coverage.

## Next step

1. Typecheck `apps/web`.
2. Fix any API compile or frontend type errors from the role workspace.
3. Run the focused browser journey if a local server is available.
4. Commit on `codex/hiring-record-redesign` and update draft PR #1.
5. Do not claim G14, G16, G17, G23, G30–G32, or a public URL.

## Do not do

- `git reset --hard`
- `docker compose down -v`
- rewrite migrations `0001`–`0005`
- invent a deployment URL
