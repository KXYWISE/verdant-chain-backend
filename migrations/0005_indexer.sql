-- Phase 9: event indexer raw store + cursors + projections
-- Plan: docs/architecture/event-indexing.md (Accepted, Agent #4 2026-08-18)
-- Decisions:
--   - Dedicated `indexer` schema (same DB, one connection pool).
--   - Keep-all raw events: append-only source of truth (AD-004); no pruning.
--   - 10-ledger trusted cutoff for projection finality.

CREATE SCHEMA IF NOT EXISTS indexer;

-- Append-only raw event store (source of truth). Natural key is
-- (contract_id, ledger_sequence, operation_index, event_index) so replays
-- and re-org re-ingestion are idempotent (INSERT ... ON CONFLICT DO NOTHING).
CREATE TABLE IF NOT EXISTS indexer.indexed_events (
    id BIGSERIAL PRIMARY KEY,
    contract_id TEXT NOT NULL,
    contract_name TEXT NOT NULL,            -- identity | verification | escrow | financing
    event_name TEXT NOT NULL,               -- e.g. VerificationCreated
    ledger_sequence BIGINT NOT NULL,
    operation_index INT NOT NULL,
    event_index INT NOT NULL,
    topics JSONB NOT NULL,                  -- decoded topics
    data JSONB NOT NULL,                    -- decoded payload
    ingested_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (contract_id, ledger_sequence, operation_index, event_index)
);

CREATE INDEX IF NOT EXISTS indexed_events_contract_ledger_idx
    ON indexer.indexed_events (contract_id, ledger_sequence, operation_index, event_index);

-- Per-contract monotonic ingestion cursor (resume point).
CREATE TABLE IF NOT EXISTS indexer.indexer_cursors (
    contract_id TEXT PRIMARY KEY,
    ledger_sequence BIGINT NOT NULL,
    operation_index INT NOT NULL,
    event_index INT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Verification projection (AgroProof). One row per va:verification:<12-digit>.
CREATE TABLE IF NOT EXISTS indexer.verification_projection (
    id TEXT PRIMARY KEY,                    -- va:verification:<12-digit>
    contract_id TEXT NOT NULL,
    batch_id TEXT NOT NULL,                 -- va:batch:<uuidv7>
    subject TEXT NOT NULL,
    proof_hash TEXT NOT NULL,
    issuer TEXT NOT NULL,
    issued_ledger BIGINT NOT NULL,
    revoked BOOLEAN NOT NULL DEFAULT FALSE,
    revoked_ledger BIGINT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS verification_projection_batch_idx
    ON indexer.verification_projection (batch_id);
CREATE INDEX IF NOT EXISTS verification_projection_subject_idx
    ON indexer.verification_projection (subject);

-- Escrow / booking projection (AgriLease). One row per va:escrow:<12-digit>.
CREATE TABLE IF NOT EXISTS indexer.escrow_projection (
    id TEXT PRIMARY KEY,                    -- va:escrow:<12-digit>
    contract_id TEXT NOT NULL,
    depositor TEXT NOT NULL,
    beneficiary TEXT NOT NULL,
    amount BIGINT NOT NULL,                 -- i128 on-chain; BIGINT at projection boundary
    released_amount BIGINT NOT NULL DEFAULT 0,
    booking_ref TEXT,                       -- va:booking:<uuidv7>
    condition_kind INT NOT NULL,            -- 0 = Manual, 1 = Milestone, 2 = Timeout
    condition_releaser TEXT,
    condition_timeout_ledger BIGINT,
    created_ledger BIGINT NOT NULL,
    updated_ledger BIGINT NOT NULL,
    status TEXT NOT NULL DEFAULT 'open',    -- open | released | refunded
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS escrow_projection_booking_idx
    ON indexer.escrow_projection (booking_ref);
CREATE INDEX IF NOT EXISTS escrow_projection_depositor_idx
    ON indexer.escrow_projection (depositor);
CREATE INDEX IF NOT EXISTS escrow_projection_beneficiary_idx
    ON indexer.escrow_projection (beneficiary);

-- Financing projection (FarmFund) intentionally NOT created yet: schema is
-- provisional until docs/contracts/financing.md is accepted (Agent #4,
-- 2026-08-18 --event-indexing-decisions.md).

-- Re-org runbook (10-ledger trusted cutoff, Agent #4 decision):
-- 1. Projection writes finalize only for ledger_sequence <= head - 10.
-- 2. On rewind: DELETE indexer.indexed_events for the contract where
--    ledger_sequence > first divergent ledger; reset indexer_cursors to that
--    ledger; re-ingest (idempotent upserts on the natural key).
-- 3. Divergence deeper than the cutoff requires a full re-index from a trusted
--    snapshot (no automated path in v1; runbook only).