-- T-319: Multi-tenant scaffolding (MVP)
-- Tabelas para organizações e API keys. Sem auth gating ativado por padrão —
-- aplicação funciona como antes, com tudo atribuído à org "personal".

CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE IF NOT EXISTS organizations (
    id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    slug        text NOT NULL UNIQUE,
    name        text NOT NULL,
    plan        text NOT NULL DEFAULT 'free' CHECK (plan IN ('free','pro','team','enterprise')),
    created_at  timestamptz NOT NULL DEFAULT now(),
    metadata    jsonb NOT NULL DEFAULT '{}'::jsonb
);

CREATE TABLE IF NOT EXISTS users (
    id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    email           text NOT NULL UNIQUE,
    display_name    text,
    -- 'github', 'google', 'local'
    auth_provider   text NOT NULL DEFAULT 'local',
    provider_id     text,
    created_at      timestamptz NOT NULL DEFAULT now(),
    last_login_at   timestamptz
);

CREATE TABLE IF NOT EXISTS memberships (
    id          bigserial PRIMARY KEY,
    org_id      uuid NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    user_id     uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role        text NOT NULL DEFAULT 'member' CHECK (role IN ('owner','admin','member','viewer')),
    created_at  timestamptz NOT NULL DEFAULT now(),
    UNIQUE (org_id, user_id)
);

CREATE TABLE IF NOT EXISTS api_keys (
    id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          uuid NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    key_prefix      text NOT NULL UNIQUE,            -- "am_live_abc12" (primeiros 12 chars)
    key_hash        text NOT NULL,                   -- sha256 do segredo completo
    name            text NOT NULL DEFAULT 'default',
    created_at      timestamptz NOT NULL DEFAULT now(),
    last_used_at    timestamptz,
    revoked_at      timestamptz
);

CREATE INDEX IF NOT EXISTS idx_api_keys_org ON api_keys(org_id);
CREATE INDEX IF NOT EXISTS idx_api_keys_active ON api_keys(key_prefix) WHERE revoked_at IS NULL;

-- Adiciona org_id em agent_tool_calls e agent_tasks (nullable; default org via app)
ALTER TABLE agent_tool_calls ADD COLUMN IF NOT EXISTS org_id uuid;
ALTER TABLE agent_tasks      ADD COLUMN IF NOT EXISTS org_id uuid;

CREATE INDEX IF NOT EXISTS idx_agent_tool_calls_org ON agent_tool_calls(org_id, started_at DESC);

-- Seed: org "personal" + key default. Usado para backward-compat (todos eventos
-- existentes ficam atribuídos a esta org via UPDATE abaixo).
INSERT INTO organizations (slug, name, plan)
    VALUES ('personal', 'Personal', 'free')
    ON CONFLICT (slug) DO NOTHING;

UPDATE agent_tool_calls SET org_id = (SELECT id FROM organizations WHERE slug = 'personal')
WHERE org_id IS NULL;

UPDATE agent_tasks SET org_id = (SELECT id FROM organizations WHERE slug = 'personal')
WHERE org_id IS NULL;
