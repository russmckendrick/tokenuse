# Unreleased

Changes that should be included in the next release go here. Keep this file current during normal development; move the relevant notes into `docs/releases/<version>.md` only when preparing a release.

## Fixed

- Codex MCP tool calls now appear in the **MCP Servers** panel. Previously the Codex parser only read the `name` field of `function_call` entries and ignored the separate `namespace` field where Codex records the `mcp__<server>__` prefix, so MCP usage from Codex sessions was silently dropped. The parser now joins `namespace` and `name` into the canonical `mcp__<server>__<tool>` form, matching how Claude Code stores MCP calls.
