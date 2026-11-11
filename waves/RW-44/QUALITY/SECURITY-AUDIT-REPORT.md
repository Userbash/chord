# Отчёт комплексного аудита безопасности, очистки секретов и SBOM SPDX 2.3 (`TASK-956`)

- **Карточка задачи:** `TASK-956` (`docs/specs/BETA-PROMOTION-PLAN-AND-TASKS.md` §3)
- **Отдел:** `devops`
- **Приоритет:** `P0`
- **Дата проверки:** `2026-11-11`
- **Статус вердикта:** `PASSED` (`0 находок / 0 leaks`, `SPDX-2.3 VALID`, `5/5 mutations killed`)

---

## 1. Статический поиск секретов по регулярным выражениям (`SUB-956-1`)

Выполнено полное сканирование исходного кода репозитория (`rust/crates`, `rust/discord-gpui/src`, `rust/gui-spike/src`, `rust/gui-spike/ui`, `scripts` — всего **581 исходный файл**) и релизного бинарного артефакта `rust/dist/discord-gpui` (**43 593 624 байт**, stripped ELF) с помощью инструмента `scripts/secret-scan.mjs` по **22 паттернам регулярных выражений**:

| № | ID правила | Критичность | Описание паттерна | Найдено утечек |
|---|---|---|---|---|
| 1 | `discord_mfa_token` | `CRITICAL` | Discord MFA User Token (`mfa.<84 chars>`) | `0` |
| 2 | `discord_bot_auth_header` | `CRITICAL` | Discord Bot Authorization Token (`Bot <jwt>`) | `0` |
| 3 | `discord_user_or_bot_token` | `CRITICAL` | Discord User / Bot Base64 Token (`[MNO]...`) | `0` |
| 4 | `discord_bearer_token` | `CRITICAL` | Discord OAuth2 Bearer Token (`Bearer <50+ chars>`) | `0` |
| 5 | `discord_webhook_url` | `HIGH` | Discord Webhook URL с секретным токеном | `0` |
| 6 | `dave_mls_private_key_pem` | `CRITICAL` | Приватные ключи PEM (`DAVE MLS / EC / RSA / OPENSSH PRIVATE KEY`) | `0` |
| 7 | `dave_mls_hardcoded_secret` | `CRITICAL` | Захардкоженные 32-байтные секреты DAVE MLS (`joiner_secret`, `epoch_secret`, `exporter_secret`, `welcome_secret`, `confirmation_key`) | `0` |
| 8 | `github_pat_classic` | `CRITICAL` | GitHub Classic Personal Access Token (`ghp_...`) | `0` |
| 9 | `github_fine_grained_pat` | `CRITICAL` | GitHub Fine-Grained PAT (`github_pat_...`) | `0` |
| 10 | `aws_access_key_id` | `CRITICAL` | AWS Access Key ID (`AKIA...` / `ASIA...`) | `0` |
| 11 | `aws_secret_access_key` | `CRITICAL` | AWS Secret Access Key (40-символьный ключ) | `0` |
| 12 | `google_api_key` | `HIGH` | Google Cloud / Firebase API Key (`AIza...`) | `0` |
| 13 | `slack_api_token` | `HIGH` | Slack Bot / User / Workspace Token (`xox[baprs]-...`) | `0` |
| 14 | `stripe_live_secret_key` | `CRITICAL` | Stripe Live Secret API Key (`sk_live_...`) | `0` |
| 15 | `openai_or_anthropic_key` | `CRITICAL` | OpenAI / Anthropic Production API Key (`sk-proj-...` / `sk-ant-...`) | `0` |
| 16 | `telegram_bot_token` | `HIGH` | Telegram Bot API Token | `0` |
| 17 | `sendgrid_api_key` | `HIGH` | SendGrid API Token (`SG....`) | `0` |
| 18 | `npm_access_token` | `CRITICAL` | npm Registry Automation / Publish Token (`npm_...`) | `0` |
| 19 | `pypi_upload_token` | `CRITICAL` | PyPI Upload API Token (`pypi-AgEIcHlwaS5vcmc...`) | `0` |
| 20 | `json_web_token` | `HIGH` | Подписанный JSON Web Token (`eyJ...`) | `0` |
| 21 | `database_uri_with_password` | `CRITICAL` | Строки подключения к СУБД с встроенным паролем | `0` |
| 22 | `generic_private_key_hex` | `CRITICAL` | Захардкоженный 64-hex приватный ключ подписи | `0` |

- **Шаг 1.1 (Токены Discord MFA, Bot, User):** `0 находок (0 leaks)`.
- **Шаг 1.2 (Приватные ключи DAVE MLS в `rust/crates/discord-voice/src/dave_v1/`):** проверены `mod.rs`, `mls.rs`, `replay.rs`, `session.rs` — `0 находок`.

---

## 2. Аудит маскирования в логах и `zeroize`-очистки памяти (`SUB-956-2`)

### 2.1. Маскирование чувствительных данных в выводе `panic` и `logcat`
- Реализована и верифицирована функция санитизации логов `maskSensitiveLogOutput`:
  - Токены многофакторной аутентификации автоматически заменяются на маску **`mfa.***`**.
  - Токены ботов в заголовках и логах заменяются на маску **`Bot ***`**.
  - OAuth2 Bearer-токены заменяются на **`Bearer ***`**, а пользовательские JWT-токены — на **`[REDACTED_TOKEN]`**.
- В Rust-крейтах (`discord-core`, `discord-voice`) подтверждено наличие **38+ точек принудительного редактирования `Debug`/`Display`** (`<redacted>` и `<скрыто>`):
  - `rust/crates/discord-core/src/rest/client.rs` и `transport.rs`: заголовки `Authorization` и `Cookie` выводятся как `<redacted>`.
  - `rust/crates/discord-core/src/ready/snapshot.rs` и `state/types.rs`: поля `session_id` и `user_settings_proto` выводятся как `<redacted>`.
  - `rust/crates/discord-voice/src/session.rs` и `lib.rs`: поля `token`, `session_id`, `secret_key`, `media_session_id` выводятся как `<redacted>`.
  - `rust/crates/discord-voice/src/dave_v1/mls.rs`: структуры `DaveMlsEpochSecrets` и `DaveMlsKeySchedule` скрывают ключевой материал как `<скрыто>`.

### 2.2. Очистка секретов в оперативной памяти перед деаллокацией (`zeroize`)
- В `rust/crates/discord-voice/src/dave_v1/mod.rs` (строки 92–96) реализована устойчивая к оптимизациям компилятора функция `zeroize_slice`:
  ```rust
  pub(crate) fn zeroize_slice(buf: &mut [u8]) {
      for byte in buf.iter_mut() {
          *byte = black_box(0);
      }
  }
  ```
- В `rust/crates/discord-voice/src/dave_v1/mls.rs`:
  - `impl Drop for DaveMlsEpochSecrets` (строки 411–422) зануляет все 8 секретов эпохи (`joiner_secret`, `welcome_secret`, `epoch_secret`, `exporter_secret`, `epoch_authenticator`, `confirmation_key`, `membership_key`, `init_secret`) перед освобождением памяти.
  - `impl Drop for DaveHashRatchet` (строки 620–628) и `delete_keys_up_to` (Forward Secrecy, строки 726–741) зануляют `next_secret` и все ключи/nonce прошлых поколений.
  - Промежуточные буферы HKDF (`key_block`, `ipad`, `opad`, `prev_t`, `member_secret`, `extracted`, `derived`, `media_secret`) зануляются сразу после вычисления ключей.

---

## 3. Генерация и верификация финального SBOM-манифеста (`SUB-956-3`)

Командой `node scripts/sbom.mjs --check --output rust/dist/sbom.spdx.json` сгенерирован и проверен воспроизводимый манифест `rust/dist/sbom.spdx.json`:

- **Стандарт документа:** `SPDX-2.3` (`dataLicense: "CC0-1.0"`, `SPDXID: "SPDXRef-DOCUMENT"`)
- **Пространство имён:** `https://discord-lab.local/spdx/321689eb7a98ffa1b90115a6fdc80588df89437be1e9e7e935eada1bffd5a223`
- **SHA-256 файла `rust/dist/sbom.spdx.json`:** `acb867f70bd6b4891a274388677363fbb9a521213234bbf384c14f9806de2921`
- **Всего пакетов в `Cargo.lock` / SBOM:** **`98`** (требование `>= 85` зависимостей выполнено)
- **Связей графа зависимостей (`relationships`):** **`184`** (`13 DESCRIBES` + `171 DEPENDS_ON`)
- **Пакетов с `NOASSERTION` или запрещёнными лицензиями:** **`0`**

### Распределение лицензий по 98 пакетам:
| SPDX-выражение лицензии | Количество пакетов | Статус политики Open Source |
|---|---|---|
| `MIT OR Apache-2.0` | `72` | Разрешена |
| `MIT` | `9` | Разрешена |
| `BSD-3-Clause` | `4` | Разрешена |
| `Apache-2.0 OR MIT` | `3` | Разрешена |
| `ISC` | `2` | Разрешена |
| `CDLA-Permissive-2.0` | `2` | Разрешена |
| `Unlicense OR MIT` | `1` | Разрешена |
| `MIT OR Apache-2.0 OR LGPL-2.1-or-later` | `1` | Разрешена (по ветке `MIT OR Apache-2.0`) |
| `Apache-2.0 AND ISC` | `1` | Разрешена |
| `Apache-2.0 OR ISC OR MIT` | `1` | Разрешена |
| `(MIT OR Apache-2.0) AND Unicode-3.0` | `1` | Разрешена |
| `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | `1` | Разрешена |

---

## 4. Верификация TDD и Мутационный гейт (`5/5 = 100%`)

- **Модульные и контрактные тесты Node:** `node --test tests/unit/security-audit-sbom-956.test.mjs` — **4/4 PASS**.
- **Контрактные тесты Rust SBOM:** `cargo test --offline --manifest-path rust/Cargo.toml -p discord-conformance --test sbom_contract` — **5/5 PASS**.
- **Мутационное тестирование (`5/5 killed`):**
  1. `M1_bypass_mfa_regex` (ослабление границы MFA-токена с 84 до 83 символов) — **KILLED**.
  2. `M2_disable_panic_mfa_mask` (отключение маски `mfa.***` в `maskSensitiveLogOutput`) — **KILLED**.
  3. `M3_disable_bot_mask` (отключение маски `Bot ***` в `maskSensitiveLogOutput`) — **KILLED**.
  4. `M4_skip_zeroize_fill` (удаление `buf.fill(0)` в `zeroizeBuffer`) — **KILLED**.
  5. `M5_invert_spdx_license_check` (разрешение запрещённой лицензии `GPL-3.0-only`) — **KILLED**.
- Исходный MD5 файла `scripts/secret-scan.mjs` (`9d491150ec59fda5d3ea0a97e6933c4a`) полностью восстановлен после мутационного прогона.
