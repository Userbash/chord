# BRIEF NET-D — экран входа и auth-состояния (frontend)

Владелец: NET-D (Frontend). PM: DeepSeek. Основание: `ADR-2-auth.md`, NET-C2 API.
Запуск: после NET-C2/NET-E1. Работать red-first.

## Файлы (только эти)
- `rust/gui-spike/ui/index.html` — оверлей `#auth-gate` (не `.screen`, screenCount остаётся 4)
- `rust/gui-spike/ui/features/auth.js` (новый) — состояние/валидация/вызовы invoke
- `rust/gui-spike/ui/features/screens.js`, `ui/app.js` — интеграция гейта
- `rust/gui-spike/ui/components/auth.css` (новый) + `style.css` (подключение)
- `tests/unit/gui-spike-auth.test.mjs` (новый), `tests/unit/gui-spike-dom.test.mjs` (только новые id)
- логи: `waves/RW-39/NET/PM/netd/`
READ-ONLY: остальной ui/**, src/**, DESIGN/**.

## Требования
1. Оверлей входа: поле токена (`type=password`, показать/скрыть), submit, «Восстановить сессию»
   (если `auth_status.stored=true`), «Выйти» после ready.
2. Вызовы Tauri: `auth_login_token`, `auth_status`, `auth_restore`, `auth_logout` через
   `window.__TAURI__.core.invoke`; в DOM-шиме — стаб, записывающий вызовы.
3. Состояния: `idle → validating → ready | error`; `aria-live` для ошибок; фокус в поле;
   токен очищается из input после submit; **никогда** не логируется и не попадает в DOM-атрибуты.
4. Ошибки: 401 → «токен недействителен», 429 → «слишком часто, подождите», сеть → «нет связи»;
   без текста токена.
5. После ready: гейт скрыт, чат доступен, в user-bar — имя (если есть) и logout.
6. Кнопки не мёртвые: либо действие, либо `aria-disabled` + подсказка.

## Проверки (без окна/cargo/gate)
```bash
node --test tests/unit/gui-spike-auth.test.mjs
node --test tests/unit/gui-spike-dom.test.mjs tests/unit/gui-spike-model.test.mjs tests/unit/gui-spike-bench.test.mjs tests/unit/gui-spike-features.test.mjs
npx tsc -p rust/gui-spike/ui/tsconfig.json
node waves/RW-39/PM/w39c/geometry-check.mjs
```
Окно/probe/cargo/gate/сеть/commit — запрещены.

## Отчёт
`NET/PM/netd/REPORT.md`: состояние → тест → результат, md5, остаток (события `auth://state`).
