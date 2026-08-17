# state.rs — центральное состояние

## Назначение
`AppState`, `Task`, `LaunchStatus`, версия приложения и утилиты.

## Как работает
- `APP_VERSION` — текущая версия лаунчера, используется в UI и update-чеке.
- `Task` — фоновые задачи (Idle/Running/Done/Error) в `Arc<Mutex<Task>>`;
- `LaunchStatus` (Idle/Launching/Running/Error) — независимо от Task, отдельно
  от игры; `game_child: Arc<Mutex<Option<Child>>>` опрашивается каждый кадр.
- Поля UI-окон: `show_install_*`, `remote_versions`, `vanilla_filter`,
  `forge_custom`, `java_custom`, `pending_delete`, `pending_close_game`,
  `update_msg: Option<(bool, String)>` (инлайн-результат проверки обновлений).
- `pending_install: Option<PendingInstall>` — выбранная в 🔄-пикере
  неустановленная версия (Launch-кнопка → «Install»). `PendingInstall` —
  kind-aware enum `Vanilla(id) / Forge(mc, build) / OptiFine(mc, build)`
  с `label()` и `matched_installed_id(&list)` (поиск результата установки
  в `installed_versions`: vanilla — точное совпадение, Forge/OptiFine —
  ожидаемые варианты имён папок).
- `rescan_versions()` — сканирует `versions/` (наличие `<id>.json` или
  `<id>.jar`), сохраняет выбор; при отсутствии выбора предпочитает
  последнюю запущенную версию из настроек (`settings.last_version`,
  поле `LastVersion`), фолбэк — первая по алфавиту.

## История изменений
### 2026-08-17 — v3.0.11
- `pending_install`: `Option<String>` → `Option<PendingInstall>` — новый
  kind-aware enum (vanilla/forge/optifine) с `matched_installed_id()`:
  main.rs (busy→idle) авто-выбирает установленную версию, умея искать и
  Forge/OptiFine-папки по ожидаемым именам.
### 2026-08-17 — v3.0.9
- `rescan_versions()`: предвыбор последней запущенной версии вместо
  алфавитно-первой (см. `settings.last_version`, запись при Launch
  в `ui/main_view.rs::launch_version`).
### 2026-08-12 — v3.0.0
- `APP_VERSION` 2.0.0 → 3.0.0.
### 2026-08-12 — v2.0.0+ (до v3.0.0)
- Добавлены `update_msg` (инлайн-статус проверки обновлений)
  и `java_custom` (кастомная мажорная версия Java).