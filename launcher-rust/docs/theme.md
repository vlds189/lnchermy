# theme.rs — темы и палитра

## Назначение
Dark/Light темы egui + акцентные цвета лаунчера.

## Как работает
- Константы: `ACCENT` (зелёный), `WARN`, `ERROR`.
- `apply(ctx, theme)`: `ctx.set_theme()` + переопределение visuals актуальной
  темы через `set_visuals_of` (скругление углов 4px, кнопочный padding).
- `visuals.interact_cursor = Some(CursorIcon::PointingHand)` — глобальный
  палец на ВСЕХ интерактивных элементах (работает только у виджетов,
  реализованных через Button; для крестика окон — см. ui/mod.rs).

## История изменений
### 2026-08-12 — v2.0.0+ (до v3.0.0)
- Добавлен `interact_cursor = PointingHand` — палец при наведении
  на кнопки/радио/selectable (TextEdit сам ставит I-beam).