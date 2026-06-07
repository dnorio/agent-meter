-- Migration 12: Performance indices + pricing corrections
-- 2026-06-07

-- ============================================================
-- 1. Missing composite indices for query performance
-- ============================================================

-- Conversations list: correlated subquery for title/initial_prompt
-- uses (conversation_id, started_at ASC) to find first user_prompt
CREATE INDEX IF NOT EXISTS idx_atc_conversation_started
  ON agent_tool_calls (conversation_id, started_at ASC)
  WHERE conversation_id IS NOT NULL;

-- Events feed keyset pagination: ORDER BY started_at DESC, event_id DESC
CREATE INDEX IF NOT EXISTS idx_atc_events_feed
  ON agent_tool_calls (started_at DESC, event_id DESC);

-- Repo filter used in top_tools, top_tasks, top_mcp_servers, calls_over_time
CREATE INDEX IF NOT EXISTS idx_atc_repo
  ON agent_tool_calls (repo)
  WHERE repo IS NOT NULL;

-- ============================================================
-- 2. Fix GPT-5.x pricing (was generic "gpt-5", now split by variant)
-- ============================================================

-- Remove the generic gpt-5 entry
DELETE FROM model_pricing WHERE model = 'gpt-5';

-- GPT-5.4: $2.50 input, $15.00 output, $0.25 cached (openai.com/api/pricing)
INSERT INTO model_pricing (model, match_kind, input_per_mtok, output_per_mtok, cached_per_mtok, priority, source, notes)
VALUES ('gpt-5.4', 'prefix', 2.500000, 15.000000, 0.250000, 10, 'openai', 'GPT-5.4 (Jun 2026)')
ON CONFLICT (model, match_kind) DO UPDATE
  SET input_per_mtok = EXCLUDED.input_per_mtok,
      output_per_mtok = EXCLUDED.output_per_mtok,
      cached_per_mtok = EXCLUDED.cached_per_mtok,
      notes = EXCLUDED.notes;

-- GPT-5.4 mini: $0.75 input, $4.50 output, $0.075 cached
INSERT INTO model_pricing (model, match_kind, input_per_mtok, output_per_mtok, cached_per_mtok, priority, source, notes)
VALUES ('gpt-5.4-mini', 'prefix', 0.750000, 4.500000, 0.075000, 10, 'openai', 'GPT-5.4 mini (Jun 2026)')
ON CONFLICT (model, match_kind) DO UPDATE
  SET input_per_mtok = EXCLUDED.input_per_mtok,
      output_per_mtok = EXCLUDED.output_per_mtok,
      cached_per_mtok = EXCLUDED.cached_per_mtok,
      notes = EXCLUDED.notes;

-- GPT-5.5: $5.00 input, $30.00 output, $0.50 cached
INSERT INTO model_pricing (model, match_kind, input_per_mtok, output_per_mtok, cached_per_mtok, priority, source, notes)
VALUES ('gpt-5.5', 'prefix', 5.000000, 30.000000, 0.500000, 10, 'openai', 'GPT-5.5 (Jun 2026)')
ON CONFLICT (model, match_kind) DO UPDATE
  SET input_per_mtok = EXCLUDED.input_per_mtok,
      output_per_mtok = EXCLUDED.output_per_mtok,
      cached_per_mtok = EXCLUDED.cached_per_mtok,
      notes = EXCLUDED.notes;

-- GPT-5.3 Instant (if used): $1.00 input, $4.00 output, $0.10 cached (estimated)
INSERT INTO model_pricing (model, match_kind, input_per_mtok, output_per_mtok, cached_per_mtok, priority, source, notes)
VALUES ('gpt-5.3', 'prefix', 1.000000, 4.000000, 0.100000, 10, 'openai', 'GPT-5.3 Instant (Jun 2026, estimated)')
ON CONFLICT (model, match_kind) DO UPDATE
  SET input_per_mtok = EXCLUDED.input_per_mtok,
      output_per_mtok = EXCLUDED.output_per_mtok,
      cached_per_mtok = EXCLUDED.cached_per_mtok,
      notes = EXCLUDED.notes;
