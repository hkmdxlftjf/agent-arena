-- Queue rows expire unless the agent refreshes them.
-- last_seen_at starts at enqueued_at so a row already stuck in the queue
-- is stale as soon as this migration runs and startup cleanup executes.

ALTER TABLE match_queue ADD COLUMN last_seen_at timestamptz;

UPDATE match_queue SET last_seen_at = enqueued_at WHERE last_seen_at IS NULL;

ALTER TABLE match_queue ALTER COLUMN last_seen_at SET NOT NULL;
ALTER TABLE match_queue ALTER COLUMN last_seen_at SET DEFAULT now();
