ALTER TABLE agent_tool_calls
  ADD COLUMN IF NOT EXISTS model text,
  ADD COLUMN IF NOT EXISTS cached_tokens integer,
  ADD COLUMN IF NOT EXISTS conversation_id text,
  ADD COLUMN IF NOT EXISTS client_ip text,
  ADD COLUMN IF NOT EXISTS user_agent text;

CREATE INDEX IF NOT EXISTS idx_agent_tool_calls_model ON agent_tool_calls(model);
CREATE INDEX IF NOT EXISTS idx_agent_tool_calls_conversation ON agent_tool_calls(conversation_id);
