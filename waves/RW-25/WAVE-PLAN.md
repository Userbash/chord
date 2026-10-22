# WAVE-PLAN — B06 / RW-25 (headless-волна: SRTP, перф-харнесс, updater)

Статус: заморожен 2026-10-22 Team Lead. Волна **B06 / RW-25** — первая headless-волна после
принятия B05. План-источник — `docs/rust-client/PLAN.md` (90 карточек), роли — `TEAM.md`,
процесс — `PROCESS.md` §3.13–§3.15.

## 0. Почему headless

GUI-batch (P0-01/P0-05/P0-06, P1-10 keyring, P2-03 SQLite) заблокирован: `tauri`, `iced`,
`egui`, `winit`, `keyring`, `rusqlite` отсутствуют в `Cargo.lock` и в офлайн-кэше cargo
(`~/.cargo/registry/cache` содержит только `chacha20`, `ed25519-dalek`, `sha2`, `serde*`,
`tar`, `semver`, `url` из внешних). Решение владельца по зависимостям не получено, поэтому
волна B06 идёт **полностью офлайн и без новых сетевых зависимостей**, кроме офлайн-кэша.

Проверено: `cargo metadata --offline` и `cargo check --offline --workspace` проходят; новые
крейты `discord-perf` и `discord-updater` собираются офлайн (`ed25519-dalek 2.1.1` +
`curve25519-dalek 4.1.3` + `sha2 0.10.9` — все в кэше, сети нет).

## 1. Карточки и лейны

| Лейн | Карточка | Крейт | Размер | Файлы (владение) |
|---|---|---|---|---|
| **W26a** | **P4-03** SRTP-крипта | `discord-voice` | L | `src/crypto/*.rs`, `src/srtp.rs`, `tests/crypto_vectors.rs`, `tests/srtp_contract.rs` |
| **W26b** | **P7-02** перф-харнесс | `discord-perf` (новый) | L | `src/workload.rs`, `tests/perf_contract.rs` |
| **W26c** | **P8-01** updater | `discord-updater` (новый) | L | `src/manifest.rs`, `src/plan.rs`, `tests/updater_contract.rs` |

Параллелизм: три независимых набора файлов, пересечений нет. Общие файлы (`Cargo.toml`
workspace, `Cargo.lock`, `src/lib.rs` крейтов, `waves/*`, доки) правит **только TL-2** —
см. §4.

## 2. Замороженные контракты

Контракты — в `TZ-W26a.md`, `TZ-W26b.md`, `TZ-W26c.md`. Сигнатуры публичного API в этих
доках обязательны буквально. Допускаются только **приватные** хелперы внутри модуля.

| Лейн | Ключевые типы/функции |
|---|---|
| W26a | `crypto::aes::{Aes256, Aes256Key}`; `crypto::gcm::{Gcm, GcmError}`; `crypto::chacha::{ChaCha20Poly1305, XChaCha20Poly1305}`; `crypto::kdf::{hkdf_sha1, srtp_key_derive}`; `srtp::{SrtpContext, SrtpError, EncryptionMode, encrypt_rtp, decrypt_rtp, encrypt_srtcp, decrypt_srtcp}` |
| W26b | `workload::{Workload, WorkloadSpec, PerfReport, build_workload, measure}` |
| W26c | `manifest::{Manifest, Artifact, ManifestError, parse_manifest, verify_manifest}`; `plan::{UpdatePlan, PlanStep, PlanError, plan_update}` |

## 3. Оракулы (RE)

- `docs/re/voice-deep.md` §2.3–2.5 — RTP-заголовок (`V=2, P=0, X=1, CC=0`), `PT=120`,
  расширение RFC 5285 (`0xBEDE`, 2 слова), хвост `[ciphertext][GCM tag 16 Б][4 Б LE-счётчик]`,
  счётчик `0x80000000 | n`, SRTCP-кадры 60/52 Б, keepalive-магии, ротация сокета +60 с.
- `docs/re/voice-deep.md` §3 — режимы шифрования из лога движка:
  `aead_aes256_gcm_rtpsize` (выбран), `aead_aes256_gcm`, `aead_xchacha20_poly1305_rtpsize`,
  `xsalsa20_poly1305*`. Выбранный режим — `aead_aes256_gcm_rtpsize` ⇒ AAD = RTP-пакет
  (заголовок + payload) целиком.
- `docs/re/voice-deep.md` §5 + `/home/sanya/re-deep/voice/` (только чтение) — разрывы:
  первые 2 пакета talkspurt (43 Б) без валидного расширения и без трейлера-счётчика.
  Контракт W26a обязан это отражать (корректная ошибка, а не паника).
- `docs/re/main-arch.md`, `docs/re/native-re.json` — контекст клиента.

Запрещено: изобретать wire-константы, которых нет в RE; неподтверждённое — `ignored`/ошибка.

## 4. Интеграционный патч TL-2 (сделан до кода)

1. `rust/crates/discord-voice/src/lib.rs` — объявлены `pub mod crypto; pub mod srtp;`.
2. Созданы каталоги-каркасы (пустые модули), чтобы крейт компилировался до прихода воркеров:
   `src/crypto/{mod,aes,gcm,chacha,kdf}.rs`, `src/srtp.rs`.
3. Создан крейт `discord-perf` (`Cargo.toml`, `src/lib.rs`, `src/workload.rs`).
4. Создан крейт `discord-updater` (`Cargo.toml`, `src/lib.rs`, `src/manifest.rs`, `src/plan.rs`).
5. `Cargo.lock` обновлён офлайн (новые пакеты крейтов + `ed25519-dalek` и его дерево).
6. **Baseline-fix `discord-ui`**: clippy-гейт был красным ещё до B06 (39 ошибок в
   скелете GUI-каркаса, заблокированного до решения владельца). Исправлено: модули
   скелета сделаны `pub`, `mod tests` получили `#[cfg(test)]`, добавлены `Default`-имплементации
   для пяти сервисов, `derivable_impls`/`collapsible_if`/`unnecessary_cast`/
   `field_reassign_with_default` вычищены, удалено неиспользуемое поле `next_tray_id`.
   После фикса: `cargo clippy --offline --workspace --all-targets --all-features -- -D warnings`
   — **чисто**, `cargo fmt --all` — чисто.

## 5. Правила для воркеров

- Strict TDD: сначала тест (red, зафиксировать команду и exit code), затем код (green).
- Только свои файлы из §1. `Cargo.toml`/`Cargo.lock`/`src/lib.rs`/доки/`waves/*` — не трогать.
- Никаких сетевых вызовов, `cargo` без `--offline`, `git commit/push`, установки пакетов.
- Никакого доступа к живому Discord (`device.spawn`/attach) и к `logs/`, `functions/`,
  `evidence/` (golden, только чтение). `/home/sanya/re-deep` — только чтение.
- Тесты сверяются с независимым оракулом (KAT-векторы RFC/NIST, RE-константы), а не с
  собственной реализацией; тест под код не «зеленить».
- Каждый воркер: `CARGO_TARGET_DIR=/tmp/opencode/target-<лейн>` (отдельный target-dir —
  чтобы три cargo-процесса не блокировали друг друга; правило «один target-dir» здесь
  читается как «не блокировать друг друга», а не «запретить параллельную сборку»).
- Сдача: отчёт в `waves/RW-25/worker-<лейн>.md` по шаблону `PROCESS.md` §4.2 + red/green-логи
  в `waves/RW-25/red-<лейн>.log`, `waves/RW-25/green-<лейн>.log`.

## 6. Гейт волны

1. `cargo fmt --all -- --check` — 0.
2. `cargo clippy --offline --workspace --all-targets --all-features -- -D warnings` — 0.
3. `cargo test --offline --workspace` — 0 failed (baseline 663 + новые тесты).
4. `npm run gate:worker` — passed.
5. Мутации по новым кейсам (QA-1) — отчёт `MUTATIONS-B06.md`.
6. Независимое ревью (REV-1/REV-2 + codex) — P0/P1/P2 = 0.
7. `accepted` → `waves/db.jsonl` + синхронизация `STATE.md`/`STATUS.md`/`ROADMAP.md`/
   `TASKS.md`/`docs/map.md` — один раз, после приёмки.
