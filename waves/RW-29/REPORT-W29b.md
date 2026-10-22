# Отчёт воркера — RW-29 / W29b (P4-04 RTP-медиа, headless)

## 1. Границы

- Продукт: `rust/crates/discord-voice/src/media/mod.rs` (новый),
  `rust/crates/discord-voice/src/media/rtp.rs` (новый),
  `rust/crates/discord-voice/src/media/jitter.rs` (новый),
  `rust/crates/discord-voice/src/lib.rs` — только строка `pub mod media;` после `pub mod crypto;`.
- Тесты: `rust/crates/discord-voice/tests/media_contract.rs` (13 тестов, m01–m13).
- Артефакты волны: `waves/RW-29/workers/W29b/{red.log,green.log}`,
  `waves/RW-29/run-mutations-w29b.py`, этот отчёт.
- Не менялись: `Cargo.toml`/`Cargo.lock`, другие крейты/модули/тесты, доки
  (`STATE/STATUS/ROADMAP/TASKS/map`), golden (`fixtures/voice/rtp-pt120-oracle.json`,
  `waves/RW-29/gen-rtp-oracle.py`, `logs/`, `functions/`, `evidence/` — только чтение),
  сеть, live Discord, `unsafe`, `git commit/push`.
- `cargo` запускался последовательно с `CARGO_TARGET_DIR=/tmp/opencode/target-w29b`;
  targeted-тесты только своего файла, workspace/clippy/fmt-гейт и npm не запускались
  (integration-лейн).

## 2. Файлы и md5

| Файл | md5 |
|---|---|
| `rust/crates/discord-voice/src/media/mod.rs` | `c62ac72aae09a2b1ea5bb8d8cdcdeb54` |
| `rust/crates/discord-voice/src/media/rtp.rs` | `c33f185bd1a3779757f528041f049eab` |
| `rust/crates/discord-voice/src/media/jitter.rs` | `51d4bc9e3c4f309c62c6cb7a69bd89af` |
| `rust/crates/discord-voice/src/lib.rs` | `200f9028c74c38f5e87cda4cb9c5c638` |
| `rust/crates/discord-voice/tests/media_contract.rs` | `9651677bab3bc785b94ddb952626697a` |
| `waves/RW-29/run-mutations-w29b.py` | `b168854eb67fd69b00b8b6f8908a1b43` |
| `waves/RW-29/workers/W29b/red.log` | `e271e453161d54314973c6bca4e3422c` |
| `waves/RW-29/workers/W29b/green.log` | `f2476d715c7bcef38c065a75504e5853` |

`lib.rs` — общий файл волны: md5 зафиксирован на момент отчёта; W29c добавляет в него
только `pub mod reconnect;` (строка `pub mod media;` остаётся без изменений).

Фикстура и генератор не изменялись: `rtp-pt120-oracle.json` =
`798dd8aea8dd14d4429ac00b1c1d0a12`, `gen-rtp-oracle.py` =
`281972076237ba6a5cec94de6f0901d3` (как в базе волны).

## 3. Сделано

**`media/rtp.rs`.** Константы `OPUS_PAYLOAD_TYPE=120`, `OPUS_CLOCK_RATE=48000`,
`FRAME_DURATION_MS=20`, `FRAME_TICKS=960`, `EXTENSION_PROFILE=0xBEDE`.
`RtpHeader::encode(ext)` собирает 12 Б (`0x80`/`0x90`, `marker<<7|PT`, seq/ts/ssrc BE)
и при `ext` — `0xBEDE`, `words=len/4` BE и данные; `len % 4 != 0` или `len > 60` →
`ExtensionNotAligned`. `parse_rtp` проверяет версию/длину/CSRC, читает расширение
(стандартный профиль — данные `words*4`, нестандартный — `standard=false`), снимает
паддинг (`0` или `> payload` → `PaddingOutOfRange`). `RtpError` — 5 вариантов с
`as_str`/`Display`/`Error`. `RtpSender::{new,sequence,timestamp,next_packet,silence}`:
marker один раз на talkspurt, `seq+1`, `ts+960`; `silence(n)` двигает только timestamp
на `960*n` и взводит marker. `classify_opus_payload`: `0`→Empty, `1..=3`→ComfortNoise,
иначе Speech.

**`media/jitter.rs`.** `JitterBuffer` с окном по модулю 2^16 (`wrapping_sub < 32768`),
`PushOutcome::{Queued,Duplicate,Late,Overflow}`, `PopOutcome::{Frame,Missing,Empty}`,
`JitterStats{received,duplicates,late,missing,overflow}`. `capacity == 0` → 1. Выдача
строго по ожидаемому seq; пропуск — ровно один `Missing` + `missing += 1`; пустой
буфер — `Empty`. История последних 512 выданных seq — для детекта дублей «уже отдан».
Паник/`unwrap`/`unsafe` в продакшн-пути нет (защитная ветка `pop` без паники).

**`lib.rs`.** Одна строка `pub mod media;` сразу после `pub mod crypto;`, без
переформатирования (файл остаётся rustfmt-clean).

Оракул: `fixtures/voice/rtp-pt120-oracle.json` (8 валидных пакетов `0xBEDE`/2 слова/
23 Б остатка/счётчики `0x80000000|n`, 2 аномальных по 43 Б); тесты читают его через
`include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../fixtures/...")))` — путь
резолвится от `rust/crates/discord-voice` до корня репозитория.

## 4. Трактовки контракта (зафиксированы в тестах и доккомментариях)

1. **m02: нестандартный профиль с `words*4 > остатка`.** §1 требует
   `TruncatedExtension`, §3/m02 — «`standard=false`, без ошибки». У аномалий capture
   профили `0x19f5`/`0x2d6d`, а words (59783/28435) заведомо не длина при 43-байтовом
   пакете. Принято: для `0xBEDE` расхождение — `TruncatedExtension` (m06), для
   нестандартного профиля хвост отдаётся как opaque-данные расширения, payload пуст
   (совпадает с `data_hex`/`remainder_hex` оракула).
2. **m11: реордер 3→1→2.** «Первый push задаёт ожидаемый seq» + «выдача 1,2,3»
   совместны только если до первой выдачи окно может расширяться назад: пакет позади
   старта ещё не отдан и не пропущен, поэтому не `Late`. После первой выдачи граница
   окна = фронт доставки и `Late` работает по §1.
3. **m12: дубль vs опоздавший.** По §1 буквально: `Duplicate` — seq уже в буфере
   **или уже отдан**; `Late` — seq позади фронта, который был пропущен и никогда не
   отдавался.
4. **Ошибка `next_packet`** (некратное расширение) не сдвигает `sequence`/`timestamp`/
   `marker_pending`.
5. **Приоритет исходов `push`:** Duplicate → Late → Overflow → Queued; на Overflow
   состояние окна не меняется.

## 5. Red (до реализации)

```sh
cd rust && CARGO_TARGET_DIR=/tmp/opencode/target-w29b \
  cargo test --offline -p discord-voice --test media_contract
# EXIT=101
# error[E0432]: unresolved import `discord_voice::media`
#   --> crates/discord-voice/tests/media_contract.rs:18:20
# лог: waves/RW-29/workers/W29b/red.log (последняя строка: # EXIT=101)
```

## 6. Green

```sh
cd rust && CARGO_TARGET_DIR=/tmp/opencode/target-w29b \
  cargo test --offline -p discord-voice --test media_contract
# EXIT=0
# test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
# m01..m13 — все ok
# лог: waves/RW-29/workers/W29b/green.log (последняя строка: # EXIT=0)
```

## 7. Мутации

`python3 waves/RW-29/run-mutations-w29b.py` — 9 точных строковых мутаций продукта,
каждая: mutate → targeted-тесты → restore из in-memory копии → md5-сверка (+`touch`,
чтобы cargo не переиспользовал артефакт мутанта).

```
base_md5=rtp:c33f185bd1a3779757f528041f049eab,jitter:51d4bc9e3c4f309c62c6cb7a69bd89af
M1: exit=101 caught restored=True [m06,m01,m04] — профиль расширения 0xBEDE подменён
M2: exit=101 caught restored=True [m08,m05,m04] — words = len/4 считается как len/8
M3: exit=101 caught restored=True [m03,m09,m08] — marker сдвинут в бит 6 вместо 7
M4: exit=101 caught restored=True [m09,m08] — timestamp растёт на 2 кадра вместо 1
M5: exit=101 caught restored=True [m08] — marker не снимается после первого пакета
M7: exit=101 caught restored=True [m10] — порог comfort-noise 3 Б → 2 Б
M8: exit=101 caught restored=True [m12] — уже отданный seq не считается дублем
M9: exit=101 caught restored=True [m11] — окно не расширяется назад до первой выдачи
M10: exit=101 caught restored=True [m13,m11,m12] — выданный seq не сдвигает окно
caught=9/9 restored_all=True base_md5=rtp:c33f185bd1a3779757f528041f049eab,jitter:51d4bc9e3c4f309c62c6cb7a69bd89af
```

Все 9 пойманы; продукт восстановлен побайтно (`restored_all=True`, md5 совпадают с
base). Скрипт временно мутирует файл продукта и восстанавливает его по md5 (проверка `restored_all=True`); при SIGKILL восстановление не гарантировано — ограничение. Лог прогона сохранён в `waves/RW-29/mutations-w29b.log`.

## 8. Риски и ограничения

- **Wire-совместимость SRTP не заявляется**: модуль описывает только RTP-заголовок/
  расширение и джиттер; ключей и крипто в capture нет (`aead_aes256_gcm_rtpsize` — P4-03).
- **Opus-SDK не использован**: классификация DTX/CN — эвристика по длине шифротекста
  (порог 3 Б из единственного talkspurt capture), не разбор Opus TOC.
- Аномальные пакеты capture трактуются как opaque-хвост (см. §4.1); при появлении
  расшифровки этого варианта трактовку нужно пересмотреть вместе с TL.
- Реордер ограничен половиной пространства sequence; «очень поздний» пакет (за
  пределами 32768) и дубль старше 512 выданных seq классифицируются как `Late`, а не
  `Duplicate`. Для 20-мс аудио это ~10 с истории — достаточно для jitter-буфера.
- Полный `fmt`/`clippy`/`workspace`/`gate:worker` не запускались по ТЗ; мои файлы
  проверены `rustfmt --check` (edition 2024) — чисто.
