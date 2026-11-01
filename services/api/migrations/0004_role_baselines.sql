-- Existing rows keep NULL and replay from the original September fixture.
-- New role streams record an explicit empty starting state.
ALTER TABLE hr_demo_streams ADD COLUMN IF NOT EXISTS baseline JSONB;
