# RW-39 — PM DECISIONS

Дата: 2026-10-24. PM: DeepSeek. Основание: `TEST-POLICY.md` §5–§6, `waves/RW-39/TZ.md`,
`waves/RW-39/DESIGN/DOM-CONTRACT.md` (актуальный md5 `a564f36f…`).

## D-39-01 — аддитивное обновление DOM-харнесса под новый shell

- **Контекст:** `tests/support/gui-dom-harness.mjs::buildDom` собирает DOM вручную; новый
  shell (`#guild-rail`, `#channel-tree`, `#chat-header`, `#composer`, `#member-panel`,
  `#user-bar`) требует добавить id/классы в шим. Иначе DOM-тесты не увидят разметку.
- **Решение:** разрешить **аддитивную** правку `buildDom` строго по `DOM-CONTRACT.md` §3
  (29 id / 76 классов). Запрещено менять/удалять существующие assert'ы, пороги и ожидания.
- **Независимый источник:** `DOM-CONTRACT.md` §3 + `SPEC.md` §9; RE-замеры W39a.
- **Компенсирующий контроль:** после правки — `node --test tests/unit/gui-spike-dom.test.mjs`
  и `tests/unit/channel-paging.test.mjs`; диф тестов проверяет REV-1 (не автор); при
  регрессе — откат правки шима.
- **Риск:** дрейф шима от `index.html`; контроль — тест «шим-разметка» + code review.

## D-39-02 — ширины shell: 240 по умолчанию, 311/256 на ≥1600px

- RE-замер: tree 311 (375.2−64), members 256. ТЗ/SPEC: 240/240 по умолчанию.
- **Решение:** 240 — базовый брейкпоинт; 311/256 включаются на `min-width: 1600px`;
  оба значения — в токенах. Визуальная сверка на 1280×820 и ≥1600.
- **Риск:** расхождение с референсом на узком окне; контроль — скриншот/AT-SPI + SPEC.

## D-39-03 — трек сообщения 44px сохраняется в Phase 2

- RW-38 тесты фиксируют `rowHeight=44`, `top=index*44`, `sizer=total*44`.
- RE-плотность 26/48 требует prefix-sum виртуализации и обновления 8+ ассертов.
- **Решение:** в Phase 2 оставить 44px (контракт RW-38 не ломаем); плотность 26/48 —
  отдельная карточка **P3-02b** с prefix-sum и независимыми тестами. Это не ослабление:
  текущий контракт сохранён, новая плотность не заявлена принятой.
- **Риск:** визуальная плотность отличается от референса; контроль — SPEC §12, P3-02b.

## D-39-04 — `window-runs.jsonl` как журнал live-прогонов

- **Решение:** каждая успешная AT-SPI/X11-проба пишет строку в `waves/window-runs.jsonl`
  (ts, pid, window_title, xid, png, unique_colors, snapshot, emoji_cells, settings_toggles, log).
  Файл append-only; используется как evidence по `TEST-POLICY` §4.
- **Контроль:** `scripts/gui-window-probe.py` пишет; `waves/RW-38`/`RW-39` отчёты ссылаются.

## D-39-05 — live-keyring skip в worker-гейте

- `discord-platform/tests/keyring_contract.rs` — 12 `#[ignore]` с причиной
  `live-keyring: нужен DISCORD_LAB_KEYRING_LIVE=1 и org.freedesktop.secrets`.
- **Решение:** worker-гейт их не запускает (skip с причиной, headless); release/qualify
  запускает `keyringLivePhase`. Доказательство: с `DISCORD_LAB_KEYRING_LIVE=1 … --ignored`
  — **12/12 pass**. Это не «приёмка мимо тестов»: live-проверка выполнена и записана.

## D-39-06 — токены: актуальный md5

- `tokens.css` регенерирован: md5 **`7ce9c1f03e26056243e13df3237b0a9d`** (205 токенов).
  `tokens.json` производный: **`12be8c11a82abbe97ba544505ce75ccc`**.
- W39b-отчёт ссылался на `62fffb9c…` — устарел; Phase 2 использует текущий `tokens.css`.
  Любая правка `tokens.css` требует `gen-tokens.mjs` и обновления ссылок в SPEC/README.

## D-144. Модельная политика воркеров (2026-10-24)

Владелец: все воркеры — OpenCode `deepseek/deepseek-flash` (DeepSeek V4.1 Flash),
Codex — только code review. Codex-сессии, запущенные ранее для G6/QA/G5/G3, остановлены;
их отчёты G5/G3 помечены как draft и перепроверяются OpenCode DeepSeek.
Текущий WIP: 1 writer (L-02-CORE-FINAL) + 5 read-only (G2/G6/QA/G3/G5).
Code-concurrency остаётся 1 до baseline commit; один cargo.

## D-145. Текущий WIP 1+5, все воркеры OpenCode DeepSeek (2026-10-24)

Активны: L-02-CORE-FINAL (writer) + G2 UI contract + G6 merge audit + QA-E2E prep
+ G3 G-05 validate + G5 voice validate. Все read-only используют
`opencode run -m deepseek/deepseek-flash`; Codex не запущен.
Codex будет использован только для code review L-02/G2 diff после отчёта.
Code-concurrency 1 до baseline commit; cargo один.

## D-144. Все активные воркеры — OpenCode DeepSeek V4.1 Flash (2026-10-24)

Codex остановлен для не-code задач. Активны 6 OpenCode-воркеров:
L-02-CORE-FINAL (writer), G2-UI-CONTRACT, G6-MERGE-AUDIT, QA-E2E-PREP,
G3-G05-VALIDATE, G5-VOICE-VALIDATE. Модель: `deepseek/deepseek-flash`.
Codex будет запущен только на review кода L-02 после отчёта.
Code-concurrency 1 до baseline commit; cargo один.

## D-146. Финальный WIP 1 writer + 5 read-only, OpenCode DeepSeek (2026-10-24)

Активны: L-02-CORE-FINAL (writer), G2 UI contract, G6 merge audit, QA-E2E prep,
G3 G-05 validate, G5 voice validate. Все read-only запущены через
`opencode run -m deepseek/deepseek-flash`; Codex не активен.
Codex будет использован только для code review после отчёта L-02.

## D-147. Новая волна: 2 writer + 5 read-only + Codex review (2026-10-24)

После L-02-CORE-FINAL освободился writer. Запущены:
- G2 L-02-UI-IMPL (P1A, writer, `ui/**` + `dispatch_ui.test.mjs`);
- G1 C-02-BEFORE (P1B, writer, `data.rs`/`messages.rs`);
- G6 re-parity, G4 QA matrix, G3 voice panel, G5 opus KAT, G4b SEC-L02 — read-only;
- Codex REVIEW-L02-CORE — только code review, без правок.

Исключение из «один writer»: два writer в одном checkout с непересекающимися файлами
(`ui/**` vs `data.rs/messages.rs`), cargo только у G1; остальные read-only.
Приоритет: L-02-UI (P1A) выше C-02-BEFORE (P1B); Codex review не блокирует UI.

## D-148. POOL-02: 3 writer + 4 read-only + Codex review (2026-10-24)

Открыт новый пул `POOL-02.md`. Приоритеты: FIX-L02-CORE, SEARCH-FIX, QA-GATE-SKIPS,
SEC-L02, G6-REPARITY-03 — P1A; C-02-UI-OLDER/P-03/M-07/G-01b — P1B.
Активны 3 writer (G1 core, G2 search, G4 gate) с непересекающимися файлами,
4 read-only (G4b/G6/G3/G5) и Codex review L-02-UI. Один cargo.
Writer-слоты после завершения: G1 → PLAT-11a/приёмка; G2 → C-02-UI-OLDER → P-03;
G4 → QA-M2-02/04/05/07.

## D-149. POOL-03: 4 writer + 3 read-only + Codex review (2026-10-24)

Приоритеты: L02-UI-FIX, REVIEW-L02-CORE-FIX, QA-M2-02, QA-M2-04, SEC-L02,
G6-REPARITY-04 — P1A; M-07-BE, C-02-UI-OLDER, P-03-VERIFY, G-01b — P1B.
Активны 4 writer с непересекающимися файлами:
G2 `ui/**`; G1 `data.rs`; G4 `tests/e2e/**`; G5 `.github/workflows/**`.
Read-only: G4b/G6/G3. Codex — review FIX-L02-CORE. Один cargo (G1).
Next: G2 C-02-UI-OLDER; G1 PLAT-11a; G4 QA-M2-05/07; G5 ALSA; G3 DP-02/STREAM.

## D-150. POOL-04: close-to-goal снимок и следующая волна (2026-10-24)

Offline матрица: 21 implemented / 36 partial / 34 absent, live verified 0; parity deferred.
Merge = block: gate red (tsc 16, plan-sync 2), e2e 0/7, accepted set 2 среза + 10 без ACCEPT.
Активны 4 writer (G2 chat.js, G1 settings_store, G4 tests/e2e, G5 CI) + G4b/G6/G3 read-only
+ Codex review L-02-UI-FIX. Один cargo. Следующая цель — merge readiness, затем live/voice.

## D-151. POOL-05: очередь на освобождение слотов (2026-10-24)

Создан `POOL-05.md`: P1A — L-02 UI/CORE accept, gate-green (16 tsc + 2 plan-sync),
QA-M2-05/07, SEC fix, merge-06, QA-DEDUP. P1B — M-07 UI, G-01b UI, P-03 verify,
C-07, G-05, live reactions/invites. P2 — V-01 impl, Opus/ALSA, ST-01, STREAM.
Текущая волна 4 writer + 3 read-only + Codex review; новые задачи выдаются по
завершении, без простоя. P0 live остаётся blocked окном.

## D-152. POOL-06: mutation/gate/merge wave (2026-10-24)

После push `4e24048` активны: L02-UI-MUTATION (G2), TSC-FIX (G4), G-01b contract (G1),
SEC-L02 (G4b), QA-M2-04 verify (G5), MERGE-READINESS-06 (G6), G3-STREAM-FIX (G3),
Codex review C02 UI. Цель — mutation evidence, tsc 0, gate-green, accepted set и merge.
Один cargo (G1); writer-файлы не пересекаются. P0 live по-прежнему blocked окном.
