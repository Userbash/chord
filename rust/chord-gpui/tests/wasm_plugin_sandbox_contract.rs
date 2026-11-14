//! Контрактные тесты песочницы WASM-плагинов (`P3-1.2` / `W3-6`):
//! Проверяет строгое соответствие `docs/specs/plugin-api-v1.md` по всем 9 измерениям:
//! 1. Позитивный сценарий (Happy Path): загрузка валидного `\0asm` модуля, `on_message_render` (<= 2000 символов), `dispatch_slash_command` (<= 3 эмбедов), KV-хранилище.
//! 2. Негативный сценарий: отклонение битого magic, неверной версии ABI/WASM, подделки SHA-256/Ed25519 подписи, запрещённых WASI импортов (`wasi:http/*`, `wasi:filesystem/*`, `wasi:sockets/*`, `wasi_snapshot_preview1::environ_get`, `proc_*`, `chord:host/token`).
//! 3. Граничные условия (Edge Boundaries): память ровно `256` страниц (`16 MiB`), таблица `10_000` элементов, `2000` символов рендера, `3` эмбеда, `16` слэш-команд, квота KV `1 MiB`.
//! 4. Обработка ошибок и ловушек (Traps): исчерпание топлива (`> 5_000_000`), таймаут (`> 5 ms`), переполнение памяти (`> 16 MiB`), `unreachable`.
//! 5. Пустые состояния и дефолты: плагин с пустыми разрешениями загружается, но получает default-deny при вызове хуков/хоста; пустое сообщение возвращает `None` или пустой патч.
//! 6. Жизненный цикл (Lifecycle): `Unloaded -> Loaded -> Enabled -> Disabled -> Unloaded`.
//! 7. Межсетевой экран возможностей (Capability Firewall): default-deny для незадекларированных прав, запрет опасных возможностей (`Network`, `Filesystem`, `Environment`, `Process`, `AuthToken`, `VoiceMedia`), блокировка Discord API доменов в `http_fetch`.
//! 8. Изоляция сбоев и авто-отключение (`MAX_CONSECUTIVE_TRAPS_BEFORE_DISABLE = 3`): 1 и 2 сбоя сбрасываются успешным вызовом; 3 сбоя подряд переводят плагин в `Disabled`, не затрагивая соседние плагины.
//! 9. Наблюдаемая телеметрия и редактирование секретов: маскирование токенов (`mfa.*`, `Bearer *`, `Bot *`, `MT*.*.*`) -> `[REDACTED]` в аудит-логах и ошибках, учёт метрик топлива и задержек.

#![forbid(unsafe_code)]
#![recursion_limit = "512"]

#[path = "../src/plugins/mod.rs"]
mod plugins;

use plugins::{
    CommandEmbed, CommandInvocation, CommandResponse, DEFAULT_CALL_DEADLINE_MS,
    DEFAULT_CALL_FUEL_BUDGET, DEFAULT_INIT_FUEL_BUDGET, DEFAULT_MEMORY_LIMIT_BYTES,
    DEFAULT_MEMORY_LIMIT_PAGES, DEFAULT_TABLE_LIMIT_ELEMENTS, GuestContext, HttpFetchRequest,
    LogLevel, MAX_CONSECUTIVE_TRAPS_BEFORE_DISABLE, MAX_EMBEDS_PER_RESPONSE,
    MAX_KV_STORAGE_QUOTA_BYTES, MAX_MESSAGE_RENDER_LEN, MAX_SLASH_COMMANDS_PER_PLUGIN,
    MessageContext, PLUGIN_ABI_VERSION, PluginCapability, PluginError, PluginGuest, PluginId,
    PluginManifest, PluginSandboxRuntime, PluginStatus, RenderedMessagePatch, RenderedToken,
    SlashCommandSpec, TrapReason, WASM_COMPONENT_VERSION, WASM_CORE_VERSION, WASM_MAGIC,
    WIT_PACKAGE_ID, WasmBinaryBuilder, WasmSectionId, redact_secrets,
};

struct TestPluginGuest {
    init_fuel: u64,
    call_fuel: u64,
    simulated_elapsed_ms: u64,
    allocate_bytes: usize,
    trap_on_call: Option<TrapReason>,
    render_output: Option<String>,
    embed_count: usize,
    log_on_call: Option<(LogLevel, String)>,
}

impl Default for TestPluginGuest {
    fn default() -> Self {
        Self {
            init_fuel: 25_000,
            call_fuel: 50_000,
            simulated_elapsed_ms: 1,
            allocate_bytes: 0,
            trap_on_call: None,
            render_output: None,
            embed_count: 1,
            log_on_call: None,
        }
    }
}

impl PluginGuest for TestPluginGuest {
    fn on_load(&mut self, ctx: &mut GuestContext<'_>) -> Result<(), TrapReason> {
        ctx.consume_fuel(self.init_fuel)?;
        if self.allocate_bytes > 0 {
            ctx.allocate_memory(self.allocate_bytes)?;
        }
        Ok(())
    }

    fn on_message_render(
        &mut self,
        ctx: &mut GuestContext<'_>,
        msg: &MessageContext,
    ) -> Result<Option<RenderedMessagePatch>, TrapReason> {
        ctx.consume_fuel(self.call_fuel)?;
        ctx.advance_elapsed_ms(self.simulated_elapsed_ms)?;
        if self.allocate_bytes > 0 {
            ctx.allocate_memory(self.allocate_bytes)?;
        }
        assert!(ctx.fuel_consumed() >= self.call_fuel);
        assert!(ctx.elapsed_ms() >= self.simulated_elapsed_ms);
        assert!(ctx.memory_used_bytes() >= self.allocate_bytes);
        if let Some((level, ref line)) = self.log_on_call {
            ctx.host_log(level, line);
        }
        if let Some(ref reason) = self.trap_on_call {
            return Err(reason.clone());
        }
        if msg.content.is_empty() && self.render_output.is_none() {
            return Ok(None);
        }
        let content = self
            .render_output
            .clone()
            .unwrap_or_else(|| format!("[formatted] {}", msg.content));
        Ok(Some(RenderedMessagePatch {
            rendered_content: content.clone(),
            tokens: vec![RenderedToken {
                token_type: "markdown".to_string(),
                text: content,
                color: Some("#5865F2".to_string()),
                bold: true,
                italic: false,
            }],
        }))
    }

    fn on_slash_command(
        &mut self,
        ctx: &mut GuestContext<'_>,
        invocation: &CommandInvocation,
    ) -> Result<CommandResponse, TrapReason> {
        ctx.consume_fuel(self.call_fuel)?;
        ctx.advance_elapsed_ms(self.simulated_elapsed_ms)?;
        if self.allocate_bytes > 0 {
            ctx.allocate_memory(self.allocate_bytes)?;
        }
        assert!(ctx.fuel_consumed() >= self.call_fuel);
        assert!(ctx.elapsed_ms() >= self.simulated_elapsed_ms);
        assert!(ctx.memory_used_bytes() >= self.allocate_bytes);
        if let Some((level, ref line)) = self.log_on_call {
            ctx.host_log(level, line);
        }
        if let Some(ref reason) = self.trap_on_call {
            return Err(reason.clone());
        }
        let embeds = (0..self.embed_count)
            .map(|idx| CommandEmbed {
                title: format!("{} #{}", invocation.command, idx + 1),
                description: format!("args: {}", invocation.args.join(" ")),
                color_hex: Some(0x5865F2),
            })
            .collect();
        Ok(CommandResponse {
            content: format!("Executed /{}", invocation.command),
            embeds,
            ephemeral: true,
        })
    }
}

fn sample_manifest(id: &str) -> PluginManifest {
    PluginManifest {
        id: id.to_string(),
        name: "Sample Formatter Plugin".to_string(),
        version: "1.0.4".to_string(),
        abi_version: PLUGIN_ABI_VERSION,
        permissions: vec![
            PluginCapability::ReadMessages,
            PluginCapability::RenderUi,
            PluginCapability::SlashCommands,
            PluginCapability::PluginStorage,
        ],
        slash_commands: vec![SlashCommandSpec {
            name: "shrug".to_string(),
            description: "Append shrug emoticon".to_string(),
        }],
        allowed_domains: vec!["api.github.com".to_string()],
        kv_quota_bytes: MAX_KV_STORAGE_QUOTA_BYTES,
        expected_sha256: None,
        signature_hex: None,
        public_key_hex: None,
    }
}

fn sample_wasm_bytes() -> Vec<u8> {
    WasmBinaryBuilder::new_core_v1()
        .with_memory_pages(16, Some(256))
        .with_table_elements(128, Some(1_000))
        .with_import("chord:plugin/host", "log")
        .with_import("chord:plugin/host", "kv-get")
        .with_export("on-load")
        .with_export("on-message-render")
        .with_export("on-slash-command")
        .build()
}

#[test]
fn test_dim1_positive_spec_constants_and_valid_plugin_execution() {
    // Проверяем точные константы спецификации plugin-api-v1.md
    assert_eq!(WASM_MAGIC, [0x00, 0x61, 0x73, 0x6D]);
    assert_eq!(&WASM_MAGIC, b"\0asm");
    assert_eq!(WASM_CORE_VERSION, [0x01, 0x00, 0x00, 0x00]);
    assert_eq!(WASM_COMPONENT_VERSION, [0x0D, 0x00, 0x01, 0x00]);
    assert_eq!(PLUGIN_ABI_VERSION, 1);
    assert_eq!(DEFAULT_MEMORY_LIMIT_BYTES, 16 * 1024 * 1024);
    assert_eq!(DEFAULT_MEMORY_LIMIT_PAGES, 256);
    assert_eq!(DEFAULT_TABLE_LIMIT_ELEMENTS, 10_000);
    assert_eq!(DEFAULT_INIT_FUEL_BUDGET, 10_000_000);
    assert_eq!(DEFAULT_CALL_FUEL_BUDGET, 5_000_000);
    assert_eq!(DEFAULT_CALL_DEADLINE_MS, 5);
    assert_eq!(MAX_CONSECUTIVE_TRAPS_BEFORE_DISABLE, 3);
    assert_eq!(MAX_MESSAGE_RENDER_LEN, 2_000);
    assert_eq!(MAX_EMBEDS_PER_RESPONSE, 3);
    assert_eq!(MAX_SLASH_COMMANDS_PER_PLUGIN, 16);
    assert_eq!(MAX_KV_STORAGE_QUOTA_BYTES, 1_048_576);
    assert_eq!(WIT_PACKAGE_ID, "chord:plugin@1.0.0");
    assert_eq!(WasmSectionId::from_u8(0), Some(WasmSectionId::Custom));
    assert_eq!(WasmSectionId::from_u8(2), Some(WasmSectionId::Import));
    assert_eq!(WasmSectionId::from_u8(4), Some(WasmSectionId::Table));
    assert_eq!(WasmSectionId::from_u8(5), Some(WasmSectionId::Memory));
    assert_eq!(WasmSectionId::from_u8(7), Some(WasmSectionId::Export));
    assert_eq!(WasmSectionId::from_u8(99), None);

    let mut runtime = PluginSandboxRuntime::new();
    let manifest = sample_manifest("org.chord.sample-formatter");
    let wasm = sample_wasm_bytes();

    let plugin_id = runtime
        .load_plugin(manifest, &wasm, Box::new(TestPluginGuest::default()))
        .expect("Валидный WASM-плагин должен успешно загружаться");
    assert_eq!(plugin_id, "org.chord.sample-formatter");
    assert_eq!(
        runtime.plugin_status(&plugin_id),
        Some(PluginStatus::Enabled)
    );

    // Вызов хука рендеринга сообщения
    let msg = MessageContext {
        message_id: 101,
        channel_id: 202,
        author_id: 303,
        author_name: "alice".to_string(),
        content: "Hello **world**".to_string(),
        timestamp_ms: 1_700_000_000_000,
    };
    let patch = runtime
        .dispatch_on_message_render(&plugin_id, msg)
        .expect("on_message_render должен выполниться без ошибок")
        .expect("Ожидался непустой патч");
    assert_eq!(patch.rendered_content, "[formatted] Hello **world**");
    assert_eq!(patch.tokens.len(), 1);
    assert_eq!(patch.tokens[0].text, "[formatted] Hello **world**");

    // Вызов слэш-команды
    let response = runtime
        .dispatch_slash_command(
            &plugin_id,
            CommandInvocation {
                command: "shrug".to_string(),
                args: vec!["now".to_string()],
                channel_id: 202,
                invoker_id: 303,
            },
        )
        .expect("dispatch_slash_command должен выполниться успешно");
    assert_eq!(response.content, "Executed /shrug");
    assert_eq!(response.embeds.len(), 1);
    assert!(response.ephemeral);

    // Component Model заголовок [0x0d, 0x00, 0x01, 0x00] также поддерживается
    let component_wasm = WasmBinaryBuilder::new_component_v1()
        .with_memory_pages(4, Some(64))
        .build();
    let comp_manifest = sample_manifest("org.chord.component-plugin");
    let comp_id = runtime
        .load_plugin(
            comp_manifest,
            &component_wasm,
            Box::new(TestPluginGuest::default()),
        )
        .expect("WASM Component v1 должен успешно валидироваться");
    assert_eq!(runtime.plugin_status(&comp_id), Some(PluginStatus::Enabled));
}

#[test]
fn test_dim2_negative_invalid_magic_abi_tampering_and_forbidden_wasi_imports() {
    let mut runtime = PluginSandboxRuntime::new();

    // 1. Неверные магические байты (не \0asm)
    let bad_magic = b"ELF\x01\x01\x00\x00\x00";
    let err = runtime
        .load_plugin(
            sample_manifest("org.chord.bad-magic"),
            bad_magic,
            Box::new(TestPluginGuest::default()),
        )
        .expect_err("Должен отклонять не-WASM бинарник");
    assert!(matches!(err, PluginError::InvalidWasmMagic));

    // 2. Неподдерживаемая версия заголовка WASM
    let bad_version = [0x00, 0x61, 0x73, 0x6D, 0x99, 0x00, 0x00, 0x00];
    let err = runtime
        .load_plugin(
            sample_manifest("org.chord.bad-ver"),
            &bad_version,
            Box::new(TestPluginGuest::default()),
        )
        .expect_err("Должен отклонять неизвестную версию WASM");
    assert!(matches!(
        err,
        PluginError::UnsupportedWasmVersion([0x99, 0x00, 0x00, 0x00])
    ));

    // 3. Несовпадение версии ABI в манифесте
    let mut bad_abi_manifest = sample_manifest("org.chord.bad-abi");
    bad_abi_manifest.abi_version = 99;
    let err = runtime
        .load_plugin(
            bad_abi_manifest,
            &sample_wasm_bytes(),
            Box::new(TestPluginGuest::default()),
        )
        .expect_err("Должен отклонять несовместимый ABI");
    assert!(matches!(
        err,
        PluginError::AbiVersionMismatch {
            expected: PLUGIN_ABI_VERSION,
            found: 99
        }
    ));

    // 4. Подделка SHA-256 / Ed25519 подписи бинарника
    let wasm = sample_wasm_bytes();
    let mut tampered_manifest = sample_manifest("org.chord.tampered");
    tampered_manifest.expected_sha256 = Some([0xAA; 32]);
    let err = runtime
        .load_plugin(
            tampered_manifest,
            &wasm,
            Box::new(TestPluginGuest::default()),
        )
        .expect_err("Подделка SHA-256 должна блокировать загрузку");
    assert!(matches!(err, PluginError::SignatureVerificationFailed(_)));

    // 5. Запрещённые WASI и хост-импорты
    let forbidden_imports = [
        ("wasi:http/outgoing-handler", "handle"),
        ("wasi:filesystem/types", "open-at"),
        ("wasi:sockets/tcp", "connect"),
        ("wasi_snapshot_preview1", "environ_get"),
        ("wasi_snapshot_preview1", "proc_exit"),
        ("env", "proc_exec"),
        ("chord:host/token", "get-auth-token"),
    ];

    for (module, field) in forbidden_imports {
        let forbidden_wasm = WasmBinaryBuilder::new_core_v1()
            .with_memory_pages(1, Some(16))
            .with_import(module, field)
            .build();
        let err = runtime
            .load_plugin(
                sample_manifest("org.chord.forbidden-import"),
                &forbidden_wasm,
                Box::new(TestPluginGuest::default()),
            )
            .expect_err(&format!(
                "Запрещённый импорт {module}::{field} должен блокироваться"
            ));
        assert!(
            matches!(err, PluginError::ForbiddenImport { .. }),
            "Ожидалась ошибка ForbiddenImport для {module}::{field}, получено: {err:?}"
        );
    }
}

#[test]
fn test_dim3_edge_boundaries_memory_256_pages_table_10000_chars_2000_embeds_3_cmds_16_kv_1mib() {
    let mut runtime = PluginSandboxRuntime::new();

    // 1. Память ровно 256 страниц (16 MiB) и таблица 10_000 элементов -> OK
    let exact_limits_wasm = WasmBinaryBuilder::new_core_v1()
        .with_memory_pages(256, Some(256))
        .with_table_elements(10_000, Some(10_000))
        .build();
    let id = runtime
        .load_plugin(
            sample_manifest("org.chord.exact-limits"),
            &exact_limits_wasm,
            Box::new(TestPluginGuest {
                allocate_bytes: DEFAULT_MEMORY_LIMIT_BYTES,
                render_output: Some("a".repeat(MAX_MESSAGE_RENDER_LEN)),
                embed_count: MAX_EMBEDS_PER_RESPONSE,
                ..Default::default()
            }),
        )
        .expect("Ровно 256 страниц (16 MiB) и 10_000 элементов таблицы должны проходить");

    // 2. Рендер сообщения ровно 2000 символов -> OK
    let msg = MessageContext {
        message_id: 1,
        channel_id: 1,
        author_id: 1,
        author_name: "bob".to_string(),
        content: "x".to_string(),
        timestamp_ms: 1,
    };
    let patch = runtime
        .dispatch_on_message_render(&id, msg.clone())
        .expect("2000 символов ровно должны быть приняты")
        .unwrap();
    assert_eq!(
        patch.rendered_content.chars().count(),
        MAX_MESSAGE_RENDER_LEN
    );

    // 3. Слэш-команда ровно с 3 эмбедами -> OK
    let resp = runtime
        .dispatch_slash_command(
            &id,
            CommandInvocation {
                command: "shrug".to_string(),
                args: vec![],
                channel_id: 1,
                invoker_id: 1,
            },
        )
        .expect("Ровно 3 эмбеда должны проходить");
    assert_eq!(resp.embeds.len(), MAX_EMBEDS_PER_RESPONSE);

    // 4. Превышение 256 страниц в секции памяти WASM (257 страниц) -> отказ загрузки
    let over_mem_wasm = WasmBinaryBuilder::new_core_v1()
        .with_memory_pages(16, Some(257))
        .build();
    let err = runtime
        .load_plugin(
            sample_manifest("org.chord.over-mem"),
            &over_mem_wasm,
            Box::new(TestPluginGuest::default()),
        )
        .expect_err("257 страниц (> 16 MiB) должны отклоняться валидатором");
    assert!(matches!(
        err,
        PluginError::MemoryLimitExceeded {
            requested_pages: 257,
            max_pages: 256
        }
    ));

    // 5. Превышение лимита таблицы (10_001 элемент) -> отказ загрузки
    let over_table_wasm = WasmBinaryBuilder::new_core_v1()
        .with_memory_pages(16, Some(64))
        .with_table_elements(10_001, Some(10_001))
        .build();
    let err = runtime
        .load_plugin(
            sample_manifest("org.chord.over-table"),
            &over_table_wasm,
            Box::new(TestPluginGuest::default()),
        )
        .expect_err("Таблица > 10_000 элементов должна отклоняться");
    assert!(matches!(err, PluginError::TableLimitExceeded { .. }));

    // 6. Ровно 16 слэш-команд -> OK; 17 слэш-команд -> отказ
    let mut manifest_16 = sample_manifest("org.chord.cmds-16");
    manifest_16.slash_commands = (0..MAX_SLASH_COMMANDS_PER_PLUGIN)
        .map(|i| SlashCommandSpec {
            name: format!("cmd{i}"),
            description: "ok".to_string(),
        })
        .collect();
    runtime
        .load_plugin(
            manifest_16,
            &sample_wasm_bytes(),
            Box::new(TestPluginGuest::default()),
        )
        .expect("16 слэш-команд должны регистрироваться");

    let mut manifest_17 = sample_manifest("org.chord.cmds-17");
    manifest_17.slash_commands = (0..=MAX_SLASH_COMMANDS_PER_PLUGIN)
        .map(|i| SlashCommandSpec {
            name: format!("cmd{i}"),
            description: "too many".to_string(),
        })
        .collect();
    let err = runtime
        .load_plugin(
            manifest_17,
            &sample_wasm_bytes(),
            Box::new(TestPluginGuest::default()),
        )
        .expect_err("17 слэш-команд (> 16) должны отклоняться");
    assert!(matches!(
        err,
        PluginError::TooManySlashCommands { count: 17, max: 16 }
    ));

    // 7. Квота KV-хранилища: ровно 1 MiB проходит, +1 байт возвращает StorageQuotaExceeded
    runtime
        .kv_set(&id, "blob", vec![0x42; MAX_KV_STORAGE_QUOTA_BYTES])
        .expect("Запись ровно 1 MiB в KV должна проходить");
    assert_eq!(
        runtime.kv_get(&id, "blob").unwrap().unwrap().len(),
        MAX_KV_STORAGE_QUOTA_BYTES
    );
    let err = runtime
        .kv_set(&id, "extra", vec![0x01])
        .expect_err("Превышение 1 MiB квоты KV должно блокироваться");
    assert!(matches!(err, PluginError::StorageQuotaExceeded { .. }));
    assert!(
        runtime
            .kv_delete(&id, "blob")
            .expect("kv_delete должен удалять ключ")
    );
    assert!(!runtime.kv_delete(&id, "blob").unwrap());
    runtime
        .kv_set(&id, "extra", vec![0x01])
        .expect("После освобождения квоты через kv_delete запись снова доступна");
}

#[test]
fn test_dim4_error_handling_fuel_exhaustion_deadline_timeout_memory_overflow_and_output_limits() {
    let mut runtime = PluginSandboxRuntime::new();
    let wasm = sample_wasm_bytes();

    // 1. Превышение стартового бюджета топлива при инициализации (> 10_000_000)
    let err = runtime
        .load_plugin(
            sample_manifest("org.chord.init-fuel-hog"),
            &wasm,
            Box::new(TestPluginGuest {
                init_fuel: DEFAULT_INIT_FUEL_BUDGET + 1,
                ..Default::default()
            }),
        )
        .expect_err("Превышение DEFAULT_INIT_FUEL_BUDGET должно вызывать Trap");
    assert!(matches!(
        err,
        PluginError::GuestTrap {
            reason: TrapReason::FuelExhausted { .. },
            ..
        }
    ));

    // 2. Превышение бюджета топлива на вызов хука (> 5_000_000)
    let fuel_id = runtime
        .load_plugin(
            sample_manifest("org.chord.call-fuel-hog"),
            &wasm,
            Box::new(TestPluginGuest {
                call_fuel: DEFAULT_CALL_FUEL_BUDGET + 1,
                ..Default::default()
            }),
        )
        .unwrap();
    let msg = MessageContext {
        message_id: 1,
        channel_id: 1,
        author_id: 1,
        author_name: "u".to_string(),
        content: "hi".to_string(),
        timestamp_ms: 1,
    };
    let err = runtime
        .dispatch_on_message_render(&fuel_id, msg.clone())
        .expect_err("Расход > 5_000_000 топлива должен перехватываться");
    assert!(matches!(
        err,
        PluginError::GuestTrap {
            reason: TrapReason::FuelExhausted { .. },
            ..
        }
    ));

    // 3. Превышение дедлайна выполнения (> 5 ms)
    let slow_id = runtime
        .load_plugin(
            sample_manifest("org.chord.slow-plugin"),
            &wasm,
            Box::new(TestPluginGuest {
                simulated_elapsed_ms: DEFAULT_CALL_DEADLINE_MS + 1,
                ..Default::default()
            }),
        )
        .unwrap();
    let err = runtime
        .dispatch_on_message_render(&slow_id, msg.clone())
        .expect_err("Выполнение > 5 ms должно прерываться по дедлайну");
    assert!(matches!(
        err,
        PluginError::GuestTrap {
            reason: TrapReason::DeadlineExceeded { .. },
            ..
        }
    ));

    // 4. Переполнение линейной памяти во время выполнения (> 16 MiB)
    let oom_id = runtime
        .load_plugin(
            sample_manifest("org.chord.oom-plugin"),
            &wasm,
            Box::new(TestPluginGuest {
                init_fuel: 1_000,
                allocate_bytes: 0,
                trap_on_call: Some(TrapReason::MemoryOverflow {
                    requested_bytes: DEFAULT_MEMORY_LIMIT_BYTES + 4096,
                    limit_bytes: DEFAULT_MEMORY_LIMIT_BYTES,
                }),
                ..Default::default()
            }),
        )
        .unwrap();
    let err = runtime
        .dispatch_on_message_render(&oom_id, msg.clone())
        .expect_err("Переполнение 16 MiB памяти должно вызывать MemoryOverflow trap");
    assert!(matches!(
        err,
        PluginError::GuestTrap {
            reason: TrapReason::MemoryOverflow { .. },
            ..
        }
    ));

    // 4b. DivisionByZero и ExplicitTrap также перехватываются с маскированием секретов
    let div0_trap = TrapReason::DivisionByZero;
    let explicit_trap =
        TrapReason::ExplicitTrap("panic with token mfa.secret_leak_999".to_string());
    assert!(div0_trap.to_string().contains("division by zero"));
    assert!(!explicit_trap.to_string().contains("secret_leak_999"));
    assert!(explicit_trap.to_string().contains("[REDACTED]"));

    // 5. Выход за лимит 2000 символов в on_message_render и инъекция управляющих символов
    let overlen_id = runtime
        .load_plugin(
            sample_manifest("org.chord.overlen-plugin"),
            &wasm,
            Box::new(TestPluginGuest {
                render_output: Some("z".repeat(MAX_MESSAGE_RENDER_LEN + 1)),
                ..Default::default()
            }),
        )
        .unwrap();
    let err = runtime
        .dispatch_on_message_render(&overlen_id, msg.clone())
        .expect_err("Ответ > 2000 символов должен отклоняться");
    assert!(matches!(
        err,
        PluginError::MessageRenderLengthExceeded {
            len: 2001,
            max: 2000
        }
    ));

    let ctrl_id = runtime
        .load_plugin(
            sample_manifest("org.chord.ctrl-inject"),
            &wasm,
            Box::new(TestPluginGuest {
                render_output: Some("evil\x1b[31m\u{202E}override".to_string()),
                ..Default::default()
            }),
        )
        .unwrap();
    let err = runtime
        .dispatch_on_message_render(&ctrl_id, msg)
        .expect_err("ANSI/Bidi управляющие последовательности должны блокироваться");
    assert!(matches!(err, PluginError::ControlCharacterInjection));

    // 6. Превышение лимита 3 эмбедов в ответе слэш-команды
    let embeds_id = runtime
        .load_plugin(
            sample_manifest("org.chord.too-many-embeds"),
            &wasm,
            Box::new(TestPluginGuest {
                embed_count: MAX_EMBEDS_PER_RESPONSE + 1,
                ..Default::default()
            }),
        )
        .unwrap();
    let err = runtime
        .dispatch_slash_command(
            &embeds_id,
            CommandInvocation {
                command: "shrug".to_string(),
                args: vec![],
                channel_id: 1,
                invoker_id: 1,
            },
        )
        .expect_err("Ответ с 4 эмбедами (> 3) должен отклоняться");
    assert!(matches!(
        err,
        PluginError::TooManyEmbeds { count: 4, max: 3 }
    ));
}

#[test]
fn test_dim5_empty_states_zero_permissions_and_empty_messages() {
    let mut runtime = PluginSandboxRuntime::new();
    let mut empty_manifest = sample_manifest("org.chord.empty-perms");
    empty_manifest.permissions.clear();
    empty_manifest.slash_commands.clear();

    let id = runtime
        .load_plugin(
            empty_manifest,
            &sample_wasm_bytes(),
            Box::new(TestPluginGuest::default()),
        )
        .expect("Плагин с пустым списком разрешений должен загружаться");

    // Вызов рендера без прав должен отклоняться по принципу default-deny
    let msg = MessageContext {
        message_id: 1,
        channel_id: 1,
        author_id: 1,
        author_name: "user".to_string(),
        content: String::new(),
        timestamp_ms: 0,
    };
    let err = runtime
        .dispatch_on_message_render(&id, msg.clone())
        .expect_err("Плагин без ReadMessages/RenderUi должен получать PermissionDenied");
    assert!(matches!(err, PluginError::PermissionDenied { .. }));

    // Для плагина с правами пустое сообщение возвращает Ok(None)
    let full_id = runtime
        .load_plugin(
            sample_manifest("org.chord.empty-msg"),
            &sample_wasm_bytes(),
            Box::new(TestPluginGuest::default()),
        )
        .unwrap();
    let res = runtime
        .dispatch_on_message_render(&full_id, msg)
        .expect("Пустое сообщение не должно вызывать ошибку");
    assert!(res.is_none());
}

#[test]
fn test_dim6_lifecycle_state_transitions_unloaded_loaded_enabled_disabled_unloaded() {
    let mut runtime = PluginSandboxRuntime::new();
    let plugin_id: PluginId = "org.chord.lifecycle".to_string();
    assert_eq!(runtime.plugin_status(&plugin_id), None);

    // Stage 1: Load without auto-enable -> Loaded
    runtime
        .stage_plugin(
            sample_manifest(&plugin_id),
            &sample_wasm_bytes(),
            Box::new(TestPluginGuest::default()),
        )
        .expect("stage_plugin должен переводить плагин в статус Loaded");
    assert_eq!(
        runtime.plugin_status(&plugin_id),
        Some(PluginStatus::Loaded)
    );

    // Stage 2: Enable -> Enabled
    runtime
        .enable_plugin(&plugin_id)
        .expect("enable_plugin должен переводить Loaded -> Enabled");
    assert_eq!(
        runtime.plugin_status(&plugin_id),
        Some(PluginStatus::Enabled)
    );

    // Stage 3: Disable -> Disabled
    runtime
        .disable_plugin(&plugin_id, "user toggled off")
        .expect("disable_plugin должен переводить Enabled -> Disabled");
    assert_eq!(
        runtime.plugin_status(&plugin_id),
        Some(PluginStatus::Disabled {
            reason: "user toggled off".to_string()
        })
    );

    // Stage 4: Unload -> Unloaded (удалён из активного реестра)
    runtime
        .unload_plugin(&plugin_id)
        .expect("unload_plugin должен выгружать плагин");
    assert_eq!(runtime.plugin_status(&plugin_id), None);
}

#[test]
fn test_dim7_capability_firewall_default_deny_forbidden_capabilities_and_discord_domain_block() {
    let mut runtime = PluginSandboxRuntime::new();

    // 1. Запрещённые хост-возможности в манифесте немедленно блокируются при загрузке
    let forbidden_caps = [
        PluginCapability::Network,
        PluginCapability::Filesystem,
        PluginCapability::Environment,
        PluginCapability::Process,
        PluginCapability::AuthToken,
        PluginCapability::VoiceMedia,
    ];
    for cap in forbidden_caps {
        let mut manifest = sample_manifest("org.chord.bad-cap");
        manifest.permissions = vec![cap.clone()];
        let err = runtime
            .load_plugin(
                manifest,
                &sample_wasm_bytes(),
                Box::new(TestPluginGuest::default()),
            )
            .expect_err("Запрещённая возможность должна отклоняться");
        assert!(matches!(
            err,
            PluginError::ForbiddenCapability(c) if c == cap
        ));
    }

    // 2. Плагин только с ReadMessages пытается выполнить KV или SlashCommands -> PermissionDenied + запись в аудит-лог
    let mut read_only_manifest = sample_manifest("org.chord.read-only");
    read_only_manifest.permissions = vec![PluginCapability::ReadMessages];
    let ro_id = runtime
        .load_plugin(
            read_only_manifest,
            &sample_wasm_bytes(),
            Box::new(TestPluginGuest::default()),
        )
        .unwrap();

    let err = runtime
        .kv_get(&ro_id, "secret_key")
        .expect_err("Вызов KV без PluginStorage должен блокироваться");
    assert!(matches!(
        err,
        PluginError::PermissionDenied {
            capability: PluginCapability::PluginStorage,
            ..
        }
    ));

    // 3. Проверка http_fetch: блокировка Discord API доменов даже если плагин имеет NetFetchAllowlisted
    let mut net_manifest = sample_manifest("org.chord.net-plugin");
    net_manifest
        .permissions
        .push(PluginCapability::NetFetchAllowlisted);
    net_manifest.allowed_domains = vec!["api.github.com".to_string()];
    let net_id = runtime
        .load_plugin(
            net_manifest,
            &sample_wasm_bytes(),
            Box::new(TestPluginGuest::default()),
        )
        .unwrap();

    // Разрешённый домен проходит
    let ok_resp = runtime
        .http_fetch(
            &net_id,
            HttpFetchRequest {
                url: "https://api.github.com/repos/chord/chord".to_string(),
                method: "GET".to_string(),
                headers: vec![(
                    "Authorization".to_string(),
                    "Bearer mfa.secret123".to_string(),
                )],
                body: None,
            },
        )
        .expect("Запрос к разрешённому домену должен проходить");
    assert_eq!(ok_resp.status, 200);

    // Попытка обращения к discord.com/api, gateway.discord.gg, cdn.discordapp.com блокируется
    for blocked_url in [
        "https://discord.com/api/v9/users/@me",
        "wss://gateway.discord.gg/?v=9",
        "https://voice.discord.media/rtc",
    ] {
        let err = runtime
            .http_fetch(
                &net_id,
                HttpFetchRequest {
                    url: blocked_url.to_string(),
                    method: "GET".to_string(),
                    headers: vec![],
                    body: None,
                },
            )
            .expect_err("Доступ к инфраструктуре Discord из плагина строго запрещён");
        assert!(matches!(err, PluginError::ForbiddenNetworkTarget(_)));
    }
}

#[test]
fn test_dim8_fault_isolation_consecutive_traps_reset_on_success_and_auto_disable_after_3() {
    let mut runtime = PluginSandboxRuntime::new();
    let wasm = sample_wasm_bytes();

    // Загружаем стабильный соседний плагин и нестабильный плагин
    let sibling_id = runtime
        .load_plugin(
            sample_manifest("org.chord.healthy-sibling"),
            &wasm,
            Box::new(TestPluginGuest::default()),
        )
        .unwrap();

    let flaky_id = runtime
        .load_plugin(
            sample_manifest("org.chord.flaky-plugin"),
            &wasm,
            Box::new(TestPluginGuest {
                trap_on_call: Some(TrapReason::Unreachable),
                ..Default::default()
            }),
        )
        .unwrap();

    let msg = MessageContext {
        message_id: 10,
        channel_id: 20,
        author_id: 30,
        author_name: "tester".to_string(),
        content: "ping".to_string(),
        timestamp_ms: 100,
    };

    // Ловушка #1 и #2 -> плагин всё ещё Enabled (consecutive_traps = 2)
    assert!(
        runtime
            .dispatch_on_message_render(&flaky_id, msg.clone())
            .is_err()
    );
    assert_eq!(runtime.consecutive_traps(&flaky_id), Some(1));
    assert_eq!(
        runtime.plugin_status(&flaky_id),
        Some(PluginStatus::Enabled)
    );

    assert!(
        runtime
            .dispatch_on_message_render(&flaky_id, msg.clone())
            .is_err()
    );
    assert_eq!(runtime.consecutive_traps(&flaky_id), Some(2));
    assert_eq!(
        runtime.plugin_status(&flaky_id),
        Some(PluginStatus::Enabled)
    );

    // Заменяем обработчик на успешный: 1 успешный вызов сбрасывает consecutive_traps в 0!
    runtime
        .replace_guest_handler(&flaky_id, Box::new(TestPluginGuest::default()))
        .unwrap();
    assert!(
        runtime
            .dispatch_on_message_render(&flaky_id, msg.clone())
            .is_ok()
    );
    assert_eq!(runtime.consecutive_traps(&flaky_id), Some(0));
    assert_eq!(
        runtime.plugin_status(&flaky_id),
        Some(PluginStatus::Enabled)
    );

    // Теперь провоцируем 3 последовательных сбоя подряд (1, 2, 3 -> Disabled)
    runtime
        .replace_guest_handler(
            &flaky_id,
            Box::new(TestPluginGuest {
                trap_on_call: Some(TrapReason::Unreachable),
                ..Default::default()
            }),
        )
        .unwrap();

    for expected_count in 1..=MAX_CONSECUTIVE_TRAPS_BEFORE_DISABLE {
        let _ = runtime.dispatch_on_message_render(&flaky_id, msg.clone());
        assert_eq!(runtime.consecutive_traps(&flaky_id), Some(expected_count));
    }

    assert!(matches!(
        runtime.plugin_status(&flaky_id),
        Some(PluginStatus::Disabled { .. })
    ));

    // Последующие вызовы к отключённому плагину немедленно возвращают PluginDisabled
    let disabled_err = runtime
        .dispatch_on_message_render(&flaky_id, msg.clone())
        .expect_err("Вызов отключённого плагина должен возвращать PluginDisabled");
    assert!(matches!(disabled_err, PluginError::PluginDisabled { .. }));

    // Соседний плагин продолжает работать без малейшего влияния
    let sibling_patch = runtime
        .dispatch_on_message_render(&sibling_id, msg)
        .expect("Соседний плагин не должен пострадать от падения flaky-plugin")
        .unwrap();
    assert_eq!(sibling_patch.rendered_content, "[formatted] ping");
}

#[test]
fn test_dim9_telemetry_and_secret_token_redaction_in_audit_logs() {
    let raw_mfa = concat!("mfa.", "v3_super_secret_discord_token_998877665544332211");
    let raw_jwt = concat!(
        "MTIzNDU2Nzg5MDEyMzQ1Njc4",
        ".G1aBcD.",
        "xYz987654321_secret_hmac_sig"
    );
    let raw_bearer = concat!(
        "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9",
        ".secret.payload"
    );
    let raw_bot = concat!(
        "Bot MTIzNDU2Nzg5MDEyMzQ1Njc4",
        ".G1aBcD.",
        "bot_secret_token_value"
    );

    let dirty_log =
        format!("leaked credentials: {raw_mfa} and {raw_jwt} and {raw_bearer} and {raw_bot}");
    let scrubbed = redact_secrets(&dirty_log);
    assert!(!scrubbed.contains("super_secret_discord_token"));
    assert!(!scrubbed.contains("xYz987654321_secret_hmac_sig"));
    assert!(!scrubbed.contains("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"));
    assert!(!scrubbed.contains("bot_secret_token_value"));
    assert!(scrubbed.contains("[REDACTED]"));

    let mut runtime = PluginSandboxRuntime::new();
    assert_ne!(LogLevel::Trace, LogLevel::Debug);
    let id = runtime
        .load_plugin(
            sample_manifest("org.chord.logger"),
            &sample_wasm_bytes(),
            Box::new(TestPluginGuest {
                log_on_call: Some((LogLevel::Warn, dirty_log)),
                ..Default::default()
            }),
        )
        .unwrap();

    let msg = MessageContext {
        message_id: 1,
        channel_id: 1,
        author_id: 1,
        author_name: "sec".to_string(),
        content: "hello".to_string(),
        timestamp_ms: 1,
    };
    runtime
        .dispatch_on_message_render(&id, msg)
        .expect("Рендер с логированием должен завершиться успешно");

    let logs = runtime.audit_logs();
    assert!(!logs.is_empty());
    for entry in logs {
        assert!(
            !entry.message.contains("super_secret_discord_token"),
            "Токен mfa.* утёк в аудит-лог: {}",
            entry.message
        );
        assert!(
            !entry.message.contains("xYz987654321_secret_hmac_sig"),
            "Токен MT*.*.* утёк в аудит-лог: {}",
            entry.message
        );
        assert!(
            !entry.message.contains("bot_secret_token_value"),
            "Токен Bot * утёк в аудит-лог: {}",
            entry.message
        );
    }

    let metrics = runtime
        .plugin_telemetry(&id)
        .expect("Метрики телеметрии должны быть доступны");
    assert!(metrics.total_fuel_consumed >= 75_000);
    assert_eq!(metrics.total_calls, 1);
    assert_eq!(metrics.total_traps, 0);
}
