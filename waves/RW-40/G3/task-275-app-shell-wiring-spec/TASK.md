# ТЗ — task-275 (`G3 Design`: `DESIGN-P1-18` — App Shell Wiring & Composer Popovers UX/DOM Spec)

## Роль и цель
Ты воркер отдела `G3 Design (UX/UI & Specs)`.
Спроектируй полную дизайн-спецификацию интеграции верхнеуровневой оболочки (`app-shell-wiring.js`, `shell-controllers-registry.js`, `chat-helpers-bridge.js`, `composer-popovers-bridge.js`) со сверкой селекторов, горячих клавиш, z-index слоёв и i18n-ключей по эталонному дампу `reference-oracle/` (`frontend-d03.md`, `ui/`..`ui8/`, `ru-catalog.json`).

## Разрешённые файлы для записи (Write-Scope)
- `docs/specs/APP-SHELL-WIRING-SPEC.md` (в рабочем дереве ветки `design/task-275-app-shell-wiring-spec`)
- `/home/user/orca/project/waves/RW-40/DESIGN/APP-SHELL-WIRING-TOKENS.md`
- `/home/user/orca/project/waves/RW-40/G3/task-275-app-shell-wiring-spec/REPORT.md`

## Требования
1. Описать в `docs/specs/APP-SHELL-WIRING-SPEC.md`:
   - Иерархию z-index и стек модальных слоёв (`quick_switcher`, `notification_inbox`, `pins_drawer`, `threads_drawer`, `user_profile_card`) против полноэкранных гильд-панелей (`roles_matrix`, `audit_log`, `automod`, `scheduled_events`, `webhooks`, `expressions`, `moderation`, `onboarding`).
   - Взаимное вытеснение поповеров композера (`slash_command_picker`, `soundboard_picker`, `emoji_picker`, `gif_picker`, `attachment_dropzone`) и клавиатурную навигацию (`ArrowUp/Down`, `Tab`, `Enter`, `Escape`, `Ctrl+K`).
   - ARIA-атрибуты (`role="dialog"`, `aria-modal="true"`, `aria-expanded`, `aria-activedescendant`) и брейкпоинты адаптивности (`1024px`, `1280px`, `1440px`).
2. Записать таблицу CSS-токенов в `/home/user/orca/project/waves/RW-40/DESIGN/APP-SHELL-WIRING-TOKENS.md` и отчёт с `md5sum` в `REPORT.md`.
3. Завершить работу вызовом `orca orchestration worker-done`. Не делать `git commit`.
