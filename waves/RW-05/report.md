# Отчёт RW-05 (P1-04) — IDENTIFY (op2) и сводка READY

**Статус:** волна завершена координатором вместо воркера.
Воркер (субагент) был прерван рейт-лимитом после `green-core`, не успев закончить
воркспейс-прогон, clippy/fmt, гейт и отчёт. Координатор проверил состояние, нашёл и исправил
дефект контракта (см. «Инцидент ТЗ»), догнал волну до конца по hard TDD. Независимое ревью —
запрошено отдельным субагентом (см. конец отчёта).

## сделано

- `rust/crates/discord-core/src/identify.rs` (315 строк, новый модуль):
  - `build_identify(&IdentifyParams)` — **payload** IDENTIFY (ровно поля `_doIdentify` из бандла
    `discord.com_assets_web.4d4b2f75fdea5470.js`, ~5017678): `token`, `capabilities`,
    `properties`, `presence` (None → `Nil`), `compress`, `client_state {guild_versions}`,
    `qos_token`; строки/ключи — `Str` (wetf `encoding:{string:"binary",key:"binary"}`), массивы —
    `List`, булевы — атомы `"true"/"false"`. Самопроверка сборки: байты обязаны целиком
    разбираться кодеком, иначе `IdentifyError::Encode` (без паник).
  - `build_identify_frame` — **проводная** форма `{op: 2, d: payload}` с ведущим `0x83`; оракул —
    `send = (e,t,n) => pack({op:e, d:t})` из клиента (~5024100).
  - `IdentifyProperties::for_rw_client()` — супер-свойства + поля `handleIdentify` (~4907630):
    16 ключей, `is_fast_connect:false`, `gateway_connect_reasons:"COLD_START"` **строкой**
    (модуль 469177 `L7()`: `reasons.sort().join(",")`, пусто → `"NO_REASONS"`), `installation_id`
    появляется только когда задан.
  - `qos_token_client_provided(bool)` — минимальные protobuf-байты `QosToken{client_provided{
    is_active}}`: `0A 02 08 00` / `0A 02 08 01`; не-UTF8 байты → `QosTokenNotUtf8` без паники.
  - `CAPABILITIES_ACTIVE = 1_734_653`, `CAPABILITIES_CHANNEL_OBFUSCATION = 1_767_421`.
  - `impl Debug for IdentifyParams` — токен всегда `"REDACTED"`, QoS-байты только длиной.
- `rust/crates/discord-core/src/ready.rs` (123 строки, новый модуль): `ReadySummary` (17 полей:
  `top_keys/guilds/private_channels/users/relationships/read_state_*/user_guild_settings_entries/
  user_keys/user_id/session_id/resume_gateway_url/sessions/experiments/merged_members`) и
  `from_dispatch`: `op==0 && t=="READY" && d.guilds` — иначе `None`; устойчиво к отсутствующим
  полям (счётчики 0), `partial` — атом или int, `user.id` — big-int или строка.
- `rust/crates/discord-core/src/ws.rs` (точечно, +46 строк): `RunOptions {identify}`,
  `connect_and_run_with(url, pipe, opts)` (без опций — прежний `connect_and_run`),
  `maybe_send_identify` — IDENTIFY **ровно один раз** после первого HELLO (в т.ч. если HELLO
  разобран до входа в цикл), ошибка сборки → `TransportError::Identify`; `GatewayPipe::last_ready()`
  (сводка READY после `Session::on_value`, READY — не терминальная ошибка).
- `rust/crates/discord-core/src/lib.rs` — `pub mod identify; pub mod ready;`, ре-экспорты
  (`IdentifyParams`, `build_identify`, `build_identify_frame`, `ReadySummary`, `RunOptions`,
  `connect_and_run_with`), статус модуля обновлён.
- `rust/crates/discord-core/tests/identify_ready.rs` (1107 строк, новый; 13 кейсов ТЗ + 3 кейса
  закрытия дыр мутационного само-ревью ниже):
  8 юнит IDENTIFY (набор ключей и значения, capabilities-фича, свойства/installation_id,
  presence, guild_versions, QoS, redaction Debug, размер 500..=1200 Б против реальных 943 Б),
  2 юнит READY (синтетический диспатч — полное равенство сводки; не-READY формы), 1 lab
  (137 кадров `gw-proto/zstd-frames-95974_1389.hexl` → READY s=1: 40/7/17/146/60/157/false/
  128 949/58, user 32 ключа, id, session_id, resume_url; skip-aware), 2 стаб-сокета
  (`ws_identify_sent_once_after_hello` — op2 ровно один раз + READY + RTT; `ws_no_identify_without_option`
  — без опции op2 нет, RW-04-путь не сломан).

## Инцидент ТЗ (и его исправление)

- В ТЗ RW-05 поля IDENTIFY были перечислены без внешнего конверта, и тест 12 сначала ожидал
  `token/capabilities/...` прямо в кадре. По оракулу (код клиента, `pack({op, d})`) проводная
  форма — `{op: 2, d: {…7 полей…}}`. Воркер собрал `build_identify` как payload (что верно),
  но `ws.rs` отправлял payload **без конверта** → тест 12 падал (`op` = None).
- Исправление: тест 12 приведён к оракульной форме (payload под `d`) — red-прогон
  `red-contract-fix.txt` (`# EXIT=101`, md5 тест-файла `52479f7723e7a5c7f28eec2c96e893a3`);
  затем `ws.rs` переключён на `build_identify_frame` (минимальная правка) → green.
  В документации `build_identify` явно помечено «payload; проводная форма —
  `build_identify_frame`».
- Диагностика стаба усилена: при неожиданном op в ошибку печатается весь терм (оставлено).

## red

- Прогон воркера (исходный, до инцидента): `red.txt` — `# EXIT=101`,
  **1 passed; 12 failed** (md5 тест-файла `ca0ba9c2a3e542feb32c3bd354d626c8`). Пройденный —
  `ws_no_identify_without_option`: негативный контроль старого пути RW-04, новых сущностей не
  требует (честная фиксация, не «подгонка»).
  Артефакт: `/tmp/opencode/rw-05/red.txt`, md5 `269ec27fd8a04c907612394c4b4ed7f2`.
- Прогон исправления контракта: `red-contract-fix.txt` — `# EXIT=101`, тест 12 падает
  («payload не под d»), md5 тест-файла `52479f7723e7a5c7f28eec2c96e893a3`.
  Артефакт: md5 `8b743ba46a29483db47e0e20675d0081`.

## green

- `cd rust && cargo test --offline -p discord-core` → **43 passed, 0 failed**
  (2 lib + 15 gateway_session + 13 identify_ready + 13 ws_transport), `# EXIT=0`.
  Артефакт: `/tmp/opencode/rw-05/green-core-3.txt` (после правки doc-комментария), md5
  `a472cf5da8a3e6973c9787b375bcc946`; промежуточные: `green-core-2.txt` md5
  `42a0f5b40b044842f8f6463b2b605925`, `green-final-dc.txt` (fmt) md5
  `985814006539118ba847799eb581fee9`.
- `cargo test --offline --workspace` → **85 passed, 0 failed**, `# EXIT=0`.
  Артефакт: `/tmp/opencode/rw-05/green-workspace-final.txt`, md5
  `103e473667476dec1a1912ae9f2160ac` (до fmt: `green-workspace.txt` md5
  `a9cf7d6d76a4e36b800151da7fa838a5`).
- `cargo clippy --offline --workspace --all-targets -- -D warnings` → `# EXIT=0`;
  `clippy.txt`, md5 `82d8e4b3d990dcf73ce1d16731c3820b`.
- `cargo fmt --all` (применён к тест-файлу воркера), затем `--check` → `# FMT_EXIT=0`.
- `npm run gate:worker` → **passed** (typecheck/meta 22/unit 169/smoke 20);
  `.gate/worker-2026-10-19T13-58-37-508Z.json`; лог `gate.txt`, md5
  `c8aaf2212f435012969868f6cbaeabc4`.

## Контрольные числа lab-кейса (RW-05)

READY (s=1) из `zstd-frames-95974_1389.hexl` через пайп: `top_keys=40`, `guilds=7`,
`private_channels=17`, `users=146`, `relationships=60`, `read_state{entries=157, partial=false,
version=128 949}`, `user_guild_settings{entries=58}`, `user_keys=32`, `user.id=384299970632876032`,
`session_id` 32 символа, `resume_gateway_url=wss://gateway-us-east1-d.discord.gg` — совпадает с
`gw-proto/msgs-run2-95974_1389.jsonl`.

## Self-review (мутационное; независимое ревью ждёт снятия рейт-лимита субагентов)

Независимый ревьюер-субагент запускался 1 раз — рейт-лимит (`Rate limit exceeded`). Пока лимит
не снят, выполнено мутационное само-ревью по тому же протоколу, что и RW-04 (мутации в
реализацию, прогон `cargo test --offline -p discord-core`, восстановление по бэкапу, md5 до/после
совпали: `17f2f310…/c7c98489…/659ee47e…`):

| # | мутация | результат |
|---|---|---|
| M1 | `ws`: слать payload без конверта `{op,d}` | пойман (`ws_identify_sent_once_after_hello`) |
| M2 | `capabilities − 1` | пойман (кейсы 1, 12) |
| M3 | `compress` всегда `true` | пойман (кейс 1) |
| M4 | `gateway_connect_reasons` списком | пойман (кейс 3) |
| M5 | `installation_id` не добавляется | пойман (кейс 3) |
| M6 | `user_id` только Int | **выжил → дыра** |
| M7 | `partial` int-1 не читается | **выжил → дыра** |
| M8 | `top_keys + 1` | пойман (кейсы 9, 11-lab) |
| M9 | IDENTIFY на каждый HELLO (снят флаг) | пойман (кейс 12) |
| M10 | IDENTIFY до HELLO | **выжил → дыра** |
| M11 | `last_ready` не запоминается | пойман (кейсы 12, 11-lab) |

Дыры закрыты red-first при живых мутантах (M6+M7+M10): новые тесты `ready_accepts_string_user_id`,
`ready_accepts_int_partial`, `ws_identify_only_after_hello` (тишина до HELLO, 300 мс) падали при
мутантах — `mut/red-gap-tests.txt` (`# EXIT=101`, 13 passed/3 failed, md5 тест-файла
`ea4c7f18e0d8d0f6472fcb9b025517e7`); после восстановления реализации — зелёные.
Логи мутаций: `/tmp/opencode/rw-05/mut/M1..M11.txt`, таблица `mut/results.txt`.
Нит N1 независимого ревью: первая автоматическая сводка `results.txt` врала (ловила строку cargo
`error: test failed` как «COMPILE-FAIL», а неисполненный M4 — как SURVIVED); сводка **пересчитана**
по признаку `test result: FAILED` — теперь сходится с таблицей выше (M6/M7/M10 — выжили, M4 — пойман).

Повторный green: identify_ready **16** тестов, discord-core **46**, воркспейс **88 passed**, 0 failed,
`# EXIT=0` (`green-workspace-final2.txt`); clippy `-D warnings` чисто; `npm run gate:worker` —
**passed** (`.gate/worker-2026-10-19T14-12-35-509Z.json`, `gate-2.txt`).

## файлы (md5 после закрытия дыр)

| файл | md5 |
|---|---|
| `rust/crates/discord-core/src/identify.rs` | `17f2f3102c3c1b9fcedcb3cc9e1f3cc6` |
| `rust/crates/discord-core/src/ready.rs` | `c7c984890895eaca1214ac3705b34019` |
| `rust/crates/discord-core/src/ws.rs` | `659ee47e55c4146b0139c969672cb230` |
| `rust/crates/discord-core/src/lib.rs` | `8986fa107cbc7e6538cf4cca8a975833` |
| `rust/crates/discord-core/tests/identify_ready.rs` | `ea4c7f18e0d8d0f6472fcb9b025517e7` |

`Cargo.toml`/`Cargo.lock` не менялись (новые зависимости не нужны).

Нит N3 независимого ревью: `ws.rs` и `lib.rs` здесь — **финальные значения волны RW-05**; волна
RW-06 легитимно их изменила (добавлен RESUME), её финальные значения: `ws.rs`
`b822d6a6790f11a306a442c6a58c975c`, `lib.rs` `69e299f265a0459f4e104c437e1bf2a5`
(см. `/tmp/opencode/rw-06/report.md`). `identify.rs`/`ready.rs`/`identify_ready.rs` RW-06 не менял.

## Вердикт независимого ревью (2026-10-19)

Отчёт: `/tmp/opencode/review-rw-0506.md` — **RW-05 «принято с нитами»**: red-first честный
(лог + независимая репродукция заглушками: 1 passed/15 failed на 16 тестах), md5 сошлись, lab-числа
перевыведены, мутации 10/10 поймано. Ниты N1 (сводка `results.txt` — исправлена), N2 (строки
`identify.rs` 313→315, `identify_ready.rs` 950→1107 — исправлено), N3 (промежуточные md5 — пояснено
выше) — закрыты координатором в тот же день.

## риски

- ТЗ RW-05 содержало ошибку формы кадра (см. «Инцидент») — исправлено по оракулу; впредь для
  проводных форм сверять конверт `pack({op,d})` до фиксации тестов.
- `ws_identify_sent_once_after_hello` зависит от таймингов стаба (60 мс, 2 удара в 5 с) — запас
  большой; прогон стабилен (в этой волне ≥ 6 прогонов).
- READY от реального сервера может иметь отсутствующие поля — сводка это допускает (0/None), но
  тогда часть чисел не сойдётся; для наблюдения это ожидаемо.
- Независимое ревью волны выполняется отдельно; вердикт — в `TASKS.md` после приёмки.
- Наследие RW-04 (в backlog): пересмотр zstd-окна P1-02 (lab s=113) — к RW-05 отношения не имеет.
