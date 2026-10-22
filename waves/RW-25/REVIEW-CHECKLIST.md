# REVIEW-CHECKLIST — B06 / RW-25 (чек-лист приёмки волны)

Seat: REV-1 / REV-2 + TL-1. Волна: **B06 / RW-25**. Карточки: **P4-03**, **P7-02**, **P8-01**.
Источник истины по контракту: `TZ-W26a.md`, `TZ-W26b.md`, `TZ-W26c.md`, `WAVE-PLAN.md`.
RE-оракул: `research/RE-SRTP-LAYOUT.md` (capture), `research/RE-NATIVE-SRTP.md` (нативный
модуль 1.0.165), `docs/re/voice-deep.md`.

## 0. Внешние мнения (не источник истины)

- Локальный LLM (Qwen 27B, `research/llm-review.md`) — задачи: раскладка SRTP, ловушки
  перф-харнесса, безопасность updater. **Его разбор байт SRTCP оказался неверным**
  (перепутал расширение с payload, PT 0x78 назвал 24) — отклонён арифметикой
  (24 + 3 + 16 + 4 = 47) и функциями Discord `HeaderLength()` / `SpaceRequiredToEncrypt()`.
  Полезные пункты приняты: см. §4.
- codex (независимое ревью) — `REVIEW-PROMPT.md`, отчёт `codex-review.md`.

## 1. P4-03 — SRTP/SRTCP (лейн W26a)

- [ ] Публичный API совпадает с `TZ-W26a.md` §3 **буквально** (имена, типы, порядок).
- [ ] AES-256: раундовые ключи, S-box, ShiftRows, MixColumns, ключевое расписание.
      KAT — FIPS-197 C.3 (не вычислено тем же кодом).
- [ ] GCM: GHASH по GF(2^128), порядок битов, тег = GHASH(AAD‖C‖len) ⊕ E(K,J0),
      AAD не пустой ⇒ проверяется.
- [ ] ChaCha20-Poly1305 и XChaCha20 (HChaCha20 для 24-байтного nonce) — RFC 8439 / draft.
- [ ] HKDF-SHA1: extract/expand, `out_len = 0` и `> 255*20` ⇒ ошибка.
- [ ] Раскладка: `[clear][ciphertext][tag 16][counter 4 LE]`; AAD = clear-область в режимах
      `*_rtpsize`; для `Aes256Gcm` — AAD пустой, clear-области нет.
- [ ] Длина clear-области RTP = 12 + 4*CC + (X ? 4 + 4*ext_words : 0).
- [ ] Счётчик: исходящий `0x80000000 | n`, общий для RTP и SRTCP, монотонный, не сбрасывается;
      на расшифровке монотонность **не** проверяется.
- [ ] Ошибки: `PacketTooShort`, `MalformedHeader` (V≠2, CC>15, расширение не влезает),
      `UnsupportedMode`, `AuthenticationFailed`, `CounterOverflow`. **Без panic на вводе.**
- [ ] 43-байтовый пакет из capture ⇒ явная ошибка, не паника и не «угадывание».
- [ ] `Debug` ключей не утекает байты.
- [ ] KAT-векторы — из RFC/NIST; сомнительные не выдуманы (в отчёте — открытые вопросы).

## 2. P7-02 — перф-харнесс (лейн W26b)

- [ ] API совпадает с `TZ-W26b.md` §3.
- [ ] Генератор детерминирован: один seed ⇒ тот же `digest`; другой seed ⇒ другой.
- [ ] `seq` строго монотонен — иначе stale-политика редьюсера отбросит события и
      `messages_ingested` окажется меньше 100 000 (тихая потеря данных).
- [ ] `members_loaded == spec.members` и `messages_ingested == spec.messages` — из
      **фактического** состояния, а не из `spec`.
- [ ] Замеры — реальные (`Instant`), а не константы.
- [ ] Бюджет холодного старта — skip-aware с явной причиной пропуска.
- [ ] `report_to_json` / `report_from_json` — round-trip без потерь.
- [ ] Правило регрессии (см. §4, находка LLM-1) — корректно.

## 3. P8-01 — updater (лейн W26c)

- [ ] API совпадает с `TZ-W26c.md` §3.
- [ ] Подпись над **исходными байтами** тела; тело не канонизируется.
- [ ] Проверка подписи обязательна до использования манифеста.
- [ ] SHA-256 файла сверяется **до** любых перемещений; при несовпадении цель не тронута.
- [ ] Staged-замена: staging → backup цели → rename; при сбое — откат, исходный файл цел.
- [ ] `rollback` возвращает прежнее содержимое и убирает backup.
- [ ] Downgrade запрещён: `is_newer_than` строгий; pre-release младше релиза.
- [ ] Версия разбирается локально (без `semver`), мусорные строки отвергаются.
- [ ] Никаких `unwrap`/`expect` в продуктовом коде; никакой сети; пути — под temp-dir.

## 4. Находки ревью и их закрытие (все — red-first)

| ID | Класс | Находка | Источник | Закрытие |
|---|---|---|---|---|
| A-1 | **P0** | Табличный S-box (`SBOX[byte]`) — индекс зависит от состояния ⇒ side-channel через кэш | codex | S-box алгебраический: `affine(x^254)`, умножение в GF(2^8) масками, таблицы убраны; тест на все 256 значений + KAT FIPS-197 C.3 (`sbox_matches_fips197_k`) |
| C-1 | **P0** | Path traversal: `artifact.name` попадает в пути staging/цели/backup | codex | `PlanError::UnsafeArtifactName`; имя — безопасный одиночный компонент; вторая линия — проверка «путь внутри `install_dir`» |
| C-2 | **P0** | `plan_update` принимает неподписанный `Manifest` | codex | тип `VerifiedManifest` (создаётся только `verify_manifest`); `plan_update` принимает только его; doctest `compile_fail` доказывает, что `&Manifest` не принимается |
| C-3 | P1 | `rollback` переименовывает поверх существующего файла (на Windows `rename` не заменяет) | codex | порядок «цель → `.failed` → backup на место → удалить `.failed`»; на Linux «красного» не видно — зафиксировано честно |
| A-4 | P1 | Переполнение счётчика ChaCha20 (`wrapping_add`) ⇒ повтор keystream | codex | `CryptoError::MessageTooLong`; проверка длины **до** аллокации (`check_message_len`) в GCM и ChaCha/XChaCha |
| A-2 | P1 | `set_counter` допускал перемотку назад ⇒ повтор nonce | TL-1 | `SrtpError::CounterRewind`; только движение вперёд |
| A-3 | P1 | Расшифровка допускала повтор трейлера ⇒ повтор nonce под тем же ключом (GCM: утечка XOR и подделка тегов) | TL-1 | `SrtpError::NonceReuse` + окно 1024 использованных трейлеров, проверка **до** расшифровки; окно приёма отделено от счётчика отправки |
| C-4 | P2 | Backup не проверялся перед откатом (сбой питания в момент swap) | локальный LLM | `PlanError::BackupCorrupted`; хэш прежней версии фиксируется при подмене и сверяется перед восстановлением |
| B-1 | P1 | Нулевая база регрессии скрывала деградацию (метрика, ставшая нулевой, проходила молча) | локальный LLM | база `0.0` и текущая `> 0` ⇒ `Regressed` (`delta_pct = INFINITY`, задокументировано) |
| B-2 | P2 | Метрики в JSON строками; без `float_roundtrip` терялся младший бит f64 | воркер + TL-2 | feature `serde_json/float_roundtrip` (правка `Cargo.toml` — TL-2), числа вместо строк, побитовый тест round-trip; эталон перегенерирован |
| B-3 | P1 | Обходной путь `PRESENCE_SAMPLE=64` маскировал O(n²) в `upsert_presence` | воркер | интеграция I1 (диапазон по ключу `(user_id, scope)`): red 3.17 с / green 0.03 с на 10 000 upserts; харнесс честно грузит presence всех 24 229 участников |
| RE-1 | P1 | `PacketCrypter::SpaceRequiredToEncrypt` ⇒ +20 Б; `HeaderLength` ⇒ длина открытой части по режиму | `research/RE-NATIVE-SRTP.md` | сверено с кодом: тег 16 + трейлер 4, clear-область по режиму — совпадает |
| RE-2 | P3 | 7 режимов движка в модуле 1.0.165 | `research/RE-NATIVE-SRTP.md` | 3 в карточке, остальные (`xsalsa20_poly1305*`) — `UnsupportedMode` |
| LLM-x | — | Разбор байт SRTCP от локального LLM | `research/llm-review.md` | **отклонён**: арифметика раскладки (24 + 3 + 16 + 4 = 47) и `HeaderLength` дают другое; LLM перепутал расширение с payload и PT 0x78 назвал 24 |

Итог после раунда фиксов: **P0/P1/P2 = 0**.

## 5. Гейт волны (порядок)

1. `cargo fmt --all -- --check` — 0.
2. `cargo clippy --offline --workspace --all-targets --all-features -- -D warnings` — 0.
3. `cargo test --offline --workspace` — 0 failed; числа выше baseline 663.
4. `npm run gate:worker` — passed.
5. Мутации (QA-1) — `MUTATIONS-B06.md`, целевой catch rate ≥ 90 %.
6. Независимое ревью (codex + REV) — P0/P1/P2 = 0.
7. `accepted` → `waves/db.jsonl` + единоразовая синхронизация `STATE.md`, `STATUS.md`,
   `ROADMAP.md`, `TASKS.md`, `docs/map.md`.

## 6. Область ответственности приёмки (TL-1)

- Коммит/push — только координатор (L1), воркеры не коммитят.
- Доки (`STATE`/`STATUS`/`ROADMAP`/`TASKS`/`map`) обновляются **один раз после accepted**.
- `docs/map.md` — добавить крейты `discord-perf`, `discord-updater` и карточку P4-03.
- `tests/meta/rust-client-sync.test.mjs` — список `CRATES` дополнить новыми крейтами.
