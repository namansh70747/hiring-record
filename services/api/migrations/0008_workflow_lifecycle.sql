ALTER TABLE hr_workflow_outbox ADD COLUMN IF NOT EXISTS status TEXT NOT NULL DEFAULT 'queued';
ALTER TABLE hr_workflow_outbox ADD COLUMN IF NOT EXISTS lease_until TIMESTAMPTZ;
ALTER TABLE hr_workflow_outbox ADD COLUMN IF NOT EXISTS outcome TEXT;
ALTER TABLE hr_workflow_outbox ADD COLUMN IF NOT EXISTS causal_sequence BIGINT;
ALTER TABLE hr_workflow_outbox ADD COLUMN IF NOT EXISTS acknowledgement_id TEXT;
