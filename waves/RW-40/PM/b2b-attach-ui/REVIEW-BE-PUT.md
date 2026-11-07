# Независимое ревью — INT-04-ATTACH-PUT-BACKEND / B2b-ATTACH-UI

Дата: 2026-10-30. HEAD: `4e24048`. Режим: read-only; `cargo` не запускался.

## Вердикт

**RETURN. P0=0, P1=1, P2=0.** Backend PUT реализован и локальные контракты проходят по приложенным логам, но P1 из `REVIEW-PASS3.md` о проверке реального Tauri IPC runtime-пути не закрыт.

## Проверки

- `REPORT-BE-PUT.md` соответствует реализации: HTTPS PUT отправляет MIME и сырые байты, проверяет размер/пустые данные, классифицирует статусы; команда получает токен через `token_or`, зарегистрирована в `generate_handler!` и передаёт вход в публичную обёртку.
- RED-лог содержит ожидаемый compile-time RED и runtime RED с пятью новыми упавшими контрактами. GREEN-лог заявляет 22/22 `attachments_contract`, 5/5 `ipc_wiring_contract`, 8/8 lib и 9/9 bin; также IPC meta 12/12, clippy и rustfmt успешны. Эти команды повторно не запускались.
- `mutations-be-put.log` подтверждает все 5 мутантов: M1–M5 каждый убит targeted-проверкой с ненулевым кодом выхода. Итоговые MD5 всех четырёх кодовых/тестовых файлов совпадают с baseline и отчётом: `attachments.rs` `84d8905b20a67c46a7908c9ac103560a`, `lib.rs` `48d01ef823c7babff52614ad95b0e60d`, `main.rs` `dc1d395fd083020f5a7d5e038925cd1d`, тест `d1113587c21f7a91e4e0f00dc4e0c435`.
- Артефакт мутаций существует под указанным в отчёте именем `mutations-be-put.log`; несоответствия имени нет.

## Finding

**P1 — runtime-передача содержимого через Tauri IPC всё ещё не доказана.** Тест `upload_put_input_deserializes_uint8array_and_vec_and_validates_ipc_payload` десериализует синтетический JSON в `UploadPutInput` и вызывает `upload_put_input` с fake transport. Отдельный wiring-тест ищет строки в `main.rs`, а не вызывает зарегистрированную Tauri-команду. Это подтверждает ожидаемую форму данных и backend логику, но не подтверждает, что реальный `invoke("attachments_upload_put", putPayload)` извлекает индексный объект `Uint8Array` в параметр `Option<UploadBytes>` и доводит байты до HTTP PUT. В `composer.js` payload передаётся плоскими полями (`id`, `channelId`, `uploadUrl`, `contentType`, `bytes`, `data`); проверка этой формы на уровне Tauri command extraction отсутствует. Следовательно, конкретный runtime-пробел, отмеченный pass 3, сохраняется.

## Ограничения

Повторные тесты, `cargo` и запуск GUI не выполнялись по условиям ревью. Логи и реализация изучены read-only; вердикт опирается на предоставленные логи и статический просмотр исходников.
