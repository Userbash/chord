//! Contract tests for `RW51-EPIC1-T1`: `specta` / `tauri-specta` builder architecture
//! and strict TypeScript/JSDoc bindings generation (`ui/bindings.js` and `ui/bindings.d.ts`).

#![forbid(unsafe_code)]

#[path = "../src/bindings.rs"]
pub mod bindings;

use std::fs;
use std::path::Path;

#[test]
fn generate_specta_builder_registers_core_commands_and_dto_types() {
    let builder = bindings::generate_specta_builder();

    let command_names: Vec<&str> = builder.commands().iter().map(|c| c.name).collect();
    for expected_cmd in [
        "ready",
        "read_state",
        "channels",
        "messages_page",
        "members_page",
        "settings",
        "voice_connect",
        "voice_rtc_status",
        "voice_set_audio_device",
    ] {
        assert!(
            command_names.contains(&expected_cmd),
            "expected command `{expected_cmd}` in SpectaBuilder, found: {command_names:?}"
        );
    }

    let type_names: Vec<&str> = builder.types().iter().map(|t| t.name).collect();
    for expected_type in [
        "ReadyUserDto",
        "SessionSummaryDto",
        "GuildSummaryDto",
        "ReadyPayloadDto",
        "GuildSyncSummaryDto",
        "VoiceRtcStatusDto",
        "ChannelAccessDto",
        "LiveSyncSnapshotDto",
        "VoiceConnectOptionsDto",
        "VoiceConnectResultDto",
    ] {
        assert!(
            type_names.contains(&expected_type),
            "expected DTO `{expected_type}` in SpectaBuilder, found: {type_names:?}"
        );
    }
}

#[test]
fn specta_builder_exports_valid_js_and_dts_files_without_any_types() {
    let builder = bindings::generate_specta_builder();
    builder
        .validate_no_any()
        .expect("generated bindings must not contain any `any` types");

    let (js_path, dts_path) = builder
        .export_default_ui_bindings()
        .expect("export ui/bindings.js and ui/bindings.d.ts");

    assert!(js_path.exists(), "ui/bindings.js must exist on disk");
    assert!(dts_path.exists(), "ui/bindings.d.ts must exist on disk");

    let js_content = fs::read_to_string(&js_path).expect("read ui/bindings.js");
    let dts_content = fs::read_to_string(&dts_path).expect("read ui/bindings.d.ts");

    assert_eq!(js_content, builder.export_javascript());
    assert_eq!(dts_content, builder.export_typescript());

    bindings::validate_no_any_in_source(&js_content, "ui/bindings.js")
        .expect("ui/bindings.js must be free of `any`");
    bindings::validate_no_any_in_source(&dts_content, "ui/bindings.d.ts")
        .expect("ui/bindings.d.ts must be free of `any`");

    assert!(
        js_content.contains("async ready()"),
        "ui/bindings.js must expose generated ready() command method"
    );
    assert!(
        js_content.contains("export const commands ="),
        "ui/bindings.js must export default `commands` client"
    );
    assert!(
        dts_content.contains("ready(): Promise<ReadyPayloadDto>;"),
        "ui/bindings.d.ts must declare ready(): Promise<ReadyPayloadDto>"
    );
    assert!(
        dts_content.contains("export declare const commands: TypedCommands;"),
        "ui/bindings.d.ts must declare exported `commands` constant"
    );
}

#[test]
fn specta_builder_tempdir_export_and_negative_any_detector_work() {
    let builder = bindings::generate_specta_builder();
    let temp = tempfile::tempdir().expect("create tempdir");
    let js_file = temp.path().join("bindings.js");
    let dts_file = temp.path().join("bindings.d.ts");

    builder
        .export_to_files(&js_file, &dts_file)
        .expect("export bindings to tempdir");

    assert!(js_file.exists());
    assert!(dts_file.exists());

    // Negative / mutation checks for `any` type rejection:
    for forbidden_sample in [
        "export type Foo = any;",
        "function bar(x: Record<string, any>): Promise<any>;",
        "/** @param {any} val */",
        "Array<any>",
    ] {
        let res = bindings::validate_no_any_in_source(forbidden_sample, "sample.d.ts");
        assert!(
            res.is_err(),
            "validate_no_any_in_source must reject sample containing `any`: {forbidden_sample}"
        );
    }
}

#[test]
fn cargo_toml_and_app_js_wire_specta_and_commands_ready() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cargo_toml =
        fs::read_to_string(manifest_dir.join("Cargo.toml")).expect("read Cargo.toml");
    assert!(
        cargo_toml.contains("specta") && cargo_toml.contains("tauri-specta"),
        "Cargo.toml must declare specta and tauri-specta configuration"
    );

    let app_js = fs::read_to_string(manifest_dir.join("ui/app.js")).expect("read ui/app.js");
    assert!(
        app_js.contains("commands.ready()"),
        "ui/app.js must invoke generated commands.ready()"
    );
}
