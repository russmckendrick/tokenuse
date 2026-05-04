# Unreleased

Changes that should be included in the next release go here. Keep this file current during normal development; move the relevant notes into `docs/releases/<version>.md` only when preparing a release.

## Highlights

- Added explicit Windows and Linux desktop update checks backed by Tauri updater artifacts on GitHub Releases.
- Replaced dashboard snapshot exports with scoped Reports across the TUI and desktop app, adding executive HTML/PDF decks, one-page SVG/PNG visual summaries, and raw JSON, Excel, and CSV-folder output.

## Notes

- Release builds now publish a Tauri `latest.json` updater manifest plus NSIS/AppImage `.sig` files. Linux `.deb` and `.rpm` packages remain manual GitHub Release updates, and macOS desktop updates remain covered by Homebrew Cask.
- Reports can target all projects or one project for the selected period, always include all tools, default to unredacted raw data, and offer generation-time redaction for prompts, commands, raw paths, session IDs, and dedup keys.
- HTML and PDF reports now render as client-ready executive decks with cover metadata, KPI ribbons, insight tiles, activity pages, and breakdown pages; SVG and PNG outputs are redesigned as 16:9 executive visual summaries.
- HTML and PDF report overview pages now lead with deeper two-row KPI panels with colored header bands instead of a large title/subtitle cover treatment.
- Excel reports write multi-sheet `.xlsx` workbooks, while CSV reports write one timestamped folder with one file per report area.
