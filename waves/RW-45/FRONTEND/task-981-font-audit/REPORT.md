# REPORT — TASK-981: Комплексный чекер системных шрифтов (`Fallback Noto Color Emoji, CJK, RTL`) и кэша атласа WGPU (`scripts/font-audit.mjs`)

- **Волна / Карточка:** Wave RW-45 / `TASK-981` (`task_df11266150ab`, dispatch `ctx_9ad2ac3f0597`)
- **Отдел:** `G2 Frontend`
- **Приоритет:** `P1`
- **Статус:** Выполнено (все критерии приёмки DoD 1–3 закрыты на 100%)

---

## 1. Сделано

1. **Подзадача 981.1 — Стенд тестирования системных шрифтов и кэша атласа WGPU [`scripts/font-audit.mjs`](file:///home/sanya/orca/project/scripts/font-audit.mjs):**
   - Реализована классификация кодовых точек Unicode по скриптам (`classifyCodePointScript`: `latin`, `cyrillic`, `emoji`, `cjk`, `rtl_arabic`, `rtl_hebrew`, `symbols_math`, `private_or_unassigned`), парсер `fc-list : family` (`parseFcListFamilies`) и зондирование системных/встроенных шрифтов (`probeSystemFonts`).
   - Реализована сегментация графемных кластеров (`segmentGraphemeClusters`: ZWJ-последовательности `👨‍👩‍👧‍👦`, модификаторы тона кожи `👋🏽`, региональные флаги `🇯🇵`, вариационные селекторы `❤️`, арабский харакат `مَرْحَبًا`) и детерминированное разрешение цепочки fallback (`resolveGlyphFallback`):
     - `Noto Color Emoji` (и fallback `Twemoji`) в цветной атлас `rgba8_unorm_srgb`;
     - `Noto Sans CJK SC/TC/JP/KR` с региональными индексами `.ttc` (`ja=0`, `ko=1`, `zh-CN=2`, `zh-TW=3` по эталону `docs/reference-projects/sources/Serein/crates/ui/src/fonts.rs`) в масочный атлас `r8_unorm`;
     - `Noto Sans Arabic` и `Noto Sans Hebrew` с `direction: "rtl"` и `harfBuzzShaped: true`;
     - Гарантированная подстановка **`Tofurion Fallback`** (`glyphId: 0`, `replacementChar: "\uFFFD"`, `isTofurion: true`, `panicked: false`) для отсутствующих глифов, PUA (`U+E000..U+F8FF`, `U+F0000..U+10FFFD`) и неназначенных скаляров (`U+0378`).
   - Реализован симулятор текстурного атласа глифов WGPU (`WgpuGlyphAtlasSimulator`): Shelf bin-packing с `1px` паддингом, переходы состояний `emoji-picker` (`rendered -> lookup glyph -> atlas cached`), защита от некорректных размеров (`0x0`, отрицательные, `> 8192px`), восстановление после `DeviceLost` и детерминированное LRU-вытеснение при заполнении страниц без паники `wgpu::Error::OutOfMemory`.
   - Выполнена сверка с независимыми оракулами (`verifyFontAuditOracleParity`): [`waves/kanban-board.json`](file:///home/sanya/orca/project/waves/kanban-board.json) (`TASK-981`), [`rust/discord-gpui/src/theme/discord_dark.rs`](file:///home/sanya/orca/project/rust/discord-gpui/src/theme/discord_dark.rs), [`rust/discord-gpui/src/state/mod.rs`](file:///home/sanya/orca/project/rust/discord-gpui/src/state/mod.rs) (`font_scale` `0.75..1.5`), [`rust/gui-spike/ui/tokens.css`](file:///home/sanya/orca/project/rust/gui-spike/ui/tokens.css) и [`docs/re/frontend-d03.json`](file:///home/sanya/orca/project/docs/re/frontend-d03.json) (`emojiPicker_c0e32c`, `emojiItem_fc7141`, `lockedEmoji_d982c2`).

2. **Подзадача 981.2 — Контрактный и модульный тест [`tests/unit/font-fallback-matrix-981.test.mjs`](file:///home/sanya/orca/project/tests/unit/font-fallback-matrix-981.test.mjs) (Strict TDD):**
   - Покрыты все 5 подсекций (`SUB-981-1`..`SUB-981-5`) и все 9 измерений `TEST-POLICY.md` (`evaluateFontAuditEdgeDimensions`), маскирование секретов (`redactFontAuditLogs`), CLI коды выхода `0 / 1 / 2`, лимиты размера кода (`794 < 800` строк) и мутационный гейт `5/5` (`100%` kill rate).

---

## 2. Red-прогон (TDD)

Лог: [`waves/RW-45/FRONTEND/task-981-font-audit/red.log`](file:///home/sanya/orca/project/waves/RW-45/FRONTEND/task-981-font-audit/red.log) (запуск до создания `scripts/font-audit.mjs`):

```text
▶ TASK-981: Комплексный чекер системных шрифтов и кэша атласа WGPU (scripts/font-audit.mjs)
  ✖ SUB-981-1: сверяет дизайн-токены, интерактивные состояния emoji-picker, классификацию скриптов Unicode, fc-list и независимые оракулы (1.069026ms)
  ✖ SUB-981-2: сегментирует графемные кластеры (ZWJ, тон кожи, флаги, харакат) и разрешает fallback для Noto Color Emoji, CJK и RTL (0.265877ms)
  ✖ SUB-981-3: заменяет отсутствующие, PUA и невалидные глифы на tofurion (.notdef / U+FFFD) без паники (0.232727ms)
  ✖ SUB-981-4: симулирует кэш текстурного атласа WGPU (Shelf bin-packing, переходы состояний emoji-picker и LRU-вытеснение без паники) (0.231387ms)
  ✖ SUB-981-5: проверяет матричный аудит, 9 измерений TEST-POLICY.md, CLI scripts/font-audit.mjs (коды 0/1/2), лимиты кода и 5/5 мутаций (0.280096ms)
ℹ tests 5 | suites 1 | pass 0 | fail 5
```

---

## 3. Green-прогон (TDD)

Лог: [`waves/RW-45/FRONTEND/task-981-font-audit/green.log`](file:///home/sanya/orca/project/waves/RW-45/FRONTEND/task-981-font-audit/green.log):

```text
▶ TASK-981: Комплексный чекер системных шрифтов и кэша атласа WGPU (scripts/font-audit.mjs)
  ✔ SUB-981-1: сверяет дизайн-токены, интерактивные состояния emoji-picker, классификацию скриптов Unicode, fc-list и независимые оракулы (19.913971ms)
  ✔ SUB-981-2: сегментирует графемные кластеры (ZWJ, тон кожи, флаги, харакат) и разрешает fallback для Noto Color Emoji, CJK и RTL (3.063649ms)
  ✔ SUB-981-3: заменяет отсутствующие, PUA и невалидные глифы на tofurion (.notdef / U+FFFD) без паники (0.393123ms)
  ✔ SUB-981-4: симулирует кэш текстурного атласа WGPU (Shelf bin-packing, переходы состояний emoji-picker и LRU-вытеснение без паники) (4.176915ms)
  ✔ SUB-981-5: проверяет матричный аудит, 9 измерений TEST-POLICY.md, CLI scripts/font-audit.mjs (коды 0/1/2), лимиты кода и 5/5 мутаций (242.781573ms)
✔ TASK-981: Комплексный чекер системных шрифтов и кэша атласа WGPU (scripts/font-audit.mjs) (271.046655ms)
ℹ tests 5 | suites 1 | pass 5 | fail 0 | skipped 0
```

---

## 4. Targeted & Мутационный гейт (5/5)

- **Targeted (`node --test tests/unit/font-fallback-matrix-981.test.mjs`):** `5/5 pass (0 fail, 0 skip)`.
- **TypeScript (`tsc`):** `0 errors` в `scripts/font-audit.mjs` и `tests/unit/font-fallback-matrix-981.test.mjs`.
- **Мутационный гейт ([`waves/RW-45/FRONTEND/task-981-font-audit/mutation.log`](file:///home/sanya/orca/project/waves/RW-45/FRONTEND/task-981-font-audit/mutation.log)):**
  1. `M1` (`colorFormat: "rgba8_unorm_srgb" -> "r8_unorm"`) — **KILLED**
  2. `M2` (`CJK_TTC_FACE_INDICES["zh-CN"]: 2 -> 0`) — **KILLED**
  3. `M3` (`direction: isRtl ? "rtl" : "ltr" -> "ltr"`) — **KILLED**
  4. `M4` (`Tofurion glyphId: 0 -> 1`) — **KILLED**
  5. `M5` (`WGPU LRU eviction disabled: this.lruEvictions += 0`) — **KILLED**
  - Итог: **5/5 мутаций убито (100% kill rate)**, оригинал файла побайтно восстановлен и проверен по MD5 (`99258d2c78a37cda853e7010821074f7`).

---

## 5. Файлы и MD5

| Файл | MD5 |
|---|---|
| `scripts/font-audit.mjs` | `99258d2c78a37cda853e7010821074f7` |
| `tests/unit/font-fallback-matrix-981.test.mjs` | `22a048a24953364285d9c26414b49670` |
| `waves/RW-45/FRONTEND/task-981-font-audit/font-audit-summary.json` | `a6f1d912577099deef30d53cd145036c` |
| `waves/RW-45/FRONTEND/task-981-font-audit/FONT-AUDIT-SUMMARY.md` | `f5b69e9c6bc61f14cf4d24dc0e4597ca` |
| `waves/RW-45/FRONTEND/task-981-font-audit/red.log` | `3c4b39e5a2122c4c7ddfb10064b941cb` |
| `waves/RW-45/FRONTEND/task-981-font-audit/green.log` | `a3e34c772d9dc674d199ce7e840da54d` |
| `waves/RW-45/FRONTEND/task-981-font-audit/mutation.log` | `e7f0b7cc60386f5fa57b78db577ed91e` |

---

## 6. Риски и замечания

- В средах без установленного системного пакета `fonts-noto-cjk` стенд автоматически переключается на встроенные бандлы `Noto Sans CJK` (`SC/TC/JP/KR`, индексы `.ttc` `0..3`), `Noto Color Emoji` / `Twemoji` и гарантированный `Tofurion Fallback` без паник текстурного атласа WGPU.
- Блокеров нет (`P0 = 0`, `P1 = 0`).
