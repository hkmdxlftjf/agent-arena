-- Cosmetic profile, catalog, and play-earned unlocks.
-- Nothing here is read by the rules engine or the signature chain.

ALTER TABLE agents
    ADD COLUMN title text,
    ADD COLUMN bio text,
    ADD COLUMN color text NOT NULL DEFAULT '#8c3a2f',
    ADD COLUMN avatar_preset text NOT NULL DEFAULT 'crane',
    ADD COLUMN avatar_image boolean NOT NULL DEFAULT false,
    ADD COLUMN frame_id text;

ALTER TABLE agents
    ADD CONSTRAINT agents_title_len CHECK (title IS NULL OR char_length(title) <= 12),
    ADD CONSTRAINT agents_bio_len CHECK (bio IS NULL OR char_length(bio) <= 80),
    ADD CONSTRAINT agents_color_hex CHECK (color ~ '^#[0-9A-Fa-f]{6}$');

CREATE TABLE cosmetic_items (
    id text PRIMARY KEY,
    kind text NOT NULL CHECK (kind IN ('frame', 'piece', 'card_back', 'board', 'emote')),
    game_id text,
    name text NOT NULL,
    rarity text NOT NULL CHECK (rarity IN ('common', 'rare', 'epic')),
    asset text NOT NULL,
    unlock_rule text NOT NULL,
    sort integer NOT NULL DEFAULT 0
);

CREATE TABLE agent_unlocks (
    agent_id uuid NOT NULL REFERENCES agents (id) ON DELETE CASCADE,
    item_id text NOT NULL REFERENCES cosmetic_items (id),
    unlocked_at timestamptz NOT NULL DEFAULT now(),
    acked_at timestamptz,
    PRIMARY KEY (agent_id, item_id)
);

CREATE TABLE agent_loadout (
    agent_id uuid NOT NULL REFERENCES agents (id) ON DELETE CASCADE,
    game_id text NOT NULL,
    kind text NOT NULL,
    item_id text NOT NULL REFERENCES cosmetic_items (id),
    PRIMARY KEY (agent_id, game_id, kind)
);

CREATE TABLE cosmetic_stats (
    agent_id uuid PRIMARY KEY REFERENCES agents (id) ON DELETE CASCADE,
    wins integer NOT NULL DEFAULT 0,
    games integer NOT NULL DEFAULT 0,
    streak integer NOT NULL DEFAULT 0,
    best_streak integer NOT NULL DEFAULT 0,
    games_played text[] NOT NULL DEFAULT '{}'
);

CREATE TABLE cosmetic_awards (
    match_id uuid PRIMARY KEY REFERENCES matches (id) ON DELETE CASCADE,
    applied_at timestamptz NOT NULL DEFAULT now()
);
