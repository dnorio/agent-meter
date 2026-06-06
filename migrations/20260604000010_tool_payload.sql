-- T-331: Capture tool call arguments (input) and result (output) for full agentic observability
ALTER TABLE agent_tool_calls
ADD COLUMN IF NOT EXISTS tool_arguments jsonb,
    ADD COLUMN IF NOT EXISTS tool_result text;
-- Index for filtering/searching by tool arguments (GIN for jsonb)
CREATE INDEX IF NOT EXISTS idx_agent_tool_calls_tool_arguments ON agent_tool_calls USING gin(tool_arguments)
WHERE tool_arguments IS NOT NULL;