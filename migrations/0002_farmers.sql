-- Farmers table (Phase 3: core agricultural identity)
CREATE TABLE farmers (
    address TEXT PRIMARY KEY,
    id TEXT NOT NULL UNIQUE,
    metadata JSONB NOT NULL,
    metadata_hash TEXT NOT NULL,
    registered BOOLEAN NOT NULL DEFAULT FALSE,
    created_ledger BIGINT,
    updated_ledger BIGINT,
    verification_markers JSONB NOT NULL DEFAULT '[]'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX farmers_id_idx ON farmers (id);

-- Document hash references (AD-004)
CREATE TABLE documents (
    id TEXT PRIMARY KEY,              -- va:doc:<sha256-hex>
    owner_address TEXT NOT NULL REFERENCES farmers(address) ON DELETE CASCADE,
    content_hash TEXT NOT NULL,
    kind TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX documents_owner_idx ON documents (owner_address);