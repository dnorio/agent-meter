-- T-332: Deep telemetry — reasoning tokens, finish reason, context config, OTLP span hierarchy
-- Every field needed to understand full consumption and agent decision flow.
ALTER TABLE agent_tool_calls
ADD COLUMN IF NOT EXISTS reasoning_tokens int,
  -- o1/o3/Claude thinking tokens (billed separately!)
ADD COLUMN IF NOT EXISTS finish_reason text,
  -- stop/length/tool_calls/content_filter
ADD COLUMN IF NOT EXISTS request_max_tokens int,
  -- gen_ai.request.max_tokens
ADD COLUMN IF NOT EXISTS request_temperature float8,
  -- gen_ai.request.temperature
ADD COLUMN IF NOT EXISTS llm_system text,
  -- openai/anthropic/google_genai/etc.
ADD COLUMN IF NOT EXISTS trace_id text,
  -- OTLP traceId (groups spans in one round)
ADD COLUMN IF NOT EXISTS span_id text,
  -- OTLP spanId
ADD COLUMN IF NOT EXISTS parent_span_id text,
  -- OTLP parentSpanId (tool → LLM link)
ADD COLUMN IF NOT EXISTS tool_call_id text;
-- LLM-assigned tool call ID
-- Indexes for common query patterns
CREATE INDEX IF NOT EXISTS idx_atc_trace_id ON agent_tool_calls(trace_id)
WHERE trace_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_atc_finish_reason ON agent_tool_calls(finish_reason)
WHERE finish_reason IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_atc_llm_system ON agent_tool_calls(llm_system)
WHERE llm_system IS NOT NULL;