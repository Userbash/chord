#!/usr/bin/env bash
# D-02: независимая проверка чисел отчёта (jq/awk/md5sum — вне кода разбора).
# Запуск: bash /home/sanya/re-deep/rest-d02-verify.sh
set -u
cd /home/sanya/re-deep

SNAP_R2=d02-snapshot/capture-run2.snap.jsonl
SNAP_RD=d02-snapshot/capture-ready.snap.jsonl

echo "== 0. снимок =="
md5sum "$SNAP_R2" "$SNAP_RD"
wc -l "$SNAP_R2" "$SNAP_RD"

echo
echo "== 1. события по типам =="
jq -r '.ev' "$SNAP_R2" "$SNAP_RD" | sort | uniq -c | sort -rn

echo
echo "== 2. req-события и уникальные requestId по файлам =="
for f in "$SNAP_R2" "$SNAP_RD"; do
  echo -n "$f req="; jq -r 'select(.ev=="req")|.requestId' "$f" | wc -l
  echo -n "$f req_uniqid="; jq -r 'select(.ev=="req")|.requestId' "$f" | sort -u | wc -l
done

echo
echo "== 3. API-запросы: количество и уникальные маршруты (нормализация {id}) =="
jq -r 'select(.ev=="req") | .url' "$SNAP_R2" "$SNAP_RD" \
  | grep -E '^https://discord\.com/api/v9/' \
  | grep -v '/science$' \
  | sed -E 's/\?.*$//; s#/[0-9]{16,21}#/{id}#g' \
  | wc -l
jq -r 'select(.ev=="req") | .method + " " + .url' "$SNAP_R2" "$SNAP_RD" \
  | grep -E ' https://discord\.com/api/v9/' \
  | grep -v '/science$' \
  | sed -E 's/https:\/\/discord\.com\/api\/v9//; s/\?.*$//; s#/[0-9]{16,21}#/{id}#g' \
  | sort -u | wc -l

echo
echo "== 4. версии API в путях (должно быть только v9) =="
jq -r 'select(.ev=="req") | .url' "$SNAP_R2" "$SNAP_RD" \
  | grep -oE '/api/v[0-9]+' | sort | uniq -c

echo
echo "== 5. Authorization: всего и по регистру ключа =="
jq -r 'select(.ev=="req") | (.headers // {}) | to_entries[] | select(.key|ascii_downcase=="authorization") | .key' "$SNAP_R2" "$SNAP_RD" \
  | sort | uniq -c
echo -n "authorization total="
jq -r 'select(.ev=="req") | (.headers // {}) | to_entries[] | select(.key|ascii_downcase=="authorization") | .key' "$SNAP_R2" "$SNAP_RD" | wc -l

echo
echo "== 6. заголовки-сателлиты auth (уникальные имена) =="
jq -r 'select(.ev=="req") | (.headers // {}) | keys[]' "$SNAP_R2" "$SNAP_RD" \
  | grep -iE 'super-properties|discord-locale|discord-timezone|debug-options|accept-language|content-type|cookie|fingerprint|installation|routing-key|client-trace|context-properties|failed-requests|science-test' \
  | sort | uniq -c | sort -rn

echo
echo "== 7. /science =="
echo -n "science req="; jq -r 'select(.ev=="req") | select(.url|endswith("/api/v9/science")) | .requestId' "$SNAP_R2" "$SNAP_RD" | wc -l
echo -n "science with Authorization="; jq -r 'select(.ev=="req") | select(.url|endswith("/api/v9/science")) | (.headers // {}) | to_entries[] | select(.key|ascii_downcase=="authorization") | 1' "$SNAP_R2" "$SNAP_RD" | wc -l
echo -n "science statuses="; jq -r 'select(.ev=="res") | select(.url|endswith("/api/v9/science")) | .status' "$SNAP_R2" "$SNAP_RD" | sort -u | tr '\n' ' '; echo

echo
echo "== 8. тела ответов: всего, не-ассеты, размеры, коды =="
echo -n "body events="; jq -r 'select(.ev=="body")|.requestId' "$SNAP_R2" "$SNAP_RD" | wc -l
echo -n "body non-asset="; jq -r 'select(.ev=="body")|.url' "$SNAP_R2" "$SNAP_RD" | grep -vc '/assets/'
jq -r 'select(.ev=="body") | (.body|tostring|length)' "$SNAP_R2" "$SNAP_RD" | sort -n | awk '{a[NR]=$1;s+=$1} END{printf "body len: n=%d min=%d median=%d max=%d sum=%d\n", NR, a[1], a[int(NR/2)], a[NR], s}'
echo -n "body requests with 20KB cap="; jq -r 'select(.ev=="body") | (.body|tostring|length)' "$SNAP_R2" "$SNAP_RD" | awk '$1>=20000' | wc -l

echo
echo "== 9. postData: количество, обрезка на 4000 =="
echo -n "postData events="; jq -r 'select(.ev=="req" and .postData!=null) | .requestId' "$SNAP_R2" "$SNAP_RD" | wc -l
jq -r 'select(.ev=="req" and .postData!=null) | (.postData|length)' "$SNAP_R2" "$SNAP_RD" | sort -n | awk '{a[NR]=$1} END{printf "postData len: n=%d min=%d median=%d max=%d\n", NR, a[1], a[int(NR/2)], a[NR]}'
echo -n "postData capped(>=4000)="; jq -r 'select(.ev=="req" and .postData!=null) | (.postData|length)' "$SNAP_R2" "$SNAP_RD" | awk '$1>=4000' | wc -l
echo -n "API(non-science) postData events="; jq -r 'select(.ev=="req" and .postData!=null) | select(.url|test("/api/v9/")) | select(.url|endswith("/science")|not) | .requestId' "$SNAP_R2" "$SNAP_RD" | wc -l

echo
echo "== 10. шум: не-API запросы по группам =="
jq -r 'select(.ev=="req") | .url' "$SNAP_R2" "$SNAP_RD" | awk '
/^data:/ {d++; next}
/\/cdn-cgi\// {cf++; next}
/^https:\/\/discord\.com\/assets/ {a++; next}
/^https:\/\/discord\.com\/api\// {api++; next}
/^wss:|^https:\/\/discord\.com\// {other++; next}
{t++}
END {printf "api=%d assets=%d cloudflare=%d data=%d other=%d thirdparty/inner=%d\n", api, a, cf, d, other, t}'
echo -n "science among api (already counted)="; jq -r 'select(.ev=="req") | select(.url|endswith("/api/v9/science")) | .requestId' "$SNAP_R2" "$SNAP_RD" | wc -l

echo
echo "== 11. коды ответов для API (join req->res по requestId внутри файла) =="
for f in "$SNAP_R2" "$SNAP_RD"; do
  echo "-- $f"
  jq -r 'select(.ev=="res") | "\(.requestId)\t\(.status)"' "$f" | LC_ALL=C sort -u -k1,1 > /home/sanya/re-deep/.vres.tsv
  jq -r 'select(.ev=="req") | select(.url|test("/api/v9/")) | select(.url|endswith("/science")|not) | "\(.requestId)\t\(.method) \(.url|sub("https://discord.com/api/v9";"")|sub("\\?.*";""))"' "$f" \
    | LC_ALL=C sort -k1,1 | join -t $'\t' - /home/sanya/re-deep/.vres.tsv | awk -F'\t' '{print $3}' | sort | uniq -c | sort -rn
done
rm -f /home/sanya/re-deep/.vres.tsv
