# Quickstart — agent-meter no WSL + captura do GitHub Copilot

Guia enxuto para instalar o agent-meter (binário standalone) em **outro WSL/computador**
e capturar a atividade do **GitHub Copilot** ao vivo. Sem cluster, sem PostgreSQL,
sem conta: 1 binário + SQLite local.

```
VS Code (Copilot Chat)  ──OTLP──►  agent-meter  ──►  SQLite local
   (Remote - WSL)          :4318     :8081 (UI)        agent-meter.db
```

---

## 1. Pré-requisitos (no WSL)

- **WSL2** com Ubuntu (ou outra distro Linux).
- **Rust** (toolchain `cargo`) — para compilar o binário:
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  source "$HOME/.cargo/env"
  ```
- **VS Code** com as extensões **Remote - WSL** (`ms-vscode-remote.remote-wsl`) e
  **GitHub Copilot Chat**.
- Portas **8081** (UI/REST) e **4318** (OTLP) livres no WSL.

> **Importante:** abra o VS Code com **Remote - WSL** (canto inferior esquerdo →
> "Connect to WSL"). Assim o Copilot roda *dentro* do WSL e enxerga o `localhost`
> do WSL — que é onde o agent-meter escuta.

---

## 2. Instalar o agent-meter

Ainda não há release binário publicado, então compile do código (rápido, ~2-3 min):

```bash
git clone https://github.com/dnorio/agent-meter.git
cd agent-meter
cargo build --release -p agent-meter-collector

# Deixe o binário no PATH com um nome curto:
mkdir -p ~/.local/bin
cp target/release/agent-meter-collector ~/.local/bin/agent-meter
export PATH="$HOME/.local/bin:$PATH"   # adicione ao ~/.bashrc para persistir
```

Confirme:

```bash
agent-meter version
```

---

## 3. Subir o collector

```bash
agent-meter serve
```

Saída esperada:

```
  agent-meter is running
  ▸ Dashboard:     http://127.0.0.1:8081
  ▸ OTLP receiver: http://127.0.0.1:4318/v1/traces
```

Deixe rodando neste terminal. (Os dados ficam em `agent-meter.db` no diretório atual.)

> Quer ver a cara da ferramenta sem configurar o Copilot ainda? Rode
> `agent-meter demo` — ele popula dados sintéticos e sobe o servidor.

---

## 4. Configurar o Copilot (VS Code dentro do WSL)

Abra o `settings.json` do VS Code (`Ctrl+Shift+P` → "Preferences: Open User
Settings (JSON)") e adicione:

```json
{
  "github.copilot.chat.otel.enabled": true,
  "github.copilot.chat.otel.otlpEndpoint": "http://localhost:4318",
  "github.copilot.chat.otel.captureContent": false
}
```

- `captureContent: false` → captura só metadados (tool, modelo, tokens, duração),
  **não** o conteúdo dos prompts. Mude para `true` se quiser ver os prompts no
  drill-down de conversas (apenas local).

Recarregue a janela do VS Code (`Ctrl+Shift+P` → "Developer: Reload Window").

---

## 5. Validar a captura ao vivo

1. No VS Code (Remote - WSL), abra o **Copilot Chat** e faça algumas interações
   reais — peça para ler arquivos, rodar comandos, editar código, etc.
2. Verifique que os eventos chegaram:

   ```bash
   # tools mais usados capturados do Copilot
   curl -s "http://localhost:8081/reports/top-tools" | jq

   # conversas agrupadas
   curl -s "http://localhost:8081/api/conversations?limit=10" | jq '.[].conversation_id'
   ```
3. Abra o **dashboard** no navegador do Windows: <http://localhost:8081>
   (o WSL2 espelha a porta automaticamente). Navegue por
   **Dashboard → Conversations → (clique numa conversa) → Reports**.

---

## 6. Troubleshooting (WSL)

| Sintoma | Causa provável | Solução |
|--------|----------------|---------|
| Nada aparece no dashboard | VS Code não está em **Remote - WSL** (Copilot roda no Windows, não enxerga o `localhost` do WSL) | Reabra via "Connect to WSL"; **ou** suba o collector com `AGENT_METER_HOST=0.0.0.0 agent-meter serve` para expô-lo ao Windows |
| `http://localhost:8081` não abre no navegador do Windows | Collector ligado só em `127.0.0.1` e o espelhamento do WSL2 não pegou | Suba com `AGENT_METER_HOST=0.0.0.0 agent-meter serve` |
| `address already in use` | Porta 8081/4318 ocupada | `AGENT_METER_PORT=8090 AGENT_METER_OTLP_PORT=4390 agent-meter serve` (ajuste o `otlpEndpoint` no settings para a nova porta) |
| `top-tools` vazio mas health OK | Copilot OTLP não habilitado / janela não recarregada | Confira as 3 chaves no `settings.json` e dê "Reload Window" |
| `agent-meter: command not found` | `~/.local/bin` fora do PATH | `export PATH="$HOME/.local/bin:$PATH"` (adicione ao `~/.bashrc`) |

### Checagem rápida de saúde

```bash
curl -s http://localhost:8081/health
# → {"status":"ok","service":"agent-meter-collector"}
```

---

## Resumo (TL;DR)

```bash
# 1. instalar Rust + clonar + build
git clone https://github.com/dnorio/agent-meter.git && cd agent-meter
cargo build --release -p agent-meter-collector
cp target/release/agent-meter-collector ~/.local/bin/agent-meter

# 2. subir (use HOST=0.0.0.0 se o VS Code não estiver em Remote-WSL)
agent-meter serve

# 3. settings.json do VS Code:
#    github.copilot.chat.otel.enabled = true
#    github.copilot.chat.otel.otlpEndpoint = http://localhost:4318

# 4. usar o Copilot Chat e abrir http://localhost:8081
```
