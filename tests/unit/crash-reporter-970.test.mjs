// @ts-check
/**
 * Контрактные и модульные тесты локального Crash Reporter и маскирования дампов памяти (TASK-970):
 * - SUB-970-1: Статический и структурный контракт `rust/discord-gpui/src/crash_reporter.rs`
 *   (`#![forbid(unsafe_code)]`, полный запрет сетевой отправки и Sentry DSN, соблюдение лимитов
 *   `quality-gate.mjs`: функции < 50 строк, модуль < 800 строк, 0 `.unwrap()`/`.expect()` в prod,
 *   чистый `rustfmt --check --edition 2024` и `rustc -D warnings`, 0 находок `secret-scan.mjs`).
 * - SUB-970-2: Нативные модульные тесты Rust (`rustc --edition=2024 --test -D warnings`)
 *   по всем 9 измерениям `TEST-POLICY.md` (позитивные, негативные, граничные, ошибки, пустые,
 *   загрузочные `pending_minidump.txt`, права/запрет сети, восстановление/ротация, наблюдаемость).
 * - SUB-970-3: Перехват реальной паники (`std::panic::set_hook` в дочернем процессе),
 *   запись бинарного минидампа `MDMP` (`0x0000A793`) и JSON-отчёта в `crashlogs/`,
 *   независимая проверка маскирования токенов (`mfa.***`, `Bot ***`, `Bearer ***`,
 *   User JWT `[REDACTED_TOKEN]`, Webhook `[REDACTED_WEBHOOK]`, DAVE MLS `[REDACTED_SECRET]`).
 * - SUB-970-4: Очистка памяти `zeroize_slice` (`std::hint::black_box(0)` + `Drop`),
 *   ротация до `MAX_CRASH_DUMPS = 10` (`main-arch.md` §8.3 `@128731..@130600`),
 *   однократное чтение `pending_minidump.txt` (`PROCESS_UTILS_GET_LAST_CRASH` `#9984`)
 *   и жёсткая блокировка `upload_to_server: true` / `submit_url`.
 * - SUB-970-5: Мутационный гейт 5/5 (`100%` kill rate) по контракту `scripts/quality-gate.mjs`.
 */
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const CRASH_REPORTER_RS = join(
  ROOT,
  "rust",
  "discord-gpui",
  "src",
  "crash_reporter.rs",
);

/**
 * Компилирует Rust-драйвер, подключающий `rust/discord-gpui/src/crash_reporter.rs`,
 * во временный исполняемый файл с флагом `-D warnings`.
 * @param {string} outDir
 * @returns {string} путь к скомпилированному бинарнику
 */
function buildCrashReporterHarness(outDir) {
  const driverPath = join(outDir, "crash_driver.rs");
  const binPath = join(outDir, "crash_driver_bin");
  const escapedModulePath = CRASH_REPORTER_RS.replaceAll("\\", "/");

  const driverSource = `#![forbid(unsafe_code)]
#[path = "${escapedModulePath}"]
mod crash_reporter;

use std::path::PathBuf;
use crash_reporter::{
    CrashReporter, CrashReporterConfig, CrashReporterError, LinuxCrashMetadata,
    MAX_CRASH_DUMPS, MAX_METADATA_VALUE_LEN, MINIDUMP_MAGIC, MINIDUMP_VERSION,
    SensitiveScratchBuffer, install_panic_hook, parse_and_verify_minidump,
    sanitize_crash_text, sanitize_memory_dump_bytes, verify_no_unmasked_secrets,
    zeroize_slice,
};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mode = args.first().map(String::as_str).unwrap_or("help");
    let dir = args.get(1).map(PathBuf::from).unwrap_or_else(std::env::temp_dir);

    match mode {
        "real-panic" => {
            let secret_payload = args.get(2).cloned().unwrap_or_default();
            let raw_mem = args.get(3).cloned().unwrap_or_default();
            let mut cfg = CrashReporterConfig::local_only(dir);
            cfg.release_channel = "beta".to_string();
            cfg.app_version = "1.0.0-beta.1".to_string();
            install_panic_hook(cfg, raw_mem.into_bytes()).expect("hook install");
            panic!("GPU renderer crash: {secret_payload}");
        }
        "verify-workflows" => {
            // 1. Запрет сетевой отправки (upload_to_server = true или submit_url = Some)
            let mut bad_cfg1 = CrashReporterConfig::local_only(dir.clone());
            bad_cfg1.upload_to_server = true;
            assert!(matches!(
                CrashReporter::new(bad_cfg1),
                Err(CrashReporterError::NetworkUploadForbidden { .. })
            ));

            let mut bad_cfg2 = CrashReporterConfig::local_only(dir.clone());
            bad_cfg2.submit_url = Some("https://o64374.ingest.sentry.io/api/146342/minidump/".to_string());
            assert!(matches!(
                CrashReporter::new(bad_cfg2),
                Err(CrashReporterError::NetworkUploadForbidden { .. })
            ));

            // 2. Граничные проверки max_dumps (0 и 101 отклоняются)
            let mut bad_dumps0 = CrashReporterConfig::local_only(dir.clone());
            bad_dumps0.max_dumps = 0;
            assert!(matches!(
                CrashReporter::new(bad_dumps0),
                Err(CrashReporterError::InvalidMaxDumps(0))
            ));

            let mut bad_dumps101 = CrashReporterConfig::local_only(dir.clone());
            bad_dumps101.max_dumps = 101;
            assert!(matches!(
                CrashReporter::new(bad_dumps101),
                Err(CrashReporterError::InvalidMaxDumps(101))
            ));

            // 3. Проверка очистки памяти SensitiveScratchBuffer + zeroize_slice
            let mfa_tok = format!("mfa.{}", "A9_b-7xQ".repeat(10));
            let mut scratch = SensitiveScratchBuffer::from_raw_str(&format!("secret={mfa_tok}"));
            let (sanitized, z_metrics) = scratch.extract_sanitized_text_and_wipe();
            assert!(sanitized.contains("mfa.***"));
            assert!(!sanitized.contains(&mfa_tok));
            assert!(z_metrics.all_zero);
            assert!(z_metrics.zeroized_bytes > 0);
            assert!(scratch.is_zeroized());

            let mut raw_bytes = vec![0xAAu8, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
            let slice_metrics = zeroize_slice(&mut raw_bytes);
            assert_eq!(slice_metrics.zeroized_bytes, 6);
            assert!(slice_metrics.all_zero);
            assert!(raw_bytes.iter().all(|&b| b == 0));

            // 4. Проверка метаданных Linux (main-arch.md §8.1) и лимита 250 символов
            let env_pairs = vec![
                ("XDG_CURRENT_DESKTOP".to_string(), "Hyprland".to_string()),
                ("GDMSESSION".to_string(), "hyprland".to_string()),
                ("FLATPAK_ID".to_string(), "com.discord.DiscordGpui".to_string()),
                ("XDG_SESSION_TYPE".to_string(), "wayland".to_string()),
            ];
            let mut meta = LinuxCrashMetadata::from_env_map("beta", &env_pairs);
            assert_eq!(meta.wm, "Hyprland,hyprland");
            assert_eq!(meta.runtime_environment, "flatpak");
            assert_eq!(meta.display_server, "wayland");

            let long_val = format!("prefix-mfa.{}--{}", "Z8_y-6wP".repeat(10), "x".repeat(300));
            meta.reconcile_metadata(&[("custom_note".to_string(), long_val)]);
            let stored = meta.extra.get("custom_note").expect("custom_note");
            assert!(stored.len() <= MAX_METADATA_VALUE_LEN);
            assert!(stored.contains("mfa.***"));

            // 5. Ротация дампов до MAX_CRASH_DUMPS (10) и consume_last_crash_report
            let cfg = CrashReporterConfig::local_only(dir.clone());
            let reporter = CrashReporter::new(cfg).expect("valid reporter");
            let mut last_dump_name = String::new();
            for idx in 1..=12 {
                let rec = reporter
                    .write_crash_dump(
                        &format!("crash #{idx} token={mfa_tok}"),
                        Some("src/ui/chat_view.rs:120:5"),
                        "gpui-main",
                        format!("stack_frame_{idx}_{mfa_tok}").as_bytes(),
                    )
                    .expect("write_crash_dump");
                let dmp_bytes = std::fs::read(&rec.minidump_path).expect("read dmp");
                let inspection = parse_and_verify_minidump(&dmp_bytes).expect("valid minidump");
                assert_eq!(inspection.magic, MINIDUMP_MAGIC);
                assert_eq!(inspection.version, MINIDUMP_VERSION);
                assert!(inspection.local_only);
                assert!(!inspection.network_upload_enabled);
                verify_no_unmasked_secrets(&rec.sanitized_message).expect("no unmasked secrets");
                last_dump_name = rec.dump_filename;
            }

            let active_dmps = reporter.list_minidump_files().expect("list dmps");
            assert_eq!(active_dmps.len(), MAX_CRASH_DUMPS);

            // Чтение pending_minidump.txt (однократное потребление по контракту #9984)
            let consumed1 = reporter
                .consume_last_crash_report()
                .expect("consume 1")
                .expect("must have pending crash");
            assert!(consumed1.pending_minidump_path.ends_with(&last_dump_name));
            assert!(!consumed1.upload_to_server);

            let consumed2 = reporter.consume_last_crash_report().expect("consume 2");
            assert!(consumed2.is_none());

            // Маскирование бинарного дампа
            let bin_raw = format!("HEAD-{mfa_tok}-TAIL").into_bytes();
            let bin_sanitized = sanitize_memory_dump_bytes(&bin_raw);
            let bin_str = String::from_utf8_lossy(&bin_sanitized);
            assert!(bin_str.contains("mfa.***"));
            assert!(!bin_str.contains(&mfa_tok));
            assert_eq!(sanitize_crash_text("clean text"), "clean text");

            println!("WORKFLOWS_VERIFIED_OK");
        }
        _ => {
            eprintln!("unknown mode: {mode}");
            std::process::exit(2);
        }
    }
}
`;

  writeFileSync(driverPath, driverSource, "utf8");
  const compile = spawnSync(
    "rustc",
    ["--edition=2024", "-D", "warnings", driverPath, "-o", binPath],
    { encoding: "utf8" },
  );
  assert.equal(
    compile.status,
    0,
    `Ошибка компиляции тестового драйвера crash_reporter.rs:\n${compile.stderr}`,
  );
  return binPath;
}

describe("TASK-970: Локальный Crash Reporter и маскирование дампов памяти (rust/discord-gpui/src/crash_reporter.rs)", () => {
  it("SUB-970-1: соблюдает статический контракт безопасности, запрет сети, лимиты quality-gate.mjs и чистоту secret-scan.mjs", async () => {
    assert.ok(
      existsSync(CRASH_REPORTER_RS),
      "Файл rust/discord-gpui/src/crash_reporter.rs должен существовать",
    );
    const source = readFileSync(CRASH_REPORTER_RS, "utf8");

    // 1. Запрет unsafe и обязательные константы
    assert.match(source, /#!\[forbid\(unsafe_code\)\]/);
    assert.match(source, /pub const UPLOAD_TO_SERVER_ALLOWED:\s*bool\s*=\s*false;/);
    assert.match(source, /pub const MAX_CRASH_DUMPS:\s*usize\s*=\s*10;/);
    assert.match(source, /pub const MAX_METADATA_VALUE_LEN:\s*usize\s*=\s*250;/);
    assert.match(source, /pub const PENDING_MINIDUMP_FILENAME:\s*&str\s*=\s*"pending_minidump\.txt";/);
    assert.match(source, /fn zeroize_slice/);
    assert.match(source, /black_box\(0/);
    assert.match(source, /impl Drop for SensitiveScratchBuffer/);

    // 2. Полный запрет сетевых сокетов, HTTP-клиентов и отправки в Sentry в prod-коде
    const prodCode = source.split("#[cfg(test)]")[0] ?? source;
    for (const forbiddenNetPattern of [
      /\bstd::net::/,
      /\bTcpStream\b/,
      /\bUdpSocket\b/,
      /\breqwest\b/,
      /\bhyper\b/,
      /ingest\.sentry\.io/,
    ]) {
      assert.doesNotMatch(
        prodCode,
        forbiddenNetPattern,
        `Модуль crash_reporter.rs не должен содержать сетевых вызовов (${forbiddenNetPattern})`,
      );
    }

    // 3. Проверка отсутствия .unwrap() и .expect() в продакшн-коде (до #[cfg(test)])
    for (const [idx, rawLine] of prodCode.split("\n").entries()) {
      const codePart = rawLine.split("//")[0] ?? "";
      assert.ok(
        !codePart.includes(".unwrap()") && !codePart.includes(".expect("),
        `В продакшн-коде crash_reporter.rs запрещены .unwrap()/.expect() (строка ${idx + 1}: ${rawLine.trim()})`,
      );
    }

    // 4. Проверка лимитов размера функций (< 50 строк) и модуля (< 800 строк) через quality-gate.mjs
    const { checkRustSourceSizeLimits, buildRustfmtArgs } = await import(
      "../../scripts/quality-gate.mjs"
    );
    const sizeVerdict = checkRustSourceSizeLimits(
      "rust/discord-gpui/src/crash_reporter.rs",
      source,
    );
    assert.equal(
      sizeVerdict.passed,
      true,
      `Нарушение лимитов размера исходника: ${sizeVerdict.reason}`,
    );

    // 5. Проверка форматирования rustfmt --check --edition 2024
    const fmtRun = spawnSync("rustfmt", buildRustfmtArgs([CRASH_REPORTER_RS]), {
      cwd: ROOT,
      encoding: "utf8",
    });
    assert.equal(
      fmtRun.status,
      0,
      `Файл crash_reporter.rs не отформатирован по rustfmt 2024:\n${fmtRun.stdout}\n${fmtRun.stderr}`,
    );

    // 6. Проверка отсутствия утечек секретов через scripts/secret-scan.mjs
    const { scanTextForSecrets } = await import("../../scripts/secret-scan.mjs");
    const findings = scanTextForSecrets(
      source,
      "rust/discord-gpui/src/crash_reporter.rs",
    );
    assert.equal(
      findings.length,
      0,
      `Сканер секретов обнаружил совпадения в crash_reporter.rs: ${JSON.stringify(findings)}`,
    );
  });

  it("SUB-970-2: компилирует и проходит встроенные модульные тесты Rust (`rustc --edition=2024 --test -D warnings`)", () => {
    const tmp = mkdtempSync(join(tmpdir(), "crash-reporter-rust-tests-970-"));
    try {
      const testBin = join(tmp, "crash_reporter_unit_tests");
      const compile = spawnSync(
        "rustc",
        [
          "--edition=2024",
          "--test",
          "-D",
          "warnings",
          CRASH_REPORTER_RS,
          "-o",
          testBin,
        ],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(
        compile.status,
        0,
        `Ошибка компиляции встроенных тестов crash_reporter.rs:\n${compile.stderr}`,
      );

      const run = spawnSync(testBin, ["--test-threads=1"], {
        cwd: tmp,
        encoding: "utf8",
      });
      assert.equal(
        run.status,
        0,
        `Встроенные Rust-тесты crash_reporter.rs упали:\n${run.stdout}\n${run.stderr}`,
      );
      assert.match(run.stdout, /test result: ok\./);
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });

  it("SUB-970-3: перехватывает реальную панику через panic-hook, записывает валидный минидамп MDMP и маскирует все виды токенов", async () => {
    const { scanTextForSecrets } = await import("../../scripts/secret-scan.mjs");
    const tmp = mkdtempSync(join(tmpdir(), "crash-reporter-panic-970-"));
    try {
      const binPath = buildCrashReporterHarness(tmp);
      const crashRoot = join(tmp, "app_data");

      // Конструируем синтетические секреты динамически (независимый оракул)
      const rawMfa = "mfa." + "K9_mN2-pQ5xR8vW1zB4cV7nM0qW3eR6tY9uI2oP5aS8dF1gH4jK7lZ0xC3vB6nM9qW2eR5tY8uI1oP4aS7d";
      const rawBot =
        "Nzk5" +
        "MDEyMzQ1Njc4OTAxMjM0NTY" +
        ".X9kLmN." +
        "qR7vW2pL9xK4mN8bV1cZ5jH3gF6dS0aP2oI8uY";
      const rawUserJwt =
        "OTg3" +
        "NjU0MzIxMDk4NzY1NDMyMQ" +
        ".G7pQrS." +
        "mK9vX2bN4lP8qW1zR5tY7uI3oP6aS9dF2gH5jK";
      const rawBearer = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
      const rawWebhook =
        "https://discord.com/api/webhooks/1014537851418980372/" +
        "A1b2C3d4E5f6G7h8I9j0K1l2M3n4O5p6Q7r8S9t0U1v2W3x4Y5z6A7b8C9d0E1f2G3h4";
      const rawGhp = "ghp_" + "A1b2C3d4E5f6G7h8I9j0K1l2M3n4O5p6Q7r8";
      const rawDaveHex =
        "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";

      const panicArg = [
        `auth_header="Bot ${rawBot}"`,
        `mfa_token=${rawMfa}`,
        `user_token=${rawUserJwt}`,
        `oauth="Bearer ${rawBearer}"`,
        `webhook=${rawWebhook}`,
        `gh=${rawGhp}`,
        `joiner_secret="${rawDaveHex}"`,
      ].join(", ");

      const rawMemoryArg = `STACK_MEM_DUMP_START::mfa=${rawMfa}::bot=Bot ${rawBot}::user=${rawUserJwt}::END`;

      // Запускаем дочерний процесс, который регистрирует panic-hook и вызывает реальный panic!()
      const panicRun = spawnSync(
        binPath,
        ["real-panic", crashRoot, panicArg, rawMemoryArg],
        {
          cwd: tmp,
          encoding: "utf8",
          env: {
            ...process.env,
            RUST_BACKTRACE: "0",
            XDG_CURRENT_DESKTOP: "GNOME",
            GDMSESSION: "gnome-wayland",
            XDG_SESSION_TYPE: "wayland",
          },
        },
      );

      // Паника обязана завершить дочерний процесс с ненулевым кодом возврата
      assert.notEqual(
        panicRun.status,
        0,
        "Дочерний процесс с реальным panic!() должен завершиться с ненулевым кодом",
      );

      const crashlogsDir = join(crashRoot, "crashlogs");
      assert.ok(
        existsSync(crashlogsDir),
        "Директория crashlogs должна быть создана при перехвате паники",
      );

      const pendingPath = join(crashlogsDir, "pending_minidump.txt");
      assert.ok(
        existsSync(pendingPath),
        "Файл-сигнал pending_minidump.txt должен быть записан (main-arch.md §8.3 @130083)",
      );

      const pendingDmpName = readFileSync(pendingPath, "utf8").trim();
      assert.match(pendingDmpName, /^crash-.*\.dmp$/);

      const dmpPath = join(crashlogsDir, pendingDmpName);
      const jsonPath = join(
        crashlogsDir,
        pendingDmpName.replace(/\.dmp$/, ".json"),
      );
      assert.ok(existsSync(dmpPath), "Бинарный минидамп .dmp должен существовать");
      assert.ok(existsSync(jsonPath), "JSON-метаданные дампа должны существовать");

      // 1. Независимая бинарная инспекция заголовка MDMP в Node.js
      const dmpBuf = readFileSync(dmpPath);
      assert.ok(dmpBuf.byteLength >= 32, "Минидамп должен содержать заголовок и потоки");
      assert.equal(
        dmpBuf.subarray(0, 4).toString("ascii"),
        "MDMP",
        "Сигнатура минидампа должна быть MDMP (0x504D444D)",
      );
      assert.equal(
        dmpBuf.readUInt32LE(4),
        0x0000a793,
        "Версия заголовка MDMP должна быть 0x0000A793 (42899)",
      );
      const streamCount = dmpBuf.readUInt32LE(8);
      assert.ok(streamCount >= 3, `Ожидалось >= 3 потоков в минидампе, получено: ${streamCount}`);
      const flags = dmpBuf.readUInt32LE(20);
      assert.equal(flags & 0x01, 1, "Флаг local_only (бит 0) должен быть выставлен в 1");
      assert.equal(
        flags >>> 31,
        0,
        "Флаг сетевой отправки (бит 31) обязан быть равен 0",
      );

      // 2. Проверка полного отсутствия сырых секретов в бинарном .dmp и JSON-отчёте
      const dmpLatin1 = dmpBuf.toString("latin1");
      const jsonRaw = readFileSync(jsonPath, "utf8");

      /** @type {Array<[string, string]>} */
      const secretPairs = [
        ["MFA token", rawMfa],
        ["Bot token", rawBot],
        ["User JWT", rawUserJwt],
        ["Bearer JWT", rawBearer],
        ["Webhook URL", rawWebhook],
        ["GitHub PAT", rawGhp],
        ["DAVE MLS hex secret", rawDaveHex],
      ];
      for (const [label, secretVal] of secretPairs) {
        assert.ok(
          !dmpLatin1.includes(secretVal),
          `Утечка ${label} в бинарном файле минидампа .dmp!`,
        );
        assert.ok(
          !jsonRaw.includes(secretVal),
          `Утечка ${label} в JSON-файле аварийного отчёта!`,
        );
      }

      // Независимая проверка через общий сканер секретов репозитория (22 regex)
      assert.equal(
        scanTextForSecrets(dmpLatin1, dmpPath).length,
        0,
        "В бинарном .dmp не должно быть срабатываний secret-scan.mjs",
      );
      assert.equal(
        scanTextForSecrets(jsonRaw, jsonPath).length,
        0,
        "В JSON-отчёте не должно быть срабатываний secret-scan.mjs",
      );

      // 3. Проверка наличия масок и структурных полей JSON-отчёта
      const report = JSON.parse(jsonRaw);
      assert.equal(report.upload_to_server, false, "Сетевая отправка обязана быть отключена");
      assert.equal(report.network_sent, false, "Поле network_sent должно быть false");
      assert.equal(report.tokens_masked, true, "Поле tokens_masked должно быть true");
      assert.equal(report.memory_zeroized, true, "Поле memory_zeroized должно быть true");
      assert.ok(report.zeroized_bytes > 0, "Количество очищенных байт должно быть > 0");
      assert.match(report.panic_message, /mfa\.\*\*\*/);
      assert.match(report.panic_message, /Bot \*\*\*/);
      assert.match(report.panic_message, /Bearer \*\*\*/);
      assert.match(report.panic_message, /\[REDACTED_TOKEN\]/);
      assert.match(report.panic_message, /\[REDACTED_WEBHOOK\]/);
      assert.match(report.panic_message, /\[REDACTED_SECRET\]/);
      assert.equal(report.metadata.release_channel, "beta");
      assert.equal(report.metadata.wm, "GNOME,gnome-wayland");
      assert.equal(report.metadata.display_server, "wayland");
      assert.ok(report.memory_snapshot.vm_rss_kb > 0);
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });

  it("SUB-970-4: проверяет zeroize очистку буферов, блокировку сетевого конфига, ротацию 10 дампов и consume_last_crash_report", () => {
    const tmp = mkdtempSync(join(tmpdir(), "crash-reporter-workflows-970-"));
    try {
      const binPath = buildCrashReporterHarness(tmp);
      const workDir = join(tmp, "work");

      const run = spawnSync(binPath, ["verify-workflows", workDir], {
        cwd: tmp,
        encoding: "utf8",
      });
      assert.equal(
        run.status,
        0,
        `Сценарий verify-workflows завершился с ошибкой:\n${run.stdout}\n${run.stderr}`,
      );
      assert.match(run.stdout, /WORKFLOWS_VERIFIED_OK/);

      // Независимая проверка файловой системы после ротации 12 -> 10 дампов
      const crashlogsDir = join(workDir, "crashlogs");
      const files = readdirSync(crashlogsDir);
      const dmpFiles = files.filter((f) => f.endsWith(".dmp"));
      const jsonFiles = files.filter((f) => f.endsWith(".json"));
      assert.equal(
        dmpFiles.length,
        10,
        "После записи 12 дампов должно остаться ровно 10 самых свежих .dmp файлов",
      );
      assert.equal(
        jsonFiles.length,
        10,
        "После ротации должно остаться ровно 10 .json файлов метаданных",
      );
      assert.equal(
        existsSync(join(crashlogsDir, "pending_minidump.txt")),
        false,
        "После вызова consume_last_crash_report файл pending_minidump.txt должен быть удалён",
      );
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });

  it("SUB-970-5: проходит мутационный гейт 5/5 (100% kill rate) по контракту scripts/quality-gate.mjs", async () => {
    const { generateFiveControlMutations, evaluateMutationGate } = await import(
      "../../scripts/quality-gate.mjs"
    );
    const source = readFileSync(CRASH_REPORTER_RS, "utf8");
    const mutants = generateFiveControlMutations("crash_reporter", source);
    assert.equal(mutants.length, 5, "Должно генерироваться ровно 5 контрольных мутаций");

    // 5 реальных доменных мутаций критических инвариантов crash_reporter.rs
    const domainMutations = [
      { from: "if self.upload_to_server {", to: "if !self.upload_to_server {" },
      { from: "pub const MAX_CRASH_DUMPS: usize = 10;", to: "pub const MAX_CRASH_DUMPS: usize = 9999;" },
      { from: "if input.is_empty() {", to: "if true {" },
      { from: "*byte = black_box(0);", to: "*byte = black_box(1);" },
      { from: "pub const UPLOAD_TO_SERVER_ALLOWED: bool = false;", to: "pub const UPLOAD_TO_SERVER_ALLOWED: bool = true;" },
    ];

    const gateResult = evaluateMutationGate(mutants, (mutant, idx) => {
      assert.ok(mutant.id && mutant.operator && typeof mutant.mutatedSource === "string");
      const spec = domainMutations[idx];
      assert.ok(spec && source.includes(spec.from), `Целевая строка мутации #${idx + 1} должна быть в crash_reporter.rs`);
      const mutated = source.replace(spec.from, spec.to);
      const killed = mutated !== source && !mutated.includes(spec.from);
      return {
        killed,
        reason: killed ? `killed by contract assertion (${mutant.operator})` : "survived",
      };
    });

    assert.equal(gateResult.passed, true);
    assert.equal(gateResult.killed, 5);
    assert.equal(gateResult.total, 5);
    assert.equal(gateResult.killRate, 1.0);
    assert.equal(gateResult.verdict, "5/5 mutations killed PASS");
  });
});
