# install_view.rs — каталоги установки и окна Java/Content

## Назначение
Фоновые фетчи каталогов Forge/OptiFine (потребляются единым install-селектором
в `main_view`), окна Java и Content: загрузка метаданных в фоне, запуск
установки.

## Как работает
- Фоновые потоки пишут данные в глобальные слоты `FORGE_SLOT`,
  `OPTIFINE_SLOT`, `CONTENT_SLOT` (`LazyLock<Mutex<Option<…>>>`), UI-поток
  читает каждый кадр. НЕ thread_local (невидимо между потоками).
- Слоты Forge/OptiFine держат `Result`: `Ok` = каталог готов, `Err` = фетч
  упал (показывается в попапе селектора и ретраится). После успешной
  установки (или при смене метаданных) слот сбрасывается в `None`, чтобы
  учесть изменения.
- `fetch_allowed(&LAST_FETCH)` — троттлинг: не чаще одного ретрая в 30 с
  (иначе упавший фетч плодил бы поток каждый кадр).
- `forge_catalog() → Option<ForgeData>` / `optifine_catalog()` /
  `forge_error()` / `optifine_error()` — читатели слотов для main_view;
  `fetch_forge_async()` / `fetch_optifine_async()` — спавн фоновых фетчей.
- `start_forge_install(state, mc, build)` / `start_optifine_install(state,
  build)` — запуск установки в потоке: проверка Java (`find_java` 17 / 8),
  `Task::Running` → `Task::Done|Error`; после завершения слот каталога
  сбрасывается в `None`.
- `render_windows()` — роутер по флагам `state.show_install_java` /
  `state.show_content` (окна Forge/OptiFine удалены — каталоги теперь в
  списке установки).
- Java-окно: три быстрые кнопки (21/17/8) + строка «Custom:» — любая мажорная
  версия 8–100 (валидация `parse::<u32>()`; невалидно → кнопка disabled +
  красная подсказка). Установка через `start_java_install(state, major)`.
- Content-окно: индекс `ContentIndex` из `CONTENT_SLOT`, «Refresh index»
  сбрасывает слот и перефетчит; кнопка файла запускает `start_content_download`.
- Каждое окно после `.show()` вызывает `super::window_close_cursor()`.

## История изменений
### 2026-08-13 — v3.0.0
- Окна выбора Forge/OptiFine удалены: слоты переведены на
  `Option<Result<…, String>>`, добавлены `forge_catalog`/`forge_error`/
  `optifine_catalog`/`optifine_error` и `fetch_*_async` для единого
  install-селектора в `main_view`.
- Ретрай фетчей с троттлингом 30 с (`FORGE_LAST_FETCH`/`OPTIFINE_LAST_FETCH`,
  `fetch_allowed()`); сброс слота каталога в `None` после установки.
- Старые записи ниже — по окнам Forge/OptiFine, удалённым в v3.0.0.

### 2026-08-12 — v2.0.0 (до v3.0.0)
- Java: добавлено поле Custom — установка любой мажорной версии 8–100.
- Все окна: добавлен `window_close_cursor()` (палец на X).