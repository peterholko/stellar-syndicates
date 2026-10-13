-- Operator audit trail for the admin dashboard. It lives beside accounts, so a
-- galaxy reset never touches it, and gameplay never reads it. Rows go only
-- with their account. IPs are stored as text: they are shown and matched,
-- never routed on.

-- One row per credential event: register, login, a new guest, or a guest
-- upgrading to a registered login. Resuming a valid cookie is not a login.
CREATE TABLE account_logins (
    id BIGSERIAL PRIMARY KEY,
    account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('register', 'login', 'guest', 'upgrade')),
    at TIMESTAMPTZ NOT NULL DEFAULT now(),
    ip TEXT,
    user_agent TEXT,
    language TEXT,
    device_id TEXT
);
CREATE INDEX account_logins_account ON account_logins(account_id, at DESC);
CREATE INDEX account_logins_device ON account_logins(device_id) WHERE device_id IS NOT NULL;

-- One row per game socket, from Join to close. A hard stop leaves rows open;
-- the next start closes them at their last heartbeat.
CREATE TABLE play_sessions (
    id UUID PRIMARY KEY,
    account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    connected_at TIMESTAMPTZ NOT NULL,
    last_seen_at TIMESTAMPTZ NOT NULL,
    disconnected_at TIMESTAMPTZ,
    ip TEXT,
    user_agent TEXT,
    device_id TEXT,
    actions INTEGER NOT NULL DEFAULT 0,
    -- Five-minute windows with at least one command, in seconds.
    active_seconds INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX play_sessions_account ON play_sessions(account_id, connected_at DESC);
CREATE INDEX play_sessions_open ON play_sessions(account_id) WHERE disconnected_at IS NULL;
CREATE INDEX play_sessions_device ON play_sessions(device_id) WHERE device_id IS NOT NULL;

-- Every command a player sent, with its message as the client sent it.
CREATE TABLE player_actions (
    id BIGSERIAL PRIMARY KEY,
    session_id UUID NOT NULL REFERENCES play_sessions(id) ON DELETE CASCADE,
    account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    at TIMESTAMPTZ NOT NULL,
    kind TEXT NOT NULL,
    detail JSONB
);
CREATE INDEX player_actions_account ON player_actions(account_id, at DESC);
