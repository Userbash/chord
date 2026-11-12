# REPORT — TASK-961: Сборка автономного AppImage пакета и headless-верификация

- **Волна / Карточка:** Wave RW-45 / `TASK-961` (`task_2c4cdea6e62c`, dispatch `ctx_6fc51e343090`)
- **Отдел:** `devops` (`DevOps / Integration`)
- **Приоритет:** `P1`
- **Статус:** Выполнено (все критерии приёмки DoD 1–3 закрыты на 100%)

---

## 1. Сделано

1. **Спецификация точки входа и метаданных `packaging/appimage/`:**
   - Разработан скрипт запуска [`packaging/appimage/AppRun`](file:///home/user/orca/project/packaging/appimage/AppRun): автоматическое вычисление `APPDIR`, экспорт `LD_LIBRARY_PATH` (`$APPDIR/usr/lib`), `XDG_DATA_DIRS`, `GSETTINGS_SCHEMA_DIR`, проверка доступности user namespaces (`unshare -Ur true` / `--no-sandbox` $\to$ `DISCORD_GPUI_SANDBOX`), поддержка софтверного рендеринга `llvmpipe` (`DISCORD_APPIMAGE_SOFTWARE_RENDER=1`) и прямой `exec "$APPDIR/usr/bin/discord-gpui" "$@"` без удержания дочернего shell-процесса в памяти.
   - Интегрированы [`packaging/appimage/discord-gpui.desktop`](file:///home/user/orca/project/packaging/appimage/discord-gpui.desktop) (валидирован системным оракулом `desktop-file-validate`, `MimeType=x-scheme-handler/discord;`, `StartupWMClass=discord-gpui`) и векторная/растровая иконки (`packaging/appimage/discord-gpui.svg`, `.DirIcon`, `usr/share/icons/hicolor/{128x128,scalable}/apps/`).

2. **Скрипт сборки и бандлинга зависимостей [`scripts/build-appimage.sh`](file:///home/user/orca/project/scripts/build-appimage.sh):**
   - Валидация входного 64-битного ELF x86_64 бинарника `rust/dist/discord-gpui`, инспекция `ldd` (отклонение неразрешённых символов и проприетарных драйверов NVIDIA/AMD).
   - Бандлинг разделяемых библиотек (`libxkbcommon.so.0`, `libxkbcommon-x11.so.0`, `libxcb-xkb.so.1`, `libXau.so.6`, `libXdmcp.so.6`) в `AppDir/usr/lib/` с генерацией манифеста `bundled-deps.json` и сохранением системного ABI для базовых библиотек (`libc.so.6`, `libm.so.6`, `libgcc_s.so.1`, `ld-linux-x86-64.so.2`, `libasound.so.2`, `libxcb.so.1`).
   - Компиляция нативного ELF64 рантайма Type 2 AppImage (`\x7fELF\x02\x01\x01` + сигнатура `AI\x02` на смещении `8..11`, выравнивание образа по 4096 байт) с поддержкой `--appimage-extract`, `--appimage-extract-and-run`, `--appimage-offset`, `--appimage-updateinformation`, `--appimage-version`.
   - Сформирован релизный пакет `rust/dist/discord-gpui-v1.0.0-beta.1-x86_64.AppImage` (`17 299 078` байт, SHA-256: `7b7a415ab0a8daaac77c21f7faccc738c70104db4445197676d35219410c2b8c`).

3. **Верификация `--headless-smoke` внутри AppImage и контрактный тест [`tests/unit/appimage-package-961.test.mjs`](file:///home/user/orca/project/tests/unit/appimage-package-961.test.mjs):**
   - Выполнен холодный запуск `rust/dist/discord-gpui-v1.0.0-beta.1-x86_64.AppImage --headless-smoke`: `status: "ok"`, `sqlite_wal_ok: true`, `fps_budget_ok: true`, потребление памяти **`VmRSS = 11.50 MB` (`11 780 KB < 60 MB`)**.

---

## 2. Red-прогон (TDD)

Лог: `waves/RW-45/DEVOPS/task-961-appimage/red.log`

```text
▶ TASK-961: Сборка автономного AppImage пакета и headless-верификация
  ✖ SUB-961-1: спецификация packaging/appimage/AppRun, desktop-файл и иконка валидны и поддерживают sandbox/env контракты (0.804239ms)
  ✖ SUB-961-2: scripts/build-appimage.sh собирает валидный Type 2 AppImage с бандлингом зависимостей, иконкой и .desktop файлом (0.241422ms)
  ✖ SUB-961-3: выполняет --headless-smoke внутри релизного AppImage (VmRSS < 60 MB) и проверяет fallback/негативные режимы (0.202522ms)
ℹ tests 3 | suites 1 | pass 0 | fail 3
```

---

## 3. Green-прогон (TDD)

Лог: `waves/RW-45/DEVOPS/task-961-appimage/green.log`

```text
▶ TASK-961: Сборка автономного AppImage пакета и headless-верификация
  ✔ SUB-961-1: спецификация packaging/appimage/AppRun, desktop-файл и иконка валидны и поддерживают sandbox/env контракты (9.888538ms)
  ✔ SUB-961-2: scripts/build-appimage.sh собирает валидный Type 2 AppImage с бандлингом зависимостей, иконкой и .desktop файлом (1905.400062ms)
  ✔ SUB-961-3: выполняет --headless-smoke внутри релизного AppImage (VmRSS < 60 MB) и проверяет fallback/негативные режимы (1691.247051ms)
ℹ tests 3 | suites 1 | pass 3 | fail 0 | skipped 0
```

---

## 4. Targeted, Typecheck & Мутационный гейт (5/5)

- **Контрактный тест (`node --test tests/unit/appimage-package-961.test.mjs`):** `3/3 pass (0 fail, 0 skip)`.
- **Typecheck (`npx tsc -p tsconfig.json`):** `0 errors`.
- **Мутационный гейт (5/5 мутаций):**
  1. `M1` (`packaging/appimage/AppRun`: искажение целевого бинарника в `exec`) — **KILLED**
  2. `M2` (`packaging/appimage/discord-gpui.desktop`: удаление URI-схемы `x-scheme-handler/discord`) — **KILLED**
  3. `M3` (`scripts/build-appimage.sh`: искажение сигнатуры Type 2 `AI\x02` $\to$ `AI\x01`) — **KILLED**
  4. `M4` (`scripts/build-appimage.sh`: исключение `libxkbcommon.so.0` из бандлинга `usr/lib/`) — **KILLED**
  5. `M5` (`scripts/build-appimage.sh`: отключение проверки верхнего порога `vmrss_mb >= max_rss_mb`) — **KILLED**
  - Результат: **5/5 убито (100% kill rate)**, все исходные файлы побайтно восстановлены и сверены по MD5.

---

## 5. Файлы и MD5

| Файл | MD5 |
|---|---|
| `packaging/appimage/AppRun` | `a64a70b109a52ea55a35380b6e3543ee` |
| `packaging/appimage/discord-gpui.desktop` | `dbd96346b35cd467ffc8518d9a38c644` |
| `packaging/appimage/discord-gpui.svg` | `326906a5f84c4c481307e42a1f73d1a9` |
| `scripts/build-appimage.sh` | `b6bbf17b43c2b3180abf557c28a06b51` |
| `tests/unit/appimage-package-961.test.mjs` | `13cce5f2b03ead4ba70df84d11604fca` |
| `rust/dist/discord-gpui-v1.0.0-beta.1-x86_64.AppImage` | `1fde2bd1cb72b1a447241cff42d4afc4` |

---

## 6. Риски и замечания

- Рисков и регрессий нет (`P0 = 0`, `P1 = 0`).
- Общий файл `rust/dist/SHA256SUMS` не изменялся напрямую для исключения конфликтов с параллельными задачами упаковки; контрольная сумма записана в sidecar-файл `rust/dist/discord-gpui-v1.0.0-beta.1-x86_64.AppImage.sha256` и JSON-отчёт `waves/RW-45/DEVOPS/task-961-appimage/appimage-build-report.json`.
