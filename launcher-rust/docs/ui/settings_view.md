# settings_view.rs — экран настроек

## Назначение
Настройки: RAM (пресеты + кастом), ник, Content Index URL, тема, обновления.

## Как работает
- Секции собраны в `ScrollArea::vertical()` внутри `CentralPanel`.
- Хедер: «‹ Back».
- Тема: два `radio` (Dark/Light) + `crate::theme::apply()`.
- Обновления: кнопка «Check for updates» (блокирующий вызов
  `update::check_latest()` на UI-потоке) + инлайн-результат `state.update_msg`
  рядом с кнопкой (зелёный = актуальная версия, красный = ошибка/новая есть);
  при наличии новинки — кнопка «Install update & restart».

## История изменений
### 2026-08-12 — v2.0.0+ (до v3.0.0)
- Результат проверки обновлений выводится инлайн рядом с кнопкой
  («You are on the latest version (v…)»), поле `update_msg` в AppState.