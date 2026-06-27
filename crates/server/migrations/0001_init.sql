-- Stellar Syndicates — initial persistence schema.
--
-- Two tables embody the event-sourced design (§14): an append-only event log
-- and periodic full-world snapshots. Restart = load latest snapshot, replay
-- events after it. M1 only writes a trivial amount; the schema is the real one.

-- Append-only log of everything the simulation emitted.
CREATE TABLE IF NOT EXISTS events (
    id          BIGSERIAL PRIMARY KEY,
    tick        BIGINT      NOT NULL,
    sim_time    DOUBLE PRECISION NOT NULL,
    payload     JSONB       NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS events_tick_idx ON events (tick);

-- Periodic full-state snapshots. `tick` is the primary key so re-snapshotting
-- the same tick is idempotent (upsert).
CREATE TABLE IF NOT EXISTS snapshots (
    tick        BIGINT      PRIMARY KEY,
    sim_time    DOUBLE PRECISION NOT NULL,
    world       JSONB       NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
