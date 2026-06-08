-- T-357: Add billing_model and credits_per_request to model_pricing
-- Allows distinguishing token-based billing (API) from subscription/credit-based (Copilot, Cursor Pro)

ALTER TABLE model_pricing
    ADD COLUMN IF NOT EXISTS billing_model text NOT NULL DEFAULT 'token'
        CHECK (billing_model IN ('token', 'credit', 'subscription')),
    ADD COLUMN IF NOT EXISTS credits_per_request numeric(8,2) DEFAULT NULL;

-- Mark subscription-based models (GitHub Copilot, Cursor built-in)
UPDATE model_pricing
SET billing_model = 'subscription', credits_per_request = 1
WHERE model ILIKE '%copilot%'
   OR source IN ('copilot', 'cursor');

-- Add column to agent_tool_calls for fast filtering
ALTER TABLE agent_tool_calls
    ADD COLUMN IF NOT EXISTS billing_model text NOT NULL DEFAULT 'token';

-- Backfill: mark events from Copilot/Cursor IDEs as subscription
UPDATE agent_tool_calls
SET billing_model = 'subscription'
WHERE ide IN ('vscode', 'cursor')
  AND billing_model = 'token';

-- Index for billing_model filtering
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_atc_billing_model
    ON agent_tool_calls (billing_model, started_at);
