# ui/selector.rs — переиспользуемый селектор

## Назначение
Готовый dropdown-селектор (обёртка над `egui::ComboBox`) с опциональным
searchbar'ом и кнопкой удаления. Внутренний компонент, не привязан к версиям
Minecraft — клиенты передают свои данные через колбэки.

## Как работает
- `SelectorItem = (String, String)` — `(id, label)`: `id` — машинное значение
  (возвращается в колбэки), `label` — что видит пользователь (может содержать
  тег, например «1.20.1  [Forge]»).
- `selector(ui, id, items, selected_idx, enabled, on_delete, on_search, search_hint)`:
  - `selected_idx: &mut Option<usize>` — индекс в исходном `items`; обновляется
    при выборе. Клиент мапит индекс на свои данные.
  - `enabled: false` — опции и 🗑 рендерятся через `add_enabled(false)`:
    нет hover/подсветки (игра запущена). Специфичный тултип
    «Close the running game first» зашит в компонент (версии — единственный
    потребитель; при других применениях вынести в параметр).
  - `on_delete: Option<&mut dyn FnMut(usize, &str)>` — если `None`, 🗑 не
    рендерится вообще; если `Some` — у каждой строки появляется корзина,
    клик вызывает колбэк с `(индекс в items, id)` и НЕ меняет выбор.
  - `on_search: Option<&mut dyn FnMut(&str) -> Vec<SelectorItem>>` — если
    `Some`, над списком рисуется TextEdit; фильтрация — колбэк: строка
    запроса → отфильтрованные `(id, label)`. Пустой запрос → показываются
    все `items`, колбэк не вызывается. Отфильтрованные строки обратно
    сопоставляются с исходным `items` по `id`, поэтому `selected_idx`
    и индексы в `on_delete` остаются стабильными.
  - `search_hint: Option<&str>` — placeholder поиска при пустом поле;
    `None` → «Search…».
- Запрос поиска хранится в `ctx.data()` (temp-data, ключ `id.with("search_query")`):
  компонент без собственного состояния, но строка переживает кадры, пока
  попап открыт. Тот же приём, что rect сайдбара в `main_view`.
- `egui::Button::selectable` вместо отдельного виджета: в egui 0.36
  `SelectableLabel` удалён из публичного API (см. AGENTS.md, gotchas).

## История изменений
### 2026-08-13 — v3.0.0+
- Создан компонент: `selector()` с опциональными `on_delete`, `on_search`,
  `search_hint`. Вынесен из инлайнового ComboBox в `version_list_section`.
