# ui/mod.rs — корень UI

## Назначение
Роутер вкладок и общие UI-хелперы.

## Как работает
- `render()` — выбор между `settings_view` и `main_view` по `state.show_settings`.
- `window_close_cursor(ctx, window_rect)`: egui рисует title-bar close button
  через `ui.interact` (containers/window.rs), минуя Button → глобальный
  `Visuals::interact_cursor` на него не действует. Хелпер эмулирует зону
  крестика (правый верхний угол окна: 36×36 px) по rect из `InnerResponse`
  после `.show()` и ставит `CursorIcon::PointingHand` вручную.

## История изменений
### 2026-08-12 — v2.0.0 (до v3.0.0)
- Добавлен `window_close_cursor()`; подключён ко всем 7 модальным окнам.