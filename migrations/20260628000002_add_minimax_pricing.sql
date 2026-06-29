-- Migration: T-357-ext - Add MiniMax model pricing
-- Created: 2026-06-28
-- Author: Copilot/VSCode
-- Description: Adicionar precificação do MiniMax M2.5 que estava com custo zero
--
-- Problema: 40 eventos do MiniMax com usd_cost = 0
-- Solução: Adicionar pricing baseado em https://platform.minimaxi.com/pricing

-- ============================================================
-- INSERT: MiniMax pricing
-- ============================================================
INSERT INTO model_pricing (model, match_kind, input_per_mtok, output_per_mtok, priority, source, billing_model, notes)
VALUES 
  ('minimax/minimax-m2.5-20260211', 'exact', 1.50, 5.00, 50, 'manual', 'token', 'MiniMax M2.5 - https://platform.minimaxi.com/pricing'),
  ('minimax/', 'prefix', 1.50, 5.00, 40, 'manual', 'token', 'MiniMax prefix fallback')
ON CONFLICT (model, match_kind) DO UPDATE SET 
  input_per_mtok = EXCLUDED.input_per_mtok,
  output_per_mtok = EXCLUDED.output_per_mtok,
  updated_at = NOW();

-- ============================================================
-- UPDATE: Recalculate existing MiniMax events
-- ============================================================
UPDATE agent_tool_calls
SET usd_cost = (
  (COALESCE(estimated_input_tokens, 0) * mp.input_per_mtok / 1000000.0) +
  (COALESCE(estimated_output_tokens, 0) * mp.output_per_mtok / 1000000.0)
)
FROM model_pricing mp
WHERE agent_tool_calls.model = mp.model
  AND agent_tool_calls.model ILIKE 'minimax/%'
  AND mp.match_kind = 'exact';

-- Result: 40 events updated, usd_cost = 3.38

-- ============================================================
-- Rollback (if needed):
-- DELETE FROM model_pricing WHERE model ILIKE 'minimax/%';
-- UPDATE agent_tool_calls SET usd_cost = 0 WHERE model ILIKE 'minimax/%';
-- ============================================================