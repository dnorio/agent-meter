ALTER TABLE agent_tool_calls
ADD COLUMN IF NOT EXISTS user_prompt text;
