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

Unsigned installer packaging remains out of scope for the first desktop pass. Use `pnpm run tauri:build` when preparing platform installers later.

The desktop app uses Svelte + Vite for the webview and Tauri commands for all local data access. The frontend calls Rust through `@tauri-apps/api/core` `invoke()` calls; native folder selection uses the Tauri dialog plugin.

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
- The default build remains local-only. The existing `refresh-currency` and `refresh-prices` feature gates still control network-backed refresh actions.

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
