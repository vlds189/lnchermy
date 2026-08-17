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
- `rescan_versions()` — сканирует `versions/` (наличие `<id>.json` или
  `<id>.jar`), сохраняет выбор; при отсутствии выбора предпочитает
  последнюю запущенную версию из настроек (`settings.last_version`,
  поле `LastVersion`), фолбэк — первая по алфавиту.

## История изменений
### 2026-08-17 — v3.0.9
- `rescan_versions()`: предвыбор последней запущенной версии вместо
  алфавитно-первой (см. `settings.last_version`, запись при Launch
  в `ui/main_view.rs::launch_version`).
### 2026-08-12 — v3.0.0
- `APP_VERSION` 2.0.0 → 3.0.0.
### 2026-08-12 — v2.0.0+ (до v3.0.0)
- Добавлены `update_msg` (инлайн-статус проверки обновлений)
  и `java_custom` (кастомная мажорная версия Java).