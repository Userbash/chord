# REPORT RW-27 — B09: headless i18n + platform (P7-01b, P6-04, P6-03)

Дата: 2026-10-22. Авторы кода: W27a (P7-01b), W27b (P6-04/P6-03).
Интеграция и фиксы: TL-2. Независимое ревью: REV-2 (general, не автор кода).

## 1. Сделано

- **P7-01b `ListFormat`** (`discord-i18n`): `format_list` для 31 включённой локали,
  3 стиля × 3 типа, данные `list_data.rs` сгенерированы Node 24 / ICU
  (`waves/RW-27/gen-list-data.mjs`), 279 строк, 1674 кейса оракула.
- **P6-04 Deep links** (`discord-platform`): разбор `discord://` (host/path/query/fragment,
  пустой host, лимит 4096, control-chars), `extract_deep_link` по RE §9.4, маршруты
  `login/one-time`, `action/oauth2/authorize`, `app/open`, `app/settings/<section>`,
  `library/<id>/launch`, `channels/...`; redaction токена.
- **P6-03 Автозапуск + бейдж** (`discord-platform`): XDG Desktop Entry, Exec-quoting,
  `autostart_path`, Unity badge-пары, `app_uri`; без D-Bus и I/O.
- **Фиксы TL-2 (red-first):**
  1. `needs_quotes` расширен до reserved-набора Desktop Entry 1.5
     (`' > < ~ | & ; * ? # ( )`) — P2 ревью; тест `a09`, red
     `workers/TL2/red-redaction-quoting.log`, green `workers/TL2/green-autostart.log`.
  2. `DeepLink` получил ручной `Debug`: `raw` не печатается, `query`/`fragment` —
     `<redacted>` (закрывает P3 из ревью RW-26); тест `d13`, red
     `workers/TL2/red-deeplink-debug.log`, green `workers/TL2/green-deeplink.log`.
- **Доказательная база исправлена:** число кейсов 1620→1674 (31 локаль, `pt-BR` добавлен),
  `middle == start` во всех 279 строках, M3/M4 признаны эквивалентными, md5 в отчёте W27a
  актуализированы; см. `workers/W27a/{mutations.md,TEST-CORRECTIONS.md}`.

## 2. Факт (финальный код)

| Проверка | Результат |
|---|---|
| `list_contract` | **10 passed / 0 failed** (1674/1674 кейса оракула) |
| `deep_link_contract` | **13 passed / 0 failed** |
| `autostart_contract` | **9 passed / 0 failed** |
| workspace | **902 passed / 0 failed** |
| fmt / clippy (`-D warnings`) | чисто |
| npm `gate:worker` | **passed**: meta 25, unit 169, smoke 20 (214) |
| Мутации ListFormat | **6/6 неэквивалентных caught**, 2 эквивалентных (M3, M4) |
| Мутации platform | **14/14 caught**, 0 missed, 0 inconclusive |
| Восстановление продукта | md5 совпадает после каждой мутации |

Логи: `gate-cargo-final.log`, `gate-worker-w27-final.log`,
`.gate/worker-2026-10-22T13-58-02-813Z.json`, `mutations/{w27a-final,w27b-final}.log`.

## 3. Ревью

Вердикт REV-2: **доработка** — P0/P1 нет, 2×P2 (Exec-quoting; расхождение доказательной
базы W27a), оба закрыты red-first/актуализацией. P3 — backlog §4. Повторного ревью после
фиксов не было (фиксы покрыты тестами и повторным gate).

## 4. Backlog P3

1. `Name` в desktop-файле не проходит string-escaping (`\t`, `\`), только отказ на
   `\n`/`\r`/`\0` — вне объёма ТЗ.
2. Строгий spec для литерального `\` в quoted-аргументе (`\\\\`) — текущий `\\`
   соответствует ТЗ, но не строгому чтению спеки.
3. `AppSettings` принимает trailing `/`; host сравнивается case-insensitive —
   либеральнее RE, не эксплуатируемо.
4. `format_list` без language-fallback: `ru-RU`/`de-DE` → en-US (контракт заморожен).
5. `DeepLink::raw` остаётся публичным полем с токеном — redaction только в `Debug`;
   потребитель обязан не логировать поле.
6. Реальный desktop-лаунчер/D-Bus и live Discord не проверялись; wire-совместимость
   автозапуска/бейджа не заявляется.

## 5. Файлы и md5

| Файл | md5 |
|---|---|
| `discord-i18n/src/list.rs` | `38a3d1d5435ef29a9d061295ab74acb3` |
| `discord-i18n/src/list_data.rs` | `4aad25e6b01d78afb13544132ceaf77f` |
| `discord-i18n/src/lib.rs` | `bf4ee71796fcf6bedc2e20ee27a4b786` |
| `discord-i18n/tests/list_contract.rs` | `7389541dbefca9f4a2137d7aec8c19b4` |
| `discord-platform/src/deep_link.rs` | `56bc6ea26b34e7200d1de99469b69e73` |
| `discord-platform/src/autostart.rs` | `954222c0a6397ca4c8b2a9c7e169b61e` |
| `discord-platform/src/lib.rs` | `ec43f8e86b5ca593dd8c73e3925a9211` |
| `discord-platform/tests/deep_link_contract.rs` | `a86384c4db56e0bc6937c57f27170965` |
| `discord-platform/tests/autostart_contract.rs` | `773c1424f0d76a7742e8d5e64f479aa6` |

## 6. DoD

- [x] red-логи (compile-red новых API + red фиксов TL-2) и green targeted;
- [x] файлы + md5, `TEST-CORRECTIONS.md` (TC-1 `RU-ru`, TC-2 1620→1674);
- [x] мутации пойманы, эквивалентные задокументированы, продукт восстановлен по md5;
- [x] batch gate passed (cargo + npm);
- [x] независимое ревью, P0/P1/P2 = 0 после фиксов;
- [ ] accepted + `db.jsonl` + `STATE.md` + STATUS/ROADMAP/TASKS/map — TL.
