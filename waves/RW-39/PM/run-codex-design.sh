#!/usr/bin/env bash
# CODEX-DESIGN — запуск независимого анализа референсов (read-only).
# Usage: run-codex-design.sh <task> [out.md]
#   task: gaps | verify | embeds | themes | menu | all
set -euo pipefail

TASK="${1:-gaps}"
OUT="${2:-/tmp/opencode/codex-design-${TASK}.md}"
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"

if ! command -v codex >/dev/null 2>&1; then
  echo "codex CLI не найден в PATH" >&2
  exit 1
fi

case "$TASK" in
  gaps)   FOCUS="Закрой пробелы дампа: 07-guild-menu (неудачный захват), 05-dm-list (пустой), 04b-search-results (пустой JSON), геометрия меню сообщения и QuickSwitcher, day divider, light/cozy темы. Для каждого — либо измерь по PNG/JSON, либо честно gap." ;;
  verify) FOCUS="Проверь SPEC-P3-02.md против 00-app.png и ui8/*: сообщения (48/26/+22), композер 56, участники (42/43, 6 групп), поиск (7 секций, сортировка), эмодзи (10 категорий, 42 ячейки), настройки (38 switch). Найди расхождения с числами спеки." ;;
  embeds) FOCUS="Разбери embeds и вложения по 00-app.png и ui8/*: link-embed, image-embed, файл-карточка, reply-quote, реакции. Дай точные размеры/отступы/радиусы/цвета или gap." ;;
  themes) FOCUS="Проверь, есть ли в дампе светлая тема и cozy/compact; если нет — предложи [A]-значения на основе tokens.json (light) и текущей dark-палитры, с контрастом." ;;
  menu)   FOCUS="Разбери меню сообщения (01-msg-menu.*) и QuickSwitcher (03-*): геометрия, порядок, иконки, состояния. Если геометрия не снята — оцени по PNG и пометь допущение." ;;
  all)    FOCUS="Сделай полный разбор: verify спеки, пробелы, embeds, меню, QuickSwitcher, темы; приоритизируй P0/P1/P2." ;;
  *) echo "неизвестный task: $TASK (gaps|verify|embeds|themes|menu|all)" >&2; exit 2 ;;
esac

PROMPT="$(cat <<EOF
Ты — независимый reference analyst / designer (CODEX-DESIGN) в репозитории $ROOT.
Читай роль и правила: waves/RW-39/PM/CODEX-DESIGN.md.

Источники (read-only): /home/user/clean-room-oracle (ui/, ui8/, ui/tokens.json, ui/log.json),
waves/RW-39/DESIGN/{ref-manifest.json,SPEC.md,SPEC-P3-02.md,REFERENCES.md,layout.json,tokens.css,DOM-CONTRACT.md}.

ЗАДАЧА ($TASK): $FOCUS

Правила:
- измеряй пиксели через python3/PIL, JSON/HTML читай напрямую; фиксируй метод;
- каждое число — с кодом [M]/[T]/[L]/[F]/[A] и путём-источником;
- ассеты не копируй, сеть не используй, файлы не редактируй, приложение/окно/cargo не запускай;
- если данных нет — пиши gap, не выдумывай.

Формат ответа (markdown):
1) Verdict: что подтверждено, что расходится.
2) Таблица: id | элемент | измерено | источник | рекомендация | priority.
3) Пробелы и [A]-предложения.
4) Что из этого менять в SPEC-P3-02/вёрстке (конкретные строки/значения).
EOF
)"

echo "codex-design: task=$TASK -> $OUT"
codex exec -C "$ROOT" -s read-only -o "$OUT" "$PROMPT"
