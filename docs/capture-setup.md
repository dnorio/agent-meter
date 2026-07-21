# agent-meter — Guia de Captura por IDE

Guia prático para enviar telemetria de IDEs, CLIs e agentes para o collector do
agent-meter. O setup padrão usa SQLite e expõe UI/REST em `127.0.0.1:8081`.

```bash
# Instalar + ver dados de exemplo em segundos
curl -fsSL https://raw.githubusercontent.com/dnorio/agent-meter/main/install.sh | bash
agent-meter demo
```

---

## Visão geral

| Método | IDEs / agentes | Setup | Qualidade |
|--------|----------------|-------|-----------|
| **OTLP nativo** | VS Code + GitHub Copilot | 2 linhas no `settings.json` | ★★★★★ |
| **REST direto** | OpenCode, Antigravity, scripts | env vars + `curl` | ★★★★★ |
| **HTTPS proxy** | Cursor, Eclipse, Claude Code, Codex CLI | `install-proxy.sh` ou release | ★★★★☆ |
| **mitmproxy legado** | Eclipse, Cursor (scripts Python) | `eclipse-proxy/` no repo | ★★★☆☆ |

**Recomendado para novos setups:** OTLP (VS Code) ou REST. Use o proxy nativo
para IDEs que não exportam OTLP.

---

## 1. VS Code + GitHub Copilot (OTLP — recomendado)

Zero proxy. O VS Code envia spans OpenTelemetry nativamente.

```
VS Code Copilot
    ↓ OTLP/HTTP  →  http://127.0.0.1:4318/v1/traces
agent-meter (serve)
    ↓
SQLite → Dashboard http://127.0.0.1:8081
```

### Pré-requisitos

- VS Code com GitHub Copilot Chat ≥ 0.26
- `agent-meter serve` rodando localmente

### settings.json

```jsonc
{
  "github.copilot.chat.otel.enabled": true,
  "github.copilot.chat.otel.otlpEndpoint": "http://127.0.0.1:4318",
  "github.copilot.chat.otel.captureContent": true
}
```

Use `captureContent: true` quando quiser observabilidade completa com prompts.
Use `false` quando quiser apenas metadados/tokens do VS Code.

> **WSL:** use o IP do host Windows ou `127.0.0.1` se o VS Code e o collector
> rodam no mesmo ambiente. Guia passo a passo:
> [QUICKSTART-WSL-COPILOT.md](QUICKSTART-WSL-COPILOT.md)

### Verificação

```bash
curl -s http://127.0.0.1:8081/api/conversations | \
  python3 -c "import sys,json; [print(c['conversation_id'][:12], c.get('ide')) for c in json.load(sys.stdin)[:5]]"
```

Deve aparecer `copilot-vscode` na coluna IDE.

---

## 2. OpenCode / Antigravity / agentes customizados (REST)

Agentes com acesso a env vars usam a REST API diretamente.

```bash
export AGENT_METER_COLLECTOR_URL="http://127.0.0.1:8081"
export AGENT_METER_IDE="opencode"
export AGENT_METER_AGENT="my-agent"
export AGENT_METER_REPO="my-app"
export AGENT_METER_BRANCH="main"
export AGENT_METER_TASK_ID="session-1"
```

```bash
curl -X POST "$AGENT_METER_COLLECTOR_URL/events/tool-call" \
  -H "Content-Type: application/json" \
  -d '{
    "tool_name": "read_file",
    "mcp_server": "filesystem",
    "started_at": "2026-06-07T10:00:00Z",
    "ended_at":   "2026-06-07T10:00:01Z",
    "ok": true,
    "ide": "opencode",
    "agent": "opencode",
    "conversation_id": "conv-abc123"
  }'
```

---

## 3. agent-meter-proxy — HTTPS proxy nativo (Cursor, CLIs)

Binário Rust em `crates/proxy/` — intercepta chamadas HTTPS para APIs de IA e
envia OTLP ao collector local. **Prebuilt** em cada release GitHub
(`agent-meter-proxy-linux-*`, `agent-meter-proxy-windows-*`).

```bash
# Install (Linux / macOS)
curl -fsSL https://raw.githubusercontent.com/dnorio/agent-meter/main/install-proxy.sh | bash

agent-meter serve &          # collector em :8081 / :4318
agent-meter-proxy setup      # gera CA (primeira vez)
agent-meter-proxy start --collector http://127.0.0.1:8081
agent-meter-proxy wrap cursor .
agent-meter-proxy wrap claude "explain this code"
agent-meter-proxy wrap gh copilot suggest "list pods"
```

Build from source (fallback):

```bash
cargo build --release -p agent-meter-proxy
export PATH="$PWD/target/release:$PATH"
```

```
IDE / CLI
    ↓ HTTPS (HTTPS_PROXY)
agent-meter-proxy :8898
    ↓ OTLP
agent-meter :4318
    ↓
SQLite → Dashboard
```

Hosts monitorados: `api.anthropic.com`, `api.openai.com`, `*.githubcopilot.com`,
`cursor.sh`, `api2.cursor.sh`.

---

## 4. Eclipse + Copilot CLI (mitmproxy legado)

Scripts Python em [`eclipse-proxy/`](../eclipse-proxy/) — alternativa ao proxy
Rust quando precisar de integração JVM/Eclipse.

```bash
cd agent-meter/eclipse-proxy
pip install mitmproxy httpx
./start_proxy.sh --setup    # gera CA + configura eclipse.ini (Windows)
./start_proxy.sh            # mitmproxy :8899
```

Verificação:

```bash
curl -s http://127.0.0.1:8081/api/conversations | \
  python3 -c "import sys,json; [print(c.get('ide')) for c in json.load(sys.stdin) if 'eclipse' in (c.get('ide') or '')]"
```

---

## 5. Cursor (mitmproxy legado)

Scripts em [`eclipse-proxy/`](../eclipse-proxy/) (interceptor compartilhado) ou
use **`agent-meter-proxy wrap cursor .`** (seção 3).

Para mitmproxy manual:

```bash
agent-meter-proxy setup
agent-meter-proxy wrap cursor .
```

---

## Referência — portas e URLs padrão

| Porta | Função |
|-------|--------|
| `8081` | UI + REST API (`/events/tool-call`, `/api/*`) |
| `4318` | OTLP receiver (`/v1/traces`) |
| `8898` | agent-meter-proxy (Cursor / Claude / Codex) |
| `8899` | mitmproxy legado (Eclipse / Copilot CLI) |

| URL | Descrição |
|-----|-----------|
| `http://127.0.0.1:8081` | Dashboard |
| `http://127.0.0.1:8081/docs` | Documentação in-app |
| `http://127.0.0.1:8081/conversations` | Sessões agrupadas |
| `http://127.0.0.1:8081/api/conversations` | API JSON |

---

## Segurança

- Proxy deve escutar em `127.0.0.1` por padrão; use bind amplo apenas quando precisar cruzar WSL/Windows ou rede confiável
- CA gerado localmente — removível a qualquer momento
- Dados vão para o collector configurado; no setup padrão, o armazenamento é SQLite
- `captureContent: true` (VS Code) inclui prompts; `false` limita a captura de conteúdo

Mais detalhes OTEL: [agent-meter-otel.md](agent-meter-otel.md)
