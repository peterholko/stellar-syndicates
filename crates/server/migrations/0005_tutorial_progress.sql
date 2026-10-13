-- Founding-charter (tutorial) progress for the admin dashboard's funnel. The
-- game loop is the authority; this keeps the first time each account reached
-- each step, per galaxy, so the record outlives a reset and new players can be
-- followed through it. Gameplay never reads it.

-- Each galaxy the server has run, and roughly when it began (wall clock).
CREATE TABLE galaxies (
    id TEXT PRIMARY KEY,
    started_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE tutorial_progress (
    account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    galaxy_id TEXT NOT NULL,
    step TEXT NOT NULL,
    reached_at TIMESTAMPTZ NOT NULL,
    -- The game clock at that moment, when the game recorded one.
    sim_time DOUBLE PRECISION,
    PRIMARY KEY (account_id, galaxy_id, step)
);
CREATE INDEX tutorial_progress_galaxy ON tutorial_progress(galaxy_id, step);
