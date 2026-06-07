-- T-357 fix: Update model_pricing to reflect real June 2026 pricing rules
-- 
-- KEY INSIGHT: Both GitHub Copilot and Cursor now use TOKEN-BASED billing.
-- There is NO "subscription = free" model anymore.
-- 
-- GitHub Copilot: 1 AI Credit = $0.01 USD, charged per token used
--   Plans include allowance: Pro=$15, Pro+=$70, Max=$200 monthly
--   Code completions are FREE (unlimited) — not billed
--   All chat/agent/CLI requests are billed per token
--
-- Cursor: Usage-based per token
--   Two pools: Auto+Composer ($1.25in/$6out) and API (model rate)
--   Pro=$20/mo includes $20 usage; Pro+=$60 includes $70; Ultra=$200 includes $400
--   Tab completions are FREE (unlimited)
--
-- Conclusion: billing_model should reflect the PRICING SOURCE, not "free/paid"
-- 'token' = standard API pricing (Anthropic/OpenAI direct)  
-- 'copilot_credit' = GitHub Copilot AI Credits pricing (per-token, slightly different rates)
-- 'cursor_usage' = Cursor usage-based pricing (per-token, two pools)

-- Drop the old billing_model constraint and update
ALTER TABLE model_pricing DROP CONSTRAINT IF EXISTS model_pricing_billing_model_check;
ALTER TABLE agent_tool_calls DROP CONSTRAINT IF EXISTS agent_tool_calls_billing_model_check;

-- New valid values for billing_model
ALTER TABLE model_pricing ADD CONSTRAINT model_pricing_billing_model_check 
    CHECK (billing_model IN ('token', 'copilot_credit', 'cursor_usage'));

-- Reset ALL events to 'token' — the correct default since everything is token-priced
UPDATE agent_tool_calls SET billing_model = 'token' WHERE billing_model = 'subscription';

-- Now reclassify based on IDE/source:
-- copilot-vscode → copilot_credit (billed in GitHub AI Credits at GitHub's rates)
UPDATE agent_tool_calls SET billing_model = 'copilot_credit'
WHERE ide ILIKE '%copilot%' OR ide ILIKE '%vscode%';

-- cursor → cursor_usage (billed per token in Cursor's usage pools)
UPDATE agent_tool_calls SET billing_model = 'cursor_usage'
WHERE ide ILIKE '%cursor%';

-- Update model_pricing: add newer models with Copilot-specific pricing
-- (GitHub charges slightly different from direct API — these are Copilot rates)
INSERT INTO model_pricing (model, match_kind, input_per_mtok, output_per_mtok, cached_per_mtok, priority, source, billing_model, notes)
VALUES
    -- Anthropic (Copilot rates - includes cache write cost)
    ('claude-haiku-4.5', 'exact', 1.00, 5.00, 0.10, 10, 'github_copilot', 'copilot_credit', 'Copilot rate; cache_write=$1.25/Mtok'),
    ('claude-sonnet-4.5', 'exact', 3.00, 15.00, 0.30, 10, 'github_copilot', 'copilot_credit', 'Copilot rate; cache_write=$3.75/Mtok'),
    ('claude-sonnet-4.6', 'exact', 3.00, 15.00, 0.30, 10, 'github_copilot', 'copilot_credit', 'Copilot rate; cache_write=$3.75/Mtok'),
    ('claude-opus-4.5', 'exact', 5.00, 25.00, 0.50, 10, 'github_copilot', 'copilot_credit', 'Copilot rate; cache_write=$6.25/Mtok'),
    ('claude-opus-4.6', 'exact', 5.00, 25.00, 0.50, 10, 'github_copilot', 'copilot_credit', 'Copilot rate; cache_write=$6.25/Mtok'),
    ('claude-opus-4.7', 'exact', 5.00, 25.00, 0.50, 10, 'github_copilot', 'copilot_credit', 'Copilot rate; cache_write=$6.25/Mtok'),
    ('claude-opus-4.8', 'exact', 5.00, 25.00, 0.50, 10, 'github_copilot', 'copilot_credit', 'Copilot rate; cache_write=$6.25/Mtok'),
    -- OpenAI (Copilot rates)
    ('gpt-5-mini', 'exact', 0.25, 2.00, 0.025, 10, 'github_copilot', 'copilot_credit', NULL),
    ('gpt-5.3-codex', 'exact', 1.75, 14.00, 0.175, 10, 'github_copilot', 'copilot_credit', NULL),
    ('gpt-5.4-nano', 'exact', 0.20, 1.25, 0.02, 10, 'github_copilot', 'copilot_credit', NULL),
    -- Google (Copilot rates)
    ('gemini-2.5-pro', 'exact', 1.25, 10.00, 0.125, 10, 'github_copilot', 'copilot_credit', NULL),
    ('gemini-3-flash', 'exact', 0.50, 3.00, 0.05, 10, 'github_copilot', 'copilot_credit', NULL),
    ('gemini-3.1-pro', 'exact', 2.00, 12.00, 0.20, 10, 'github_copilot', 'copilot_credit', NULL),
    ('gemini-3.5-flash', 'exact', 1.50, 9.00, 0.15, 10, 'github_copilot', 'copilot_credit', NULL),
    -- Microsoft/GitHub (Copilot rates)
    ('raptor-mini', 'exact', 0.25, 2.00, 0.025, 10, 'github_copilot', 'copilot_credit', 'GitHub fine-tuned model'),
    ('mai-code-1-flash', 'exact', 0.75, 4.50, 0.075, 10, 'github_copilot', 'copilot_credit', 'Microsoft model'),
    -- Cursor-specific models
    ('composer-2.5', 'exact', 0.50, 2.50, 0.20, 10, 'cursor', 'cursor_usage', 'Cursor own model'),
    ('composer-2.5-fast', 'exact', 3.00, 15.00, 0.50, 10, 'cursor', 'cursor_usage', 'Cursor fast variant'),
    ('grok-build-0.1', 'exact', 1.00, 2.00, 0.20, 10, 'cursor', 'cursor_usage', 'xAI via Cursor')
ON CONFLICT (model, match_kind) DO UPDATE SET
    input_per_mtok = EXCLUDED.input_per_mtok,
    output_per_mtok = EXCLUDED.output_per_mtok,
    cached_per_mtok = EXCLUDED.cached_per_mtok,
    source = EXCLUDED.source,
    billing_model = EXCLUDED.billing_model,
    notes = EXCLUDED.notes,
    updated_at = now();

-- Update existing models to correct Copilot pricing where applicable
UPDATE model_pricing SET
    input_per_mtok = 3.00, output_per_mtok = 15.00, cached_per_mtok = 0.30,
    source = 'github_copilot', billing_model = 'copilot_credit', updated_at = now()
WHERE model IN ('claude-sonnet-4') AND source != 'github_copilot';

-- Fix GPT-5.3 → match Copilot's "GPT-5.3-Codex" rate
UPDATE model_pricing SET
    input_per_mtok = 1.75, output_per_mtok = 14.00, cached_per_mtok = 0.175,
    source = 'github_copilot', billing_model = 'copilot_credit', updated_at = now()
WHERE model = 'gpt-5.3';

-- Update gpt-5.4 to Copilot rate
UPDATE model_pricing SET
    input_per_mtok = 2.50, output_per_mtok = 15.00, cached_per_mtok = 0.25,
    source = 'github_copilot', billing_model = 'copilot_credit', updated_at = now()
WHERE model = 'gpt-5.4';

-- Rebuild idx for new billing_model values
DROP INDEX IF EXISTS idx_atc_billing_model;
CREATE INDEX idx_atc_billing_model ON agent_tool_calls (billing_model, started_at);
