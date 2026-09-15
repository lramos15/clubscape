CREATE TABLE game_lifecycle_commands (
    account_id UUID NOT NULL REFERENCES accounts (account_id),
    operation_id UUID NOT NULL CHECK (operation_id <> '00000000-0000-0000-0000-000000000000'),
    world_id UUID NOT NULL REFERENCES game_worlds (world_id),
    auth_digest BYTEA NOT NULL CHECK (octet_length(auth_digest) = 32),
    intent_hash BYTEA NOT NULL CHECK (octet_length(intent_hash) = 32),
    committed_result JSONB NOT NULL CHECK (
        jsonb_typeof(committed_result) = 'object'
        AND octet_length(committed_result::text) <= 524288
    ),
    committed_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (account_id, operation_id)
);
