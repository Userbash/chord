# BASELINE-QUALITY — стартовые метрики и слабые места

Дата: 2026-10-24. PM. Источник: локальный `rg`/`find`/`cargo`, без сети.
Цель — не «красивая цифра», а список конкретных мест для аудита G1/G2/G4.

## 1. Зелёные гейты на старте

| Проверка | Результат |
|---|---|
| frontend (dom/model/bench/features/auth) | **212/0** |
| core `cargo test -p discord-core` | **574/0** |
| gui-spike | **161/0** |
| workspace | **1112/0** |
| meta / unit / smoke | **39/0 · 381/0 · 22/0** |
| e2e | 4 теста, **4 skip** |
| gate:release | **passed_with_skips** |
| typecheck / geometry | 0 / 31/31 |

## 2. Rust: production-код (без tests)

| Метрика | Значение | Комментарий |
|---|---:|---|
| `unwrap(` | **106** | не все плохие; аудит по crate/ветке |
| `expect(` | **30** | часть — startup/FFI; нужен контекст |
| `panic!(` | **2** | проверить достижимость |
| `#[allow(...)]` | **31** | 11 `unused_imports` в `ready/snapshot.rs` — подозрительно |
| `unsafe {` | **62** | FFI: store 24, platform 38; safety-комментарии обязательны |

### Крупнейшие production-файлы

| Файл | Размер | Статус |
|---|---:|---|
| `discord-i18n/src/relative_data.rs` | 238 КБ | **generated**, не рефакторить вручную |
| `discord-i18n/src/duration_data.rs` | 201 КБ | **generated** |
| `gui-spike/src/data.rs` | 75 КБ | кандидат на split по домену |
| `gui-spike/src/auth.rs` | 64 КБ | кандидат на split (flow/store/errors) |
| `discord-core/src/ws.rs` | 40 КБ | аудит transport/state |
| `platform/src/dbus/message.rs` | 33 КБ | аудит FFI/D-Bus |
| `core/src/state/presence.rs` | 30 КБ | аудит reducer |
| `gui-spike/src/main.rs` | 29 КБ | команды/`too_many_arguments` |

## 3. Frontend

| Файл | Размер/строки | Комментарий |
|---|---:|---|
| `ui/features/search-overlay.js` | 43 КБ / 1049 | remote+local+фильтры+пагинация — split |
| `ui/features/settings.js` | 32 КБ / 746 | 38 toggles/hydrate/save |
| `ui/features/auth.js` | 26 КБ | login/MFA/captcha/busy |
| `ui/features/chat.js` | 25 КБ / 577 | rows/tombstone/menu |
| `ui/features/emoji.js` | 21 КБ | picker |
| `ui/model.js` | 18 КБ | PageStore/height index |
| `tests/unit/gui-spike-features.test.mjs` | 2303 строки | монолит, разбить по фичам |
| `tests/support/gui-dom-harness.mjs` | 1397 строк | общий шим, менять аддитивно |
| `tests/unit/gui-spike-auth.test.mjs` | 1425 строк | разбить по flow |

## 4. Конкретные smelly-места (стартовые)

| ID | Место | Что не так | P |
|---|---|---|---|
| Q-01 | `main.rs:244,255` | два Tauri-command `join_guild`/`enter_guild` — один контракт, лишняя поверхность | P2 |
| Q-02 | `ui/features/search-overlay.js` | 1049 строк, несколько состояний/режимов в одном модуле | P1 |
| Q-03 | `tests/unit/gui-spike-features.test.mjs` | 2303 строки, медленная навигация/дубли setup | P2 |
| Q-04 | `gui-spike/src/data.rs` / `auth.rs` | крупные модули, вероятно несколько ответственностей | P1 |
| Q-05 | `core/src/ready/snapshot.rs:393-413` | 11 подряд `#[allow(unused_imports)]` | P2 |
| Q-06 | `ui/app.js:228`, `metrics.js:125` | `console.log` вне тестов | P3 |
| Q-07 | `main.rs:375` | `too_many_arguments` — command/params стоит сгруппировать | P3 |
| Q-08 | `unsafe` 62 | нет единого реестра safety-инвариантов | P1 security |
| Q-09 | e2e 4 skip | нет fixture-прогона без live | P2 |
| Q-10 | DP-02 | 44/40 вместо 26/48/42 | P1 parity |

## 5. Правила чтения метрик

- `discord-i18n` data — generated; не считать «долгом» и не рефакторить вручную.
- `unwrap` в тестах/`#[cfg(test)]` не смешивать с production; аудит даст точный split.
- Длинный тест не «плохой»; плохой — дублирующий setup и отсутствие независимого оракула.
- LOC не цель: цель — меньше связности/дублей при тех же тестах.

## 6. Команды baseline

```bash
rg -c 'unwrap\(|expect\(|panic!\(|#\[allow|unsafe\s*\{' rust/gui-spike/src rust/crates/discord-*/src --glob '*.rs'
find rust/gui-spike/src rust/crates/discord-*/src -name '*.rs' -printf '%s %p\n' | sort -nr
find rust/gui-spike/ui -name '*.js' -printf '%s %p\n' | sort -nr
node --test tests/unit/gui-spike-*.test.mjs
cd rust && cargo test --workspace
npm run gate:release
```
