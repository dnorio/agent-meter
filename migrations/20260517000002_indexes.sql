create index idx_agent_tool_calls_task_id on agent_tool_calls(task_id);
create index idx_agent_tool_calls_started_at on agent_tool_calls(started_at);
create index idx_agent_tool_calls_tool on agent_tool_calls(mcp_server, tool_name);
create index idx_agent_tool_calls_ide on agent_tool_calls(ide);
create index idx_agent_tool_calls_agent on agent_tool_calls(agent);
create index idx_agent_tool_calls_skill on agent_tool_calls(skill);
create index idx_agent_tool_calls_metadata_gin on agent_tool_calls using gin(metadata);
