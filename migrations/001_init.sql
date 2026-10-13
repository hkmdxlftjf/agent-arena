-- Agent Arena schema (PostgreSQL 16).
-- Types are native Postgres (uuid, jsonb, timestamptz, bytea, text[]) so the
-- service does not carry a SQLite dialect. sqlx migrations live in this folder.

CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE users (
    id uuid PRIMARY KEY,
    handle text UNIQUE NOT NULL,
    email text UNIQUE NOT NULL,
    email_verified_at timestamptz,
    password_hash text NOT NULL,
    role text NOT NULL DEFAULT 'user' CHECK (role IN ('user', 'moderator', 'admin')),
    theme text NOT NULL DEFAULT 'light' CHECK (theme IN ('light', 'dark', 'system')),
    locale text NOT NULL DEFAULT 'zh-CN',
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE sessions (
    id text PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL,
    ip text,
    ua text
);
CREATE INDEX sessions_user_idx ON sessions (user_id);

CREATE TABLE agents (
    id uuid PRIMARY KEY,
    owner_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    name text NOT NULL,
    slug text NOT NULL,
    description text,
    model_label text,
    allow_comments boolean NOT NULL DEFAULT false,
    visibility text NOT NULL DEFAULT 'public',
    last_seen_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (owner_id, slug)
);
CREATE INDEX agents_owner_idx ON agents (owner_id);

CREATE TABLE agent_keys (
    kid text PRIMARY KEY,
    agent_id uuid NOT NULL REFERENCES agents (id) ON DELETE CASCADE,
    alg text NOT NULL CHECK (alg IN ('ed25519')),
    public_key bytea NOT NULL,
    public_key_b64 text NOT NULL,
    label text,
    status text NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'rotated', 'revoked')),
    pop_signature bytea NOT NULL,
    pop_challenge text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    last_signed_at timestamptz,
    revoked_at timestamptz
);
CREATE INDEX agent_keys_agent_idx ON agent_keys (agent_id);

CREATE TABLE key_challenges (
    id uuid PRIMARY KEY,
    agent_id uuid NOT NULL REFERENCES agents (id) ON DELETE CASCADE,
    nonce text NOT NULL,
    purpose text NOT NULL,
    payload jsonb NOT NULL,
    expires_at timestamptz NOT NULL,
    used_at timestamptz
);

CREATE TABLE server_keys (
    kid text PRIMARY KEY,
    alg text NOT NULL DEFAULT 'ed25519',
    public_key bytea NOT NULL,
    public_key_b64 text NOT NULL,
    private_key_ref text NOT NULL,
    valid_from timestamptz NOT NULL DEFAULT now(),
    valid_until timestamptz
);

CREATE TABLE agent_tokens (
    id uuid PRIMARY KEY,
    agent_id uuid NOT NULL REFERENCES agents (id) ON DELETE CASCADE,
    token_hash bytea UNIQUE NOT NULL,
    token_prefix text NOT NULL,
    scopes text[] NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz,
    last_used_at timestamptz,
    revoked_at timestamptz
);
CREATE INDEX agent_tokens_agent_idx ON agent_tokens (agent_id);

CREATE TABLE games (
    id text PRIMARY KEY,
    builtin boolean NOT NULL DEFAULT false,
    name text NOT NULL,
    description text,
    visibility text NOT NULL DEFAULT 'public',
    ranked_eligible boolean NOT NULL DEFAULT false,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE game_versions (
    id uuid PRIMARY KEY,
    game_id text NOT NULL REFERENCES games (id),
    version text NOT NULL,
    engine text NOT NULL CHECK (engine IN ('native', 'js-sandbox')),
    manifest jsonb NOT NULL,
    rules_hash text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (game_id, version)
);

CREATE TABLE matches (
    id uuid PRIMARY KEY,
    game_version_id uuid NOT NULL REFERENCES game_versions (id),
    game_id text NOT NULL,
    variant text,
    mode text NOT NULL DEFAULT 'casual' CHECK (mode IN ('ranked', 'casual')),
    room_code text UNIQUE,
    created_by uuid REFERENCES users (id),
    time_limit_ms integer NOT NULL DEFAULT 60000,
    timeout_policy text NOT NULL CHECK (timeout_policy IN ('auto_pass', 'auto_action', 'forfeit')),
    max_consecutive_timeouts integer NOT NULL DEFAULT 3,
    grace_ms integer NOT NULL DEFAULT 2000,
    allow_comments boolean NOT NULL DEFAULT true,
    seed_commit text,
    seed text,
    rules_hash text,
    server_kid text REFERENCES server_keys (kid),
    status text NOT NULL CHECK (status IN ('waiting', 'running', 'finished', 'aborted')),
    started_at timestamptz,
    ended_at timestamptz,
    end_reason text,
    state jsonb,
    open_decisions jsonb,
    tip_seq integer,
    tip_hash text,
    result jsonb,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX matches_status_idx ON matches (status);

CREATE TABLE match_players (
    match_id uuid NOT NULL REFERENCES matches (id) ON DELETE CASCADE,
    seat smallint NOT NULL,
    agent_id uuid NOT NULL REFERENCES agents (id),
    owner_id uuid NOT NULL REFERENCES users (id),
    kid text REFERENCES agent_keys (kid),
    timeouts integer NOT NULL DEFAULT 0,
    forfeited boolean NOT NULL DEFAULT false,
    score double precision,
    PRIMARY KEY (match_id, seat)
);
CREATE INDEX match_players_agent_idx ON match_players (agent_id);

CREATE TABLE match_events (
    match_id uuid NOT NULL REFERENCES matches (id) ON DELETE CASCADE,
    seq integer NOT NULL,
    seat smallint,
    kind text NOT NULL,
    action jsonb,
    signer text NOT NULL CHECK (signer IN ('agent', 'server')),
    kid text,
    alg text NOT NULL,
    decision_seq integer,
    payload jsonb NOT NULL,
    payload_hash bytea NOT NULL,
    signature bytea NOT NULL,
    prev_hash bytea NOT NULL,
    hash bytea NOT NULL,
    server_ack_sig bytea,
    received_at timestamptz NOT NULL,
    entry_line text NOT NULL,
    PRIMARY KEY (match_id, seq)
);
CREATE UNIQUE INDEX match_events_hash_idx ON match_events (hash);
CREATE UNIQUE INDEX match_events_decision_idx
    ON match_events (match_id, seat, decision_seq)
    WHERE decision_seq IS NOT NULL AND seat IS NOT NULL;

CREATE TABLE move_rejections (
    id bigserial PRIMARY KEY,
    match_id uuid,
    seat smallint,
    payload_hash bytea,
    code text NOT NULL,
    nack_sig bytea,
    received_at timestamptz NOT NULL DEFAULT now()
);
