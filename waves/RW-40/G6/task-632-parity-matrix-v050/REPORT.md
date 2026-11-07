# Отчёт воркера — TASK-632 (`G6 Tech Writer Lead`): Parity Matrix Wave 42 & Release Notes v0.5.0

## 1. Batch / Карточка
- **Карточка / Задача:** `TASK-632` (`task_6c4d601fa545`, dispatch `ctx_bd5698f8941c`)
- **Отдел:** `G6 Tech Writer / Parity`
- **Ветка:** `docs/task-632-parity-matrix-v050`
- **Write-Scope:**
  - `waves/RW-40/PARITY/MERGE-READINESS-V0.5.0.md`
  - `RELEASE-NOTES-V0.5.0.md`
  - `tests/meta/parity-matrix.test.mjs`
  - `TDD-PLAN.md`

---

## 2. Что сделано
1. **Сверка реализованных функций Спринтов 1–13 (`BUILD-004`, `BUILD-005`, `BUILD-006`, `TASK-601..TASK-630`) с эталонным RE-оракулом `/home/sanya/re-deep/`:**
   - Проведена инвентаризация всех подсистем клиента против артефактов `/home/sanya/re-deep/` (`rest-d02.md`, `frontend-d03.md`, `voice-deep.md`, `gateway-schema.md`, `decoded-index.json`, `backend-ipc.md`, `native-re.md`).
2. **Составлена комплексная матрица паритета `waves/RW-40/PARITY/MERGE-READINESS-V0.5.0.md`:**
   - Описаны **54 позиции паритета** (`53 Implemented = 98.1%`, `1 Partial (In Progress)` — `PM-AUTH-05` `TASK-629` QR RemoteAuth) с разбивкой по всем **8 обязательным доменам**:
     - `1. Voice` (`PM-VOICE-01..08`: RTC WS, UDP discovery/keepalive, SRTP/SRTCP, `TASK-623` DAVE MLS v2, `TASK-615` RTP H.264/AV1 video packetizer, jitter/DTX, audio devices, WebRTC diagnostics);
     - `2. REST` (`PM-REST-01..08`: HTTP/1.1 + `X-Super-Properties`, `TASK-611` `POST /guilds`, `TASK-617` полнотекстовый поиск, `TASK-626` Nitro Gifts & Entitlements, сообщения/реакции, инвайты, вложения, BFF роутер);
     - `3. Gateway` (`PM-GW-01..06`: ETF `0x83`/zstd, IDENTIFY/RESUME, READY, DISPATCH 30 событий, Go Live `op 18..22`, `TASK-619` Permissions Bitfield Engine v9);
     - `4. UI` (`PM-UI-01..10`: 4-экранный каркас + `PageStore`, `TASK-610` `#captcha-modal`, `TASK-612` `#create-guild-dialog`, `TASK-616` `Stream Viewer` + `PiP`, `TASK-618` `#search-popover`, `TASK-620` контекстное меню ПКМ со слайдером `0..200%`, `TASK-627` `#gift-modal`, `TASK-628` `Advanced Settings` (`Keybinds`, `Soundboard`, `Clips`), Markdown/реакции, мосты оболочки);
     - `5. Platform` (`PM-PLAT-01..06`: `TASK-625` `discord://` и `disclip://`, `libsecret` keyring, D-Bus уведомления, питание/`logind` `SCM_RIGHTS`, системный трей + `hunspell`, `TASK-621`/`TASK-622`/`TASK-630` Headless `Xvfb` E2E + `15/15` мутации);
     - `6. RPC` (`PM-RPC-01..05`: `TASK-624` локальный сокет `discord-ipc-0..9`, бинарный фрейминг `16 KiB`, `SET_ACTIVITY` Rich Presence, `INVITE_BROWSER`, защита Tauri IPC);
     - `7. Media` (`PM-MEDIA-01..05`: `TASK-613` PipeWire `ScreenCast` и X11 `MIT-SHM`, `TASK-614` 60 FPS `LatestFrameSlot`, конвертация `BGRA -> I420`, рендер медиа-вложений и `waveform`);
     - `8. Auth` (`PM-AUTH-01..06`: логин/пароль, MFA 5 методов, Captcha-гейт, восстановление сессии из keyring, `TASK-629` QR RemoteAuth, маскирование секретов `<redacted>`).
3. **Составлены релизные ноты `RELEASE-NOTES-V0.5.0.md`:**
   - Подробное описание изменений Спринтов 1–13 (`BUILD-004`..`BUILD-006`), Mermaid-диаграмма архитектуры (12 крейтов Rust + Tauri 2 BFF + 111 модулей WebView UI), карта соответствия RE-оракулам `/home/sanya/re-deep/`, сводка контура качества и таблица системных требований.
4. **Реализован мета-тест `tests/meta/parity-matrix.test.mjs` и обновлён `TDD-PLAN.md` §8:**
   - Экспортированы функции `parseParityRows` и `verifyDomainSummary`, проверяющие наличие и размер документов, все 8 доменов, уникальность ID, ссылки на `/home/sanya/re-deep/`, реальное существование всех упомянутых путей `rust/...` и `tests/...` в репозитории и точное совпадение сводной таблицы §9 с фактическим числом строк.

---

## 3. Red-прогон (Strict TDD)
- **Команда:** `node --test tests/meta/parity-matrix.test.mjs` (до создания `MERGE-READINESS-V0.5.0.md` и `RELEASE-NOTES-V0.5.0.md`)
- **Лог:** `waves/RW-40/G6/task-632-parity-matrix-v050/red.log`
- **Результат:** ожидаемое падение `5 fail / 0 pass` (`AssertionError: waves/RW-40/PARITY/MERGE-READINESS-V0.5.0.md: файл отсутствует`).

---

## 4. Green-прогон и проверки контура качества (`T0`/`T1`)
- **Targeted (`parity-matrix.test.mjs`):** `5/5 pass, 0 fail` (`waves/RW-40/G6/task-632-parity-matrix-v050/green.log`).
- **Typecheck (`npm run typecheck`):** `0 errors` (`tsc -p tsconfig.json && tsc -p tsconfig.agent.json`).
- **Полный мета-сьют (`npm run test:meta`):** `123/123 pass, 0 fail, 0 skipped` (`19` сьютов, включая `plan-sync.test.mjs` и `test-policy.test.mjs`).

---

## 5. Мутационное тестирование (`5/5 = 100% KILLED`)
- **Лог:** `waves/RW-40/G6/task-632-parity-matrix-v050/mutations.log`
  - `M1` (`## 6. RPC` → `## 6. RemoteProcedure` в `MERGE-READINESS-V0.5.0.md`) — **KILLED** (`exit=1`)
  - `M2` (дублирование `PM-VOICE-02` → `PM-VOICE-01` в `MERGE-READINESS-V0.5.0.md`) — **KILLED** (`exit=1`)
  - `M3` (искажение сводной таблицы §9 `54/53/1` → `54/54/0` в `MERGE-READINESS-V0.5.0.md`) — **KILLED** (`exit=1`)
  - `M4` (подмена пути файла `dave.rs` → `dave_missing.rs` в `MERGE-READINESS-V0.5.0.md`) — **KILLED** (`exit=1`)
  - `M5` (удаление раздела `## 6. Системные требования и окружение` в `RELEASE-NOTES-V0.5.0.md`) — **KILLED** (`exit=1`)
- Все файлы побайтно восстановлены после прогона мутаций (`md5` до и после идентичны).

---

## 6. Изменённые и созданные файлы + MD5

| Файл | MD5 | Статус |
|---|---|---|
| `waves/RW-40/PARITY/MERGE-READINESS-V0.5.0.md` | `6ee9fbc42476b52cdaa31056fe5d2424` | Создан |
| `RELEASE-NOTES-V0.5.0.md` | `4f6346de3fe9e72915fac33fc93c808d` | Создан |
| `tests/meta/parity-matrix.test.mjs` | `6f351800f8178ce7d90226f3d7274aae` | Создан |
| `TDD-PLAN.md` | `3254f97ec01450e633401f0e8e00857d` | Обновлён (§8 реестр тестов) |

---

## 7. Риски и примечания
- Блокирующих рисков нет (`P0 = 0`, `P1 = 0`).
- Все ссылки на файлы репозитория и артефакты `/home/sanya/re-deep/` в матрице паритета автоматически верифицируются мета-тестом `tests/meta/parity-matrix.test.mjs`.
