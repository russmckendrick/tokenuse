# Documentation

`tokenuse` docs are split into three sections:

## Guides

- [Installation](guides/installation.md): install the TUI and Apple Silicon macOS desktop app from Homebrew, or download TUI and desktop assets from GitHub Releases.
- [TUI usage](guides/tui-usage.md): dashboard navigation, filters, keyboard shortcuts, reloads, configuration, session drill-down, export, and the Usage page.
- [Desktop app usage](guides/desktop-usage.md): install, open, navigate, refresh, configure, and export from the Tauri desktop app.

## Development

- [Development overview](development/README.md): source layout and what to read before changing the project.
- [Architecture](development/architecture.md): archive, sync, aggregation, pricing, export, and frontend state flow.
- [Local development](development/local-development.md): commands for Rust, TUI, desktop, pricing, currency, and generated assets.
- [Source control](development/source-control.md): branch hygiene, generated files, version bumps, and release-prep notes.
- [Deployments](development/deployments.md): release workflows, binary assets, desktop notarization, and Homebrew tap updates.
- [Tool parsers](development/tools/README.md): parser contracts for Claude Code, Codex, Cursor, GitHub Copilot, and Gemini.

## Project

Release pages on the website are sourced from GitHub Releases. The checked-in [release notes](releases/) remain maintainer source material, but the website does not copy this folder into product docs.
