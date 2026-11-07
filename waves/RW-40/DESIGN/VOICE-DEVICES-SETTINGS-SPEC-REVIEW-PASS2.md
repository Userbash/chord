# Review: G3-VOICE-DEVICES-SETTINGS-SPEC (Pass 2)

Вердикт: **PASS** (P0: 0, P1: 0, P2: 0)

Независимое read-only ревью выполнено 2026-10-30. Продуктовый код, тесты и
спецификация в рамках проверки не изменялись.

## Проверка исправлений

### P1 — hot-plug fallback reconciliation: PASS

Спецификация §3.2 (строки 361–373) теперь явно разделяет два контура:

- `VOICE.md` §3.2 / `voice-spec.json` `autoswitch: false`: hot-plug не вызывает
  `service.selectInput`/`service.selectOutput`; фактическое переключение возможно
  только по явному выбору пользователя.
- `VoiceDevicesState::update_device_list` и локальный `<select>` вычисляют
  `fallbackId` (`isDefault`, затем первое устройство) только как локальный
  предвыбранный кандидат, эмитят `DeviceFallback`/`NoDeviceAvailable` и не меняют
  аудиосервис автоматически. Для активного звонка отдельно зафиксировано
  `#voice-panel[data-state="device-missing"]` без разрыва разговора.

Матрица сценариев §3.2 и проверки `T-V07-UI-05`/`T-V07-UI-06` также фиксируют
нулевое число вызовов `service.select*`, `aria-disabled` и корректные `status` /
`alert` live-регионы. Противоречие между fallback-моделью и `autoswitch: false`
устранено.

### P2 — VAD threshold и sensitivity mapping: PASS

§1.1 и §3.4 маркируют `vadThresholdDb` как authored adaptation `[A]`, отдельно
от прямого дампового контрола чувствительности `0..=100`. Нормативно заданы обе
стороны отображения:

`thresholdDb = clamp_i8(round(sensitivityPercent) - 100, -128, 127)`

и

`sensitivityPercent = clamp(vadThresholdDb + 100, 0, 100)`.

Указаны диапазоны Rust `-128..=127`, шкала оракула `0..=100`, значение дампа
`26.109%`, дефолт `-40 дБ` и поведение авто-чувствительности через
`aria-disabled`. Расхождение с `voice-spec.json` теперь объяснено и
детерминированно сопоставлено.

### P2 — ARIA-группы и PTT blocked state: PASS

В каноническом DOM (§2.2) `#voice-devices-vad-settings` и
`#voice-devices-ptt-settings` имеют `role="group"` и доступные `aria-label`;
это повторно закреплено в таблице §5.1 и контракте `T-V07-UI-08`.

§3.5 и §5.1/§5.2 полностью описывают `blocked`: потеря фокуса или недоступность
`GlobalShortcuts` отменяет активный захват, выставляет
`data-ptt-status="blocked"` (и `#voice-panel[data-ptt="blocked"]`), запрещает
передачу вне фокуса и публикует локализованное сообщение `vxfv7v` в
`#voice-devices-notice` с `role="alert"`, `aria-live="assertive"`; возврат
фокуса/портала снимает блокировку и скрывает предупреждение. Контракт
`T-V07-UI-09` фиксирует эти переходы.

## Итог

Все P1/P2 замечания из `VOICE-DEVICES-SETTINGS-SPEC-REVIEW.md` имеют явные,
согласованные и проверяемые требования в актуальной спецификации. Дополнительные
прогоны `cargo`/тестов не выполнялись: это независимое документное ревью.

