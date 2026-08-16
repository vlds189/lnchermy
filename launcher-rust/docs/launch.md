# launch.rs — сборка команды запуска и spawn java

## Назначение
Собирает полную командную строку JVM (classpath, JVM-args, game-args с
подстановкой `${var}`) и запускает игру (`launch()`), возвращая `Child` для
отслеживания процесса.

## Как работает
- `launch()` → `build_command()` + распаковка natives + `Command::spawn()`
  (cwd = корень игры).
- `build_command()`: `load_resolved()` (merge `inheritsFrom`) → подбор Java
  (точное совпадение версии, см. `java.rs`) → classpath с дедупликацией
  библиотек и Forge-спецификой (BootstrapLauncher: без родительского jar,
  явный `-cp` + `-Djava.library.path`) → подстановка переменных.
- `offline_uuid(name)` — оффлайн-UUID игрока, побайтово равный
  `UUID.nameUUIDFromBytes("OfflinePlayer:"+name)` из Java (RFC 4122 v3,
  MD5). Попадает в `--uuid`. Именно из этого UUID клиент выбирает дефолтный
  скин: `DEFAULT_SKINS[floorMod(uuid.hashCode(), 18)]` (1.19.3+; индексы
  0–8 — slim/«женские», 9–17 — wide). Нулевой UUID давал всегда индекс 0
  (`slim/alex`) — баг «женский скин при любом нике». Значения сверены с
  UUID, которые сама игра вычисляет для LAN-игроков (usercache.json).
- `md5()` — инлайн-MD5 (RFC 1321, табличный T/S), только для `offline_uuid`;
  по конвенции «хэш инлайном, без крейта» (как SHA-1 в `install/vanilla.rs`).
- `clientid` остаётся нулевой константой — это идентификатор лаунчера,
  не игрока.

## История изменений
### 2026-08-16 — v3.0.5+
- `auth_uuid`: нулевая константа → `offline_uuid(username)` (фикс дефолтного
  скина по нику); добавлены `md5()`, `offline_uuid()` + тесты на векторы MD5
  и игровые эталонные UUID.
