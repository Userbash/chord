# ТЗ — task-274 (`G7 RE / REA`: `RE-GAP-22` — Voice WebRTC Diagnostics & Connection Stats Overlay)

## Роль и цель
Ты воркер отдела `G7 RE / REA` (read-only аудит эталонного дампа `reference-oracle/`).
Выполни сверку нашего клиента (`rust/crates/discord-voice/`, `rust/gui-spike/src/voice_*.rs`, `rust/gui-spike/ui/features/voice-*.js`) с эталонным дампом `reference-oracle/` (`voice-deep.md`, `voice/`, `frontend-d03.md`, `decoded-index.json`, `ru-catalog.json`) по подсистеме **Voice WebRTC Connection Diagnostics, Ping/Packet-Loss/Jitter Graph & RTC Debug Popout**.

## Разрешённые файлы для записи (Write-Scope)
- `/home/user/orca/project/waves/RW-40/RE-AUDIT/RE-GAP-22-VOICE-DIAGNOSTICS.md`
- `/home/user/orca/project/waves/RW-40/PM/BACKLOG-RE-22.md`
- `/home/user/orca/project/waves/RW-40/G7/task-274-re-gap-22-voice-diagnostics/REPORT.md`

## Требования
1. Найти в `reference-oracle/` точные константы, пороги качества связи (зелёный/жёлтый/красный индикатор пинга и потерь пакетов), структуру `RTCConnection` stats (`ping`, `outboundBitrate`, `inboundBitrate`, `packetsLost`, `jitter`, `codec`, ` transport`, `ssrc`), DOM-селекторы и i18n-ключи попапа диагностики голоса.
2. Оформить матрицу расхождений в `RE-GAP-22-VOICE-DIAGNOSTICS.md` и карточки задач (`BE`, `FE`, `Voice`, `Design`) в `BACKLOG-RE-22.md`.
3. Записать отчёт с `md5sum` в `REPORT.md` и завершить работу через `orca orchestration worker-done`. Не делать `git commit`.
