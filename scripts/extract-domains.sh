#!/usr/bin/env bash
# Классификация функций по темам: WebRTC, сеть, RTT, медиа.
#
#   scripts/extract-domains.sh
#
# Читает functions/static/*.nm.txt, деманглит имена (c++filt) и раскладывает
# функции по доменам в functions/domains/<domain>.csv:
#   module,name,demangled,address,size
#
# Классификация эвристическая: по подстрокам в демангленном имени.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
static="$root/functions/static"
out="$root/functions/domains"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

mkdir -p "$out"

# Объединённая таблица: module <tab> name <tab> address <tab> size <tab> demangled
for file in "$static"/*.nm.txt; do
  [[ -s "$file" ]] || continue
  module="$(basename "$file" .nm.txt)"
  awk -v m="$module" 'NF >= 3 && $2 ~ /^[TtWw]$/ { size = ($4 != "" ? $4 : "0"); print m "\t" $1 "\t" $3 "\t" size }' "$file"
done >"$work/all.tsv"
cut -f2 "$work/all.tsv" | c++filt >"$work/demangled.txt"
paste "$work/all.tsv" "$work/demangled.txt" >"$work/joined.tsv"

domains="$work/domains.txt"
cat >"$domains" <<'EOF'
webrtc|[Ww]eb[Rr]tc
rtp-rtcp|Rtp|RTP|rtp_|_rtp|Rtcp|RTCP|rtcp|SRTP|Srtp
ice|(^|[^A-Za-z])ICE([^A-Za-z]|$)|Ice[A-Z]|(^|[^A-Za-z])(ice|ICE)_|icecandidate|icetransport|p2ptransport
stun|[Ss]tun|STUN
turn|[Tt]urn|TURN
sdp|[Ss]dp|SDP|SessionDescription
dtls|[Dd]tls|DTLS
rtt|RTT|Rtt[A-Z]|Rtt$|rtt_|_rtt
jitter|[Jj]itter
latency|[Ll]atency
audio|[Aa]udio|[Oo]pus|OPUS
video|[Vv]ideo|VP8|VP9|H264|H265|AV1|Codec|codec
camera|[Cc]amera|CaptureDevice|VideoCapture|ScreenCast|ScreenShare
network|[Ss]ocket|Network|network|Transport|transport|Connection|connection|Packet|packet|Bandwidth|bandwidth
voice|[Vv]oice|VoiceEngine|MediaConn
krisp-noise|[Kk]risp
EOF

summary="$out/summary.txt"
: >"$summary"

while IFS='|' read -r domain pattern; do
  printf 'module,name,demangled,address,size\n' >"$out/$domain.csv"
  printf 'module\tname\tdemangled\taddress\tsize\n' >"$out/$domain.tsv"
  awk -F'\t' -v pat="($pattern)" '$5 ~ pat { gsub(/"/, "\"\"", $5); printf "%s,%s,\"%s\",%s,%s\n", $1, $2, $5, $3, $4 }' \
    "$work/joined.tsv" >>"$out/$domain.csv"
  awk -F'\t' -v pat="($pattern)" '$5 ~ pat { print $1 "\t" $2 "\t" $5 "\t" $3 "\t" $4 }' \
    "$work/joined.tsv" >>"$out/$domain.tsv"
  count="$(($(wc -l <"$out/$domain.csv") - 1))"
  printf '%-12s %8d\n' "$domain" "$count" | tee -a "$summary"
done <"$domains"

total="$(wc -l <"$work/joined.tsv")"
echo "---" | tee -a "$summary"
echo "всего функций проанализировано: $total" | tee -a "$summary"
echo "сводка: functions/domains/summary.txt"
