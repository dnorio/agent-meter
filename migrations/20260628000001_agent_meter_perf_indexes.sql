-- Migration: T-361 - agent-meter database performance indexes
-- Created: 2026-06-28
-- Author: Copilot/VSCode
-- Description: Otimizar queries analíticas do agent-meter
--
-- Performance improvement:
-- - Tasks leaderboard: 354ms → 44ms (8x faster)
-- - Events feed: 2.7ms (already fast)
-- - Cost summary: already has idx_atc_usd_cost

-- ============================================================
-- INDEX 1: Conversation stats covering index
-- Purpose: Optimize GROUP BY conversation_id queries
-- Used by: /api/tasks, /api/conversations leaderboards
-- ============================================================
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_atc_conversation_covering
ON agent_tool_calls (conversation_id, started_at DESC)
INCLUDE (estimated_total_tokens, duration_ms, ok, tool_name)
WHERE conversation_id IS NOT NULL;

-- ============================================================
-- INDEX 2: Conversation tool stats
-- Purpose: Optimize COUNT(DISTINCT tool_name) in GROUP BY
-- ============================================================
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_atc_conversation_tool_stats
ON agent_tool_calls (conversation_id, tool_name, started_at DESC)
WHERE conversation_id IS NOT NULL;

-- ============================================================
-- Rollback (if needed):
-- DROP INDEX IF EXISTS idx_atc_conversation_covering;
-- DROP INDEX IF EXISTS idx_atc_conversation_tool_stats;
-- ============================================================