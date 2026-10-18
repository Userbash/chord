#!/usr/bin/env bash
# Человекочитаемый отчёт по доменам: functions/domains/REPORT.md
#
#   scripts/domain-report.sh
#
# Берет functions/domains/*.tsv (их создаёт extract-domains.sh) и пишет
# сводку: сколько функций в домене, какие модули дают вклад, примеры имён.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out="$root/functions/domains"
report="$out/REPORT.md"

{
  echo "# Функции Discord по доменам (WebRTC, сеть, RTT, медиа)"
  echo
  echo "Источник: таблицы символов functions/static/*.nm.txt (115 150 функций),"
  echo "деманглинг — c++filt. Классификация эвристическая (по подстрокам имени),"
  echo "поэтому возможны единичные ложные попадания (например, openssl/zbus в krisp)."
  echo
  echo "| домен | функций |"
  echo "|---|---:|"
  for file in "$out"/*.tsv; do
    domain="$(basename "$file" .tsv)"
    count="$(($(wc -l <"$file") - 1))"
    printf '| %s | %s |\n' "$domain" "$count"
  done
  echo
  for file in "$out"/*.tsv; do
    domain="$(basename "$file" .tsv)"
    count="$(($(wc -l <"$file") - 1))"
    echo "## $domain ($count)"
    echo
    echo "Модули:"
    echo
    echo '```'
    awk -F'\t' 'NR > 1 { c[$1]++ } END { for (m in c) print c[m], m }' "$file" | sort -rn | head -6
    echo '```'
    echo "Примеры функций:"
    echo
    echo '```'
    awk -F'\t' 'NR > 1 { print $3 }' "$file" | grep -E 'webrtc|Opus|G7|G722|V4L2|ScreenCast|Vp8|VP8|Vp9|VP9|H264|H265|AV1|Rtp|Rtp|RTCP|NetEq|CallStats' | sort -u | head -8 || true
    awk -F'\t' 'NR > 1 { print $3 }' "$file" | sort -u | head -3 || true
    echo '```'
    echo
  done
  cat <<'NOTES'
## Наблюдения

- `webrtc::` присутствует целиком: медиа-конвейер Discord — это их форк libwebrtc.
- RTP/RTCP, RTT-статистика (`CallStats::OnRttUpdate`, `RTCPSender::SetNonSenderRttMeasurement`),
  jitter/latency, NetEq (буфер воспроизведения), NACK/FEC, bandwidth/pacer — на месте.
- Кодеки: Opus (аудио), VP8/VP9/H264/H265/AV1 (видео).
- Камера/демонстрация экрана: V4L2 (`VideoCaptureModuleV4L2`), PipeWire (`ScreenCastPortal`).
- Символов ICE/STUN/DTLS/SRTP/cricket нет: в этой сборке P2P-стек WebRTC не используется —
  подключение к голосовым серверам Discord идёт через их собственный транспортный слой.
- `discord_krisp` — ONNX-модель шумодава (onnxruntime внутри); сетевые/ice-совпадения там —
  это openssl/zbus, а не медиа.
NOTES
} >"$report"

echo "Отчёт: $report"
