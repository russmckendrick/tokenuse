# Installation

`tokenuse` ships as a terminal UI for Linux, macOS, and Windows, plus a desktop app for all three platforms. Both frontends read the same local archive and configuration directory.

There is no API key, proxy, telemetry endpoint, daemon, or live file watcher. Usage ingestion stays local-only; outbound network is limited to explicit Config-page downloads and maintainer refresh or release paths.

## macOS

Install the terminal UI with Homebrew:

```bash
brew install russmckendrick/tap/tokenuse
tokenuse
```

Install the desktop app with Homebrew Cask:

```bash
brew install --cask russmckendrick/tap/tokenuse-desktop
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

## Linux Desktop

Linux desktop releases are published as unsigned AppImage, deb, and rpm assets for AMD64 and ARM64. Verify the checksum before installing or running one:

```bash
curl -L -O https://github.com/russmckendrick/tokenuse/releases/latest/download/tokenuse-desktop-linux-amd64.AppImage
curl -L -O https://github.com/russmckendrick/tokenuse/releases/latest/download/tokenuse-desktop-linux-amd64.AppImage.sha256
sha256sum -c tokenuse-desktop-linux-amd64.AppImage.sha256
chmod +x tokenuse-desktop-linux-amd64.AppImage
./tokenuse-desktop-linux-amd64.AppImage
```

Use the matching `arm64` asset on ARM64 Linux. Debian-based systems can install the `.deb` asset with `sudo apt install ./tokenuse-desktop-linux-amd64.deb`; RPM-based systems can install the `.rpm` asset with `sudo dnf install ./tokenuse-desktop-linux-amd64.rpm`.

## Windows TUI

Download the latest Windows AMD64 executable from PowerShell:

```powershell
Invoke-WebRequest -Uri "https://github.com/russmckendrick/tokenuse/releases/latest/download/tokenuse-windows-amd64.exe" -OutFile "tokenuse-windows-amd64.exe"
Invoke-WebRequest -Uri "https://github.com/russmckendrick/tokenuse/releases/latest/download/tokenuse-windows-amd64.exe.sha256" -OutFile "tokenuse-windows-amd64.exe.sha256"
Get-FileHash .\tokenuse-windows-amd64.exe -Algorithm SHA256
.\tokenuse-windows-amd64.exe
```

Compare the hash output with the first value in `tokenuse-windows-amd64.exe.sha256`.

## Windows Desktop

Windows desktop releases are published as unsigned AMD64 NSIS and MSI installers. Verify the checksum before installing:

```powershell
Invoke-WebRequest -Uri "https://github.com/russmckendrick/tokenuse/releases/latest/download/tokenuse-desktop-windows-amd64-setup.exe" -OutFile "tokenuse-desktop-windows-amd64-setup.exe"
Invoke-WebRequest -Uri "https://github.com/russmckendrick/tokenuse/releases/latest/download/tokenuse-desktop-windows-amd64-setup.exe.sha256" -OutFile "tokenuse-desktop-windows-amd64-setup.exe.sha256"
Get-FileHash .\tokenuse-desktop-windows-amd64-setup.exe -Algorithm SHA256
Start-Process .\tokenuse-desktop-windows-amd64-setup.exe
```

Compare the hash output with the first value in `tokenuse-desktop-windows-amd64-setup.exe.sha256`. The MSI is also available as `tokenuse-desktop-windows-amd64.msi` for environments that prefer Windows Installer packages.

## Build From Source

For development or unreleased builds:

```bash
git clone https://github.com/russmckendrick/tokenuse
cd tokenuse
cargo run
```

Use a terminal at least `120x40`. Smaller terminals show a resize notice instead of the full dashboard.
