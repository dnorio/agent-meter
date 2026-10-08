# agent-meter — Guia de Captura por IDE

> Como configurar a captura de telemetria (tokens, custos, tool calls, conversas)
> para cada IDE suportada. Escolha o método correspondente à sua ferramenta.

---

## 🆕 agent-meter-proxy — Binário Único (Recomendado)

> Substitui todos os scripts Python/shell (mitmproxy, cursor-metered, copilot-cli-metered, start_proxy.sh).
> Um único executável nativo, cross-platform, sem dependências — instala via `curl | sh`.

### Instalação

```bash
# Linux / macOS / WSL / Git Bash
curl -fsSL https://raw.githubusercontent.com/ToolHQ/production-site/main/apps/agent-meter/install.sh | sh

# Windows (PowerShell)
irm https://raw.githubusercontent.com/ToolHQ/production-site/main/apps/agent-meter/install.ps1 | iex
```

### Uso

```bash
# 1. Gerar e instalar CA no sistema (primeira vez)
agent-meter-proxy setup

# 2. Iniciar o proxy
agent-meter-proxy start

# 3. Abrir qualquer IDE/CLI com captura automática
agent-meter-proxy wrap cursor .
agent-meter-proxy wrap claude "explain this code"
agent-meter-proxy wrap gh copilot suggest "list pods"
agent-meter-proxy wrap codex "refactor auth module"

# Comandos de gestão
agent-meter-proxy status
agent-meter-proxy stop
agent-meter-proxy ca-info
```

### Como funciona

```
IDE / CLI Tool
    ↓ HTTPS (roteado via HTTPS_PROXY env var)
agent-meter-proxy :8898
    │  intercepta AI hosts (Anthropic, OpenAI, Copilot, Cursor)
    ↓ OTLP spans
agent-meter collector /v1/traces
    ↓
PostgreSQL → Dashboard
```

O comando `wrap` automaticamente:

- Inicia o proxy em background (daemon) se não estiver rodando
- Injeta `HTTPS_PROXY`, `SSL_CERT_FILE`, `NODE_EXTRA_CA_CERTS`, `REQUESTS_CA_BUNDLE` no processo filho
- Captura todas as chamadas LLM (tokens, model, tool calls, streaming SSE)
- Agrupa conversas por sessão (janela de 30 min)

### Hosts e paths monitorados

| Host                                               | Serviço detectado |
| -------------------------------------------------- | ----------------- |
| `api.anthropic.com`                                | `claude-code`     |
| `api.openai.com`                                   | `copilot`         |
| `api.githubcopilot.com` / `*.githubcopilot.com`    | `copilot`         |
| `copilot-proxy.githubusercontent.com`              | `copilot`         |
| `cursor.sh` / `api2.cursor.sh` / `proxy.cursor.sh` | `cursor`          |

---

## Métodos Legados (Scripts Python)

> ⚠️ Os métodos abaixo usam scripts Python/shell e mitmproxy. Para novos setups,
> prefira o **agent-meter-proxy** acima.

---

## Matriz de Compatibilidade

| IDE                   | Método          | Setup                      | Dados capturados                                      | Latência   | Qualidade |
| --------------------- | --------------- | -------------------------- | ----------------------------------------------------- | ---------- | --------- |
| **VS Code** (Copilot) | OTLP nativo     | 2 linhas no settings.json  | tool calls, LLM spans, tokens, modelos, trace_id      | Tempo real | ★★★★★     |
| **JetBrains** (Copilot) | OTLP nativo   | Settings → Tools > GitHub Copilot > Chat + `service.name=copilot-jetbrains` | tool/chat GenAI spans | Tempo real | ★★★★☆     |
| **Eclipse** (Copilot) | mitmproxy HTTPS | `./start_proxy.sh --setup` | LLM calls, tokens, model, tool calls, streaming       | Tempo real | ★★★★☆     |
| **Cursor**            | agent-meter-proxy MITM/`wrap` | `setup` + `wrap cursor` | LLM calls (Claude/GPT), tokens, tool calls, streaming | Tempo real | ★★★★★     |
| **OpenCode**          | REST direto     | env vars                   | tool calls, tasks                                     | Tempo real | ★★★★★     |
| **Antigravity**       | REST direto     | env vars                   | tool calls, tasks                                     | Tempo real | ★★★★★     |
| **Copilot CLI**       | OTLP nativo / MITM wrap | `COPILOT_OTEL_ENABLED` + endpoint, ou wrapper | LLM/tool spans, tokens, model | Tempo real | ★★★★★     |

---

## 1. VS Code + GitHub Copilot (OTLP Nativo)

**Método recomendado.** O VS Code envia spans OpenTelemetry nativamente — zero overhead, zero proxy.

### Como funciona

```
VS Code Copilot
    ↓ OTLP/HTTP JSON (porta 4318)
agent-meter collector
    ↓
PostgreSQL → Dashboard
```

O VS Code usa `traceId` para correlacionar spans `panel/editAgent` (LLM) com `copilot-chat` (tool calls). O agent-meter agrupa tudo em **1 conversa** via `COALESCE(trace_id, conversation_id)`.

### Requisitos

- VS Code com extensão GitHub Copilot Chat ≥ 0.26
- agent-meter acessível em rede (via ingress `agent-meter.dnor.io` ou port-forward local)

### Configuração (settings.json)

```jsonc
{
    // Habilita envio de telemetria OTLP
    "github.copilot.chat.otel.enabled": true,

    // Endpoint do collector — use o ingress de produção:
    "github.copilot.chat.otel.otlpEndpoint": "https://agent-meter.dnor.io",

    // Não captura conteúdo (prompts) — só metadados de performance
    // Mude para true se quiser ver o prompt inicial nas conversas
    "github.copilot.chat.otel.captureContent": true,
}
```

> **WSL / Remote-WSL**: substitua pelo endpoint acessível do WSL.
> Se usando port-forward local: `"http://localhost:4318"` (porta 4318 = OTLP).

### Verificação

```bash
# Abra o VS Code, execute qualquer chat com o Copilot, depois:
curl -s https://agent-meter.dnor.io/api/conversations | \
  python3 -c "import sys,json; [print(c['conversation_id'][:12], c['event_count'], c.get('ide')) for c in json.load(sys.stdin)[:5]]"
```

Deve aparecer `copilot-vscode` na coluna IDE.

### Atributos capturados

| Campo                     | Origem OTLP                    | Descrição                                            |
| ------------------------- | ------------------------------ | ---------------------------------------------------- |
| `agent`                   | `gen_ai.agent.name`            | ex: `panel/editAgent`, `copilotLanguageModelWrapper` |
| `tool_name`               | `gen_ai.tool.name`             | ex: `read_file`, `run_in_terminal`                   |
| `model`                   | `gen_ai.response.model`        | ex: `claude-sonnet-4-6`                              |
| `estimated_input_tokens`  | `gen_ai.usage.input_tokens`    | tokens de entrada                                    |
| `estimated_output_tokens` | `gen_ai.usage.output_tokens`   | tokens de saída                                      |
| `conversation_id`         | `copilot_chat.chat_session_id` | ID da sessão de chat                                 |
| `trace_id`                | `traceId` do span OTLP         | agrupa panel+copilot-chat                            |
| `ok`                      | `status.code` (1=OK, 2=Error)  | sucesso ou falha                                     |

---

## 2. Eclipse + GitHub Copilot (mitmproxy)

**Técnica de proxy HTTPS.** Nenhum plugin necessário — intercepta o tráfego HTTP do copilot-language-server.

### Como funciona

```
Eclipse → copilot-language-server.exe
    ↓ HTTPS (roteado via proxy)
mitmproxy :8899
    │  copilot_interceptor.py
    ↓ OTLP JSON
agent-meter /v1/traces
    ↓
PostgreSQL → Dashboard
```

### Requisitos

- Python 3.9+ com `mitmproxy` e `httpx`: `pip install mitmproxy httpx`
- Eclipse rodando no Windows (WSL ou nativo)
- Acesso de escrita ao `eclipse.ini`

### Setup (primeira vez)

```bash
cd ~/production-site/apps/agent-meter/eclipse-proxy

# Gera CA, importa no Windows, configura eclipse.ini
./start_proxy.sh --setup
```

O `--setup`:

1. Gera o CA do mitmproxy em `~/.mitmproxy/mitmproxy-ca-cert.pem`
2. Copia para `C:\Users\<user>\mitmproxy-ca.crt`
3. Importa para `Cert:\CurrentUser\Root` via PowerShell
4. Adiciona as JVM args ao `eclipse.ini`:
    ```
    -Dhttps.proxyHost=<WSL_IP>
    -Dhttps.proxyPort=8899
    ```

### Iniciar o proxy

```bash
# Inicia mitmproxy com o interceptor
./start_proxy.sh

# Em outro terminal, abra o Eclipse normalmente — o tráfego será interceptado
```

### Verificação

```bash
# Com Eclipse aberto e Copilot ativo, veja os logs:
# [copilot-interceptor] #1 LLM POST api.githubcopilot.com/chat/completions
# → 200 [1230ms, model=gpt-4o, in=8432, out=512]

# Verifique no dashboard:
curl -s "https://agent-meter.dnor.io/api/conversations" | \
  python3 -c "import sys,json; [print(c['conversation_id'][:12], c.get('ide')) \
  for c in json.load(sys.stdin) if c.get('ide')=='copilot-eclipse']" | head -5
```

### Atributos capturados

| Campo                     | Origem                                | Descrição                     |
| ------------------------- | ------------------------------------- | ----------------------------- |
| `model`                   | corpo JSON da requisição/resposta     | ex: `gpt-4o`, `gpt-4.1`       |
| `estimated_input_tokens`  | `usage.prompt_tokens`                 | tokens de entrada             |
| `estimated_output_tokens` | `usage.completion_tokens`             | tokens de saída               |
| `user_prompt`             | última mensagem `role: user` (limpa)  | prompt real do usuário        |
| `tool_calls`              | `choices[0].message.tool_calls`       | ferramentas chamadas pelo LLM |
| `conversation_id`         | `vscode-sessionid` header             | ID da sessão                  |
| `ide`                     | detectado por `service.name: eclipse` | `copilot-eclipse`             |

### Troubleshooting Eclipse

| Sintoma                             | Causa provável                                  | Solução                                          |
| ----------------------------------- | ----------------------------------------------- | ------------------------------------------------ |
| Eclipse não usa proxy               | JVM args ausentes no eclipse.ini                | Verifique `-Dhttps.proxyHost`                    |
| Erro SSL no copilot-language-server | CA não importado                                | Re-execute `--setup`                             |
| Nenhum span no dashboard            | copilot-language-server.exe ignora proxy da JVM | Verifique `NODE_EXTRA_CA_CERTS` no processo Node |
| Proxy não inicia                    | Porta 8899 ocupada                              | `PROXY_PORT=8900 ./start_proxy.sh`               |

---

## 3. Cursor (`agent-meter-proxy`)

**Método oficial.** Cursor não tem OTLP nativo — capture via MITM do binário Rust `agent-meter-proxy` (`setup` / `start` / `wrap cursor`). Scripts Python `cursor-metered` / mitmproxy ficam **deprecated** (só legado).

### Como funciona

```
Cursor (Electron)  ← HTTPS_PROXY + NODE_EXTRA_CA_CERTS (via wrap)
    ↓ HTTPS — Anthropic / OpenAI / Copilot / api2.cursor.sh
agent-meter-proxy :8898
    │  interceptor (UA → service.name=cursor)
    ↓ OTLP JSON → agent-meter :4318 /v1/traces
PostgreSQL → Dashboard
```

### Requisitos

- `agent-meter-proxy` no PATH (`cargo install --path crates/proxy` ou release)
- Cursor instalado (0.43+)
- Linux nativo ou WSL2

### Setup (primeira vez)

```bash
agent-meter-proxy setup
# gera CA + instala no trust store do SO
agent-meter-proxy ca-info   # confirma path do cert
```

### Uso diário

```bash
# Sobe proxy (daemon) se ainda não estiver up, e lança o Cursor com env certo:
agent-meter-proxy wrap cursor .

# Operação:
agent-meter-proxy start --daemon
agent-meter-proxy status
agent-meter-proxy stop
```

`wrap` injeta `HTTPS_PROXY` / `SSL_CERT_FILE` / `NODE_EXTRA_CA_CERTS` no processo filho — Cursor sozinho costuma ignorar proxy de sistema.

### WSL (Cursor no Windows)

```bash
agent-meter-proxy setup
# importe o CA impresso por ca-info no certmgr.msc (Trusted Root)
# HTTPS_PROXY=http://<WSL_IP>:8898
# NODE_EXTRA_CA_CERTS=<path-windows-do-ca>
agent-meter-proxy start --daemon
```

### Agrupamento de conversas

1. **`x-session-id`** / **`x-cursor-session`** quando o build envia
2. **Bearer token prefix** (estável por login)
3. **Janela de 30 min** de idle → mesma conversa

### Hosts monitorados

| Host                           | API                    | Modelos típicos                    |
| ------------------------------ | ---------------------- | ---------------------------------- |
| `api.anthropic.com`            | `/v1/messages`         | claude-sonnet-4-_, claude-opus-4-_ |
| `api.openai.com`               | `/v1/chat/completions` | gpt-4o, gpt-4.1                    |
| `api.githubcopilot.com`        | `/chat/completions`    | gpt-4o (Copilot plan)              |
| `cursor.sh` / `api2.cursor.sh` | `/v1/chat/completions` | modelos via Cursor proxy           |

### Verificação

```bash
agent-meter-proxy status
# Dashboard: https://agent-meter.dnor.io/conversations → filtro Cursor
# CI LIVE: capture-live-e2e case "cursor" (UA cursor/)
```

Harness: `bash scripts/harness/validate_cursor_capture_recipe.sh`

### Troubleshooting Cursor

| Sintoma                           | Causa                         | Solução                                      |
| --------------------------------- | ----------------------------- | -------------------------------------------- |
| Erro de certificado no Cursor     | CA fora do trust store        | `agent-meter-proxy setup` (+ import Windows) |
| Zero spans                        | Cursor sem env de proxy       | Sempre `wrap cursor` (não abrir IDE solta)   |
| `conversation_id` “aleatório”     | Sem header de sessão          | Normal — janela 30 min                       |
| `agent-meter-proxy: command not found` | Binário fora do PATH     | `cargo install --path apps/agent-meter/crates/proxy` |

> **Deprecated:** `cursor-proxy/start_proxy.sh`, `cursor-metered`, mitmproxy Python. Não use em setups novos.

---

## 4. OpenCode / Antigravity / Agentes Customizados (REST direto)

Agentes que têm acesso às env vars do ambiente usam a REST API diretamente.

### Variáveis de ambiente

```bash
# Collector (obrigatório)
export AGENT_METER_COLLECTOR_URL="https://agent-meter.dnor.io"

# Contexto (opcional mas recomendado)
export AGENT_METER_IDE="opencode"          # ou: antigravity, cursor, codex
export AGENT_METER_AGENT="my-agent"
export AGENT_METER_REPO="production-site"
export AGENT_METER_BRANCH="main"
export AGENT_METER_TASK_ID="T-123"
```

### Envio de evento

```bash
curl -X POST "$AGENT_METER_COLLECTOR_URL/events/tool-call" \
  -H "Content-Type: application/json" \
  -d '{
    "tool_name": "read_file",
    "mcp_server": "filesystem",
    "started_at": "2026-06-07T10:00:00Z",
    "ended_at":   "2026-06-07T10:00:01Z",
    "ok": true,
    "request_bytes": 1200,
    "response_bytes": 30000,
    "ide": "opencode",
    "agent": "opencode",
    "conversation_id": "conv-abc123"
  }'
```

---

## 5. CLI Tools (Copilot CLI, Claude Code, Codex CLI)

A técnica de proxy mitmproxy é **agnóstica em relação ao transporte**. Qualquer ferramenta CLI que faz chamadas HTTPS para APIs de IA é capturada automaticamente — sem necessidade de plugin por ferramenta.

### Por que funciona para qualquer CLI

```
CLI Tool (gh copilot, claude, codex, ...)
    ↓ HTTPS — detecta HTTPS_PROXY env var
mitmproxy :8898
    │  interceptor.py (cursor ou eclipse)
    ↓ OTLP JSON
agent-meter /v1/traces
    ↓
PostgreSQL → Dashboard
```

Todas as CLIs usam bibliotecas HTTP padrão (Node.js `https`, Python `requests`, Go `net/http`) que respeitam `HTTPS_PROXY`. Basta configurar as variáveis de ambiente.

### 5.1. GitHub Copilot CLI (`gh copilot`)

```bash
# Opção A: Wrapper dedicado (recomendado)
cd ~/production-site/apps/agent-meter/eclipse-proxy
./copilot-cli-metered.sh suggest "como listar pods no kubernetes"
./copilot-cli-metered.sh explain "kubectl get pods -A"

# Opção B: Variáveis manuais
export HTTPS_PROXY="http://127.0.0.1:8899"
export SSL_CERT_FILE="$HOME/.mitmproxy/mitmproxy-ca-cert.pem"
gh copilot suggest "como listar pods"
```

**Detecção**: O collector identifica via user-agent pattern `copilot-cli` → badge `copilot-cli` no dashboard.

### 5.2. Claude Code (Anthropic CLI)

```bash
# Usa o mesmo proxy do Cursor (porta 8898, intercepta api.anthropic.com)
export HTTPS_PROXY="http://127.0.0.1:8898"
export SSL_CERT_FILE="$HOME/.mitmproxy/mitmproxy-ca-cert.pem"
export NODE_EXTRA_CA_CERTS="$HOME/.mitmproxy/mitmproxy-ca-cert.pem"

# Usar claude normalmente:
claude "explain this code"
claude code --task "refactor auth module"
```

**Detecção**: Via user-agent patterns `claude-code` / `claude_code` → badge `claude-code`.

> **Nota**: Como Claude Code e Cursor usam o mesmo endpoint (`api.anthropic.com`), o `cursor_interceptor.py` captura ambos. A distinção é feita pelo user-agent no OTLP span.

### 5.3. Codex CLI (OpenAI)

```bash
# Mesmas variáveis (porta 8898, intercepta api.openai.com)
export HTTPS_PROXY="http://127.0.0.1:8898"
export SSL_CERT_FILE="$HOME/.mitmproxy/mitmproxy-ca-cert.pem"

codex "refactor this function to use async/await"
```

**Detecção**: Via user-agent pattern `codex` → badge `codex`.

### 5.4. Configuração permanente no shell

Para capturar **todas** as ferramentas CLI automaticamente, adicione ao `.bashrc` / `.zshrc`:

```bash
# agent-meter proxy (captura AI CLI tools)
if ss -tlnp 2>/dev/null | grep -q ":8898"; then
  export HTTPS_PROXY="http://127.0.0.1:8898"
  export SSL_CERT_FILE="$HOME/.mitmproxy/mitmproxy-ca-cert.pem"
  export NODE_EXTRA_CA_CERTS="$HOME/.mitmproxy/mitmproxy-ca-cert.pem"
  export REQUESTS_CA_BUNDLE="$HOME/.mitmproxy/mitmproxy-ca-cert.pem"
fi
```

Isso configura o proxy **apenas quando ele está rodando** — zero impacto quando desligado.

### Compatibilidade de CLIs

| CLI Tool              | Endpoint interceptado              | Proxy port | Detecção (ide.rs) |    Status     |
| --------------------- | ---------------------------------- | :--------: | ----------------- | :-----------: |
| `gh copilot`          | api.githubcopilot.com              |    8899    | `copilot-cli`     |  ✅ Testado   |
| `claude` (Anthropic)  | api.anthropic.com                  |    8898    | `claude-code`     |  ✅ Testado   |
| `codex` (OpenAI)      | api.openai.com                     |    8898    | `codex`           |  ✅ Testado   |
| `aider`               | api.openai.com / api.anthropic.com |    8898    | (user-agent)      | ✅ Compatível |
| `continue` (CLI)      | api.openai.com                     |    8898    | (user-agent)      | ✅ Compatível |
| Qualquer HTTPS AI CLI | Qualquer AI API                    | 8898/8899  | Auto-detect       | ✅ Agnóstico  |

---

## Referência Rápida — Ports e Endpoints

| Porta  | Protocolo | Função                                            |
| ------ | --------- | ------------------------------------------------- |
| `3000` | HTTP      | REST API (`/events/tool-call`, `/api/*`) + Web UI |
| `4318` | HTTP      | OTLP receiver (`/v1/traces`) — VS Code nativo     |
| `8898` | HTTP      | mitmproxy Cursor / Claude Code / Codex CLI        |
| `8899` | HTTP      | mitmproxy Eclipse / Copilot CLI                   |

| URL                                             | Descrição                    |
| ----------------------------------------------- | ---------------------------- |
| `https://agent-meter.dnor.io`                   | Produção (ingress público)   |
| `https://agent-meter.dnor.io/docs`              | Documentação in-app          |
| `https://agent-meter.dnor.io/conversations`     | Dashboard de conversas       |
| `https://agent-meter.dnor.io/api/conversations` | API JSON                     |
| `http://localhost:3000`                         | Local (docker compose / dev) |
| `http://localhost:4318`                         | OTLP local (port-forward)    |

---

## Segurança

- **O proxy mitmproxy é local** (`127.0.0.1`) — nunca exposto à rede
- **O CA gerado é específico para este uso** — pode ser removido a qualquer momento
- **Nenhum dado sai para terceiros** — tudo vai para o seu próprio agent-meter
- **`captureContent: false`** (VS Code) impede captura de conteúdo completo dos prompts
- Os tokens de autenticação (Bearer) são usados **apenas como prefixo de agrupamento** — nunca armazenados completos

Para remover o CA do sistema:

```bash
# Linux
sudo rm /usr/local/share/ca-certificates/mitmproxy-*.crt
sudo update-ca-certificates

# macOS
security delete-certificate -c "mitmproxy" ~/Library/Keychains/login.keychain

# Windows
certmgr.msc → Autoridades Certificadoras Raiz Confiáveis → remover "mitmproxy"
```
