# Installation

`tokenuse` ships as a terminal UI for Linux, macOS, and Windows, plus a signed macOS desktop app. Both read the same local archive and configuration directory.

There is no API key, proxy, telemetry endpoint, daemon, or live file watcher. Usage ingestion stays local-only; outbound network is limited to explicit Config-page downloads and maintainer refresh or release paths.

## macOS

Install the terminal UI with Homebrew:

```bash
brew install russmckendrick/tap/tokenuse
tokenuse
```

Install the desktop app with Homebrew Cask:

```bash
brew install --cask russmckendrick/tap/tokenuse
open -a "Token Use"
```

The desktop DMG is signed with Developer ID Application, notarized through App Store Connect, and published as `tokenuse-desktop-macos-universal.dmg`.

## Linux TUI

Download the latest AMD64 release:

```bash
curl -L -O https://github.com/russmckendrick/tokenuse/releases/latest/download/tokenuse-linux-amd64
curl -L -O https://github.com/russmckendrick/tokenuse/releases/latest/download/tokenuse-linux-amd64.sha256
sha256sum -c tokenuse-linux-amd64.sha256
chmod +x tokenuse-linux-amd64
sudo install -m 0755 tokenuse-linux-amd64 /usr/local/bin/tokenuse
tokenuse
```

Download the latest ARM64 release:

```bash
curl -L -O https://github.com/russmckendrick/tokenuse/releases/latest/download/tokenuse-linux-arm64
curl -L -O https://github.com/russmckendrick/tokenuse/releases/latest/download/tokenuse-linux-arm64.sha256
sha256sum -c tokenuse-linux-arm64.sha256
chmod +x tokenuse-linux-arm64
sudo install -m 0755 tokenuse-linux-arm64 /usr/local/bin/tokenuse
tokenuse
```

## Windows TUI

Download the latest Windows AMD64 executable from PowerShell:

```powershell
Invoke-WebRequest -Uri "https://github.com/russmckendrick/tokenuse/releases/latest/download/tokenuse-windows-amd64.exe" -OutFile "tokenuse-windows-amd64.exe"
Invoke-WebRequest -Uri "https://github.com/russmckendrick/tokenuse/releases/latest/download/tokenuse-windows-amd64.exe.sha256" -OutFile "tokenuse-windows-amd64.exe.sha256"
Get-FileHash .\tokenuse-windows-amd64.exe -Algorithm SHA256
.\tokenuse-windows-amd64.exe
```

Compare the hash output with the first value in `tokenuse-windows-amd64.exe.sha256`.

## Build From Source

For development or unreleased builds:

```bash
git clone https://github.com/russmckendrick/tokenuse
cd tokenuse
cargo run
```

Use a terminal at least `120x40`. Smaller terminals show a resize notice instead of the full dashboard.
