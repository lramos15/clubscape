CREATE TABLE game_content_migrations (
    world_id UUID NOT NULL REFERENCES game_worlds(world_id),
    from_artifact TEXT NOT NULL CHECK (from_artifact ~ '^[0-9a-f]{64}$'),
    to_artifact TEXT NOT NULL CHECK (to_artifact ~ '^[0-9a-f]{64}$'),
    world_revision BIGINT NOT NULL CHECK (world_revision > 0),
    migrated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (world_id, to_artifact)
);
