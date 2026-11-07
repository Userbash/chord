# Независимое ревью — INT-04-ATTACH-PUT-BACKEND-PASS2

Дата: 2026-10-30. HEAD: `4e240481`. Режим: read-only; `cargo` не запускался.

## Вердикт

**PASS. P0=0, P1=0, P2=0 в объёме карточки.** P1 из `REVIEW-BE-PUT.md` закрыт: плоские поля `putPayload` теперь объявлены параметрами Tauri-команды, `id` сохраняется как идентификатор слота, индексные `Uint8Array`-байты извлекаются и доходят до HTTP PUT с заданным `Content-Type`.

## Проверки

- `composer.js:744-761` передаёт плоский объект с `id`, `uploadUrl`, `contentType`, `bytes` и `data`. В `main.rs` сигнатура `#[tauri::command] attachments_upload_put` содержит соответствующие аргументы `id`, `upload_url`, `content_type`, `data` и `bytes`; обработчик собирает их в `UploadPutCommandArgs` и передаёт в `execute_attachments_upload_put`, который вызывает `discord_gui_spike::attachments_upload_put` с URL, MIME и `payload_bytes()`.
- `UploadBytes` десериализует индексный JSON-объект последовательных байтов. Контрактный тест строит плоский payload с `id: "slot-1"` и `{"0":137,"1":80}`, проверяет извлечённые аргументы и доставку `[137, 80]` вместе с `image/png` в fake HTTP PUT. Отдельный тест в `main.rs` проверяет wiring команды и ту же передачу байтов. Тесты моделируют Tauri extraction через сигнатуру и алгоритм ключей; сам Tauri runtime отдельно не поднимался.
- `red-be-put-pass2.log` фиксирует падение до исправления (в сигнатуре не было `id`) и runtime assertion RED. `green-be-put-pass2.log` фиксирует targeted: lib 8/8, bin 10/10, `attachments_contract` 23/23, `ipc_wiring_contract` 5/5; `tests/meta/ipc-path.test.mjs` — 12/12, без skip/fail. Clippy и rustfmt завершились с кодом 0.
- `mutations-be-put-pass2.log` фиксирует пять ожидаемых мутантов M1–M5, все убиты целевыми тестами с exit 101; восстановленные MD5 совпадают с baseline и отчётом для всех четырёх назначенных Rust-файлов.

## Ограничения

Проверены исходники и сохранённые логи; `cargo build/test`, GUI и отдельный Tauri runtime-прогон не выполнялись согласно условиям ревью. Meta-тест `tests/meta/ipc-path.test.mjs` подтверждает отсутствие запрещённых файловых аргументов в IPC-сигнатурах.
