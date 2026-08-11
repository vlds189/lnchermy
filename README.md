# lnchermy

A portable Minecraft Java Edition launcher — install and play vanilla, Forge and OptiFine without the official launcher.

Two editions live in this repo:

- **`mc_console.ps1`** + `mc_console.bat` — the original PowerShell console (v1.2.0).
- **`launcher-rust/`** — the native GUI edition (Rust + egui, v2.0.0). A single ~11 MB `.exe`, no runtime required.

## Features

Both editions share the same feature set and the same game-folder layout (`versions/`, `libraries/`, `assets/`, `jdk-*`, `mods/`, …):

- **Launch Minecraft** — vanilla, Forge and OptiFine. Parses version JSON, handles `inheritsFrom` merging, native library extraction, OS/feature rules, and both modern (1.13+) and legacy (≤1.12.2) launch arguments. Forge's `BootstrapLauncher` module-path setup and library de-duplication are reproduced exactly.
- **Install Minecraft** — downloads any vanilla release from Mojang: client jar (SHA1-checked), libraries (with a legacy maven-repository fallback for old Forge libs that lack a `downloads` field), and the full asset set (`objects/<hash[:2]>/<hash>`).
- **Install Forge** — parses `maven-metadata.xml`, groups builds by MC version, fetches recommended/latest labels, downloads the installer and opens it with the working directory pointing at your game folder (so it installs in place).
- **Install OptiFine** — scrapes `optifine.net/downloads`, resolves the ad-walled direct download link via the `adloadx` page, then runs the installer.
- **Install Java** — downloads portable Eclipse Temurin JDK 8 / 17 / 21 (Adoptium). The launcher picks the right Java per Minecraft version automatically (exact-match preference, so 1.7.10 runs on Java 8, not Java 17).
- **Download content** — fetches mods / resourcepacks / shaderpacks from your own JSON index of direct links.
- **Settings** — memory (RAM), in-game username, content index URL, dark/light theme (GUI edition).
- **Self-update** — checks `version.json` in this repo on startup and from Settings; downloads and swaps the binary when the user accepts.

## Requirements

- Windows 10 / 11
- Internet connection (for downloads)

## Using the PowerShell edition

Double-click **`mc_console.bat`** (it bypasses PowerShell's ExecutionPolicy).

```
1. Launch Minecraft
2. Install Minecraft
3. Install Forge
4. Install Java
5. Settings
6. Download content (mods/resourcepacks/shaders)
7. Install OptiFine
0. Exit
```

First-time setup: `4 (Java) → 2 (Minecraft) → 1 (Launch)`. Add Forge/OptiFine as needed.

## Using the Rust (GUI) edition

Build from source:

```bash
cd launcher-rust
cargo build --release
# binary: target/release/mc-launcher.exe (~11 MB)
```

Or grab a prebuilt `mc-launcher.exe` from Releases and double-click it. No installation, no runtime — it's a single self-contained executable.

The GUI provides the same actions as buttons, with live progress bars for downloads and background workers so the window never freezes.

## Java version per Minecraft release

| Minecraft | Java |
|---|---|
| 1.20.5+ | Java 21 |
| 1.17 – 1.20.4 | Java 17 |
| 1.16.5 and older | Java 8 |

The launcher reads `javaVersion` from each version's JSON and selects the matching JDK automatically.

## Self-update mechanism

Both editions compare their version against `version.json` in this repo:

- PowerShell: updates `mc_console.ps1` (keeps a `.bak` backup).
- Rust: updates the `.exe` (keeps a `.exe.bak` backup).

To publish a new version, bump `$APP_VERSION` (PowerShell) / `APP_VERSION` (Rust), update `version.json`, commit and push.

## Repository layout

```
mc_console.bat            # PowerShell launcher wrapper
mc_console.ps1            # PowerShell launcher (v1.2.0)
version.json              # current published version (self-updater reads this)
content_index.example.json # template for the content-download index
launcher-rust/            # Rust + egui GUI edition (v2.0.0)
  Cargo.toml
  src/
    main.rs               # eframe App entry point
    state.rs settings.rs theme.rs update.rs
    rules.rs maven.rs versions.rs natives.rs java.rs launch.rs
    http.rs content.rs
    install/ vanilla.rs forge.rs optifine.rs java_jdk.rs
    ui/ main_view.rs settings_view.rs install_view.rs
  tests/ real_version.rs
.gitignore                # excludes ~1.4 GB of game data, per-user settings, logs
```

The Rust edition is ~4400 lines across 22 source files, with 50 unit/integration tests (including a live smoke test that actually launches Minecraft 1.20.1).

## Notes

- OptiFine is **not** compatible with Forge on Minecraft 1.19.3+; install and run them as separate versions.
- The `mods/` folder is shared across all versions in a single install — mind cross-version mod compatibility.
- Neither edition sends any telemetry. The only outbound requests are to Mojang, Forge maven, optifine.net, Adoptium, raw.githubusercontent.com (update check), and the user-configured content index.
