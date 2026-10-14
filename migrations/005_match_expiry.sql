-- Waiting rooms can expire without a result. Unseen-timeout streaks survive a restart.

ALTER TABLE matches DROP CONSTRAINT matches_status_check;
ALTER TABLE matches ADD CONSTRAINT matches_status_check
    CHECK (status IN ('waiting', 'running', 'finished', 'aborted', 'expired'));

ALTER TABLE match_players
    ADD COLUMN unseen_timeouts integer NOT NULL DEFAULT 0;
