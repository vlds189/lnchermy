# lnchermy

A portable, all-in-one PowerShell console for installing and launching **Minecraft Java Edition** (vanilla, Forge, OptiFine) — no official launcher required.

Everything runs from a single folder and stays self-contained: Java, game versions, libraries and assets are downloaded next to the scripts.

## Features

- **Launch Minecraft** — scans the `versions/` folder, supports vanilla, Forge and OptiFine (parses version JSON, handles `inheritsFrom`, natives extraction, OS/feature rules, modern + legacy launch args).
- **Install Minecraft** — downloads any vanilla release from Mojang, including client jar, libraries and assets.
- **Install Forge** — lists Forge builds grouped by Minecraft version, downloads the installer and opens it (auto-creates the `launcher_profiles.json` the installer requires).
- **Install OptiFine** — parses optifine.net, resolves the ad-walled direct download link, downloads the installer and opens it.
- **Install Java** — downloads portable Eclipse Temurin JDK 8 / 17 / 21 (Adoptium) into the folder. The launcher picks the right Java per Minecraft version automatically.
- **Download content** — fetches mods / resourcepacks / shaderpacks from your own JSON index (direct links), placing them in the correct folders.
- **Settings** — memory (RAM), in-game username, content index URL.
- **Self-update** — checks GitHub for a newer version on startup (and from Settings) and replaces the script when the user accepts.

## Requirements

- Windows 10 / 11
- Internet connection (for downloads)
- Run **`mc_console.bat`** (not the `.ps1` directly — the `.bat` bypasses PowerShell's ExecutionPolicy)

## Usage

1. Put `mc_console.bat` + `mc_console.ps1` (+ optional `content_index.example.json`) in a folder.
2. Double-click `mc_console.bat`.
3. Follow the menu:
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

First-time setup order: **4 (Java) → 2 (Minecraft) → 1 (Launch)**. Add Forge/OptiFine as needed.

## Java version per Minecraft release

| Minecraft | Java |
|---|---|
| 1.20.5+ | Java 21 |
| 1.17 – 1.20.4 | Java 17 |
| 1.16.5 and older | Java 8 |

The launcher reads `javaVersion` from each version's JSON and selects the matching JDK automatically.

## Self-update mechanism

The launcher compares its `APP_VERSION` against `version.json` in this repo. To publish a new release:

1. Edit `$APP_VERSION` at the top of `mc_console.ps1`.
2. Update `version.json` to the same value.
3. Commit and push. Existing installs will pick it up on next launch.

## Files tracked in this repo

- `mc_console.bat` — PowerShell launcher wrapper.
- `mc_console.ps1` — the launcher itself.
- `content_index.example.json` — template for the content download index.
- `version.json` — current published version (used by the self-updater).
- `.gitignore` — excludes all downloaded game data (~1.4 GB), per-user settings and logs.

## Notes

- OptiFine is **not** compatible with Forge on Minecraft 1.19.3+; install/run them as separate versions.
- The `mods/` folder is shared across all versions in a single install — mind cross-version mod compatibility.
- Updates replace only `mc_console.ps1` (a `.bak` backup of the previous file is kept). User settings and game data are untouched.
