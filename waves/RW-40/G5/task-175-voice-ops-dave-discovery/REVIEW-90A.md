# Техническое ревью 90A: PR-61 (task-175 / BE-P3-05 & BE-P3-06)

- **Ветка:** `feature/task-175-voice-ops-dave-discovery` → `unstable`
- **Пулл-реквест:** `PR-61`
- **Вердикт:** **APPROVED (Ready for Merge)**
- **P0 = 0, P1 = 0**, мутации **5/5 убиты** (7 подмутаций, включая обе ветки для предиката и для раскладки порта)
- **Дата:** 2026-10-31, ревьюер: Technical Reviewer (Review 90A, DeepSeek)

---

## 1. Границы изменений

`git status --porcelain` содержит ровно три файла, все — из разрешённого ТЗ множества:

```
 M rust/crates/discord-voice/src/lib.rs
 M rust/crates/discord-voice/src/udp.rs
 M rust/crates/discord-voice/tests/signaling.rs
```

`git diff --stat unstable...HEAD` пуст (коммитов нет), весь объём — рабочее дерево:
`368 insertions(+), 2 deletions(-)`. Golden-пути (`logs/`, `functions/`, `evidence/`, `fixtures/`) не тронуты,
`git commit` не выполнялся.

MD5 совпадают с таблицей `REPORT.md` §5:

| Файл | MD5 | Совпадает с отчётом |
|---|---|---|
| `src/lib.rs` | `c45a41ef0b569cabfa3596a04659b82e` | да |
| `src/udp.rs` | `1623b890f2af31886cd7be182d243775` | да |
| `tests/signaling.rs` | `70cacf9be751349290a5c2f3da888cbb` | да |

Радиус поражения ограничен: `discord_voice` не импортируется ни одним другим крейтом воркспейса
(`grep -rln "discord_voice" --include=*.rs rust/ | grep -v crates/discord-voice/` — пусто).

---

## 2. Независимая сверка с оракулом

Оракул: `/home/sanya/re-deep/voice/strace-media-s3.jsonl` (182 записи, read-only).

| Что проверено | Как | Результат |
|---|---|---|
| Строка 1 (порт 0) | `python3` сравнил `hex` из файла с литералом в тесте | совпадает побайтно, 74 Б, хвост `00 00` |
| Строка 2 (порт 33801) | то же | совпадает побайтно, 74 Б, хвост `84 09` |
| SSRC в запросах | декод `hex[8:16]` | `0x000023f8` = `9208`, как в тесте |
| Отличие строк 1 и 2 | сравнение префиксов | различаются только последними 2 байтами |

То есть hex-оракул в тесте не выведен из реализации, а списан с реального дампа — это настоящая проверка
контракта, а не тавтология. `export/parse` замкнут: `DiscoveryPacket::parse` для `kind == 1` требует нулей
в `8..72` и читает порт BE из `72..74` — реализация этому соответствует.

`send_discovery` переведён на `send_discovery_with_local_port`, но байты не изменились: старый путь
`DiscoveryPacket::Request { ssrc, port }.encode()` писал тот же порт BE в `72..74` (см. `udp.rs:167-235`),
поэтому обратная совместимость сохраняется. Это подтверждает существующий тест
`tests/udp.rs::discovery_matches_strace_request_and_response_oracles` — зелёный.

---

## 3. Прогоны (общий target-dir)

```bash
CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo test --manifest-path rust/Cargo.toml -p discord-voice
CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo clippy --manifest-path rust/Cargo.toml -p discord-voice --tests -- -D warnings
cargo fmt --manifest-path rust/Cargo.toml -p discord-voice -- --check
```

| Проверка | Результат |
|---|---|
| `cargo test -p discord-voice` | **172 passed, 0 failed, 0 ignored** (14 unittests + 16 crypto_vectors + 5 devices + 16 media + 10 reception + 10 reconnect + 10 regions + 16 rtcp + 18 signaling + 22 srtp + 20 stats + 15 udp; doc-tests 0) |
| `cargo clippy --tests -D warnings` | 0 предупреждений (после принудительной пересборки `touch` + `Checking discord-voice`) |
| `cargo fmt --check` | чисто, exit 0 |

Красный прогон (`red.log`) — подлинный: падение на компиляции
`E0599: no variant ... named Speaking/Resume/Resumed/ClientsConnect`, `no method named name/is_dave_or_mls`,
`no variant ... encode_request_with_port`, `no method ... send_discovery_with_local_port`. Это red-фаза TDD,
а не подгонка тестов под код: тесты написаны до кода и опираются на внешний артефакт.

---

## 4. Мутации: 5/5 убиты

Каждая мутация применялась к рабочему дереву, прогонялся `cargo test -p discord-voice --test signaling`,
затем файл восстанавливался из копии (MD5 после отката совпал с исходным).

| # | Мутация | Файл | Убито тестом | Итог |
|---|---|---|---|---|
| 1 | `Speaking` → опкод 4 вместо 5 (`as_u8`) | `lib.rs` | `test_voice_op_extended_variants_roundtrip_0_to_32_and_unknown` | KILLED |
| 2a | `is_dave_or_mls`: `21..=32` (захватывает `NoRoute`) | `lib.rs` | `test_voice_op_is_dave_or_mls_predicate` | KILLED |
| 2b | `is_dave_or_mls`: `22..=31` (теряет 21) | `lib.rs` | `test_voice_op_is_dave_or_mls_predicate` | KILLED |
| 3 | `name()` для `Speaking` → `"Speaking"` вместо `"SPEAKING"` | `lib.rs` | `test_voice_op_extended_variants_roundtrip_0_to_32_and_unknown` | KILLED |
| 4a | порт в `encode_request_with_port` через `to_le_bytes` | `udp.rs` | `test_discovery_request_trailing_2_bytes_port_layout` + loopback-тест (2 падения) | KILLED |
| 4b | порт в `70..72` вместо `72..74` | `udp.rs` | те же два теста (2 падения) | KILLED |
| 5 | `Unknown(value)` → `Unknown(0)` (потеря сырого байта) | `lib.rs` | `test_voice_op_extended_variants_roundtrip_0_to_32_and_unknown` (3 падения) | KILLED |

Ни одна мутация не выжила; ни один тест при этом не удалён и не ослаблен.

---

## 5. Замечания

### P0 — нет

### P1 — нет

### P2

**P2-1. `encode_request_with_port` принимает адрес, который собственный парсер отвергает.**
`udp.rs:207-231` при непустом `address` пишет ASCII IP в `bytes[8..72]` для пакета `type = 1`,
а `DiscoveryPacket::parse` (`udp.rs:125-130`) для запроса требует, чтобы `8..72` были нулями.
Получается пакет, который наш же парсер считает невалидным, и который оракул не подтверждает:
в обеих записях strace адресное поле нулевое. Сейчас это не стреляет — единственный вызывающий
`send_discovery_with_local_port` передаёт `""`, — но публичный API (и свободная функция
`udp::encode_request_with_port`) приглашает передать адрес и получить мусор в эфир.
Что сделать: убрать параметр `address` из request-пути (порт + SSRC достаточно) либо возвращать
`UdpError::InvalidAddress` при непустом адресе для `type = 1`; добавить негативный тест
`encode(with address) -> Err` и тест «encode→parse замкнут».

**P2-2. Цифры в `REPORT.md` §3 не соответствуют прогону.**
Отчёт перечисляет `reconnect_contract` 6 (фактически 10), `regions_contract` 16 (фактически 10),
не упоминает `rtcp_contract` (16), итог 158 и «154 существовавших» теста. Реальный прогон:
**172 passed**, из них 168 существовавших + 4 новых. Код зелёный, дефект только в артефакте-доказательстве;
таблица в §3 настоящего ревью — источник истины. Правку цифр нужно внести до записи в `waves/db.jsonl`,
иначе журнал волны зафиксирует неверный базис.

### P3

- **P3-1.** Дублирование кодирования discovery-запроса: `udp.rs::encode_request{,_with_port}` (возвращает
  `Result`) и `session.rs:492-505` `encode_discovery_request{,_with_port}` (непогрешимая, при ошибке молча
  отдаёт нули). Раскладка одна и та же, но два пути разъедутся при первой правке. Кандидат на унификацию
  в отдельной карточке.
- **P3-2.** `impl fmt::Display for VoiceOp` печатает `UNKNOWN` без номера, поэтому в логах теряется
  значение неизвестного опкода. Полезнее `UNKNOWN(33)`; `Debug` это умеет, но `Display` используется в
  сообщениях.
- **P3-3.** Таблица «номер → имя» для расширенных опкодов **не подтверждена локальным дампом**:
  в `/home/sanya/re-deep/voice-deep.json` `signaling.ops` есть только наблюдённые 0/1/2/3/4/6/8/15/16,
  `NO_ROUTE` в бандле — это `VOICE_CONNECTION_STATE`, а не опкод, строк `MLS_KEY_PACKAGE` /
  `SECURE_FRAMES_*_TRANSITION` в `decoded/` нет. Числа и имена взяты из документированного RTC-протокола
  Discord, что для P3-перечисления приемлемо, но перед использованием `name()` в UI/логах нужен живой
  трейс опкодов 21..31. Наблюдённые опкоды совпадают с дампом: 15 → `MEDIA_SINK_WANTS`, 16 →
  `VOICE_BACKEND_VERSION`.
- **P3-4.** `test_udp_media_socket_send_discovery_with_local_port` делает блокирующий `recv_from` без
  таймаута: при потере датаграммы тест зависнет вместо падения. На loopback риск минимальный, но
  `set_read_timeout` стоит добавить.

### Вне скоупа PR-61 (для отдельной карточки)

`rust/crates/discord-voice/tests/session_contract.rs` не компилируется ни на этой ветке, ни на `unstable`:
он ссылается на `SessionEvent` / `SessionEffect`, которых в `src/` нет вообще (`git show unstable:...` —
0 вхождений в `session.rs`, 98 в тесте, 431 ошибка компиляции под `--features v01_session`).
Файл спрятан за необязательной фичей `v01_session`, поэтому обычный прогон его не задевает, но
`scripts/ci.sh:63` использует `--all-features --all-targets` — там он красный и до PR-61. К этому PR
отношения не имеет, блокировать его нельзя; нужен отдельный `bugfix` на восстановление или удаление
устаревшего файла.

---

## 6. Что проверено сверх диффа

1. `parse_frame` (`lib.rs:642-662`) заканчивается веткой `other => VoicePayload::Other` — новые варианты
   не ломают разбор и меняют только типизацию `frame.op`; поведение payload для 5/7/9/11..13/15/16/21..32
   осталось прежним.
2. `OBSERVED_RTC_OPS` и его тест на длину 9 не тронуты.
3. `Unknown(u8)` сохранён последним вариантом, `from_u8`/`as_u8` взаимообратны на 0..=255 (тест гоняет
   весь диапазон для предиката и 0..=32 + 6 значений для roundtrip).
4. Red-first соблюдён: `signaling.rs` падал до появления кода, `red.log` содержит компиляционные ошибки
   отсутствующих API, а не подогнанные ассерты.
5. `send_discovery` не изменил байты на проводе — проверено по коду старого `encode()` и зелёным
   `tests/udp.rs`.

---

## 7. Решение

**APPROVED (Ready for Merge).** Блокирующих замечаний нет: P0 = 0, P1 = 0, мутации 5/5 убиты,
172/172 тестов зелёные, clippy и fmt чистые, границы файлов соблюдены.

До записи в журнал волны: исправить цифры в `REPORT.md` (P2-2). P2-1, P3-1..P3-4 и сломанный
`session_contract.rs` — отдельными карточками, слияние не блокируют.

`git commit` не выполнялся: слияние и коммиты остаются за координатором/интегратором.
