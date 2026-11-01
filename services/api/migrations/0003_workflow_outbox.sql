CREATE TABLE IF NOT EXISTS hr_workflow_outbox (
    workflow_id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES hr_demo_streams(session_id),
    generation BIGINT NOT NULL,
    receipt_id TEXT NOT NULL,
    dispatched BOOLEAN NOT NULL DEFAULT FALSE,
    attempts INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS hr_pending_workflows ON hr_workflow_outbox(created_at) WHERE NOT dispatched;
