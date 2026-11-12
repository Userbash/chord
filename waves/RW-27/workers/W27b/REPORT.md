# Отчёт воркера — RW-27 / W27b (P6-04 Deep links + P6-03 Автозапуск/бейдж)

## 1. Границы

- Продукт: `rust/crates/discord-platform/src/deep_link.rs` (новый),
  `rust/crates/discord-platform/src/autostart.rs` (новый),
  `rust/crates/discord-platform/src/lib.rs` (только `pub mod autostart/deep_link`).
- Тесты: `rust/crates/discord-platform/tests/deep_link_contract.rs` (12),
  `rust/crates/discord-platform/tests/autostart_contract.rs` (8).
- Не менялись: `Cargo.toml`/`Cargo.lock`, другие крейты/тесты, доки
  (`STATE/STATUS/ROADMAP/TASKS/map`), golden (`logs/`, `functions/`, `evidence/`,
  `reference-oracle/` — только чтение), сеть, D-Bus, live Discord, `unsafe`, коммиты.
- `lib.rs` диффом — только две строки `pub mod` (+пустая строка).

## 2. Сделано

**P6-04 deep links.** `parse_deep_link` режет `discord://host/path?query#fragment`
(пустой host допустим, `discord:///library/42/launch`), проверяет пустоту, лимит
`MAX_DEEP_LINK_LEN = 4096`, control-символы `< 0x20`/`0x7F`, схему ASCII-регистронезависимо,
`//`, пробелы в scheme/host; пустые `?`/`#` → `None`. `extract_deep_link` повторяет
RE `extractUrlFromArgs` (`docs/re/main-arch.md` §9.4): `--url` только при `--` с индексом
не меньше, иначе первый argv с индекса 1 с префиксом `discord://`. `route` — только
RE-подтверждённые формы: `login/one-time` (+`token`), `action/oauth2/authorize`, `app/open`,
`app/settings/<section>`, `discord:///library/<id>/launch`, `channels/...` (включая
`discord://-/channels/...` и `discord://channels/@me` → канонический `/channels/@me`).
`Route::LoginOneTime` имеет ручной `Debug` с `"<redacted>"`.

**P6-03 autostart/бейдж.** `desktop_entry` собирает XDG-файл (`[Desktop Entry]`,
`Type=Application`, `Name`, `Exec`, `X-GNOME-Autostart-enabled`, `Terminal=false`,
завершающий `\n`), экранирует Exec по спеке (пробел/таб/`"`/`\`/`$`/`` ` ``/`%` → кавычки;
`"`→`\"`, `\`→`\\`, `$`→`\$`, `` ` ``→`` \` ``, `%`→`%%`, пустой аргумент → `""`).
`hidden=true` выключает автозапуск **одним** механизмом `X-GNOME-Autostart-enabled=false`
(зафиксировано тестом a02; `Hidden=true` не дублируется). Отказ на пустые name/exec и
`\n`/`\r`/`\0` в любом поле. `autostart_path` = `<config_home>/autostart/discord-lab.desktop`
(без I/O). `badge_pairs` — стабильный порядок `count`, `count-visible`, `urgent`;
`app_uri("x.desktop")` = `application://x.desktop`. Wire/D-Bus не реализованы.

Оракулы: RE-цитата §9.4 и строки бандлов (`login/one-time?token=…`,
`action/oauth2/authorize`, `app/open#…`, `app/settings/nitro`, `/library/<id>/launch`),
XDG Desktop Entry 1.5 (точные строки файла и правила quoting), Unity Launcher API (свойства).

## 3. Red (до реализации)

```sh
cd rust && CARGO_TARGET_DIR=/tmp/opencode/target-w27b \
  cargo test --offline -p discord-platform --test deep_link_contract
# EXIT=101 — compile-red: E0432 unresolved import `discord_platform::deep_link`
# лог: waves/RW-27/workers/W27b/red-deep.log

cd rust && CARGO_TARGET_DIR=/tmp/opencode/target-w27b \
  cargo test --offline -p discord-platform --test autostart_contract
# EXIT=101 — compile-red: E0432 unresolved import `discord_platform::autostart`
# (+ каскад E0277/E0599 от неизвестного типа); лог: .../red-autostart.log
```

## 4. Green (targeted, после реализации)

```sh
cd rust && CARGO_TARGET_DIR=/tmp/opencode/target-w27b \
  cargo test --offline -p discord-platform \
  --test deep_link_contract --test autostart_contract
# EXIT=0: autostart 8 passed / 0 failed; deep_link 12 passed / 0 failed
```

Лог: `waves/RW-27/workers/W27b/green-targeted.log` (`EXIT=0`).

Дополнительно (не вместо targeted): `cargo test --offline -p discord-platform` — 21 passed
(1 lib + 8 + 12); targeted `clippy -p discord-platform --all-targets -- -D warnings` — exit 0;
`cargo fmt -p discord-platform -- --check` — чисто.

## 5. Мутации

`waves/RW-27/workers/W27b/mutations.md`: **14 caught, 0 missed, 0 inconclusive**
(deep links 8: D1–D8; autostart 6: A1–A6). Каждая мутация — точная замена строки,
targeted-прогон, затем восстановление из бэкапа + `touch` (mtime), чтобы cargo не
переиспользовал бинарь мутанта; после каждого восстановления md5 сверен с базой.
Финальные md5 продукта совпадают с базой мутационного прогона.

## 6. Файлы и md5

| Файл | md5 |
|---|---|
| `rust/crates/discord-platform/src/deep_link.rs` | `33755f7090389613e61b0d7d53ecde0d` |
| `rust/crates/discord-platform/src/autostart.rs` | `5e6935bdf71ffa0f0be9a1ebe5428c38` |
| `rust/crates/discord-platform/src/lib.rs` | `ec43f8e86b5ca593dd8c73e3925a9211` |
| `rust/crates/discord-platform/tests/deep_link_contract.rs` | `9ccba7e89692a44e40d8eec83556ef97` |
| `rust/crates/discord-platform/tests/autostart_contract.rs` | `343c6cf41eda708e238043e3f86ef849` |
| `waves/RW-27/workers/W27b/red-deep.log` | `b876f9112f3a129d85f28e942025028c` |
| `waves/RW-27/workers/W27b/red-autostart.log` | `a24321d1ca6e92d551ee00ce50cbeaf3` |
| `waves/RW-27/workers/W27b/green-targeted.log` | `ba0918b19e8cbc7e6f4659a1408bdd10` |
| `waves/RW-27/workers/W27b/mutations.md` | `4a065920fc70454312799489880cd5fd` |

## 7. Риски/вопросы

- `DeepLink` сохраняет контрактный `derive(Debug)`, поэтому его `raw`/`query` могут
  содержать `token`; в модуле явно задокументировано: логировать только `Route`
  (его `Debug` redacted). Это не меняет замороженный контракт.
- `token` в `Route::LoginOneTime` — сырой срез query без percent-декодирования
  (задокументировано и зафиксировано тестом d12). Потребитель декодирует сам.
- `Name` в desktop-файле выводится verbatim после проверки `\n`/`\r`/`\0` (ТЗ не требует
  string-экранирования Name); backslash-последовательности в Name не переэкранируются.
  Exec экранируется полностью.
- Control-символы в autostart, кроме `\n`/`\r`/`\0` (например ESC), не отклоняются —
  ровно объём ТЗ; таб разрешён и quoting'уется по Exec-спеке.
- `channels` с host `channels` канонизируется в `/channels/...`; `library` принимается
  только с пустым host (RE-форма `discord:///library/<id>/launch`), `discord://-/library/...`
  → `Unknown` (не подтверждено RE, зафиксировано в d10).
- Хост в маршрутах сравнивается регистронезависимо (RFC), путь — регистрозависимо.

## 8. Передача

- next: TL-2 — integration-дифф, batch gate, независимое ревью.
- Полный workspace/gate не запускал (по ТЗ — batch-лейн TL-2); коммитов не делал.

## 9. Корректировка TL-2 (2026-10-22)

- `autostart.rs`: `needs_quotes` расширен до reserved-набора Desktop Entry 1.5
  (`' > < ~ | & ; * ? # ( )`), тест `a09`; red/green — `waves/RW-27/workers/TL2/`.
- `deep_link.rs`: ручной `Debug` для `DeepLink` скрывает `raw` и печатает
  `query`/`fragment` как `<redacted>`; тест `d13`.
- Итоговые md5 — в `waves/RW-27/REPORT.md`; platform-мутации 14/14 на финальном коде.
