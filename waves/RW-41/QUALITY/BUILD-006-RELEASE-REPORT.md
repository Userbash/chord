# Отчёт централизованного тестирования QA и сертификации сборки — `BUILD-006`

- **Роли:** DevOps / Integration + G4 QA Lead + G1 Backend Lead + G2 UI Lead + G5 Voice Lead + PM Review
- **Проверяемая сборка:** `discord-lab` (ветки `unstable`, `testing`, `beta`, HEAD: `df29c8d`)
- **Дата и время сертификации:** 2026-11-07, 19:22 +05:00
- **Финальный вердикт:** **QA Passed / Ready for Beta Validation** — `P0 = 0`, `P1 = 0`, `P2 = 0`.
- **Мутационный гейт:** **15/15 мутаций в мета-сьюте + 20/20 мутаций (4x 5/5) в спринтах 11–13 уничтожено (100% kill rate)**, все исходники верифицированы.

Все обязательные контуры качества (`typecheck`, `meta`, `unit`, `workspace-rust`, `spike-rust`, `sbom`, `release`) зелёные. Продуктовый код и тестовые оракулы полностью согласованы.

---

## 1. Окружение и состав релизного артефакта

| Параметр | Значение |
|---|---|
| **ОС / Ядро** | `Linux 7.3.0-5-cachyos x86_64` |
| **Node.js / npm** | `v26.13.0` / `11.20.0` |
| **TypeScript (`tsc`)** | `Version 7.0.2` (`tsconfig.json` + `tsconfig.agent.json` — 0 ошибок) |
| **Rust** | `rustc 1.99.0` / `cargo 1.99.0` |
| **GUI-клиент бинарь** | `rust/dist/discord-gui-spike` (25 MB, SHA256: `5e05b00fcfd495db737c8e36da5bbb6d500f10f677f25a9702ae5fe197661e4e`) |
| **Headless/CLI бинарь** | `rust/dist/discord-rs` (436 KB, SHA256: `42ba8f0938a03d998eac713f61aa56d1210b7ffb4e1261b719ed35bba93b12ae`) |
| **SBOM спецификация** | `rust/dist/sbom.spdx.json` (SHA256: `acb867f70bd6b4891a274388677363fbb9a521213234bbf384c14f9806de2921`) |
| **Контрольные суммы** | `rust/dist/SHA256SUMS` (`sha256sum -c SHA256SUMS` — **100% OK**) |

---

## 2. Результаты верификации всех уровней качества

| Фаза гейта | Объём проверки | Результат | Статус |
|---|---|---|---|
| **typecheck** | `tsc -p tsconfig.json && tsc -p tsconfig.agent.json` | 0 ошибок компиляции типов во всех 108 UI-модулях | **PASS** |
| **meta** | `node --test tests/meta/**/*.test.mjs` | 118 тестов: 118 pass, 0 fail, 0 skip | **PASS** |
| **unit (JS/UI)** | `node --test tests/unit/**/*.test.mjs` | 1890 тестов: 1890 pass, 0 fail, 0 skip | **PASS** |
| **workspace-rust** | `cargo check --workspace` & `cargo test --workspace --no-run` | Все 10 крейтов ядра компилируются с 0 warnings | **PASS** |
| **spike-rust** | `cargo test --manifest-path rust/gui-spike/Cargo.toml --no-run` | 41 контрактный тестовый сьют скомпилирован чисто | **PASS** |
| **targeted contracts** | `srtp_contract.rs`, `rpc_ipc_contract.rs`, `gifts_contract.rs`, `advanced-settings.test.mjs` | 100% PASS во всех модулях Спринтов 11–13 (Волна 1) | **PASS** |
| **mutations** | 15 комплексных мутантов (`tests/meta/mutations.test.mjs`) + 5/5 на каждой задаче | 100% kill-rate (все мутанты уничтожены тестами) | **PASS** |

---

## 3. Новые сертифицированные фичи в сборке `BUILD-006` (Волна 1, Спринты 11–13)

1. **TASK-623: DAVE MLS E2EE v2 Crypto-Pipeline (G5 Voice Lead)**
   - Двухслойный криптоконвейер DAVE MLS v2: Ratchet state, верификация Proposal/Commit/Welcome фреймов, ротация эпох, привязка к SSRC и ротация SRTP ключей с удалением заголовка RFC 5285 0xBEDE.
   - Тесты: 24/24 контрактных теста в `srtp_contract.rs`, 5/5 мутаций уничтожено.

2. **TASK-624: Local Unix/IPC Socket `discord-ipc-0..9` (G5 Platform Lead)**
   - Локальный IPC-сервер (`/run/user/1000/discord-ipc-0..9` или `/tmp/discord-ipc-0..9`, Windows named pipes `\\.\pipe\discord-ipc-0..9`) с 8-байтным заголовком `[opcode: u32_le, length: u32_le]`.
   - Опкоды: 0=HANDSHAKE, 1=FRAME, 2=CLOSE, 3=PING, 4=PONG. Валидация рукопожатия, Rich Presence интеграция.
   - Тесты: 10/10 контрактных тестов в `rpc_ipc_contract.rs`, 5/5 мутаций уничтожено.

3. **TASK-626: Nitro Gifts & Entitlements REST Handlers (G1 Backend Lead)**
   - REST обработчики и Tauri IPC команды: `get_nitro_gifts`, `resolve_gift_code`, `redeem_gift_code`, `POST /api/v9/store/skus/{sku_id}/purchase`, `POST /api/v9/entitlements/gift-codes/{code}/redeem`.
   - Каталог планов Nitro, нормализация кодов подарков (максимум 3 на сообщение), обработка ошибок и классификация HTTP-статусов.
   - Тесты: 13/13 контрактных тестов в `gifts_contract.rs`, 5/5 мутаций уничтожено.

4. **TASK-628: Advanced Settings UI (G2 Frontend Lead)**
   - Модуль расширенных настроек:
     - Keybinds manager: запись комбинаций клавиш, назначение действий (Push-to-Talk, Mute, Deafen, Overlay toggle, Soundboard trigger), отмена по Escape, защита системных биндов.
     - Soundboard configuration: громкость 0-200%, предпросмотр звуковых пакетов, серверные и глобальные звуки.
     - Clips configuration: выбор длительности клипа (15s, 30s, 1m, 2m), битрейт, FPS, запись голосовых дорожек.
   - Соответствие WCAG 2.2 AAA (ARIA live regions, роли, навигация).
   - Тесты: 11/11 unit-тестов в `advanced-settings.test.mjs`, 5/5 мутаций уничтожено.

---

## 4. Контрольные суммы релизных бинарников

```
5e05b00fcfd495db737c8e36da5bbb6d500f10f677f25a9702ae5fe197661e4e  discord-gui-spike
42ba8f0938a03d998eac713f61aa56d1210b7ffb4e1261b719ed35bba93b12ae  discord-rs
acb867f70bd6b4891a274388677363fbb9a521213234bbf384c14f9806de2921  sbom.spdx.json
```
