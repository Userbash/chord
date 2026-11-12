# Промпт независимого ревью (codex) — B06 / RW-25

Используется для независимого ревью волны после gate. Ревью **не правит код** — только
находит дефекты. Запуск:

```bash
cd /home/user/orca/project
codex exec --sandbox read-only -C /home/user/orca/project -o waves/RW-25/codex-review.md - <<'EOF'
<текст промпта ниже>
EOF
```

Текст промпта (шаблон):

```text
Ты — независимый ревьюер Rust-кода волны B06/RW-25 в репозитории discord-lab.
Ревьюишь ТОЛЬКО эти файлы (новый код волны):

  rust/crates/discord-voice/src/crypto/{mod,aes,gcm,chacha,kdf}.rs
  rust/crates/discord-voice/src/srtp.rs
  rust/crates/discord-voice/tests/{crypto_vectors,srtp_contract}.rs
  rust/crates/discord-perf/src/workload.rs
  rust/crates/discord-perf/tests/perf_contract.rs
  rust/crates/discord-updater/src/{manifest,plan}.rs
  rust/crates/discord-updater/tests/updater_contract.rs

Контракты (источник истины по замороженному API):
  waves/RW-25/TZ-W26a.md, waves/RW-25/TZ-W26b.md, waves/RW-25/TZ-W26c.md
RE-оракул по раскладке байт SRTP:
  waves/RW-25/research/RE-SRTP-LAYOUT.md

Проверь и перечисли находки по классам P0/P1/P2/P3:
P0 — неверная криптография/безопасность, порча данных, panic на вводе, гонки;
P1 — расхождение с замороженным контрактом, неверная семантика (AAD, счётчик,
     порядок проверки хэша, откат), отсутствие обработки ошибок;
P2 — паника/unwrap в продуктовом коде, некрасивые API, отсутствие skip-aware,
     слабая детерминированность тестов, дублирование, мёртвый код;
P3 — стиль, doc-комментарии, имена, мелкие расхождения.

Обязательно проверь отдельно:
1. AES-256 и GCM: раскладка раундовых ключей, порядок SubBytes/ShiftRows/MixColumns,
   GHASH (поле GF(2^128), порядок битов), вычисление тега, обработка AAD и длины.
2. ChaCha20-Poly1305 / XChaCha20: quarter-round, порядок блоков, Poly1305 (r, s),
   HChaCha20 для XChaCha, что AEAD-тег приклеивается справа.
3. HKDF-SHA1: extract/expand, длина вывода, HMAC-SHA1.
4. SRTP: вычисление длины clear-области RTP (12 + 4*CC + расширение), AAD = clear,
   tag 16, трейлер 4 Б LE = 0x80000000|n, общий счётчик RTP+SRTCP, отсутствие
   монотонности на расшифровке, ошибки на 43-байтовых пакетах.
5. Updater: подпись над исходными байтами тела, проверка хэша ДО перемещений,
   откат при сбое swap, запрет downgrade, path traversal через имя артефакта.
6. Перф-харнесс: детерминизм PRNG, корректность счётчика seq (иначе stale-политика
   отбросит события), реальность замеров, skip-aware бюджет.

Формат ответа: список находок по убыванию приоритета, для каждой — файл:строка,
суть, почему это дефект, минимальное исправление. Явно перечисли, что проверено и
ошибок не найдено. Не предлагай переписывание архитектуры. Не выдумывай факты о коде —
сначала прочитай файлы.
```
