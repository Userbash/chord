# REPORT — TASK-990: Кросс-компиляция и пакеты для Linux ARM64 (aarch64, Raspberry Pi 5, Asahi)

- **Волна / Карточка:** Wave RW-46 / `TASK-990` (`task_f9789a3ea9aa`, dispatch `ctx_866753838922`)
- **Отдел:** `devops` (`DevOps / Integration`)
- **Приоритет:** `P2`
- **Статус:** Выполнено (все критерии приёмки DoD 1–3 закрыты на 100%, 5/5 мутаций убиты)

---

## 1. Сделано

1. **Конфигурация кросс-компиляции Cargo для `aarch64-unknown-linux-gnu` ([`packaging/arm64/cargo-config.toml`](file:///home/user/orca/project/packaging/arm64/cargo-config.toml), [`rust/dist/cargo-config-aarch64.toml`](file:///home/user/orca/project/rust/dist/cargo-config-aarch64.toml)):**
   - Настроены секции `[build]` (`target = "aarch64-unknown-linux-gnu"`), `[target.aarch64-unknown-linux-gnu]` (`linker = "clang"`, `ar = "llvm-ar"`, `rustflags` с `-fuse-ld=lld`, `-Wl,--build-id=sha1`, `-Wl,-z,relro,-z,now,-z,noexecstack`, `-Wl,-z,max-page-size=16384,-z,common-page-size=16384`, `target-feature=+neon,+crc,+aes,+sha2`) и `[env]` (`CC_aarch64_unknown_linux_gnu`, `CXX_aarch64_unknown_linux_gnu`, `AR_aarch64_unknown_linux_gnu`, `PKG_CONFIG_ALLOW_CROSS="1"`, `PKG_CONFIG_SYSROOT_DIR="/usr/aarch64-linux-gnu"`).
   - Реализована поддержка трёх аппаратных профилей ARM64 (`--profile generic|rpi5|asahi`):
     - `generic`: `target-cpu=generic` (`-march=armv8-a+simd+crc+crypto`)
     - `rpi5`: `target-cpu=cortex-a76` (`-mcpu=cortex-a76+crc+crypto`, Raspberry Pi 5 Broadcom BCM2712)
     - `asahi`: `target-cpu=apple-m1` (`-mcpu=apple-m1`, Apple Silicon Asahi Linux M1/M2/M3 с 16 КБ страницами памяти)

2. **Скрипт кросс-сборки и упаковки [`scripts/build-arm64.sh`](file:///home/user/orca/project/scripts/build-arm64.sh):**
   - Генерация нативного 64-битного бинарника ELF64 `aarch64` (`rust/dist/discord-gpui-aarch64`, `e_machine = 183 / 0xb7 = EM_AARCH64`, `ET_DYN` PIE, интерпретатор `/lib/ld-linux-aarch64.so.1`, выравнивание `PT_LOAD` `0x4000 = 16384` байт для ядер 16K страниц Raspberry Pi 5 и Asahi Linux, `GNU_RELRO` + `BIND_NOW`, `GNU_STACK` RW, `NT_GNU_BUILD_ID`, `stripped`).
   - Сборка релизного архива `rust/dist/discord-gpui-v1.0.0-beta.1-linux-aarch64.tar.gz` (содержит `discord-gpui`, `LICENSE-MIT`, `LICENSE-APACHE`, `sbom.spdx.json`, `BETA-TESTING-GUIDE.md`, `RELEASE-NOTES-v1.0.0-beta.1.md`, `cargo-config-aarch64.toml`).
   - Сборка пакета Debian/Ubuntu `rust/dist/discord-gpui_1.0.0-beta.1_arm64.deb` (`Architecture: arm64`, зависимости `libc6 (>= 2.31)` и `libpipewire-0.3-0`, `DEBIAN/md5sums`, `postinst`, `postrm`).
   - Сборка автономного пакета Type 2 ARM64 AppImage `rust/dist/discord-gpui-v1.0.0-beta.1-aarch64.AppImage` (ELF64 `EM_AARCH64` рантайм + сигнатура `AI\x02` на смещении `8..11` + выровненный по 4096 байт образ `AppDir` + трейлер `AI2PAYLOAD`).
   - Обновление и самопроверка контрольных сумм в [`rust/dist/SHA256SUMS`](file:///home/user/orca/project/rust/dist/SHA256SUMS) (`sha256sum -c SHA256SUMS`).

3. **Контрактный тест [`tests/unit/arm64-packaging-990.test.mjs`](file:///home/user/orca/project/tests/unit/arm64-packaging-990.test.mjs):**
   - Охватывает 5 подзадач (`SUB-990-1` .. `SUB-990-5`) и 9 измерений `TEST-POLICY.md` с независимыми бинарными и системными оракулами (`python3 tomllib`, `readelf -h -l -d -n`, `file`, `tar`, `ar`, `awk`, `sha256sum -c`).

---

## 2. Red-прогон (TDD)

Лог: [`waves/RW-46/DEVOPS/task-990-arm64-packaging/red.log`](file:///home/user/orca/project/waves/RW-46/DEVOPS/task-990-arm64-packaging/red.log)

```text
▶ TASK-990: Кросс-компиляция и пакеты для Linux ARM64 (aarch64, Raspberry Pi 5, Asahi)
  ✖ SUB-990-1: скрипт scripts/build-arm64.sh и конфигурация Cargo для aarch64-unknown-linux-gnu валидны и поддерживают профили generic, rpi5, asahi (0.725472ms)
  ✖ SUB-990-2: генерирует валидный 64-битный ELF aarch64 (EM_AARCH64=183, 16K page alignment, Full RELRO, stripped) по оракулам readelf и file (0.180056ms)
  ✖ SUB-990-3: собирает и верифицирует ARM64 пакеты .tar.gz, .deb (Architecture: arm64) и Type 2 .AppImage (EM_AARCH64) (1.189881ms)
  ✖ SUB-990-4: контрольные суммы всех ARM64 артефактов записаны в rust/dist/SHA256SUMS и проходят sha256sum -c и --verify (0.301003ms)
  ✖ SUB-990-5: проверяет изолированную сборку профилей rpi5/asahi, негативные/граничные сценарии CLI (коды 0/1/2), повреждение артефактов и маскирование секретов (2.215306ms)
ℹ tests 5 | suites 1 | pass 0 | fail 5
```

---

## 3. Green-прогон (TDD)

Лог: [`waves/RW-46/DEVOPS/task-990-arm64-packaging/green.log`](file:///home/user/orca/project/waves/RW-46/DEVOPS/task-990-arm64-packaging/green.log)

```text
▶ TASK-990: Кросс-компиляция и пакеты для Linux ARM64 (aarch64, Raspberry Pi 5, Asahi)
  ✔ SUB-990-1: скрипт scripts/build-arm64.sh и конфигурация Cargo для aarch64-unknown-linux-gnu валидны и поддерживают профили generic, rpi5, asahi (30.160131ms)
  ✔ SUB-990-2: генерирует валидный 64-битный ELF aarch64 (EM_AARCH64=183, 16K page alignment, Full RELRO, stripped) по оракулам readelf и file (6.49014ms)
  ✔ SUB-990-3: собирает и верифицирует ARM64 пакеты .tar.gz, .deb (Architecture: arm64) и Type 2 .AppImage (EM_AARCH64) (16.285223ms)
  ✔ SUB-990-4: контрольные суммы всех ARM64 артефактов записаны в rust/dist/SHA256SUMS и проходят sha256sum -c и --verify (165.518802ms)
  ✔ SUB-990-5: проверяет изолированную сборку профилей rpi5/asahi, негативные/граничные сценарии CLI (коды 0/1/2), повреждение артефактов и маскирование секретов (372.004354ms)
ℹ tests 5 | suites 1 | pass 5 | fail 0 | skipped 0
```

---

## 4. Targeted, Regression & Мутационный гейт (5/5)

- **Targeted (`node --test tests/unit/arm64-packaging-990.test.mjs`):** `5/5 pass (0 fail, 0 skip)`.
- **Регрессионный прогон всех пакетов (`958`, `961`, `962`, `963`, `964`, `965`, `966`, `967`, `968`, `969`, `990`):** `51/51 pass (0 fail, 0 skip)`.
- **Мутационный гейт (5/5 мутаций, лог `waves/RW-46/DEVOPS/task-990-arm64-packaging/mutation.log`):**
  1. `M1` (`scripts/build-arm64.sh`: искажение `target-cpu` для профиля `rpi5` `cortex-a76` $\to$ `cortex-a53`) — **KILLED**
  2. `M2` (`scripts/build-arm64.sh`: искажение константы архитектуры `EM_AARCH64` в валидаторе `183` $\to$ `62`) — **KILLED**
  3. `M3` (`scripts/build-arm64.sh`: уменьшение выравнивания страниц памяти в конфигурации Cargo `16384` $\to$ `4096`) — **KILLED**
  4. `M4` (`scripts/build-arm64.sh`: искажение магической сигнатуры Type 2 AppImage `AI\x02` $\to$ `AI\x01`) — **KILLED**
  5. `M5` (`scripts/build-arm64.sh`: отключение маскирования секретных токенов `mfa.*` в отчётах) — **KILLED**
  - Итог: **5/5 убито (100% kill rate)**, исходный файл побайтно восстановлен и сверен по MD5.

---

## 5. Файлы и MD5

| Файл | MD5 |
|---|---|
| `scripts/build-arm64.sh` | `cbeb21916d347f77657b26c714fa7c22` |
| `tests/unit/arm64-packaging-990.test.mjs` | `9d10b54ea0e0f745c06e0596c59f5bcf` |
| `packaging/arm64/cargo-config.toml` | `fd4c35f8ab7c1b1546809789d2373eb0` |
| `rust/dist/cargo-config-aarch64.toml` | `fd4c35f8ab7c1b1546809789d2373eb0` |
| `rust/dist/discord-gpui-aarch64` | `fbfca1bbe77b9fc219e37ae43790a64d` |
| `rust/dist/discord-gpui-v1.0.0-beta.1-linux-aarch64.tar.gz` | `2691a6acbb9ea5f81e35cccdbca6eee5` |
| `rust/dist/discord-gpui_1.0.0-beta.1_arm64.deb` | `c62cb0c224dc326da8346e8702de2b7c` |
| `rust/dist/discord-gpui-v1.0.0-beta.1-aarch64.AppImage` | `250c7803b1922d1f0ceab316f8c0b3e8` |
| `rust/dist/SHA256SUMS` | `32430b0df10089e7be272327bafecf52` |

---

## 6. Риски и замечания

- Рисков и регрессий нет (`P0 = 0`, `P1 = 0`). Все существующие записи в `rust/dist/SHA256SUMS` сохранены, `sha256sum -c SHA256SUMS` проходит без ошибок.
