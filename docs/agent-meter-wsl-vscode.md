# agent-meter — WSL + VSCode Setup Guide

> Configure o VSCode rodando no WSL2 para enviar telemetria de tool-calls ao agent-meter collector no cluster OCI.

## Architecture

```
┌──────────────────────────────────────────────────────────┐
│  Windows                                                  │
│  ┌─────────────────┐      ┌────────────────────────────┐│
│  │  VSCode (Remote  │─────>│  WSL2 (Ubuntu)             ││
│  │  - WSL)          │      │                            ││
│  │                  │      │  agent-meter CLI           ││
│  │  Copilot agent   │      │  ~/.config/agent-meter/    ││
│  │  faz tool-calls  │      │  env.sh                    ││
│  └─────────────────┘      │                            ││
│                           │  kubectl port-forward       ││
│                           │  ┌─> agent-meter:3000 ───┐  ││
│                           └──┤───────────────────────┤──┘│
│                              │                       │   │
│                              ▼                       ▼   │
│                        ┌──────────┐          ┌────────┐ │
│                        │  SSH     │          │ OCI K8s│ │
│                        │  Tunnel  │◄─────────│ Cluster│ │
│                        │ (master) │          │☸️      │ │
│                        └──────────┘          └────────┘ │
└──────────────────────────────────────────────────────────┘
```

## Pré-requisitos

- **WSL2** com Ubuntu (ou qualquer distro Linux)
- **VSCode** com extensão **Remote - WSL** (ms-vscode-remote.remote-wsl)
- **kubectl** configurado no WSL com acesso ao cluster OCI
- **SSH tunnel** ativo para o master node (`setup-dev-deploy.sh`)
- **Rust toolchain** (`cargo`) ou **Docker** para compilar o CLI
- **Porta 8081** livre no WSL (para port-forward local)

## Passo 1 — Setup do agent-meter CLI

Execute o script universal dentro do WSL:

```bash
# Na worktree do OpenCode (ou de qualquer worktree com o repositório)
cd ~/REDACTED-worktree

# Setup para Copilot/VSCode com MCP wrapper
apps/agent-meter/scripts/setup-agent.sh --agent copilot --mcp-wrapper
```

O script:
1. Compila o CLI (`agent-meter`) e o MCP wrapper (`agent-meter-mcp-wrapper`)
2. Cria `~/.config/agent-meter/env.sh` com as env vars corretas
3. Adiciona `source ~/.config/agent-meter/env.sh` ao `~/.bashrc`
4. Configura vars do MCP wrapper no `env.sh`

### WSL detection (`setup-agent.sh`)

O script detecta automaticamente se está rodando dentro do WSL e ajusta:

- `COLLECTOR_URL` — usa `http://localhost:8081` em vez de `http://agent-meter:3000` (in-cluster)
- `~/.bashrc` — fonte correto (WSL usa `.bashrc`, não `.bash_profile`)
- PATH — inclui `$HOME/.local/bin` se ausente
- Port-forward — exibe instruções para tunnel local

### Pós-setup

```bash
# Recarregar env vars
source ~/.bashrc

# Verificar instalação
agent-meter --help

# Verificar env vars
env | grep AGENT_METER
```

**Saída esperada:**

```
AGENT_METER_COLLECTOR_URL=http://localhost:8081
AGENT_METER_IDE=copilot-vscode
AGENT_METER_AGENT=copilot
AGENT_METER_REPO=agent-meter-worktree
```

## Passo 2 — Tunnel para o collector

O agent-meter collector roda dentro do cluster K8s (`agent-meter:3000`). Para acessá-lo do WSL, use port-forward:

### Setup do tunnel SSH (uma vez por sessão)

```bash
# No diretório do cluster (worktree Antigravity)
cd ~/REDACTED-worktree
source oci-k8s-cluster/scripts/setup-dev-deploy.sh
```

### Port-forward do collector

```bash
# Terminal 1: tunnel do collector
kubectl port-forward svc/agent-meter 8081:3000
```

### Script automático (recomendado)

Adicione ao `~/.bashrc` (já incluso pelo `setup-agent.sh`):

```bash
# agent-meter tunnel helper
agent-meter-tunnel() {
  local KUBECONFIG="${KUBECONFIG:-$HOME/REDACTED-worktree/oci-k8s-cluster/kubeconfig_tunnel.yaml}"
  if [ ! -f "$KUBECONFIG" ]; then
    echo "KUBECONFIG não encontrado em $KUBECONFIG"
    echo "Rode: source ~/REDACTED-worktree/oci-k8s-cluster/scripts/setup-dev-deploy.sh"
    return 1
  fi
  KUBECONFIG="$KUBECONFIG" kubectl port-forward svc/agent-meter 8081:3000
}
```

Uso:

```bash
agent-meter-tunnel
# Deixe rodando em um terminal separado
```

### Verificação

```bash
curl -s http://localhost:8081/health
# → {"status":"ok"}
```

## Passo 3 — VSCode + Copilot

### Configuração de ambiente

O VSCode Remote - WSL herda as env vars do shell do WSL. Após o setup-agent.sh, as vars já estão no `~/.bashrc`.

Para verificar se o VSCode está vendo as vars:

```bash
# No terminal integrado do VSCode (Ctrl+`)
echo $AGENT_METER_COLLECTOR_URL
# → http://localhost:8081
```

### Task lifecycle automático

Adicione ao `~/.bashrc` hooks para start/end de task ao abrir/fechar o VSCode:

```bash
# agent-meter task hooks (VSCode)
if [ -n "$TERM_PROGRAM" ] && [ "$TERM_PROGRAM" = "vscode" ]; then
  # Gera um ID único por sessão VSCode
  if [ -z "$AGENT_METER_TASK_ID" ]; then
    export AGENT_METER_TASK_ID="vscode-$(hostname)-$(date +%s)"
    agent-meter task start "$AGENT_METER_TASK_ID" \
      --repo my-app 2>/dev/null || true
  fi

  # End task on shell exit
  agent-meter-task-end() {
    if [ -n "$AGENT_METER_TASK_ID" ]; then
      agent-meter task end "$AGENT_METER_TASK_ID" 2>/dev/null || true
    fi
  }
  trap agent-meter-task-end EXIT
fi
```

### MCP Wrapper (opcional)

Se você usa MCP servers no VSCode (ex: GitHub, Playwright), o MCP wrapper mede os tool-calls automaticamente.

```bash
# Terminal: inicia o wrapper
agent-meter-mcp-wrapper &
```

Configure o VSCode para usar o wrapper como proxy MCP:

```json
// .vscode/mcp.json
{
  "servers": {
    "github": {
      "type": "url",
      "url": "http://localhost:3001"
    }
  }
}
```

O wrapper escuta em `:3001` e encaminha para `MCP_UPSTREAM_URL` (configurado no `env.sh`).

## Passo 4 — Teste

### Enviar um tool-call manual

```bash
# Tunnel ativo?
curl -s http://localhost:8081/health

# Enviar evento de teste
agent-meter event tool-call \
  --tool-name test_wsl_setup \
  --mcp-server manual \
  --ok \
  --duration-ms 100
```

### Verificar no dashboard

```bash
# Abrir dashboard
open http://localhost:8081
```

Ou via API:

```bash
# Top tools
curl -s http://localhost:8081/reports/top-tools?agent=copilot | jq

# Calls over time
curl -s "http://localhost:8081/reports/calls-over-time?range=1h&agent=copilot" | jq
```

### Smoke test completo

```bash
# Usar o smoke script (requer tunnel ativo)
apps/agent-meter/scripts/smoke-otel.sh
```

## Troubleshooting

| Problema | Causa | Solução |
|----------|-------|---------|
| `curl: Connection refused` no `:8081` | Port-forward não ativo | Rode `agent-meter-tunnel` em outro terminal |
| `agent-meter: command not found` | PATH não configurado | `export PATH=$PATH:$HOME/.local/bin` |
| env vars não aparecem no VSCode | VSCode não leu `.bashrc` | Abra terminal integrado (`Ctrl+`) — se funcionar lá, as vars estão OK |
| `AGENT_METER_COLLECTOR_URL=http://agent-meter:3000` (URL do cluster) | WSL não detectado | Edite `~/.config/agent-meter/env.sh` e mude para `http://localhost:8081` |
| MCP wrapper não conecta | Upstream URL errada | Verifique `MCP_UPSTREAM_URL` em `~/.config/agent-meter/env.sh` |
| Task não aparece no dashboard | Task não foi finalizada | Rode `agent-meter task list` e `agent-meter task end <id>` |

### WSL-specific: PATH do Windows no WSL

Se o VSCode foi instalado pelo Windows (não pelo WSL), o PATH do WSL inclui `/mnt/c/...`. O `agent-meter` CLI precisa estar no PATH do **WSL** (`~/.local/bin`).

```bash
# Verificar se está no PATH correto
which agent-meter
# → /home/<user>/.local/bin/agent-meter  ✓
# → /mnt/c/... ❌ (não vai funcionar)
```

### WSL-specific: Networking

O WSL2 usa NAT por padrão. O `localhost` do Windows é diferente do `localhost` do WSL. O `kubectl port-forward` dentro do WSL escuta no `localhost` do WSL — o VSCode (rodando no Windows) não consegue acessá-lo diretamente.

**Solução**: Tudo (CLI, curl, VSCode Remote) roda dentro do WSL — o `localhost` do WSL funciona para todos os processos no WSL.

Se precisar acessar do Windows (ex: browser Windows), use `localhost` normal — o WSL2 faz proxy automático de portas desde o Windows 10 build 19043+.

### WSL-specific: Inicialização automática do tunnel

Adicione ao `~/.bashrc` para iniciar o tunnel automaticamente ao abrir o WSL:

```bash
# Auto-tunnel agent-meter (WSL)
if [ -z "$AGENT_METER_TUNNEL_PID" ] && [ -f "$HOME/REDACTED-worktree/oci-k8s-cluster/kubeconfig_tunnel.yaml" ]; then
  KUBECONFIG="$HOME/REDACTED-worktree/oci-k8s-cluster/kubeconfig_tunnel.yaml" \
    kubectl port-forward svc/agent-meter 8081:3000 &>/dev/null &
  export AGENT_METER_TUNNEL_PID=$!
fi
```

## Referências

- `docs/agent-meter-otel.md` — documentação geral de OTEL
- `scripts/setup-agent.sh` — script universal de setup
- `.agents/skills/agent-meter-integration/SKILL.md` — skill reutilizável
- `apps/agent-meter/crates/cli/` — código do CLI
- Dashboard: `http://localhost:8081` (via ingress, sem tunnel)
