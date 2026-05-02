# Unreleased

Changes that should be included in the next release go here. Keep this file current during normal development; move the relevant notes into `docs/releases/<version>.md` only when preparing a release.

## Highlights

- The desktop app now forces dark native chrome and dark in-app scrollbars so the window matches the Token Use console palette even when macOS is in light mode.
- Clicking the macOS Dock icon now restores the hidden desktop window when Token Use is still running in the background.
- Left-clicking the desktop tray/menu-bar icon now opens a compact dark 24-hour usage popover, while the right-click menu still offers **Show Token Use** and **Quit Token Use**.
- The desktop Config page now includes Open at Login and Dock/taskbar icon toggles, backed by the shared `config.json` `desktop` settings block.
- macOS desktop release builds now publish an Apple Silicon DMG as `tokenuse-desktop-macos-arm64.dmg`; the Homebrew desktop cask now points at that arm64-only artifact.

## Notes
