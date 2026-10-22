# REPORT-B06 — волна B06 / RW-25 (P4-03, P7-02, P8-01)

Дата приёмки: **2026-10-22**. Seat: TL-1 (Team Lead). Волна: **B06 / RW-25** — первая
**headless**-волна: офлайн, без новых внешних зависимостей (только офлайн-кэш cargo).

## 1. Итог

| Карточка | Крейт | Размер | Тесты | Статус |
|---|---|---|---|---|
| **P4-03** SRTP-крипта | `discord-voice` | L | 38 новых (16 KAT + 22 контрактных), крейт 65/65 | принято |
| **P7-02** перф-харнесс | `discord-perf` (новый) | L | 17 | принято |
| **P8-01** updater | `discord-updater` (новый) | L | 34 + 1 doctest | принято |

Факт: `cargo test --offline --workspace` — **785 passed / 0 failed** (baseline 663 → +122);
`discord-core` 531; fmt чисто; clippy `-D warnings` чисто; `npm run gate:worker` — passed
(`.gate/worker-2026-10-22T05-52-40-267Z.json`); мутации **18 / 18 caught, 0 missed**,
продукт восстановлен по md5. План: **35 / 90** принято.

## 2. Почему headless

GUI-batch (P0-01/P0-05/P0-06, P1-10 keyring, P2-03 SQLite) заблокирован: `tauri`, `iced`,
`egui`, `winit`, `keyring`, `rusqlite` отсутствуют и в `Cargo.lock`, и в офлайн-кэше cargo.
Решения владельца по зависимостям нет, поэтому B06 собрана так, чтобы **не требовать
решения**: три карточки полностью headless.

Проверено до старта: `cargo metadata --offline` и `cargo check --offline --workspace`
проходят; новые крейты собираются офлайн. Для updater взяты **уже лежащие** в кэше
`ring 0.17` (уже в `Cargo.lock` через rustls), `sha2 0.10`, `serde_json` — новых пакетов
в lock не добавлено, кроме дерева `serde_json`/`zmij`. Первая попытка взять
`ed25519-dalek` потребовала бы `fiat-crypto` (его нет в кэше, лицензия неразрешима) —
поэтому Ed25519 сделан на `ring`.

## 3. Как шла работа

Три **непересекающихся** набора файлов, три кодовых воркера параллельно, у каждого свой
`CARGO_TARGET_DIR` (`/tmp/opencode/target-w26a|b|c`) — иначе cargo блокируется общим
lock'ом на `target-dir`. Общие файлы (`Cargo.toml`/`Cargo.lock` крейтов, `src/lib.rs`,
`scripts/sbom.mjs`, `discord-core/src/state/presence_reducer.rs`) правил только TL-2.

Порядок: research (RE) → freeze (3 ТЗ) → integration-патч I0 (скелеты крейтов, `Cargo.lock`,
модули в `lib.rs`) → код тремя лейнами → targeted-логи → batch gate → независимое ревью
(codex + TL-1) → red-first фиксы → повторный gate → мутации → accepted → единоразовая
синхронизация доков.

**Локальный LLM как отдельный агент** (Qwen 27B, `llama-server` 127.0.0.1:8083, пул
`qwen_workers.py --profile qwen`) получил три read-only задачи: разбор раскладки SRTP,
ловушки перф-харнесса, безопасность updater (`waves/RW-25/research/llm-review.md`).
Его разбор байт SRTCP **отклонён** (перепутал расширение с payload, PT 0x78 назвал 24) —
арифметика раскладки (24 + 3 + 16 + 4 = 47) и функции самого клиента
(`PacketCrypter::HeaderLength`) дают другое. Приняты три содержательных пункта:
нулевая база регрессии, целостность backup, ловушка stale-seq.

**codex** (независимое ревью, `waves/RW-25/codex-review.md`) дало 4 находки.

**REA** использовался для статического разбора модуля `discord_voice.node` (1.0.165):
Ghidra не уложился в `REA_GHIDRA_STARTUP_TIMEOUT_MS` (330 с) на бинаре 127 МБ,
`inspect_binary_layout` недоступен (не задан `REA_PWNTOOLS_PYTHON`) — ограничения
зафиксированы в `research/RE-NATIVE-SRTP.md` §5; результат получен обычным путём
(`strings` + `readelf` + `nm` + `gdb` по DWARF).

## 4. RE-основание (не выдумано)

- `docs/re/voice-deep.md` §2.3–2.5 — раскладка RTP/SRTCP из strace-дампа.
- `waves/RW-25/research/RE-SRTP-LAYOUT.md` — байтовый разбор capture: 47/60/28/52/43 Б,
  счётчик `0x80000000 | n` (общий RTP+SRTCP), 43-байтные пакеты как аномалия.
- `waves/RW-25/research/RE-NATIVE-SRTP.md` — модуль клиента 1.0.165: libsodium
  (`crypto_aead_aes256gcm_*`, `crypto_aead_xchacha20poly1305_ietf_*`), 7 режимов движка,
  `discord::encryption::PacketCrypter` с `HeaderLength` / `SpaceRequiredToEncrypt` —
  прямая корреляция с выведенной раскладкой (+20 Б = тег 16 + трейлер 4).

Честная граница: **совместимость с wire Discord не заявлена** — ключевого материала в
capture нет; проверены раскладка и примитивы, не вывод ключей.

## 5. Ревью и фиксы (все — red-first)

| ID | Класс | Находка | Источник | Закрытие |
|---|---|---|---|---|
| A-1 | P0 | Табличный S-box (`SBOX[byte]`) — side-channel через кэш | codex | S-box алгебраический `affine(x^254)`, таблиц нет; тест на все 256 значений + KAT |
| C-1 | P0 | Path traversal: `artifact.name` в путях | codex | `PlanError::UnsafeArtifactName`, безопасный компонент, проверка «путь внутри `install_dir`» |
| C-2 | P0 | План строится по неподписанному манифесту | codex | тип `VerifiedManifest` (создаётся только `verify_manifest`), `plan_update` принимает только его + doctest `compile_fail` |
| C-3 | P1 | Откат переименовывает поверх существующего файла (Windows) | codex | порядок «цель → `.failed` → backup на место → удалить» |
| C-4 | P2 | Backup не проверяется перед откатом | локальный LLM | `PlanError::BackupCorrupted`, хэш прежней версии фиксируется при подмене |
| A-4 | P1 | Переполнение счётчика ChaCha20 (`wrapping_add`) | codex | `CryptoError::MessageTooLong`, проверка длины до аллокации |
| A-2 | P1 | `set_counter` допускает перемотку назад ⇒ повтор nonce | TL-1 | `SrtpError::CounterRewind`, только вперёд |
| A-3 | P1 | Расшифровка допускает повтор трейлера ⇒ повтор nonce под тем же ключом | TL-1 | `SrtpError::NonceReuse` + окно 1024 использованных трейлеров |
| B-1 | P1 | Нулевая база регрессии скрывает деградацию | локальный LLM | база 0.0 и текущая > 0 ⇒ `Regressed` |
| B-2 | P2 | Метрики в JSON строками (потеря младшего бита f64) | воркер + TL-2 | feature `serde_json/float_roundtrip`, числа вместо строк, побитовый тест round-trip |
| B-3 | P1 | Обходной путь `PRESENCE_SAMPLE` маскировал O(n²) в редьюсере | воркер | I1 (см. ниже) + честная нагрузка на всех 24 229 участников |

Изменения контракта (санкционированы TL-1 в раунде фиксов): `CryptoError::MessageTooLong`,
`SrtpError::{CounterRewind, NonceReuse}`, `PlanError::{UnsafeArtifactName, BackupCorrupted}`,
`VerifiedManifest`, публичные `aes::sbox`/`inv_sbox`, `check_message_len`.

**Интеграция I1** (TL-2, общий файл `discord-core/src/state/presence_reducer.rs`):
`upsert_presence` сканировал весь `state.presences` и пересобирал его через `retain` —
O(n²) на 24 229 участниках. Заменено на диапазон по ключу `(user_id, scope)`.
Red: 3.17 с на 10 000 upserts; green: 0.03 с. Тест `r19` в
`discord-core/tests/presence_reducer.rs`.

## 6. Baseline-фиксы до кода (TL-2)

- **clippy-гейт был красным ещё до B06** — 39 ошибок в скелете `discord-ui` (GUI-каркас,
  заблокированный до решения владельца): модули сделаны `pub`, `mod tests` получили
  `#[cfg(test)]`, добавлены `Default`-имплементации пяти сервисов, вычищены
  `derivable_impls`/`collapsible_if`/`unnecessary_cast`/`field_reassign_with_default`,
  удалено неиспользуемое поле `next_tray_id`.
- **SBOM-контракт падал** на `fiat-crypto` (нет в кэше) и на устаревшей slash-форме
  лицензии (`MIT/Apache-2.0`): в `scripts/sbom.mjs` добавлена нормализация slash-формы в
  SPDX (`MIT OR Apache-2.0`).

## 7. Артефакты

`waves/RW-25/`: `WAVE-PLAN.md`, `TZ-W26a/b/c.md`, `REVIEW-CHECKLIST.md`, `REVIEW-PROMPT.md`,
`REPORT-B06.md`, `TEST-MATRIX-B06.md`, `MUTATIONS-B06.md`, `run-mutations.py`,
`mutations/` (18 логов + `mutations.json`), `research/` (RE-SRTP-LAYOUT, RE-NATIVE-SRTP,
media-packet-layout.mjs, llm-tasks.jsonl, llm-review.md, llm-worker-events.jsonl),
`codex-review.md`, red/green-логи по лейнам и по раундам фиксов, отчёты воркеров
(`worker-W26a.md`, `worker-W26a-fix.md`, `worker-W26b.md`, `worker-W26b-fix.md`,
`worker-W26c.md`, `worker-W26c-fix.md`), `red-I1-presence.log`, `green-I1-presence.log`.

## 8. Что дальше

Следующий batch — **B07** (GUI/keyring/SQLite) — ждёт решения владельца по зависимостям
и окну на живой Discord. Разблокированные headless-остатки (P3-бэклог по i18n/UI-моделям,
DAVE-фрейминг, RTCP-статистика) можно вести отдельными волнами.

Коммит/push — только координатор (L1); рабочее дерево остаётся dirty.
