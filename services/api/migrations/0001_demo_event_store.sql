-- Compatible with the original local demo tables; no existing event data is removed.
CREATE TABLE IF NOT EXISTS hr_demo_streams (
    session_id TEXT PRIMARY KEY,
    generation BIGINT NOT NULL DEFAULT 0,
    sequence BIGINT NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS hr_demo_events (
    session_id TEXT NOT NULL REFERENCES hr_demo_streams(session_id),
    generation BIGINT NOT NULL,
    sequence BIGINT NOT NULL,
    event JSONB NOT NULL,
    PRIMARY KEY (session_id, generation, sequence)
);
CREATE OR REPLACE FUNCTION hr_demo_prevent_event_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'demo events are append-only';
END $$;
DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname = 'hr_demo_events_append_only' AND tgrelid = 'hr_demo_events'::regclass) THEN
        CREATE TRIGGER hr_demo_events_append_only BEFORE UPDATE OR DELETE ON hr_demo_events
        FOR EACH ROW EXECUTE FUNCTION hr_demo_prevent_event_mutation();
    END IF;
END $$;
