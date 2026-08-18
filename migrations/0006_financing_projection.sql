-- Phase 9: financing projection (FarmFund) — schema locked 2026-08-18
-- Design: docs/contracts/financing.md (Accepted v1.0, Agent #4)
-- Ingested from FinancingCreated / FinancingDeposited / FinancingReleased /
-- FinancingRefunded per the accepted event spec.

CREATE TABLE IF NOT EXISTS indexer.financing_projection (
    id TEXT PRIMARY KEY,                    -- va:financing:<12-digit>
    contract_id TEXT NOT NULL,
    funder TEXT NOT NULL,
    beneficiary TEXT NOT NULL,
    total_amount BIGINT NOT NULL,           -- i128 on-chain; BIGINT at projection boundary
    drawn_amount BIGINT NOT NULL DEFAULT 0, -- cumulative drawdown; deposits increment it
    milestone_count INT NOT NULL DEFAULT 0,
    milestones JSONB NOT NULL DEFAULT '[]'::jsonb,
    drawn_ledger BIGINT,
    repaid_amount BIGINT NOT NULL DEFAULT 0,
    defaulted BOOLEAN NOT NULL DEFAULT FALSE,
    defaulted_ledger BIGINT,
    updated_ledger BIGINT,
    status TEXT NOT NULL DEFAULT 'active',  -- active | defaulted | refunded | closed
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS financing_projection_funder_idx
    ON indexer.financing_projection (funder);
CREATE INDEX IF NOT EXISTS financing_projection_beneficiary_idx
    ON indexer.financing_projection (beneficiary);