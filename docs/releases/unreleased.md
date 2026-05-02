# Unreleased

Changes that should be included in the next release go here. Keep this file current during normal development; move the relevant notes into `docs/releases/<version>.md` only when preparing a release.

## Highlights

- TUI Overview, Deep Dive, and Usage now use a refreshed graph-forward layout with chronological activity pulses, ranked bars, and per-tool usage consoles. Short dashboard periods use hourly activity buckets; longer periods use daily buckets.
- Desktop Overview, Deep Dive, and Usage now mirror the refreshed dashboard language with activity pulses, ranked bars, compact gauges, and per-tool usage consoles.
- The `1` period now means a rolling 24-hour window in both TUI and desktop, based on the current time rather than local midnight.
- Desktop app window closes now keep Token Use running in the background with tray/menu-bar restore and configurable usage-jump notifications.

## Notes

- Background usage alert thresholds live in `config.json` under `background_alerts`; the defaults notify after `$1.00`, `100k` tokens, or `25` calls of new automatic-refresh usage with a 30-minute cooldown.
- The desktop tray/menu-bar icon now uses generated transparent bar glyphs from dedicated SVG sources instead of the full square app icon.
- Homebrew tap updates now publish the desktop app as the `tokenuse-desktop` cask and record a cask rename from the old `tokenuse` cask, leaving `tokenuse` dedicated to the TUI formula.
- Version bumps and numbered release notes are release-prep tasks, not part of every user-visible change.
