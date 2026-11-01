CREATE TABLE IF NOT EXISTS hr_request_results (
    session_id TEXT NOT NULL REFERENCES hr_demo_streams(session_id),
    request_key TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    response JSONB NOT NULL,
    PRIMARY KEY (session_id, request_key)
);
