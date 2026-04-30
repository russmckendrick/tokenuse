# Unreleased

Changes that should be included in the next release go here. Keep this file current during normal development; move the relevant notes into `docs/releases/<version>.md` only when preparing a release.

## Highlights

- Added a runtime sort mode for both TUI and desktop dashboards, cycling between spend, latest date, and token use.
- Session call rows now open a detail modal in both desktop and TUI, showing the full stored prompt, tool metadata, shell commands, and token breakouts.
- The TUI header now uses the bars mark with the `Token Use` title and omits the version label.
- Desktop builds now regenerate app icons from `desktop/tokenusebars.svg`.
- The desktop header now uses the bars mark with the `Token Use` title and omits the version and source labels.
- Desktop dashboard sections now render all rows and scroll overflowing data inside each section.
- Tagged releases now publish unsigned Windows and Linux desktop app assets with checksum files alongside the signed macOS DMG.

## Notes

- Version bumps and numbered release notes are release-prep tasks, not part of every user-visible change.
