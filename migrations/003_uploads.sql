-- User-uploaded rule modules. Versions stay immutable; source is the JS text.

ALTER TABLE games ADD COLUMN author_id uuid REFERENCES users (id);
ALTER TABLE games ADD COLUMN moderation text NOT NULL DEFAULT 'visible'
    CHECK (moderation IN ('visible', 'hidden'));

ALTER TABLE game_versions ADD COLUMN source text;
ALTER TABLE game_versions ADD COLUMN source_sha256 text;

CREATE TABLE upload_quotas (
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    day date NOT NULL,
    publishes integer NOT NULL DEFAULT 0,
    checks integer NOT NULL DEFAULT 0,
    PRIMARY KEY (user_id, day)
);
