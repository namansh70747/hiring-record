CREATE TABLE IF NOT EXISTS hr_operator_audit (
    id BIGSERIAL PRIMARY KEY,
    actor TEXT NOT NULL,
    action TEXT NOT NULL,
    scope TEXT NOT NULL,
    request_id TEXT,
    result TEXT NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE IF NOT EXISTS hr_session_tombstones (
    session_id TEXT PRIMARY KEY,
    purged_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
