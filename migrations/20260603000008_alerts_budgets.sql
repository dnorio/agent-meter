-- T-320: Alerts & Budgets (MVP)

CREATE TABLE IF NOT EXISTS alert_rules (
    id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          uuid REFERENCES organizations(id) ON DELETE CASCADE,
    name            text NOT NULL,
    -- 'cost_spike' | 'error_rate' | 'latency_p95' | 'token_burn' | 'tool_failure'
    rule_type       text NOT NULL,
    -- janela em minutos
    window_minutes  integer NOT NULL DEFAULT 60,
    threshold       numeric(20,6) NOT NULL,
    comparator      text NOT NULL DEFAULT '>' CHECK (comparator IN ('>','>=','<','<=')),
    -- filtros opcionais (JSON: {ide, agent, model, ...})
    filters         jsonb NOT NULL DEFAULT '{}'::jsonb,
    enabled         boolean NOT NULL DEFAULT true,
    -- silenciar nova notificação por X minutos após disparo
    cooldown_minutes integer NOT NULL DEFAULT 60,
    created_at      timestamptz NOT NULL DEFAULT now(),
    updated_at      timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_alert_rules_org_enabled ON alert_rules(org_id, enabled);

CREATE TABLE IF NOT EXISTS budgets (
    id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          uuid REFERENCES organizations(id) ON DELETE CASCADE,
    name            text NOT NULL,
    -- 'daily' | 'weekly' | 'monthly'
    period          text NOT NULL DEFAULT 'monthly' CHECK (period IN ('daily','weekly','monthly')),
    amount_usd      numeric(12,2) NOT NULL,
    soft_threshold_pct  numeric(5,2) NOT NULL DEFAULT 80,    -- avisa em 80%
    hard_threshold_pct  numeric(5,2) NOT NULL DEFAULT 100,   -- bloqueia em 100% se hard_cap=true
    hard_cap        boolean NOT NULL DEFAULT false,
    filters         jsonb NOT NULL DEFAULT '{}'::jsonb,
    enabled         boolean NOT NULL DEFAULT true,
    created_at      timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS notification_channels (
    id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id      uuid REFERENCES organizations(id) ON DELETE CASCADE,
    name        text NOT NULL,
    -- 'slack' | 'email' | 'webhook'
    kind        text NOT NULL CHECK (kind IN ('slack','email','webhook')),
    -- config JSON: slack→{webhook_url}; email→{to}; webhook→{url, headers}
    config      jsonb NOT NULL DEFAULT '{}'::jsonb,
    enabled     boolean NOT NULL DEFAULT true,
    created_at  timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS alert_history (
    id              bigserial PRIMARY KEY,
    rule_id         uuid REFERENCES alert_rules(id) ON DELETE CASCADE,
    org_id          uuid REFERENCES organizations(id) ON DELETE CASCADE,
    fired_at        timestamptz NOT NULL DEFAULT now(),
    observed_value  numeric(20,6) NOT NULL,
    threshold       numeric(20,6) NOT NULL,
    severity        text NOT NULL DEFAULT 'warning' CHECK (severity IN ('info','warning','critical')),
    payload         jsonb NOT NULL DEFAULT '{}'::jsonb,
    notified        boolean NOT NULL DEFAULT false
);

CREATE INDEX IF NOT EXISTS idx_alert_history_rule ON alert_history(rule_id, fired_at DESC);
CREATE INDEX IF NOT EXISTS idx_alert_history_org_fired ON alert_history(org_id, fired_at DESC);
