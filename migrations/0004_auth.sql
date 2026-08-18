-- SEP-40/SEP-10 wallet auth (docs/architecture/auth-flow.md)
-- Challenge nonces are single-use, stored server-side with TTL.
CREATE TABLE IF NOT EXISTS auth_challenges (
    nonce TEXT PRIMARY KEY,
    domain TEXT NOT NULL,
    address TEXT NOT NULL,
    issued_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,
    used BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE INDEX IF NOT EXISTS auth_challenges_address_domain_idx
    ON auth_challenges (address, domain);

-- Backend session records keyed by the Stellar public key (G…).
-- token_hash stores SHA-256 of the opaque bearer token (never the raw token).
CREATE TABLE IF NOT EXISTS auth_sessions (
    id BIGSERIAL PRIMARY KEY,
    token_hash TEXT NOT NULL UNIQUE,
    address TEXT NOT NULL,
    roles TEXT[] NOT NULL DEFAULT '{farmer}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS auth_sessions_address_idx ON auth_sessions (address);