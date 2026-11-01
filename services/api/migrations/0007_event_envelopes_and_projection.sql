-- Envelope metadata sits beside immutable event JSON. Notifications carry only stream identity and version.
ALTER TABLE hr_demo_events ADD COLUMN IF NOT EXISTS envelope JSONB;
ALTER TABLE hr_demo_streams ADD COLUMN IF NOT EXISTS projection_version TEXT;
CREATE OR REPLACE FUNCTION hr_notify_role_change() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    PERFORM pg_notify('hr_role', NEW.session_id || ':' || NEW.generation::text || ':' || NEW.sequence::text);
    RETURN NEW;
END $$;
DO $$ BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_trigger
        WHERE tgname = 'hr_demo_events_notify' AND tgrelid = 'hr_demo_events'::regclass
    ) THEN
        CREATE TRIGGER hr_demo_events_notify
        AFTER INSERT ON hr_demo_events
        FOR EACH ROW EXECUTE FUNCTION hr_notify_role_change();
    END IF;
END $$;
