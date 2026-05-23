-- Report-focused indexes to reduce query cost on low-resource nodes.
-- Keep index count small to avoid heavy insert overhead.

CREATE INDEX IF NOT EXISTS idx_agent_tool_calls_report_dims_time
  ON agent_tool_calls (ide, agent, model, started_at DESC);

CREATE INDEX IF NOT EXISTS idx_agent_tool_calls_conversation_started_at
  ON agent_tool_calls (conversation_id, started_at DESC)
  WHERE conversation_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_agent_tool_calls_tool_model
  ON agent_tool_calls (mcp_server, tool_name, model)
  WHERE model IS NOT NULL;
