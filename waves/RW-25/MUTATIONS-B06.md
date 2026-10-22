# MUTATIONS-B06 — матрица мутаций волны B06 / RW-25

Дата: 2026-10-22. Seat: QA-1. Скрипт: `waves/RW-25/run-mutations.py` (логи — `waves/RW-25/mutations/`).

**Итог: 18 / 18 поймано, 0 пропущено, 0 «шаблон не найден». Продукт восстановлен по md5
после каждой мутации (проверено в логах: `restore=True`).**

Catch rate: **100 %** (цель ≥ 90 %).

## Матрица

| ID | Файл | Что ломается | Целевые тесты | Результат |
|---|---|---|---|---|
| M1 | `discord-voice/src/srtp.rs` | маркер старшего бита трейлера (`0x80000000 \| n` → `0x00000000`) | `srtp_contract` | поймано (exit 101) |
| M2 | `discord-voice/src/srtp.rs` | длина AEAD-тега 16 → 12 | `srtp_contract` | поймано |
| M3 | `discord-voice/src/srtp.rs` | AAD GCM заменён на пустой | `srtp_contract` | поймано |
| M4 | `discord-voice/src/srtp.rs` | длина clear-области RTP фиксирована (12 Б вместо расчёта) | `srtp_contract` | поймано |
| M5 | `discord-voice/src/srtp.rs` | длина расширения RTP игнорируется (`+4` вместо `+4+4*words`) | `srtp_contract` | поймано |
| M6 | `discord-voice/src/srtp.rs` | счётчик не растёт после шифрования | `srtp_contract` | поймано |
| M7 | `discord-voice/src/srtp.rs` | AAD XChaCha заменён на пустой | `srtp_contract` | поймано |
| M8 | `discord-voice/src/crypto/aes.rs` | константа аффинного преобразования S-box `0x63` → `0x62` | `crypto_vectors` | поймано |
| M15 | `discord-voice/src/crypto/aes.rs` | маска в `gf_mul` искажена (`^ 0x01`) | `crypto_vectors` | поймано |
| M16 | `discord-voice/src/crypto/kdf.rs` | внешняя подкладка HMAC `0x5c` → `0x5d` | `crypto_vectors` | поймано |
| M17 | `discord-voice/src/srtp.rs` | clear-область SRTCP 8 Б → 4 Б | `srtp_contract` | поймано |
| M18 | `discord-voice/src/srtp.rs` | проверка повторного nonce на приёме отключена | `srtp_contract` | поймано |
| M9 | `discord-updater/src/plan.rs` | проверка безопасного имени артефакта отключена | `updater_contract` | поймано |
| M10 | `discord-updater/src/plan.rs` | сверка SHA-256 отключена | `updater_contract` | поймано |
| M11 | `discord-updater/src/plan.rs` | запрет `/` и `\` в имени артефакта снят | `updater_contract` | поймано |
| M12 | `discord-updater/src/manifest.rs` | pre-release считается не младше релиза | `updater_contract` | поймано |
| M13 | `discord-updater/src/plan.rs` | проверка целостности backup отключена | `updater_contract` | поймано |
| M14 | `discord-perf/src/workload.rs` | порог регрессии 10 % → 100 % | `perf_contract` | поймано |

## Замечания

- M8 в первой редакции матрицы била по `static SBOX` — таблицы убраны в раунде фиксов
  (side-channel через кэш), поэтому мутация переписана на константу аффинного
  преобразования; KAT FIPS-197 C.3 остаётся оракулом.
- Мутации применялись к продуктовому коду, каждый раз с последующим восстановлением по
  md5 — состояние репозитория после прогона совпадает с исходным (проверено `restore=True`
  в каждом логе `mutations/mNN.log`).
- Отдельной мутации на `discord-core` (интеграция I1) в матрицу не включён: мутация
  O(n²) в `upsert_presence` — это **производительность**, а не семантика; её ловит
  интеграционный тест `r19` по времени (red 3.17 с / green 0.03 с), а не мутация.
