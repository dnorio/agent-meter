-- T-319.1 / T-321: Auth & Billing fields
-- Adiciona campos para GitHub OAuth, sessions persistentes e Stripe billing.
-- Stripe billing nas orgs
ALTER TABLE organizations
ADD COLUMN IF NOT EXISTS stripe_customer_id text UNIQUE,
  ADD COLUMN IF NOT EXISTS stripe_subscription_id text,
  ADD COLUMN IF NOT EXISTS plan_status text NOT NULL DEFAULT 'active' CHECK (
    plan_status IN (
      'active',
      'past_due',
      'canceled',
      'trialing',
      'incomplete'
    )
  ),
  ADD COLUMN IF NOT EXISTS plan_renews_at timestamptz;
-- GitHub OAuth nos users
ALTER TABLE users
ADD COLUMN IF NOT EXISTS avatar_url text,
  ADD COLUMN IF NOT EXISTS github_login text;
CREATE INDEX IF NOT EXISTS idx_users_provider ON users(auth_provider, provider_id);
-- Sessions persistentes (cookie token → user)
CREATE TABLE IF NOT EXISTS sessions (
  token_hash text PRIMARY KEY,
  -- sha256 do session token (cookie)
  user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  org_id uuid REFERENCES organizations(id) ON DELETE
  SET NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL,
    last_seen_at timestamptz NOT NULL DEFAULT now(),
    user_agent text,
    ip text
);
CREATE INDEX IF NOT EXISTS idx_sessions_user ON sessions(user_id);
CREATE INDEX IF NOT EXISTS idx_sessions_expires ON sessions(expires_at);
-- Eventos de billing recebidos via Stripe webhook (auditoria + idempotência)
CREATE TABLE IF NOT EXISTS billing_events (
  id text PRIMARY KEY,
  -- stripe event id (evt_...)
  org_id uuid REFERENCES organizations(id) ON DELETE
  SET NULL,
    event_type text NOT NULL,
    payload jsonb NOT NULL,
    received_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_billing_events_org ON billing_events(org_id, received_at DESC);