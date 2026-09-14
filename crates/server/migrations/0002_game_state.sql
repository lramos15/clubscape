CREATE TABLE game_worlds (
    world_id UUID PRIMARY KEY CHECK (world_id <> '00000000-0000-0000-0000-000000000000'),
    content_revision TEXT NOT NULL CHECK (octet_length(content_revision) BETWEEN 1 AND 256),
    schema_version INTEGER NOT NULL CHECK (schema_version > 0),
    state JSONB NOT NULL,
    revision BIGINT NOT NULL CHECK (revision >= 0),
    tick BIGINT NOT NULL CHECK (tick >= 0),
    last_tick_result JSONB,
    lease_owner UUID,
    lease_fence BIGINT NOT NULL DEFAULT 0 CHECK (lease_fence >= 0),
    lease_expires_at TIMESTAMPTZ,
    CONSTRAINT game_worlds_state_shape CHECK (
        jsonb_typeof(state) = 'object' AND octet_length(state::text) <= 8388608
        AND (state->>'schema_version')::bigint = schema_version
        AND state->>'content_revision' = content_revision
        AND (state->>'revision')::bigint = revision
        AND (state->>'tick')::bigint = tick
    ),
    CONSTRAINT game_worlds_tick_result_size CHECK (
        last_tick_result IS NULL OR (
            jsonb_typeof(last_tick_result) = 'object'
            AND octet_length(last_tick_result::text) <= 524288
        )
    ),
    CONSTRAINT game_worlds_lease_shape CHECK (
        (lease_owner IS NULL AND lease_expires_at IS NULL)
        OR (lease_owner IS NOT NULL AND lease_expires_at IS NOT NULL
            AND lease_owner <> '00000000-0000-0000-0000-000000000000' AND lease_fence > 0)
    )
);

-- Character JSON lives only in game_worlds.state.characters. These are identity/version indexes.
CREATE TABLE game_characters (
    account_id UUID PRIMARY KEY REFERENCES accounts (account_id),
    actor_id TEXT COLLATE "C" NOT NULL UNIQUE,
    world_id UUID NOT NULL REFERENCES game_worlds (world_id),
    revision BIGINT NOT NULL CHECK (revision > 0),
    last_sequence BIGINT NOT NULL DEFAULT 0 CHECK (last_sequence >= 0),
    CONSTRAINT game_characters_actor_format CHECK (
        octet_length(actor_id) <= 160
        AND actor_id ~ '^actor\.[a-z0-9_-]+(\.[a-z0-9_-]+)*$'
    ),
    CONSTRAINT game_characters_identity UNIQUE (account_id, actor_id, world_id)
);

CREATE INDEX game_characters_world ON game_characters (world_id, actor_id);

CREATE TABLE game_sessions (
    account_id UUID PRIMARY KEY,
    actor_id TEXT COLLATE "C" NOT NULL UNIQUE,
    world_id UUID NOT NULL,
    session_id UUID NOT NULL UNIQUE CHECK (session_id <> '00000000-0000-0000-0000-000000000000'),
    token_digest BYTEA NOT NULL CHECK (octet_length(token_digest) = 32),
    created_at TIMESTAMPTZ NOT NULL,
    heartbeat_at TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT game_sessions_character FOREIGN KEY (account_id, actor_id, world_id)
        REFERENCES game_characters (account_id, actor_id, world_id),
    CONSTRAINT game_sessions_time_order CHECK (
        heartbeat_at >= created_at AND expires_at > heartbeat_at
    )
);

-- No FK to account_sessions: live-token validation also handles logout, pruning and expiry.
CREATE TABLE processed_game_commands (
    account_id UUID NOT NULL,
    actor_id TEXT COLLATE "C" NOT NULL,
    world_id UUID NOT NULL,
    operation_id UUID NOT NULL CHECK (operation_id <> '00000000-0000-0000-0000-000000000000'),
    sequence BIGINT NOT NULL CHECK (sequence > 0),
    intent_version SMALLINT NOT NULL CHECK (intent_version = 1),
    intent_hash BYTEA NOT NULL CHECK (octet_length(intent_hash) = 32),
    world_revision BIGINT NOT NULL CHECK (world_revision > 0),
    character_revision BIGINT NOT NULL CHECK (character_revision > 0),
    committed_result JSONB NOT NULL CHECK (
        jsonb_typeof(committed_result) = 'object'
        AND octet_length(committed_result::text) <= 524288
    ),
    committed_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (account_id, operation_id),
    UNIQUE (actor_id, sequence),
    CONSTRAINT processed_game_commands_character FOREIGN KEY (account_id, actor_id, world_id)
        REFERENCES game_characters (account_id, actor_id, world_id)
);
