-- Accounts are durable independently of a galaxy. Resetting snapshots/events
-- must never delete these rows. No credentials or sessions belong in World.
CREATE TABLE accounts (
    id UUID PRIMARY KEY,
    -- u64 bit pattern in signed storage (new IDs are positive). The full range
    -- leaves an explicitly authorized legacy-ID migration possible; never infer
    -- such a binding from a public corporation name.
    player_id BIGINT NOT NULL UNIQUE CHECK (player_id <> 0),
    login TEXT NOT NULL UNIQUE,
    corporation_name TEXT NOT NULL,
    corporation_key TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    disabled BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    password_changed_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- One current login per account. Store only a SHA-256 digest of a random
-- 256-bit bearer token; unlike a password, this token has machine entropy.
CREATE TABLE account_sessions (
    account_id UUID PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
    token_hash BYTEA NOT NULL UNIQUE CHECK (octet_length(token_hash) = 32),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX account_sessions_expiry ON account_sessions(expires_at);
