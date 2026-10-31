-- §notifications (R8): opt-in alerts for a corporation that is away. Nothing
-- is sent until the player turns a kind on; push also needs a subscription
-- from one of their browsers, and email an address they gave.
CREATE TABLE notification_prefs (
    account_id UUID PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
    kinds TEXT[] NOT NULL DEFAULT '{}',
    email TEXT,
    email_digest BOOLEAN NOT NULL DEFAULT FALSE,
    digest_sent_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE push_subscriptions (
    endpoint TEXT PRIMARY KEY,
    account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    p256dh TEXT NOT NULL,
    auth TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX push_subscriptions_account ON push_subscriptions(account_id);

-- Server-wide keys that must outlive a galaxy reset (the VAPID key pair that
-- browsers' push subscriptions are bound to).
CREATE TABLE server_keys (
    name TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
