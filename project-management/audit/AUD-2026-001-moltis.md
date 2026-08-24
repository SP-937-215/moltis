# Audit Template: Moltis

> **Audit ref:** `AUD-2026-001`
> **Date:** 2026-08-24
> **Auditor(s):** Pi coding agent (manual + targeted static inspection)
> **Scope:** Full monorepo review (Rust crates, web UI/E2E surface, Docker/CI, docs) + cross-check of upstream open issues. **No fixes applied in this audit.**
> **Version/Branch:** audit baseline `upstream/main` @ `fc65e527`; fork work inspected on `feat/xai-oauth` / related fix branches for context only
> **Status:** `COMPLETE` (report-only; remediation deferred)

---

## 1. Executive Summary

### 1.1 Overview

Moltis is a large Rust monorepo (~75 crates) for a secure persistent personal agent server: multi-provider LLMs, sandboxed tool execution, channels (Telegram/Slack/Discord/etc.), voice, memory, MCP, and a web UI. The project is mature (CI, E2E, provider-integration workflows, extensive docs) and security-conscious by design (sandbox backends, vault, auth middleware, observability redaction).

This audit combines:
- static inspection of architecture / auth / providers / heartbeat / TTS / sandbox paths
- scan outputs under `project-management/audit/scan-results/`
- Docker-validated findings from recent fork work (not merged)
- review of ~50+ open upstream issues (bugs + enhancements)

### 1.2 Key Findings Summary

| Severity | Count | Key Areas |
|----------|-------|-----------|
| 🔴 Critical | 0 | — |
| 🟠 High | 5 | Heartbeat active-hours dead code / `24:00` parse bug; OAuth device-flow empty scopes; TTS Coqui false-configured; Docker sandbox arm64 DMI mounts; auth-disabled Docker UX |
| 🟡 Medium | 8 | Idle CPU / polling density; OAuth refresh races (provider-class); sandbox Apple Container naming; MS Teams / Slack shared-channel residual bugs; provider entitlement UX; CI checks not reported on fork PRs |
| 🔵 Low | 6 | Prefer-models UI bugs; update banner Docker awareness; high unwrap/expect density under clippy allows; plans vs prompts convention; missing Dockerfile HEALTHCHECK |
| ⚪ Info | 4 | Feature backlog (Landlock, Netbird, failover, STT engines); strong observability/docs baseline |

### 1.3 Overall Health Score

**Score:** `74/100`

| Category | Score | Trend |
|----------|-------|-------|
| Architecture | `8/10` | ✅ modular crates, clear provider/sandbox ports |
| Code Quality | `7/10` | 🟡 clippy-deny culture strong, but many `allow(unwrap)` islands |
| Security | `8/10` | ✅ vault/auth/sandbox; residual Docker-auth UX & sandbox mounts |
| Performance | `6/10` | 🟡 idle CPU complaint + dense interval loops |
| Testing | `8/10` | ✅ broad unit + Playwright E2E + weekly provider integration |
| Dependencies | `7/10` | ✅ lockfile + nightly pin; large graph / heavy optional features |
| Documentation | `8/10` | ✅ docs site + SECURITY + sandbox docs |
| Technical Debt | `6/10` | 🟡 giant `gateway`/`tools` crates; dead helpers (heartbeat hours) |
| CI/CD | `7/10` | ✅ multi-workflow; fork PR checks currently `action_required` / unreported |
| Database | `7/10` | ✅ SQLite + migrations present; ops backup story less visible |
| Observability | `8/10` | ✅ tracing + OTLP/prometheus exporters + redaction helpers |

### 1.4 Top 5 Remediation Priorities

1. **Heartbeat active hours (#1205/#1223)** — config is user-facing but ineffective on `main` (parse bug + never called). Effort: **S**.
2. **OAuth device-flow empty `scope=""`** — breaks scoped providers (xAI/Kimi-class). Effort: **XS–S**.
3. **TTS Coqui auto-select false positive (#1114)** — noisy failures / wrong provider selection. Effort: **S**.
4. **Docker sandbox arm64 DMI mounts (#1085)** — sandbox broken on Apple Silicon Docker. Effort: **S–M**.
5. **Auth-disabled Docker UX (#1112)** — onboarding/auth state confusing for local-only Docker users. Effort: **M**.

---

## 2. Methodology

### 2.1 Audit Scope

| Dimension | In Scope | Out of Scope |
|-----------|----------|--------------|
| Rust crates (`crates/`, `apps/`) | ✅ | Full clippy/nextest of entire workspace (resource limits) |
| Web UI / Playwright E2E | ✅ (structure + coverage inventory) | Full Playwright run |
| Infrastructure / Docker | ✅ | Production deploy penetration |
| CI/CD Pipelines | ✅ | Fork PR approval gating policy changes |
| Upstream open issues | ✅ | Closing/triaging issues |
| Security penetration test | ❌ | Dedicated pen-test |
| Live provider billing abuse tests | ❌ | — |

### 2.2 Tools Used

- Manual architecture inspection + `rg` static scans
- `gh` for upstream issues / PR / workflow status
- Docker (`rust:trixie` + pinned nightly) for targeted crate tests on fork branches
- Scan artifacts in `project-management/audit/scan-results/`

### 2.3 Process

1. Inventory crates, CI, docs, sandbox/provider surfaces
2. Static scans (unwrap/TODO/unsafe/auth/security headers)
3. Confirm high-confidence bugs still present on `upstream/main`
4. Map findings to open GitHub issues where applicable
5. Compile severity-ranked remediation plan (**no code changes**)

---

## 3. Architecture Review

### Verdict

Strong modularization for a personal-agent gateway: providers, oauth, provider-setup, cron/heartbeat, tools/sandbox, channels, httpd, web UI. Composition roots are large (`gateway`, `httpd`) which is expected but creates review/test cost.

### Findings

- **A-001 [Medium]** — `gateway` crate size concentration
  *Location:* `crates/gateway/` (~167 `.rs` files)
  *Description:* Gateway concentrates startup, heartbeat execution, voice, connectors, pairing, vault lifecycle.
  *Impact:* Higher regression risk; harder incremental CI; audits/PRs become broad.
  *Root cause:* Natural aggregation of “runtime orchestration” without further subsystem crates.

- **A-002 [High]** — Dead architectural seam: heartbeat active-hours helper unused
  *Location:* `crates/cron/src/heartbeat.rs` (`is_within_active_hours`); callers only in tests on `main`
  *Description:* Config schema documents `[heartbeat.active_hours]`, helper exists, but heartbeat jobs are upserted as unconditional `every_ms` and the helper is never consulted at execution time.
  *Impact:* Users believe quiet-hours work; LLM spend / notifications continue overnight (#1205/#1223).
  *Root cause:* Feature partially implemented (parse helper + config) without wiring into agent-turn path.

- **A-003 [Info]** — Provider dual-path pattern (API key vs OAuth) is coherent
  *Location:* `crates/providers`, `crates/provider-setup`, `crates/oauth`
  *Description:* Codex/Copilot/Kimi patterns exist; xAI API-key path exists; subscription OAuth is the obvious gap (tracked by fork/#1239).
  *Impact:* Positive extensibility, but device-flow scope bug blocks new OAuth providers.
  *Root cause:* N/A (strength).

---

## 4. Code Quality & Standards Compliance

### Verdict

Project culture is high: nightly pin, clippy deny settings, conventional commits, CONTRIBUTING expects tests. Residual quality debt is concentrated in allowlisted unwrap islands and a few logic bugs with tests that don’t assert behavior.

### Findings

- **CQ-001 [High]** — OAuth device-flow always posts empty scope
  *Location:* `crates/oauth/src/device_flow.rs` (`request_device_code_with_headers`)
  *Description:* On `upstream/main`, form includes `("scope", "")` instead of `config.scopes`.
  *Impact:* Providers needing `offline_access` / custom scopes (xAI, potentially others) get wrong grants or brittle tokens.
  *Root cause:* Device-flow path not updated when scoped OAuth configs were added.

- **CQ-002 [High]** — Heartbeat `end="24:00"` parse order bug
  *Location:* `crates/cron/src/heartbeat.rs`
  *Description:* Code special-cases `"24:00"` **after** `parse_hhmm`, but chrono `%H` rejects 24 → fail-open always-active.
  *Impact:* Default config window is a no-op (#1223).
  *Root cause:* Sentinel handled too late; tests only smoke “doesn’t panic”.

- **CQ-003 [Medium]** — Production `expect` on DB open / migrations in startup
  *Location:* `crates/gateway/src/server/prepare_core.rs` (multiple `.expect("failed to ...")`)
  *Description:* Startup uses expect for DB path/open/migrations.
  *Impact:* Hard process abort vs typed startup error surface; acceptable for boot, but inconsistent with “no panic in production” guidance.
  *Root cause:* Bootstrapping pragmatism.

- **CQ-004 [Low]** — Large number of files with `allow(clippy::unwrap_used)`
  *Location:* ~419 files matching allow lint
  *Description:* Unwrap discipline is uneven; scan shows very high unwrap counts in tools/skills/import/memory crates (includes tests mixed in counts).
  *Impact:* Panic risk in edge paths; review noise.
  *Root cause:* Historical growth + test-heavy modules sharing allow attributes.

---

## 5. Security Audit

### Verdict

Security posture is above average for an agent runtime: sandbox backends (Docker/Podman/Apple/Firecracker), vault, auth middleware, setup-required gates, channel webhook rate limits, observability redaction. Main risks are operator UX footguns and sandbox platform bugs—not obvious auth bypasses from this pass.

### Findings

- **SEC-001 [High]** — Auth disable in Docker does not match user expectations (#1112)
  *Location:* `crates/httpd/src/auth_middleware.rs`, `crates/auth`, onboarding SPA gates
  *Description:* Users report `auth.disabled` / local-only Docker still forces password onboarding. Code distinguishes config `auth.disabled`, DB `auth_state.auth_disabled`, and setup-complete gates.
  *Impact:* Local single-user Docker UX broken; risk that users weaken networking/auth incorrectly to work around it.
  *Root cause:* Multiple sources of truth (TOML vs SQLite auth_state vs setup completion) + Docker bind/non-localhost heuristics.

- **SEC-002 [Medium]** — Sandbox DMI sysfs mounts break arm64 Docker (#1085)
  *Location:* `crates/tools/src/sandbox/docker.rs` (`/sys/class/dmi`, `/sys/devices/virtual/dmi`)
  *Description:* Hardcoded tmpfs mounts assume x86 SMBIOS nodes; absent on Docker Desktop arm64 read-only sysfs.
  *Impact:* Exec sandbox fails on Apple Silicon Docker—security feature becomes availability failure (failover may drop isolation).
  *Root cause:* Host assumptions not arch-gated.

- **SEC-003 [Low]** — Dockerfile runs as non-root (`USER moltis`) but no `HEALTHCHECK`
  *Location:* `Dockerfile`
  *Description:* Non-root is good; missing HEALTHCHECK reduces orchestrator signal.
  *Impact:* Weaker deploy health semantics.
  *Root cause:* Omission.

- **SEC-004 [Info]** — Secrets handling patterns look intentional
  *Location:* `secrecy::Secret`, oauth token store, key store, redact module
  *Description:* No clear hardcoded production secrets found in this pass; candidates filtered heavily toward config/env patterns.
  *Impact:* N/A
  *Root cause:* N/A

---

## 6. Performance Analysis

### Findings

- **PERF-001 [Medium]** — Idle high CPU (#328) aligns with dense background tickers
  *Location:* `crates/httpd/src/server/runtime.rs`, `crates/gateway/src/server/*`, connectors maintenance loops
  *Description:* Multiple `interval.tick()` loops (some 30s, some tighter). Idle CPU reports remain open.
  *Impact:* Laptop battery / VPS CPU waste; perception of “heavy” daemon.
  *Root cause:* Many subsystems poll independently without a unified scheduler budget.

- **PERF-002 [Low]** — Feature-heavy default builds
  *Location:* workspace feature graphs (`cli`/`gateway` defaults)
  *Description:* Full feature sets pull browser/chromiumoxide-class deps; observed OOM risk compiling full gateway tests in constrained Docker.
  *Impact:* Slow CI/dev loops; memory spikes.
  *Root cause:* Batteries-included product defaults.

---

## 7. Testing Coverage & Quality

### Verdict

Testing culture is strong: crate unit tests, Playwright specs (oauth/onboarding/channels/chat/cron/mcp…), weekly provider-integration workflow, CodSpeed.

### Findings

- **TST-001 [High]** — Heartbeat active-hours tests do not assert behavior
  *Location:* `crates/cron/src/heartbeat.rs` tests
  *Description:* Tests call helper and ignore results / only check invalid→active.
  *Impact:* Allowed #1223 to persist.
  *Root cause:* Non-deterministic time avoided instead of injecting clock / `now_minutes`.

- **TST-002 [Medium]** — TTS default-state tests sensitive to ambient config/env (#1114 follow-ups)
  *Location:* `crates/gateway/src/voice/tts_service.rs`
  *Description:* Status/auto-select assertions can fail on developer machines with keys configured unless isolated/pure helpers are used.
  *Impact:* Flaky CI/dev tests; false confidence.
  *Root cause:* Helpers load discoverable config + env fallbacks.

- **TST-003 [Info]** — E2E inventory is broad
  *Location:* `crates/web/ui/e2e/specs/`
  *Description:* Dozens of specs covering auth, onboarding providers, channels, chat, oauth, cron.
  *Impact:* Positive.
  *Root cause:* N/A

- **E2E-001 [Medium]** — Fork PR checks currently not usefully reported
  *Location:* GitHub Actions on fork PRs (`action_required`, “no checks reported”)
  *Description:* Upstream CI appears to require approval for first-time contributors; local Docker was needed for validation.
  *Impact:* Maintainers may wait; authors get weak signal.
  *Root cause:* Fork PR security defaults / workflow permissions.

---

## 8. Dependency Health

### Findings

- **DEP-001 [Low]** — Very large dependency surface (channels + browser + matrix + whatsapp + voice…)
  *Location:* workspace `Cargo.toml` / lockfile
  *Description:* Optional features mitigate runtime, but compile-time cost is high.
  *Impact:* Build times, supply-chain review burden.
  *Root cause:* Product breadth.

- **DEP-002 [Info]** — Nightly pinned via `rust-toolchain.toml` (`nightly-2026-06-20`)
  *Description:* Reproducible toolchain; good.
  *Impact:* N/A

---

## 9. Documentation Completeness

### Findings

- **DOC-001 [Low]** — Plan location convention conflict
  *Location:* `CLAUDE.md` says plans in `prompts/`; `.gitignore` ignores `prompts/`; repo historically uses `plans/`
  *Description:* Greptile flagged plan path; gitignore makes `prompts/` unusable for tracked docs.
  *Impact:* Contributor confusion / bot noise.
  *Root cause:* Docs and ignore rules diverged.

- **DOC-002 [Info]** — Security/sandbox/provider docs are comparatively strong
  *Location:* `docs/src/security.md`, `sandbox.md`, `providers.md`, `SECURITY.md`

---

## 10. Technical Debt Analysis

### Findings

- **TD-001 [High]** — Heartbeat quiet-hours feature is half-landed (config + helper − wiring)
- **TD-002 [Medium]** — Provider OAuth refresh single-flight not universal
  *Location:* provider adapters (Codex/Kimi/xAI-class)
  *Description:* Rotating refresh tokens (xAI) require serialization; not all adapters share a lock pattern.
  *Impact:* Concurrent chats near expiry can invalidate sessions.
  *Root cause:* Per-provider refresh copy/paste without shared token service.
- **TD-003 [Medium]** — Sandbox platform matrix debt (Apple Container name limits #1137, arm64 DMI #1085, proxy DNS #1086)
- **TD-004 [Low]** — Channel parity debt (MS Teams #324, Slack shared tools #1224 despite recent #1238)

---

## 11. CI/CD & DevOps

### Findings

- **CICD-001 [Medium]** — First-time/fork PR workflows sit in `action_required`
  *Impact:* Slow feedback for external contributors.
- **CICD-002 [Low]** — No Dockerfile HEALTHCHECK
- **CICD-003 [Info]** — Positive: dedicated `e2e.yml`, `provider-integration.yml` (scheduled), `codspeed.yml`, release workflows

---

## 12. Database Schema & Migration Review

### Verdict

SQLite-centric with multiple migration sets (sessions/cron/webhooks/gateway/vault/projects). Not a classic Postgres goose tree; still appears structured.

### Findings

- **DB-001 [Medium]** — Fresh Docker Compose DB file confusion (#293)
  *Description:* Users report missing DB file on fresh compose.
  *Impact:* Onboarding friction; possible volume/path mismatch.
- **DB-002 [Low]** — Backup/restore runbook not obvious from quick docs scan
  *Impact:* Ops risk for single-node personal data stores.

---

## 13. API Contract Review

### Findings

- **API-001 [Medium]** — Provider entitlement errors need distinct UX contracts
  *Description:* OAuth success + inference 403 (tier gate) should not instruct re-login (seen in xAI ecosystem; relevant if/when `xai-oauth` lands).
  *Impact:* Support load / credential churn.
- **API-002 [Low]** — Preferred models UI labeling bugs (#282/#1094)

---

## 14. Frontend Audit

### Findings

- **FE-001 [Medium]** — Mobile multiline input / stop button UX (#1107)
- **FE-002 [Low]** — Heartbeat screen scroll (#223), update banner ignores Docker installs (#1109)
- **FE-003 [Info]** — Playwright coverage of onboarding/oauth/chat is a strength

---

## 15. Observability & Monitoring

### Verdict

`crates/observability` with exporters, recorder, and `redact.rs` is a solid baseline. Metrics feature flags exist.

### Findings

- **OBS-001 [Low]** — Idle CPU issue lacks linked profiling artifact in issue (#328)
  *Impact:* Harder to confirm which ticker dominates.
- **OBS-002 [Info]** — Redaction helpers present (good for agent logs)

---

## 16. Remediation Plan

> **No fixes performed in this audit.** Below is a recommended order only.

### 16.1 Suggested sequencing

1. Quiet-hours correctness (heartbeat parse + call site)
2. OAuth device-flow scope serialization
3. TTS Coqui eligibility alignment + hermetic tests
4. Sandbox arm64 DMI mount gating
5. Auth-disabled/setup-complete Docker UX clarification
6. Idle CPU profiling + ticker consolidation
7. Channel residual bugs (Slack shared tools, MS Teams)
8. Docs convention cleanup (`plans/` vs `prompts/`)

### 16.2 Action Items

| ID | Finding | Severity | Effort | Upstream issue | Notes |
|----|---------|----------|--------|----------------|-------|
| A-002/CQ-002/TD-001 | Heartbeat active hours | 🟠 High | S | #1205 #1223 | Also covered by fork PR #1241 (not audit work) |
| CQ-001 | Device-flow empty scope | 🟠 High | XS–S | (no dedicated issue) | Blocks xAI OAuth class features |
| TTS/Coqui | False configured | 🟠 High | S | #1114 | Fork PR #1242 |
| SEC-002 | arm64 DMI mounts | 🟠 High | S–M | #1085 | Sandbox availability |
| SEC-001 | Auth disabled Docker | 🟠 High | M | #1112 | Multi-source auth state |
| PERF-001 | Idle CPU | 🟡 Medium | M | #328 | Needs profiling first |
| TD-002 | OAuth refresh single-flight | 🟡 Medium | S–M | related to #1239/provider work | |
| TD-003 | Apple container name limit | 🟡 Medium | S | #1137 | |
| CICD-001 | Fork PR check gating | 🟡 Medium | S | process | |
| DOC-001 | prompts vs plans | 🔵 Low | XS | — | |
| FE/UI nits | Prefer models / banners | 🔵 Low | S | #282 #1094 #1109 | |

### 16.3 Quick wins (XS/S)

- Fix device-flow scope posting
- Fix heartbeat `24:00` parse order + wire check into agent turn
- Coqui `is_configured()` in `list_providers`
- Gate DMI mounts on arch / path existence
- Clarify docs: tracked plans live in `plans/`

### 16.4 Out of scope for immediate remediation (backlog)

- Landlock FS isolation (#818/#868)
- Netbird (#764)
- Sub-agent provider failover (#949)
- FunASR/SenseVoice (#1102)
- New channels (LINE/SMS/Lark)

---

## 17. Appendices

### A. Confirmed still broken on `upstream/main` (spot checks)

| Bug | Evidence on `main` |
|-----|--------------------|
| Heartbeat hours ineffective | `is_within_active_hours` only referenced from tests; `"24:00"` parsed after chrono failure |
| Device-flow empty scope | `("scope", "")` literal in `device_flow.rs` |
| Coqui always listed configured | `(TtsProviderId::Coqui, true)` in `tts_service.rs` |
| Docker DMI mounts | hardcoded `/sys/class/dmi` paths in sandbox docker backend |

### B. Upstream open bug themes

- **Runtime correctness:** heartbeat hours, idle CPU, auth-disabled Docker
- **Sandbox portability:** arm64 Docker mounts, Apple Container naming/DNS
- **Channels:** Slack shared tools (#1224 open; #1238 merged recently—verify residual), MS Teams (#324)
- **Providers/voice:** Coqui false configured, Codex OAuth unknown_error (#207), ZAI model gating (#250)

### C. Scan artifacts

Stored under `project-management/audit/scan-results/`:
- `todos.txt`, `panic-unwrap.txt`, `unsafe.txt`, `secrets-candidates.txt`
- `auth-disabled.txt`, `security-headers.txt`
- `heartbeat-active-hours.txt`, `oauth-refresh.txt`, `tts-coqui.txt`

### D. Related fork PRs (context only; not part of this audit’s changes)

- https://github.com/moltis-org/moltis/pull/1240 — xAI OAuth
- https://github.com/moltis-org/moltis/pull/1241 — heartbeat active hours
- https://github.com/moltis-org/moltis/pull/1242 — Coqui false-configured

### E. Explicit non-actions

- No code fixes in this audit
- No upstream issues created from this report yet (awaiting your go-ahead)
- No dependency upgrades / clippy cleanup applied
