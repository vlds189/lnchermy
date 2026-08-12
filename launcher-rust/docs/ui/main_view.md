# main_view.rs — главный экран

## Назначение
Главная вкладка лаунчера: список установленных версий, Launch-кнопка,
инсталляция, прогресс, диалоги подтверждения.

## Как работает
- `render()` — сборка панелей: `Panel::top` (заголовок + версия), `Panel::left`
  «side_panel» (сайдбар: переключатель темы ☀/🌙 и кнопка «⚙ Settings»),
  `Panel::bottom` (статус-бар Task), `CentralPanel` (секции «Installed versions»,
  «Launch options», «Install»).
- `version_list_section()`: selectable-кнопки версий; при `LaunchStatus::Running`
  ВСЕ метки рендерятся через `add_enabled(false, …)` — disabled-виджеты в egui
  не ховерятся, поэтому у них нет подсветки и нет курсора.
- `launch_options_section()`: Launch-кнопка; при hover на Running показывает
  «✖ Close Game» (глиф ✖, а не ✕ — последний отсутствует в шрифтах egui),
  клик открывает confirm-диалог. Кнопка всегда enabled, чтобы ловить hover.
- Модальные окна (vanilla-пикер, delete, close game) обёрнуты в
  `if let Some(inner) = … .show(…)` и вызывают `super::window_close_cursor()`.
- `MANIFEST` — глобальный `LazyLock<Mutex<Option<Vec<String>>>>` для передачи
  списка версий из фонового потока в UI-поток (НЕ thread_local!).

## История изменений
### 2026-08-12 — v2.0.0+ (до v3.0.0)
- «✕ Close Game» → «✖ Close Game» (✕ = квадрат-заглушка в шрифтах egui).
- Все метки версий grey-out при запущенной игре (раньше подсвечивались).
- Хедер: убраны «⚙ Settings» и тумблер темы из top bar → левый сайдбар
  `Panel::left` 110 px; удалена дублирующая линия-сепаратор под хедером.
- Все окна: добавлен `window_close_cursor()` — палец на X окна
  (egui сам не ставит курсор на title-bar close button, он рисуется
  через `ui.interact` в обход Button).