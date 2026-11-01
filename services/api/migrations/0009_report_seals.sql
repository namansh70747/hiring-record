CREATE TABLE IF NOT EXISTS hr_report_seals (
    seal_id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    role_id TEXT NOT NULL,
    generation BIGINT NOT NULL,
    sequence BIGINT NOT NULL,
    rules_version TEXT NOT NULL,
    bundle JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS hr_report_seals_session ON hr_report_seals(session_id, role_id);
