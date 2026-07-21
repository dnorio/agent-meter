# Hardened container runtime

The standalone Docker image and `docker-compose.standalone.yml` use a hardened
runtime profile suitable for local or small-team deployments.

## Defaults

| Control | Value |
|---------|-------|
| User | `agent-meter` (non-root, UID/GID from system accounts) |
| Root filesystem | read-only (`read_only: true` in Compose) |
| Writable paths | `/data` (named volume), `/tmp` (tmpfs) |
| Capabilities | all dropped (`cap_drop: [ALL]`) |
| Privilege escalation | blocked (`no-new-privileges:true`) |
| Healthcheck | `agent-meter check` (DB connectivity + config) |

## Volumes

SQLite lives at `/data/agent-meter.db`. Compose declares a named volume
`agent-meter-data` mounted at `/data`. Without it, the read-only root
filesystem cannot persist state.

```bash
docker compose -f docker-compose.standalone.yml up -d --build
docker volume inspect agent-meter_agent-meter-data
```

## Troubleshooting

**Container exits immediately / permission errors**

- Ensure the named volume exists and is writable by the `agent-meter` user.
- Do not bind-mount host directories with root-only permissions onto `/data`.

**Healthcheck failing**

- Verify `DATABASE_URL` points at `/data/...` inside the container.
- Run `docker compose exec agent-meter agent-meter check` for the exact error.
- Confirm migrations can run: `docker compose exec agent-meter agent-meter migrate`.

**Read-only root surprises**

- Only `/data` and `/tmp` are writable. Logs go to stdout/stderr (`RUST_LOG`).

## Exposed deployments

`docker-compose.standalone.yml` binds ingest on `0.0.0.0` without auth. For LAN or
shared hosts, prefer the secure profile:

```bash
docker compose -f docker-compose.secure.yml up -d --build
docker exec agent-meter agent-meter keys create --name my-client
```

Set `AGENT_METER_REQUIRE_API_KEY=1` (enabled in the secure compose file) and pass
`Authorization: Bearer <secret>` from SDKs and agents. Admin reset/delete remain
localhost-only inside the container.

## CVE posture

- **Runtime base:** `debian:trixie-slim` — track Debian security advisories.
- **Build base:** `rust:1.97-slim-trixie` — only in builder stage, not shipped.
- **Runtime packages:** `ca-certificates` only (no shell utilities such as `wget`).
- **Accepted trade-offs:** single static binary + minimal OS; scan with your registry
  tool of choice and rebuild on base-image updates.
- **Fixable findings:** rebuild and redeploy; open an issue if a scanner flags
  something that needs a Dockerfile change.
