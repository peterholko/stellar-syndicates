-- §season (docs/endgame-keystone.md): every galaxy's ending, kept past its
-- reset. One row per galaxy; the Sovereign's syndicate or the Charter's winners.
CREATE TABLE hall_of_fame (
    galaxy_id TEXT PRIMARY KEY,
    ended_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    ending TEXT NOT NULL,
    holder TEXT,
    world TEXT,
    winners JSONB NOT NULL DEFAULT '[]'
);
