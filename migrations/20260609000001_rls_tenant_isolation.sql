-- T-322: Row-Level Security for multi-tenant isolation
-- Enables RLS on tenant-scoped tables. The application must SET LOCAL
-- app.current_org_id before executing queries when REQUIRE_API_KEY is on.
-- Until then, the superuser bypasses RLS automatically (backward-compat).

-- 1. Enable RLS on data tables
ALTER TABLE agent_tool_calls ENABLE ROW LEVEL SECURITY;
ALTER TABLE agent_tasks       ENABLE ROW LEVEL SECURITY;
ALTER TABLE alert_rules       ENABLE ROW LEVEL SECURITY;
ALTER TABLE budgets           ENABLE ROW LEVEL SECURITY;
ALTER TABLE notification_channels ENABLE ROW LEVEL SECURITY;
ALTER TABLE billing_events    ENABLE ROW LEVEL SECURITY;

-- 2. Create an application role (least-privilege for the API server)
DO $$
BEGIN
  IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'agent_meter_app') THEN
    CREATE ROLE agent_meter_app LOGIN;
  END IF;
END $$;

-- Grant necessary table permissions to the app role
GRANT SELECT, INSERT, UPDATE, DELETE ON agent_tool_calls TO agent_meter_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON agent_tasks TO agent_meter_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON alert_rules TO agent_meter_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON budgets TO agent_meter_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON notification_channels TO agent_meter_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON billing_events TO agent_meter_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON organizations TO agent_meter_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON users TO agent_meter_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON memberships TO agent_meter_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON api_keys TO agent_meter_app;
GRANT USAGE ON ALL SEQUENCES IN SCHEMA public TO agent_meter_app;

-- 3. RLS Policies — filter by app.current_org_id session variable
-- The superuser (owner role) always bypasses RLS, so existing queries
-- continue to work until the app explicitly sets the session variable.

CREATE POLICY tenant_isolation ON agent_tool_calls
  USING (org_id = current_setting('app.current_org_id', true)::uuid)
  WITH CHECK (org_id = current_setting('app.current_org_id', true)::uuid);

CREATE POLICY tenant_isolation ON agent_tasks
  USING (org_id = current_setting('app.current_org_id', true)::uuid)
  WITH CHECK (org_id = current_setting('app.current_org_id', true)::uuid);

CREATE POLICY tenant_isolation ON alert_rules
  USING (org_id = current_setting('app.current_org_id', true)::uuid)
  WITH CHECK (org_id = current_setting('app.current_org_id', true)::uuid);

CREATE POLICY tenant_isolation ON budgets
  USING (org_id = current_setting('app.current_org_id', true)::uuid)
  WITH CHECK (org_id = current_setting('app.current_org_id', true)::uuid);

CREATE POLICY tenant_isolation ON notification_channels
  USING (org_id = current_setting('app.current_org_id', true)::uuid)
  WITH CHECK (org_id = current_setting('app.current_org_id', true)::uuid);

CREATE POLICY tenant_isolation ON billing_events
  USING (org_id = current_setting('app.current_org_id', true)::uuid)
  WITH CHECK (org_id = current_setting('app.current_org_id', true)::uuid);

-- 4. Allow-all policy for the table owner (superuser bypass is implicit,
-- but if the app connects as agent_meter_app we need explicit access
-- when org_id IS NULL for backward-compat during migration period)
CREATE POLICY allow_null_org ON agent_tool_calls
  FOR ALL TO agent_meter_app
  USING (org_id IS NULL);

CREATE POLICY allow_null_org ON agent_tasks
  FOR ALL TO agent_meter_app
  USING (org_id IS NULL);
