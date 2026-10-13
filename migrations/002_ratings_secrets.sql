-- Hidden commitments, ratings, and the ranked queue.

CREATE TABLE match_secrets (
    match_id uuid NOT NULL REFERENCES matches (id) ON DELETE CASCADE,
    seat smallint NOT NULL,
    commitment text NOT NULL,
    data jsonb NOT NULL,
    salt text NOT NULL,
    PRIMARY KEY (match_id, commitment)
);

CREATE TABLE ratings (
    agent_id uuid NOT NULL REFERENCES agents (id) ON DELETE CASCADE,
    game_id text NOT NULL REFERENCES games (id),
    season text NOT NULL DEFAULT '2026',
    system text NOT NULL CHECK (system IN ('glicko2', 'openskill')),
    rating double precision NOT NULL,
    rd double precision NOT NULL,
    volatility double precision NOT NULL,
    mu double precision NOT NULL,
    sigma double precision NOT NULL,
    games integer NOT NULL DEFAULT 0,
    wins integer NOT NULL DEFAULT 0,
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (agent_id, game_id, season)
);

CREATE TABLE rating_events (
    id bigserial PRIMARY KEY,
    match_id uuid NOT NULL REFERENCES matches (id) ON DELETE CASCADE,
    agent_id uuid NOT NULL REFERENCES agents (id) ON DELETE CASCADE,
    game_id text NOT NULL,
    season text NOT NULL,
    before_rating double precision NOT NULL,
    after_rating double precision NOT NULL,
    before_rd double precision NOT NULL,
    after_rd double precision NOT NULL,
    score double precision NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX rating_events_agent_idx ON rating_events (agent_id, created_at DESC);

CREATE TABLE match_queue (
    id uuid PRIMARY KEY,
    agent_id uuid NOT NULL UNIQUE REFERENCES agents (id) ON DELETE CASCADE,
    owner_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    game_id text NOT NULL,
    variant text NOT NULL,
    kid text NOT NULL,
    rating double precision NOT NULL,
    enqueued_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX match_queue_game_idx ON match_queue (game_id, variant, enqueued_at);
