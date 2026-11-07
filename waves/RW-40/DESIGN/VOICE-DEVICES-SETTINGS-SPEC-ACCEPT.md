# Приёмка дизайн-спецификации: `RW-40` / `G3-VOICE-DEVICES-SETTINGS-SPEC` (Pass 2)

- **Статус**: `ACCEPTED` (Design Spec #5)
- **Дата**: 2026-10-30
- **Координатор**: Team Lead / PM (`discord-lab`)
- **Спецификация**: `waves/RW-40/DESIGN/VOICE-DEVICES-SETTINGS-SPEC.md`
- **Независимое ревью**: `waves/RW-40/DESIGN/VOICE-DEVICES-SETTINGS-SPEC-REVIEW-PASS2.md` — `PASS` (`P0=0, P1=0, P2=0`)

## Проверенные инварианты

1. **Hot-plug fallback (`P1` закрыт)**: разведены локальный предвыбор `VoiceDevicesState::update_device_list` (`DeviceFallback`/`NoDeviceAvailable`) и запрет автопереключения `service.selectInput`/`service.selectOutput` (`autoswitch: false` по `VOICE.md` §3.2 / `voice-spec.json`).
2. **Шкала чувствительности VAD (`P2` закрыт)**: зафиксировано взаимно-однозначное отображение `thresholdDb (-128..=127)` ↔ `sensitivityPercent (0..=100)`.
3. **Семантика ARIA-групп и PTT blocked state (`P2` закрыт)**: `#voice-devices-vad-settings` и `#voice-devices-ptt-settings` имеют `role="group"`, состояние `data-ptt-status="blocked"` (`vxfv7v`) специфицировано.
4. **CSS custom properties**: все 55 токенов `var(--*)` присутствуют в `rust/gui-spike/ui/tokens.css`.
