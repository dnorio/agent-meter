-- T-355: Pre-compute usd_cost on insertion for performance
-- Instead of calling compute_event_usd() on every aggregation query,
-- store the cost at insert time in a dedicated column.

ALTER TABLE agent_tool_calls ADD COLUMN IF NOT EXISTS usd_cost numeric(12,6);

-- Backfill existing rows
UPDATE agent_tool_calls
SET usd_cost = compute_event_usd(model, estimated_input_tokens, estimated_output_tokens, cached_tokens)
WHERE usd_cost IS NULL;

-- Index for cost aggregation queries
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_atc_usd_cost ON agent_tool_calls (started_at, usd_cost)
WHERE usd_cost IS NOT NULL;
