# Unreleased

Changes that should be included in the next release go here. Keep this file current during normal development; move the relevant notes into `docs/releases/<version>.md` only when preparing a release.

## Highlights

- Added explicit Windows and Linux desktop update checks backed by Tauri updater artifacts on GitHub Releases.

## Notes

- Release builds now publish a Tauri `latest.json` updater manifest plus NSIS/AppImage `.sig` files. Linux `.deb` and `.rpm` packages remain manual GitHub Release updates, and macOS desktop updates remain covered by Homebrew Cask.
