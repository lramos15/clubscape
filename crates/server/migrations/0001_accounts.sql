CREATE TABLE accounts (
    account_id UUID PRIMARY KEY,
    login_name TEXT COLLATE "C" NOT NULL,
    password_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    CONSTRAINT accounts_login_name_key UNIQUE (login_name),
    CONSTRAINT accounts_login_name_format CHECK (login_name ~ '^[a-z0-9_]{3,20}$'),
    CONSTRAINT accounts_password_hash_format CHECK (
        length(password_hash) BETWEEN 40 AND 512 AND password_hash LIKE '$argon2id$%'
    )
);

CREATE TABLE account_sessions (
    token_digest BYTEA PRIMARY KEY,
    account_id UUID NOT NULL REFERENCES accounts (account_id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT account_sessions_digest_size CHECK (octet_length(token_digest) = 32),
    CONSTRAINT account_sessions_expiry_order CHECK (expires_at > created_at)
);

CREATE INDEX account_sessions_account_created
    ON account_sessions (account_id, created_at DESC, token_digest DESC);
