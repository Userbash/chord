# Независимое ревью — B2b-ATTACH-UI-FIX

Дата: 2026-10-30. HEAD: `4e24048`. Продуктовый код и тесты read-only.

## Вердикт

**RETURN. P0=1, P1=1.**

### P0 — `@ts-ignore` остались в продуктовом модуле

Удаление директив из `tests/unit/composer.test.mjs` подтверждается, но в изменённом
`rust/gui-spike/ui/features/composer.js` остаются пять новых `// @ts-ignore`:
строки 593, 659, 672, 868 и 881. Это запрещённое отключение проверки типов по
TEST-POLICY §2.4; заявление отчёта про закрытие P0 неполное.

### P1 — проверенный PUT не работает в приложении

`startUpload` вызывает `attachments_upload_put` с метаданными слота, размером и именем,
но без байтов файла (`composer.js:733-745`). `addFiles` сохраняет только `name/size/type`
и не удерживает сам `File` (`:787-797`). Кроме того, ни `attachments_upload_slots`, ни
`attachments_upload_put` не зарегистрированы в `tauri::generate_handler!`
(`rust/gui-spike/src/main.rs:730-767`); приложение не передаёт `uploadPut` в
`createComposer` (`ui/app.js:1362-1388`). Тесты подменяют обе операции успешными mock-ами,
поэтому доказывают порядок вызовов, но не загрузку содержимого в целевое хранилище.
Заявленное закрытие P1 PUT не подтверждено на runtime-пути.

## Evidence

- Текущие MD5 соответствуют отчёту для `composer.js` (`2b1d80a50dcd8495a204b32390a63dc0`)
  и теста (`eafbdf850a02c0c26822b37a20c5cc7f`).
- `node --test tests/unit/composer.test.mjs` → exit 0, 26 passed, 0 failed.
- Логи red/green и пять пойманных мутаций проверены; они подтверждают тестовую логику,
  но не устраняют P0 и разрыв runtime-интеграции.
- `cargo` не запускался.
