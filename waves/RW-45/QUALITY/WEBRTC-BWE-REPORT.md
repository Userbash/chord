# Отчёт верификации контроля перегрузки сети WebRTC BWE (TASK-982)

- **Статус**: `PASS`
- **Диапазон адаптации битрейта**: `500..8000 kbps`
- **Максимальное снижение за один шаг**: `15.00%` (порог `<= 15.00%`)
- **Сброс соединения (`connectionDropped`)**: `false` (`reconnectCount = 0`)
- **SSRC из оракула `rtc-frames.jsonl`**: video=`17844`, rtx=`17845`, AV1 PT=`101`
- **MD5 модуля `rust/discord-voice/src/congestion.rs`**: `5447cc03a9806dbefa3ceef351bd633e`
