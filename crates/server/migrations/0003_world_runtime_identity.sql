ALTER TABLE game_worlds
    ADD COLUMN runtime_artifact_sha256 TEXT,
    ADD COLUMN runtime_random_key BYTEA,
    ADD CONSTRAINT game_worlds_runtime_identity CHECK (
        (runtime_artifact_sha256 IS NULL AND runtime_random_key IS NULL)
        OR (runtime_artifact_sha256 IS NOT NULL AND runtime_random_key IS NOT NULL
            AND runtime_artifact_sha256 ~ '^[0-9a-f]{64}$'
            AND octet_length(runtime_random_key) = 32)
    );
