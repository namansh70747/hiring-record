-- Persisted synthetic sessions and explicit ownership. Existing event rows stay in place.
CREATE TABLE IF NOT EXISTS hr_sessions (
    session_id TEXT PRIMARY KEY,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    credential_hash TEXT
);
ALTER TABLE hr_demo_streams ADD COLUMN IF NOT EXISTS owner_session TEXT;
ALTER TABLE hr_demo_streams ADD COLUMN IF NOT EXISTS baseline_digest TEXT;
ALTER TABLE hr_demo_streams ADD COLUMN IF NOT EXISTS baseline_schema TEXT;
ALTER TABLE hr_demo_streams ADD COLUMN IF NOT EXISTS created_at TIMESTAMPTZ NOT NULL DEFAULT now();
ALTER TABLE hr_demo_streams ADD COLUMN IF NOT EXISTS last_active_at TIMESTAMPTZ NOT NULL DEFAULT now();
