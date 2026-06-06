-- T-318: Cost Attribution Engine
-- Pricing por modelo (USD por 1M tokens) para cálculo de custo em tempo de query.

CREATE TABLE IF NOT EXISTS model_pricing (
    id              bigserial PRIMARY KEY,
    model           text NOT NULL,
    -- match estratégia: 'exact' = match exato; 'prefix' = startsWith
    match_kind      text NOT NULL DEFAULT 'exact' CHECK (match_kind IN ('exact','prefix')),
    -- USD por 1.000.000 de tokens
    input_per_mtok  numeric(12,6) NOT NULL DEFAULT 0,
    output_per_mtok numeric(12,6) NOT NULL DEFAULT 0,
    cached_per_mtok numeric(12,6) NOT NULL DEFAULT 0,
    -- ordem de avaliação (maior = mais específico, ex: prefixos longos primeiro)
    priority        integer NOT NULL DEFAULT 0,
    source          text NOT NULL DEFAULT 'manual',
    notes           text,
    created_at      timestamptz NOT NULL DEFAULT now(),
    updated_at      timestamptz NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_model_pricing_model_kind ON model_pricing(model, match_kind);
CREATE INDEX IF NOT EXISTS idx_model_pricing_priority ON model_pricing(priority DESC);

-- Seed pricing (oficiais, junho/2026 — ajustar via UI futuramente)
INSERT INTO model_pricing (model, match_kind, input_per_mtok, output_per_mtok, cached_per_mtok, priority, source) VALUES
    ('claude-opus-4',       'prefix', 15.00, 75.00, 1.50, 100, 'anthropic'),
    ('claude-sonnet-4',     'prefix',  3.00, 15.00, 0.30, 100, 'anthropic'),
    ('claude-haiku-4',      'prefix',  0.80,  4.00, 0.08, 100, 'anthropic'),
    ('claude-3-7-sonnet',   'prefix',  3.00, 15.00, 0.30,  90, 'anthropic'),
    ('claude-3-5-sonnet',   'prefix',  3.00, 15.00, 0.30,  90, 'anthropic'),
    ('claude-3-5-haiku',    'prefix',  0.80,  4.00, 0.08,  90, 'anthropic'),
    ('claude-3-opus',       'prefix', 15.00, 75.00, 1.50,  90, 'anthropic'),
    ('gpt-5',               'prefix',  2.00,  8.00, 0.20, 100, 'openai'),
    ('gpt-4o',              'prefix',  2.50, 10.00, 1.25,  90, 'openai'),
    ('gpt-4o-mini',         'prefix',  0.15,  0.60, 0.075, 90, 'openai'),
    ('gpt-4-turbo',         'prefix', 10.00, 30.00, 0.00,  80, 'openai'),
    ('o1-preview',          'prefix', 15.00, 60.00, 7.50,  90, 'openai'),
    ('o1-mini',             'prefix',  3.00, 12.00, 1.50,  90, 'openai'),
    ('gemini-2-5-pro',      'prefix',  1.25,  5.00, 0.31, 100, 'google'),
    ('gemini-2-5-flash',    'prefix',  0.075, 0.30, 0.019, 100, 'google'),
    ('gemini-1-5-pro',      'prefix',  1.25,  5.00, 0.31,  90, 'google'),
    ('gemini-1-5-flash',    'prefix',  0.075, 0.30, 0.019, 90, 'google'),
    ('grok-4',              'prefix',  3.00, 15.00, 0.00,  90, 'xai'),
    ('deepseek-v3',         'prefix',  0.27,  1.10, 0.07,  90, 'deepseek'),
    ('llama-3-3',           'prefix',  0.40,  0.40, 0.00,  80, 'meta')
ON CONFLICT (model, match_kind) DO NOTHING;

-- Função: calcula USD para um evento dado os tokens e o modelo.
-- Estratégia de match: tenta exact match primeiro, depois prefix mais específico (priority DESC).
-- Retorna 0 se não encontrar pricing (não bloqueia ingest).
CREATE OR REPLACE FUNCTION compute_event_usd(
    p_model text,
    p_input_tokens integer,
    p_output_tokens integer,
    p_cached_tokens integer
) RETURNS numeric(12,6) AS $$
DECLARE
    v_input  integer := COALESCE(p_input_tokens, 0);
    v_output integer := COALESCE(p_output_tokens, 0);
    v_cached integer := COALESCE(p_cached_tokens, 0);
    v_billed_input integer;
    v_pricing model_pricing%ROWTYPE;
BEGIN
    IF p_model IS NULL OR length(p_model) = 0 THEN
        RETURN 0;
    END IF;

    -- exact match
    SELECT * INTO v_pricing FROM model_pricing
        WHERE match_kind = 'exact' AND model = p_model
        ORDER BY priority DESC LIMIT 1;

    IF NOT FOUND THEN
        SELECT * INTO v_pricing FROM model_pricing
            WHERE match_kind = 'prefix' AND p_model LIKE model || '%'
            ORDER BY length(model) DESC, priority DESC LIMIT 1;
    END IF;

    IF NOT FOUND THEN
        RETURN 0;
    END IF;

    -- input cobrado = total - cached (cached é faturado pelo preço de cache)
    v_billed_input := GREATEST(v_input - v_cached, 0);

    RETURN ROUND(
        (v_billed_input::numeric / 1000000.0) * v_pricing.input_per_mtok
      + (v_output::numeric       / 1000000.0) * v_pricing.output_per_mtok
      + (v_cached::numeric       / 1000000.0) * v_pricing.cached_per_mtok
    , 6);
END;
$$ LANGUAGE plpgsql IMMUTABLE;

-- View: agent_tool_calls + usd_cost computed
CREATE OR REPLACE VIEW agent_tool_calls_with_cost AS
SELECT
    t.*,
    compute_event_usd(t.model, t.estimated_input_tokens, t.estimated_output_tokens, t.cached_tokens) AS usd_cost
FROM agent_tool_calls t;
