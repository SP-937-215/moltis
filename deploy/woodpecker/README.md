# Woodpecker → GHCR → TrueNAS (Moltis fork)

Public CI: **https://ci.qod.name** (Pegasus + Pangolin Newt)

## One-time Woodpecker repo setup

1. Open https://ci.qod.name → enable **SP-937-215/moltis**
2. Repo **Settings**:
   - **Trusted** (needed for `/var/run/docker.sock`)
   - Events: **push** + **manual**
3. GHCR auth (same as TMS — **no per-repo secrets required**):
   - On Pegasus host once: `docker login ghcr.io -u SP-937-215`
   - Pipeline mounts `~/.docker/config.json`
   - Only add `GHCR_USERNAME` / `GHCR_TOKEN` secrets if you later wire `from_secret` again

## Pipeline

Root [`.woodpecker.yml`](../../.woodpecker.yml) runs on:

- push to `fix/aud-2026-001-remediation` or `main`
- manual runs

It builds the repo `Dockerfile` as `linux/amd64` with
`MOLTIS_VERSION=20260902.03` (override via env) so the in-app update banner
stays quiet, and pushes:

| Tag | Meaning |
|-----|---------|
| `ghcr.io/sp-937-215/moltis:fork` | stable pointer for TrueNAS |
| `ghcr.io/sp-937-215/moltis:<branch>` | branch tip |
| `ghcr.io/sp-937-215/moltis:<sha12>` | immutable build |

## TrueNAS Custom App

- **Image repository:** `ghcr.io/sp-937-215/moltis`
- **Image tag:** `fork`
- If the package is private: Sign in to GHCR with a PAT (`read:packages`)
- Persist:
  - `/home/moltis/.config/moltis`
  - `/home/moltis/.moltis`
- Suggested env for NAS/LAN:
  - `MOLTIS_TRUST_DOCKER_NETWORK=1`
  - and/or config `[auth] disabled = true` for single-user local
- Ports: `13131` (UI), optionally `13132`, `1455`
- Optional: mount Docker sock if you want container sandboxes
- Runtime image includes `jq` (restricted-host PATH is `/usr/local/bin:/usr/bin:/bin`). Do not enable Docker sandbox just to get CLI tools — the default package list is huge.

## First login after deploy

```bash
moltis auth login --provider xai-oauth
```

Then select `grok-4.6`.

## Manual trigger

Woodpecker UI → **moltis** → Manual pipeline on `fix/aud-2026-001-remediation`.
