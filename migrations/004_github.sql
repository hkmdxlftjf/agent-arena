-- GitHub OAuth identities. Password login stays; GitHub-only accounts have no password hash.

ALTER TABLE users ALTER COLUMN password_hash DROP NOT NULL;
ALTER TABLE users ADD COLUMN github_id bigint UNIQUE;
ALTER TABLE users ADD COLUMN github_login text;

CREATE TABLE oauth_states (
    state text PRIMARY KEY,
    purpose text NOT NULL CHECK (purpose IN ('login', 'link')),
    user_id uuid REFERENCES users (id) ON DELETE CASCADE,
    expires_at timestamptz NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX oauth_states_expires_idx ON oauth_states (expires_at);
