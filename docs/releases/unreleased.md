# Unreleased

Changes that should be included in the next release go here. Keep this file current during normal development; move the relevant notes into `docs/releases/<version>.md` only when preparing a release.

## Added

- **Opt-in subscription-quota gauges for Claude.ai and ChatGPT (Codex).** Two new tool adapters — `claude_subscription` and `codex_subscription` — fetch live 5-hour / 7-day / Opus / Sonnet / Extra Usage gauges from `claude.ai/api/organizations/{uuid}/usage` (plus the overage endpoint) and from `chatgpt.com/backend-api/wham/usage` (after exchanging the session-token cookie for a bearer token at `/api/auth/session`). Both adapters write a local sidecar under `<config dir>/tokenuse/limits/` and tag the resulting `LimitSnapshot` rows with the existing `claude-code` / `codex` tool IDs, so the gauges appear inside the existing tool sections. The feature is gated behind the `quota-sync` Cargo feature (on by default) and surfaced as two confirmed Config-page actions — **Claude.ai subscription quota → Sync** and **ChatGPT (Codex) subscription quota → Sync**. Each first-run requires the user to store the session cookie via `tokenuse --set-claude-cookie <value>` / `tokenuse --set-codex-cookie <value>` (or the Tauri `set_claude_session_cookie` / `set_codex_session_cookie` commands), which writes the cookie to the OS keychain (`keyring` crate v3: Keychain on macOS, Credential Manager on Windows, Secret Service on Linux). See [docs/development/tools/claude-subscription.md](../development/tools/claude-subscription.md) and [docs/development/tools/codex-subscription.md](../development/tools/codex-subscription.md).

## Fixed

- Codex MCP tool calls now appear in the **MCP Servers** panel. Previously the Codex parser only read the `name` field of `function_call` entries and ignored the separate `namespace` field where Codex records the `mcp__<server>__` prefix, so MCP usage from Codex sessions was silently dropped. The parser now joins `namespace` and `name` into the canonical `mcp__<server>__<tool>` form, matching how Claude Code stores MCP calls.
