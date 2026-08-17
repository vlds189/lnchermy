# AGENTS.md — Minecraft Launcher (Rust + egui)

## Overview

A portable Minecraft Java Edition launcher built in Rust + egui 0.36.
Single ~11 MB `.exe`, no runtime dependencies. Feature-parity with the
sibling PowerShell console (`mc_console.ps1` in the parent folder).

## Build & Test

```bash
cargo build --release        # → target/release/mc-launcher.exe (~11 MB)
cargo check                  # fast type-check
cargo test -- --skip spawns_java   # all tests except the live Minecraft launch smoke test
cargo test                   # all tests (spawns_java actually launches MC 1.20.1)
```

Release profile: `opt-level="z"`, `lto=true`, `strip=true`, `panic="abort"`.

## Handover workflow (mandatory)

Before building `--release` and handing the `.exe` to the user for manual
verification, ALWAYS self-review the uncommitted diff first:

1. Load the `code-review` skill and run it against `git diff` (plus any new
   untracked files in `src/`).
2. Fix the findings it reports; re-run `cargo check` / `cargo test` as needed.
3. Only then run `cargo build --release` and hand over.

This is enforced mechanically too: the `review-gate` plugin (`.opencode/plugins/`)
blocks the first `cargo build --release` while the working tree is dirty and
the diff hash differs from the last reviewed one (`.opencode/review-gate.sha`).
The retry after the review is the confirmation signal. `cargo check`/`cargo test`
and reinstalls of an already-approved diff are never blocked.

## Release workflow (mandatory)

The moment a release version is being chosen (user asks to release / the next
version number is about to be picked — before ANY edit to `version.json`,
`src/state.rs::APP_VERSION`, docs headers or tags):

1. Load the `release-version-check` skill (`.opencode/skills/`) and run it
   against the proposed version: it verifies semver, that the version is
   higher than the last local AND remote git tag and the last GitHub release,
   that `version.json` / `APP_VERSION` stay in sync, and that no `vX.Y.Z+`
   doc placeholders remain. Report the verdict before bumping.
2. Only then bump version + docs, rebuild release (the `review-gate` retry
   flow still applies), and release.

The `release-version-hook` plugin (`.opencode/plugins/`) appends a reminder
to release-intent messages so the skill fires automatically; treat the
reminder as an instruction to run the skill right away.

## Architecture

### Entry point
- `main.rs` — `LauncherApp` (impl `eframe::App`), owns `AppState`. The `ui()`
  callback runs each frame: applies theme, polls game process (`try_wait`),
  detects task busy→idle transitions (rescan versions), renders UI.

### Core modules

| Module | Responsibility |
|---|---|
| `state.rs` | `AppState` (settings, work_dir, versions, task, launch_status, game_child). `Task` and `LaunchStatus` enums. `rescan_versions()`. |
| `settings.rs` | `Settings` struct, load/save `mc_console_settings.json` (PascalCase keys: `RAM_MIN`, `RAM_MAX`, `ContentIndexUrl`, `Username`, `Theme`). Username validation. |
| `theme.rs` | Dark/Light egui themes with Minecraft-green accent (`ACCENT`). |
| `update.rs` | Self-update: fetch `version.json` from GitHub, semver compare, download+swap `.exe` with `.bak` backup. |

### Minecraft logic

| Module | Responsibility |
|---|---|
| `versions.rs` | Mojang version JSON structs (`VersionJson`, `Library`, `Rule`, `ArgValue`). `load_resolved()` merges `inheritsFrom`. `expand_arguments()` handles `${var}` + rules. `native_classifier()` detects natives (old `natives` map + new `:natives-os`). |
| `rules.rs` | `rules_allowed()` — evaluates OS (`windows`/`linux`/`osx`) + feature gates (`is_demo_user`, `has_custom_resolution`, etc.). `compare_mc_version()` — tolerant dotted-version comparator. |
| `maven.rs` | `maven_rel_path()`, `dedup_libraries()` — group:artifact[:classifier] key, last wins (Forge overrides guava 15→17). |
| `natives.rs` | `extract_natives()` — zip extraction, skips META-INF + directory entries. |
| `java.rs` | `find_java(min_version)` — scans `jdk-*` dirs, exact-match preferred, then lowest ≥min. `get_java_version()` — parses `java -version` stderr, Java 8 `"1.8"` special-case. |
| `launch.rs` | `build_command()` — assembles classpath (dedup, Forge special-casing), JVM args, game args. `launch()` — spawns java, returns `Child` handle. |
| `http.rs` | Shared `reqwest::blocking::Client` (LazyLock) with connection pooling + 30s timeout. `download_file()` atomic (temp+rename). Browser UA for optifine.net. |

### Install pipeline

| Module | Responsibility |
|---|---|
| `install/vanilla.rs` | Full vanilla download: manifest → version JSON → client.jar (SHA1) → assets (`objects/<hash[:2]>/<hash>`) → libraries (with legacy maven fallback for libs without `downloads`). SHA-1 implemented inline (no extra dep). |
| `install/forge.rs` | Parse `maven-metadata.xml`, group by MC version, fetch promotions. `install_forge()` downloads installer + creates `launcher_profiles.json` + runs GUI. |
| `install/optifine.rs` | Scrape `optifine.net/downloads` (regex), bypass ad-wall (`adloadx` → `downloadx?f=...`). Browser UA required. |
| `install/java_jdk.rs` | Adoptium download, zip extract, rename to `jdk-<major>`. |
| `content.rs` | Content index JSON → download mods/resourcepacks/shaderpacks. |

### UI

| Module | Responsibility |
|---|---|
| `ui/mod.rs` | Routes between main view and settings. |
| `ui/main_view.rs` | Main screen: version selector, launch options (color-coded Launch button), install buttons, progress overlay, vanilla version picker window, delete confirm dialog. |
| `ui/selector.rs` | Reusable `selector()` dropdown: hand-drawn ComboBox-style button + own `egui::Popup` (ComboBox's internal ScrollArea caused a second scrollbar that scrolled the search bar away). Optional search (`on_search` + `search_hint`), per-option 🗑 delete (`on_delete`), `none_text`, `loading`/`loading_error` states; fixed 220 px list height (`LIST_H`), virtualized rows (`show_rows`). Indices map back to caller items by id. |
| `ui/settings_view.rs` | RAM presets + custom, username, content index URL, theme toggle, update check. |
| `ui/install_view.rs` | Forge/OptiFine/Java/Content modal windows. Background fetch via global `LazyLock<Mutex<Option<…>>>` slots. |

## Critical gotchas

### egui 0.36 breaking changes (vs older versions)
- `TopBottomPanel` → **`Panel`** (`Panel::top(id)`, `Panel::bottom(id)`)
- `Rounding` → **`CornerRadius`**
- `run_simple_native` → **`run_native`** + impl `App` trait with `fn ui(&mut self, ui: &mut Ui, frame: &mut Frame)`
- Panels take `&mut Ui`, not `&Context`
- egui has its own `Theme::Dark/Light` + `ctx.set_theme()`
- `TextEdit::singleline(...).desired_width(x).show(ui).response` — `desired_width` is on the builder, not the Response; and in 0.36 `.show(ui).response` is an **`AtomLayoutResponse`** (Deref → `Response`) — the concrete `Response` is `.show(ui).response.response` (see `ui/input.rs`)

### Thread communication
- Background workers communicate with the UI thread via **global `LazyLock<Mutex<Option<…>>>`** slots — NOT `thread_local!` (which is per-thread and invisible to other threads). This was a critical bug that caused version lists to never appear.
- Game process tracking: `Child` handle stored in `Arc<Mutex<Option<Child>>>`, polled each frame via `try_wait()`.
- Launch status is separate from Task: `LaunchStatus` enum (Idle/Launching/Running/Error) tracked independently.

### Forge special-casing
- When `mainClass` contains `BootstrapLauncher`: do NOT add the parent vanilla jar to classpath (causes `Module _1._20._1` conflict). Add explicit `-cp ${classpath}` + `-Djava.library.path` (Forge JSON omits both).
- Library dedup: `group:artifact:version` entries collapse (last wins = Forge override). `group:artifact:version:classifier` entries stay distinct (LWJGL base vs natives).

### Legacy library download
- Some libraries (Forge 1.7.10 era) have no `downloads.artifact` field — only `name`. Build maven path from coordinates and try `libraries.minecraft.net` then `maven.minecraftforge.net`.

### OptiFine ad-wall
- `optifine.net/adloadx?f=<file>` → scrape HTML for `downloadx?f=<token>` → real download URL. Browser User-Agent required on all optifine.net requests.

### SVG icons (ui/icons.rs)
- All UI icons are SVG files in `assets/icons/` (11 files, lucide-style
  strokes), embedded via `egui::include_image!` and rasterized by the
  `egui_extras` SvgLoader — `install_image_loaders()` runs once in main.rs.
  Assets are monochrome and tinted with the surrounding color at render time.
- egui's `Button` content is text-only (no `WidgetText::Image`), so ALL icon
  buttons go through `ui/icons.rs::icon_button()` — painter-drawn Button
  visuals (hover/active/disabled), same pattern as selector.rs's combo button.
- Painters that need a texture use `icons::texture(ctx, source, size)`
  (loader caches per uri+size, so it's a cheap lookup per frame).
- Never introduce new emoji/glyph icons in UI strings — add an SVG asset
  instead (`▶ ✓ ⚠ 🗑 ✖` worked in egui's fonts, `⟳ ✕` did NOT — hence SVG).

## Conventions

- **Error handling:** functions return `Result<T, String>` for fallible operations. Background threads communicate errors via `LaunchStatus::Error` or `Task::Error`. UI shows errors in status bar / below buttons.
- **HTTP:** all requests go through `http.rs` shared clients. Never create ad-hoc `reqwest::Client` instances.
- **File writes:** use temp file + rename (atomic) pattern from `http::download_file`.
- **Tests:** unit tests in each module + `tests/real_version.rs` integration tests against real installed versions. Live network tests skip gracefully on failure.
- **Game folder layout:** everything under the launcher's own directory (self-contained, portable). Same as PowerShell version.
- **Docs:** per-file documentation lives in `docs/` (mirrors `src/` tree, Russian, change history per file). Keep it fresh when editing code.
