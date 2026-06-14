-- Fix: Copilot pricing overcharge correction
--
-- Problem: Model names with hyphens (claude-opus-4-7, claude-sonnet-4-6)
-- had no exact pricing entry, falling back to prefix 'claude-opus-4' at $15/$75
-- (direct API rate) instead of the correct Copilot rate ($5/$25).
-- Additionally, early events for 'claude-opus-4.6' were computed BEFORE
-- the exact pricing entry existed, using the $15/$75 prefix rate.
--
-- Total historical overcharge: ~$439

-- 1. Add missing exact pricing entries for hyphenated model variants
INSERT INTO model_pricing (model, match_kind, input_per_mtok, output_per_mtok, cached_per_mtok, priority, source, billing_model, notes)
VALUES
    ('claude-opus-4-7',  'exact', 5.00, 25.00, 0.50, 10, 'github_copilot', 'copilot_credit', 'Copilot rate; hyphenated variant'),
    ('claude-opus-4-6',  'exact', 5.00, 25.00, 0.50, 10, 'github_copilot', 'copilot_credit', 'Copilot rate; hyphenated variant (future-proof)'),
    ('claude-sonnet-4-6','exact', 3.00, 15.00, 0.30, 10, 'github_copilot', 'copilot_credit', 'Copilot rate; hyphenated variant')
ON CONFLICT (model, match_kind) DO UPDATE SET
    input_per_mtok = EXCLUDED.input_per_mtok,
    output_per_mtok = EXCLUDED.output_per_mtok,
    cached_per_mtok = EXCLUDED.cached_per_mtok,
    source = EXCLUDED.source,
    billing_model = EXCLUDED.billing_model,
    notes = EXCLUDED.notes,
    updated_at = now();

-- 2. Recalculate usd_cost for ALL events using current (correct) pricing
-- This fixes both: historical overcharge on opus-4.6 AND ongoing overcharge on opus-4-7
UPDATE agent_tool_calls
SET usd_cost = compute_event_usd(model, estimated_input_tokens, estimated_output_tokens, COALESCE(cached_tokens, 0)::int)
WHERE model IS NOT NULL AND length(model) > 0;
