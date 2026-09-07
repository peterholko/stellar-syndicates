-- Guests are real, durable player identities, authenticated only by their
-- random session cookie until registration. Never invent a shared password or
-- let a public corporation name claim an existing player.
ALTER TABLE accounts ADD COLUMN is_guest BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE accounts ALTER COLUMN login DROP NOT NULL;
ALTER TABLE accounts ALTER COLUMN password_hash DROP NOT NULL;
ALTER TABLE accounts ADD CONSTRAINT account_credentials_match_kind CHECK (
    (is_guest AND login IS NULL AND password_hash IS NULL)
    OR (NOT is_guest AND login IS NOT NULL AND password_hash IS NOT NULL)
);
