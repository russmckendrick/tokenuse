# Desktop App

`tokenuse` also has a Tauri v2 desktop shell under `desktop/`. It is a second frontend over the same Rust ingestion, archive, currency, pricing, and export code used by the TUI.

The TUI remains the default command:

```bash
cargo run
```

## Running Locally

Install the desktop frontend dependencies once:

```bash
cd desktop
pnpm install
```

Launch the Tauri development app:

```bash
pnpm run tauri:dev
```

Build a local macOS app bundle:

```bash
pnpm run tauri:build:app
```

Build a local macOS DMG:

```bash
pnpm run tauri -- build --bundles dmg
```

The desktop app uses Svelte + Vite for the webview and Tauri commands for all local data access. The frontend calls Rust through `@tauri-apps/api/core` `invoke()` calls; native folder selection and Config-page download confirmations use the Tauri dialog plugin.

## Distribution

Tagged releases build a universal macOS desktop DMG named `tokenuse-desktop-macos-universal.dmg`, sign it with a Developer ID Application certificate, notarize it through App Store Connect, and upload it to the GitHub Release with a `.sha256` checksum.

The release workflow also updates the existing Homebrew tap with a desktop cask. Install the desktop app with:

```bash
brew install --cask russmckendrick/tap/tokenuse
```

The macOS release job requires these repository secrets:

| Secret | Purpose |
| --- | --- |
| `APPLE_CERTIFICATE` | Base64-encoded Developer ID Application `.p12` certificate |
| `APPLE_CERTIFICATE_PASSWORD` | Password for the exported certificate |
| `KEYCHAIN_PASSWORD` | Temporary CI keychain password |
| `APPLE_API_ISSUER` | App Store Connect issuer ID |
| `APPLE_API_KEY` | App Store Connect key ID |
| `APPLE_API_PRIVATE_KEY` | App Store Connect `.p8` private key contents |
| `HOMEBREW_TAP_TOKEN` | Token with push access to `russmckendrick/homebrew-tap` |

The workflow imports the `.p12`, verifies that it contains a `Developer ID Application` identity, and passes the discovered identity to Tauri as `APPLE_SIGNING_IDENTITY`. If this step reports no identity, export the certificate from Keychain Access > My Certificates so the `.p12` includes the private key.

Release tags must match the version in `Cargo.toml`, `desktop/src-tauri/Cargo.toml`, `desktop/package.json`, and `desktop/src-tauri/tauri.conf.json`. When preparing a release, bump all four version fields together before tagging.

## Shared Data

The desktop app and TUI share the platform config directory under `tokenuse`:

| File / directory | Shared purpose |
| --- | --- |
| `config.json` | User overrides, currently display currency |
| `archive.db` | Durable local usage archive |
| `rates.json` | Optional local currency snapshot |
| `pricing-snapshot.json` | Optional local LiteLLM-derived pricing snapshot |
| `exports/` | Fallback export directory |

Changing currency or refreshing the local archive in the desktop app affects the same data the TUI reads. Export folder changes are runtime-only, matching the TUI behavior.

## Architecture Notes

- `desktop/src-tauri/src/lib.rs` owns the Tauri builder, managed state, and commands.
- `desktop/src-tauri/src/main.rs` is only a thin passthrough to `tokenuse_desktop_lib::run()`.
- `desktop/src-tauri/capabilities/default.json` grants the narrow default permissions: core Tauri APIs and dialog access.
- Desktop refresh uses the same background archive refresher as the TUI. The frontend polls snapshots so completed refreshes are applied without blocking the UI.
- Default desktop builds include confirmed Config-page downloads for `rates.json` and `pricing-snapshot.json`. Build with `--no-default-features` for a no-download desktop binary; the `refresh-currency` and `refresh-prices` feature gates still control whether those network-backed actions compile in.

## Checks

```bash
cargo fmt --check
cargo test

cd desktop
pnpm run check
pnpm run build
pnpm run tauri:build:app
cd src-tauri
cargo check
```
