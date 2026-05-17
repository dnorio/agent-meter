create table agent_tasks (
  id bigserial primary key,
  task_id text not null unique,
  repo text,
  branch text,
  ide text,
  agent text,
  skill text,
  started_at timestamptz not null default now(),
  ended_at timestamptz,
  metadata jsonb not null default '{}'::jsonb
);

create table agent_tool_calls (
  id bigserial primary key,
  event_id uuid not null unique,
  task_id text,
  repo text,
  branch text,
  ide text,
  agent text,
  skill text,
  mcp_server text,
  tool_name text not null,
  started_at timestamptz not null,
  ended_at timestamptz not null,
  duration_ms integer not null,
  ok boolean not null,
  error text,
  request_bytes integer,
  response_bytes integer,
  estimated_input_tokens integer,
  estimated_output_tokens integer,
  estimated_total_tokens integer,
  request_sha256 text,
  response_sha256 text,
  metadata jsonb not null default '{}'::jsonb,
  created_at timestamptz not null default now()
);

create table agent_mcp_schemas (
  id bigserial primary key,
  schema_id uuid not null unique,
  mcp_server text not null,
  tool_name text,
  schema_sha256 text not null,
  schema_bytes integer not null,
  estimated_schema_tokens integer not null,
  metadata jsonb not null default '{}'::jsonb,
  created_at timestamptz not null default now()
);
