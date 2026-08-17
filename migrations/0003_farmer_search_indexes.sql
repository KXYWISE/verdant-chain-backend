-- Indexes for farmer search (AD-010: AgriScout directory search)
-- Enable pg_trgm extension for trigram-based ILIKE search (must be first)
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- GIN indexes on JSONB metadata fields for ILIKE substring search
CREATE INDEX IF NOT EXISTS farmers_metadata_name_idx ON farmers USING GIN ((metadata->>'name') gin_trgm_ops);
CREATE INDEX IF NOT EXISTS farmers_metadata_region_idx ON farmers USING GIN ((metadata->>'region') gin_trgm_ops);
CREATE INDEX IF NOT EXISTS farmers_metadata_district_idx ON farmers USING GIN ((metadata->>'district') gin_trgm_ops);