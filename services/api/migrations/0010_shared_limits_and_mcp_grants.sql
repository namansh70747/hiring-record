CREATE TABLE IF NOT EXISTS hr_rate_buckets (
    bucket_key TEXT PRIMARY KEY,
    window_start TIMESTAMPTZ NOT NULL,
    hits INTEGER NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL
);
CREATE TABLE IF NOT EXISTS hr_mcp_grants (
    grant_id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    token_hash TEXT NOT NULL UNIQUE,
    scope TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ
);
