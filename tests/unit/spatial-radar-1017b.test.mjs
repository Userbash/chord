// @ts-check
/**
 * Контрактные и модульные тесты интерактивного радара участников голосового канала `SpatialRadar`
 * с расстановкой аватаров в пространстве и сохранением координат в SQLite (TASK-1017B):
 * - SUB-1017B-0: Статический и компиляционный контракт `rust/discord-gpui/src/voice/spatial_radar.rs`
 *   (`#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()` в продуктовом коде, соблюдение лимитов
 *   `scripts/quality-gate.mjs`: функции < 50 строк, модуль < 800 строк, чистый `rustfmt --check --edition 2024`,
 *   компиляция `rustc --test --edition 2024 -D warnings` без предупреждений, 0 утечек в `secret-scan.mjs`).
 * - SUB-1017B-1: Сверка карточек `TASK-1017B`, `TASK-1017A` и `TASK-986` в `waves/kanban-board.json`,
 *   геометрии `Spatial Radar Canvas` (`240px circle`, `padding: 8px`, `spacing: 0px`), дизайн-токенов
 *   (`#1e1f22`, `#2b2d31`, `#ffffff`, `#5865f2`, `#3f4147`, кольцо речи `#23a55a`), интерактивных состояний
 *   `radar-avatar` (`positioned` -> `drag icon` -> `dragging`), а также RE-оракулов голосового UI
 *   `/home/sanya/re-deep/voice/ui/01-candidates.json` (`voiceUser__07f91 clickable__07f91 userSmall__07f91`,
 *   `voiceChannelEffectsContainer__2f4f7`, `rtcConnectionStatusWrapper__06d62`),
 *   `/home/sanya/re-deep/voice/ui/02-requiremodule-voice.json` (`isSpatialAudioEnabled`, `setOutputVolume`)
 *   и `docs/re/frontend-d03.json` (`avatar_fdd9da`, `tooltip_c36707`).
 * - SUB-1017B-2: WGPU-радар (`SpatialRadarCanvas`, круг `240px`, центр `(120, 120)`, рабочий радиус `112px`),
 *   тригонометрическое преобразование полярных координат `(azimuth_deg -180..+180, distance_m 1..50)` <->
 *   декартовых координат холста `(x_px, y_px)`, hit-test аватаров (`32px`, радиус захвата `16px`),
 *   перетаскивание мышью с квантованием/фиксацией угла азимута и ограничением периметром круга.
 * - SUB-1017B-3: SQLite-привязка (`SpatialRadarSqliteStore`, схема `spatial_radar_positions`, заголовок
 *   `SQLite format 3\0`, `PRAGMA journal_mode = WAL`, параметризованный `UPSERT ?1..?8`, сохранение и
 *   восстановление индивидуальных координат участников по `(channel_id, user_id)` с контрольной суммой FNV-1a).
 * - SUB-1017B-4: Покрытие всех 9 обязательных измерений `TEST-POLICY.md` (позитивные, негативные, граничные:
 *   Snowflake `0`/`u64::MAX`, азимут `-180°..+180°`, дистанция `1..50 м`, угол места `-90°..+90°`,
 *   2000 символов, 10 файлов/25 МБ, `offset=9975`, громкость `0..200%`, бейдж `99+`; ошибки HTTP
 *   `400/401/403/404/413/429 retry_after_ms/5xx`; пустой радар `0` пиров; прогрев `skeleton`/`HTTP 202`;
 *   права `VIEW_CHANNEL`/`CONNECT`/`SPEAK`/`AGE_GATED_SPACES`; откат при повреждении страницы SQLite;
 *   маскирование секретов).
 * - SUB-1017B-5: Мутационный гейт `5/5` (`100%` kill rate) для `rust/discord-gpui/src/voice/spatial_radar.rs`.
 */
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, before, describe, it } from "node:test";
import { fileURLToPath } from "node:url";

import {
  checkRustSourceSizeLimits,
  evaluateMutationGate,
} from "../../scripts/quality-gate.mjs";
import { scanTextForSecrets } from "../../scripts/secret-scan.mjs";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const RUST_MODULE_REL_PATH = "rust/discord-gpui/src/voice/spatial_radar.rs";
const RUST_MODULE_PATH = join(ROOT, RUST_MODULE_REL_PATH);
const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const RE_VOICE_CANDIDATES_PATH = "/home/sanya/re-deep/voice/ui/01-candidates.json";
const RE_VOICE_MODULE_PATH =
  "/home/sanya/re-deep/voice/ui/02-requiremodule-voice.json";
const RE_DEEP_FRONTEND_D03_PATH = "/home/sanya/re-deep/frontend-d03.json";
const DOCS_FRONTEND_D03_PATH = join(ROOT, "docs", "re", "frontend-d03.json");

/** @type {string} */
let workDir = "";
/** @type {string} */
let testBinPath = "";

/**
 * Компилирует `rust/discord-gpui/src/voice/spatial_radar.rs` через `rustc --test --edition 2024 -D warnings`.
 * @param {string} sourcePath
 * @param {string} outputBin
 * @returns {{ status: number | null, stdout: string, stderr: string }}
 */
function compileRustModuleTests(sourcePath, outputBin) {
  const res = spawnSync(
    "rustc",
    [
      "--test",
      "--edition",
      "2024",
      "-D",
      "warnings",
      sourcePath,
      "-o",
      outputBin,
    ],
    {
      cwd: ROOT,
      encoding: "utf8",
      timeout: 30_000,
    },
  );
  return {
    status: res.status,
    stdout: res.stdout ?? "",
    stderr: res.stderr ?? "",
  };
}

/**
 * Запускает скомпилированный тестовый бинарник с опциональным фильтром теста.
 * @param {string} binPath
 * @param {string} [filter]
 * @returns {{ status: number | null, stdout: string, stderr: string }}
 */
function runCompiledRustTests(binPath, filter) {
  const args = filter ? [filter, "--nocapture"] : ["--nocapture"];
  const res = spawnSync(binPath, args, {
    cwd: ROOT,
    encoding: "utf8",
    timeout: 15_000,
  });
  return {
    status: res.status,
    stdout: res.stdout ?? "",
    stderr: res.stderr ?? "",
  };
}

/**
 * Независимый JS-оракул перевода полярных координат участника `(azimuthDeg, distanceM)`
 * в координаты круглого радара `240px` (центр `(120, 120)`, рабочий радиус `112px`).
 * @param {number} azimuthDeg
 * @param {number} distanceM
 * @returns {{ xPx: number, yPx: number, radiusPx: number }}
 */
function oraclePolarToRadarXy(azimuthDeg, distanceM) {
  const centerX = 120.0;
  const centerY = 120.0;
  const usableRadius = 112.0;
  const minDist = 1.0;
  const maxDist = 50.0;

  let normAz = ((((azimuthDeg + 180) % 360) + 360) % 360) - 180;
  if (Math.abs(normAz - -180) < 1e-6 && azimuthDeg > 0) {
    normAz = 180;
  }
  const clampedDist = Math.max(minDist, Math.min(maxDist, distanceM));
  const normR = (clampedDist - minDist) / (maxDist - minDist);
  const radiusPx = normR * usableRadius;
  const azRad = (normAz * Math.PI) / 180;
  const xPx = centerX + radiusPx * Math.sin(azRad);
  const yPx = centerY - radiusPx * Math.cos(azRad);

  return { xPx, yPx, radiusPx };
}

/**
 * Независимый JS-оракул обратного перевода точки холста `(xPx, yPx)` в `(azimuthDeg, distanceM)`
 * с опциональным шагом фиксации угла `snapStepDeg`.
 * @param {number} xPx
 * @param {number} yPx
 * @param {number | null} [snapStepDeg]
 * @returns {{ azimuthDeg: number, distanceM: number, clampedRadiusPx: number }}
 */
function oracleRadarXyToPolar(xPx, yPx, snapStepDeg = null) {
  const centerX = 120.0;
  const centerY = 120.0;
  const usableRadius = 112.0;
  const minDist = 1.0;
  const maxDist = 50.0;

  const dx = xPx - centerX;
  const dy = centerY - yPx;
  const rawRadius = Math.hypot(dx, dy);
  const clampedRadiusPx = Math.min(usableRadius, rawRadius);

  if (rawRadius < 1e-4) {
    return { azimuthDeg: 0, distanceM: minDist, clampedRadiusPx: 0 };
  }

  let azimuthDeg = (Math.atan2(dx, dy) * 180) / Math.PI;
  if (snapStepDeg && snapStepDeg > 0) {
    azimuthDeg = Math.round(azimuthDeg / snapStepDeg) * snapStepDeg;
  }
  azimuthDeg = ((((azimuthDeg + 180) % 360) + 360) % 360) - 180;
  if (Math.abs(azimuthDeg - -180) < 1e-6 && dx >= 0) {
    azimuthDeg = 180;
  }

  const distanceM =
    minDist + (clampedRadiusPx / usableRadius) * (maxDist - minDist);
  return { azimuthDeg, distanceM, clampedRadiusPx };
}

describe("TASK-1017B: Интерактивный радар участников голосового канала SpatialRadar с сохранением координат в SQLite (rust/discord-gpui/src/voice/spatial_radar.rs)", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "spatial-radar-1017b-"));
    testBinPath = join(workDir, "spatial_radar_1017b_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-1017B-0: проверяет статический и компиляционный контракт rust/discord-gpui/src/voice/spatial_radar.rs (<50 строк/fn, <800 строк/файл, #![forbid(unsafe_code)], rustfmt 2024, 0 .unwrap()/.expect() и отсутствие секретов)", () => {
    assert.ok(
      existsSync(RUST_MODULE_PATH),
      `Отсутствует Rust-модуль ${RUST_MODULE_REL_PATH}`,
    );

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    assert.match(
      rustSource,
      /^#!\[forbid\(unsafe_code\)\]/m,
      "В модуле spatial_radar.rs обязательна директива #![forbid(unsafe_code)]",
    );

    const prodSection = rustSource.split("#[cfg(test)]")[0] ?? "";
    assert.equal(
      prodSection.includes(".unwrap()") || prodSection.includes(".expect("),
      false,
      "В продуктовом коде spatial_radar.rs запрещены .unwrap() и .expect()",
    );

    for (const requiredSymbol of [
      'pub const RADAR_CONTAINER: &str = "Spatial Radar Canvas";',
      'pub const RADAR_DIMENSIONS: &str = "240px circle";',
      "pub const RADAR_DIAMETER_PX: u32 = 240;",
      "pub const RADAR_PADDING_PX: u32 = 8;",
      "pub const RADAR_SPACING_PX: u32 = 0;",
      'pub const RADAR_AVATAR_ELEMENT: &str = "radar-avatar";',
      "pub enum RadarAvatarInteractiveState",
      "pub struct RadarParticipantNode",
      "pub struct SpatialRadarCanvas",
      "pub struct SpatialRadarSqliteStore",
      "pub fn polar_to_radar_xy(",
      "pub fn radar_xy_to_polar(",
      "pub fn build_wgpu_radar_scene(",
    ]) {
      assert.ok(
        rustSource.includes(requiredSymbol),
        `В spatial_radar.rs должен присутствовать контракт TASK-1017B: ${requiredSymbol}`,
      );
    }

    const sizeCheck = checkRustSourceSizeLimits(
      RUST_MODULE_REL_PATH,
      rustSource,
    );
    assert.equal(
      sizeCheck.passed,
      true,
      `Нарушение лимитов размера кода: ${sizeCheck.reason}`,
    );
    assert.ok(
      sizeCheck.fileLines >= 250 && sizeCheck.fileLines < 800,
      `Ожидался размер файла 250..799 строк, получено: ${sizeCheck.fileLines}`,
    );

    const secretFindings = scanTextForSecrets(rustSource, RUST_MODULE_REL_PATH);
    assert.equal(secretFindings.length, 0);

    const fmtCheck = spawnSync(
      "rustfmt",
      ["--check", "--edition", "2024", RUST_MODULE_PATH],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(
      fmtCheck.status,
      0,
      `rustfmt --check завершился с ошибкой:\n${fmtCheck.stderr || fmtCheck.stdout}`,
    );

    const compiled = compileRustModuleTests(RUST_MODULE_PATH, testBinPath);
    assert.equal(
      compiled.status,
      0,
      `Ошибка компиляции rustc --test -D warnings:\n${compiled.stderr}`,
    );
    assert.equal(
      compiled.stderr.includes("warning:"),
      false,
      `Не должно быть предупреждений компилятора:\n${compiled.stderr}`,
    );

    const allTests = runCompiledRustTests(testBinPath);
    assert.equal(
      allTests.status,
      0,
      `Ошибка выполнения встроенных Rust-тестов:\n${allTests.stderr || allTests.stdout}`,
    );
    assert.match(allTests.stdout, /test result: ok\./);
  });

  it("SUB-1017B-1: сверяет карточки TASK-1017B/TASK-1017A/TASK-986 в waves/kanban-board.json и оракулы голосового UI (/home/sanya/re-deep/voice/ui/01-candidates.json, 02-requiremodule-voice.json, frontend-d03.json)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);

    const kanban = JSON.parse(kanbanRaw);
    const card1017B = kanban.cards.find(
      (/** @type {any} */ c) => c.id === "TASK-1017B",
    );
    assert.ok(
      card1017B,
      "Карточка TASK-1017B должна присутствовать в waves/kanban-board.json",
    );
    assert.equal(card1017B.department, "G2 Frontend");
    assert.equal(card1017B.priority, "P2");
    assert.deepEqual(card1017B.layout, {
      container: "Spatial Radar Canvas",
      dimensions: "240px circle",
      padding: "8px",
      spacing: "0px",
    });
    assert.deepEqual(card1017B.design_tokens, {
      background: "#1e1f22",
      surface: "#2b2d31",
      text_primary: "#ffffff",
      accent: "#5865f2",
      border: "#3f4147",
    });
    assert.deepEqual(card1017B.interactive_states, [
      {
        element: "radar-avatar",
        default: "positioned",
        hover: "drag icon",
        active: "dragging",
      },
    ]);

    const card1017A = kanban.cards.find(
      (/** @type {any} */ c) => c.id === "TASK-1017A",
    );
    assert.ok(card1017A);
    assert.equal(card1017A.layout.container, "HRTF Convolver");
    assert.deepEqual(card1017A.interactive_states[0], {
      element: "spatial-panner",
      default: "center",
      hover: "panning",
      active: "3d localized",
    });

    const card986 = kanban.cards.find(
      (/** @type {any} */ c) => c.id === "TASK-986",
    );
    assert.ok(card986);
    assert.equal(card986.layout.container, "Spatial Audio Canvas");
    assert.deepEqual(card986.interactive_states[0], {
      element: "peer-avatar-3d",
      default: "center",
      hover: "drag position",
      active: "panned",
    });

    if (existsSync(RE_VOICE_CANDIDATES_PATH)) {
      const candidates = JSON.parse(
        readFileSync(RE_VOICE_CANDIDATES_PATH, "utf8"),
      );
      assert.ok(
        candidates.some(
          (/** @type {any} */ c) =>
            c.cls === "voiceUser__07f91 clickable__07f91 userSmall__07f91" &&
            c.h === 32,
        ),
      );
      assert.ok(
        candidates.some(
          (/** @type {any} */ c) =>
            c.cls === "voiceChannelEffectsContainer__2f4f7",
        ),
      );
    }

    if (existsSync(RE_VOICE_MODULE_PATH)) {
      const voiceMod = JSON.parse(readFileSync(RE_VOICE_MODULE_PATH, "utf8"));
      assert.ok(voiceMod.keys.includes("isSpatialAudioEnabled"));
      assert.ok(voiceMod.keys.includes("setOutputVolume"));
    }

    const d03Path = existsSync(RE_DEEP_FRONTEND_D03_PATH)
      ? RE_DEEP_FRONTEND_D03_PATH
      : DOCS_FRONTEND_D03_PATH;
    const d03 = JSON.parse(readFileSync(d03Path, "utf8"));
    const avatarComp = d03.components.find(
      (/** @type {any} */ c) => c.name === "Avatar",
    );
    assert.ok(avatarComp);
    assert.match(avatarComp.evidence[0], /avatar_fdd9da/);

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    for (const marker of [
      "Spatial Radar Canvas",
      "240px circle",
      "#1e1f22",
      "#2b2d31",
      "#ffffff",
      "#5865f2",
      "#3f4147",
      "radar-avatar",
      "positioned",
      "drag icon",
      "dragging",
      "voiceUser__07f91 clickable__07f91 userSmall__07f91",
      "voiceChannelEffectsContainer__2f4f7",
      "rtcConnectionStatusWrapper__06d62",
      "isSpatialAudioEnabled",
      "avatar_fdd9da",
      "tooltip_c36707",
    ]) {
      assert.ok(
        rustSource.includes(marker),
        `В spatial_radar.rs должен быть зафиксирован маркер оракула: ${marker}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_1017b_1_");
    assert.equal(
      run.status,
      0,
      `Тест SUB-1017B-1 в Rust упал:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1017b_1_.*ok/);
  });

  it("SUB-1017B-2: проверяет WGPU-радар (круг 240px, радиус 112px), перетаскивание аватаров мышью с фиксацией угла азимута и сверку геометрии с независимым тригонометрическим оракулом", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const top = oraclePolarToRadarXy(0, 50);
    assert.ok(Math.abs(top.xPx - 120) < 1e-4);
    assert.ok(Math.abs(top.yPx - 8) < 1e-4);

    const right = oraclePolarToRadarXy(90, 50);
    assert.ok(Math.abs(right.xPx - 232) < 1e-4);
    assert.ok(Math.abs(right.yPx - 120) < 1e-4);

    const leftHalf = oraclePolarToRadarXy(-90, 25.5);
    assert.ok(Math.abs(leftHalf.xPx - 64) < 1e-4);
    assert.ok(Math.abs(leftHalf.yPx - 120) < 1e-4);

    const inv = oracleRadarXyToPolar(232, 120, 15);
    assert.equal(inv.azimuthDeg, 90);
    assert.ok(Math.abs(inv.distanceM - 50) < 1e-4);

    const outside = oracleRadarXyToPolar(300, 120, 5);
    assert.equal(outside.azimuthDeg, 90);
    assert.equal(outside.clampedRadiusPx, 112);
    assert.ok(Math.abs(outside.distanceM - 50) < 1e-4);

    const run = runCompiledRustTests(testBinPath, "test_sub_1017b_2_");
    assert.equal(
      run.status,
      0,
      `Тест SUB-1017B-2 (WGPU SpatialRadar и drag-and-drop аватаров) упал:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1017b_2_.*ok/);
  });

  it("SUB-1017B-3: проверяет сохранение индивидуальных координат расстановки участников в SQLite (SpatialRadarSqliteStore, параметризованный UPSERT, заголовок SQLite format 3 и файловый round-trip)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const run = runCompiledRustTests(testBinPath, "test_sub_1017b_3_");
    assert.equal(
      run.status,
      0,
      `Тест SUB-1017B-3 (сохранение координат в SQLite) упал:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1017b_3_.*ok/);
  });

  it("SUB-1017B-4: покрывает все 9 обязательных измерений TEST-POLICY.md (граничные Snowflake 0/u64::MAX, -180..+180 deg, 1..50 м, 2000 символов, 10 файлов/25 МБ, offset=9975, громкость 0..200%, бейдж 99+, коды 400/401/403/404/413/429/5xx, пустой радар, skeleton/202, права CONNECT/SPEAK/AGE_GATED_SPACES, откат повреждённой БД и маскирование секретов)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const run = runCompiledRustTests(testBinPath, "test_sub_1017b_4_");
    assert.equal(
      run.status,
      0,
      `Тест SUB-1017B-4 (9 измерений TEST-POLICY.md) упал:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1017b_4_.*ok/);
  });

  it("SUB-1017B-5: убивает 5/5 контрольных мутаций модуля spatial_radar.rs (100% mutation kill rate)", () => {
    assert.ok(
      existsSync(RUST_MODULE_PATH),
      "Исходник модуля должен существовать",
    );
    const originalRust = readFileSync(RUST_MODULE_PATH, "utf8");
    const originalMd5 = createHash("md5").update(originalRust).digest("hex");

    const mutationSpecs = [
      {
        id: "MUT-1017B-01",
        operator: "RadarUsableRadiusTamper",
        from: "pub const RADAR_USABLE_RADIUS_PX: f32 = 112.0;",
        to: "pub const RADAR_USABLE_RADIUS_PX: f32 = 60.0;",
      },
      {
        id: "MUT-1017B-02",
        operator: "RadarAvatarDraggingStateTamper",
        from: 'Self::Dragging => "dragging",',
        to: 'Self::Dragging => "positioned",',
      },
      {
        id: "MUT-1017B-03",
        operator: "PolarXCoordinateSignInversion",
        from: "let x_px = RADAR_CENTER_X_PX + r_px * az_rad.sin();",
        to: "let x_px = RADAR_CENTER_X_PX - r_px * az_rad.sin();",
      },
      {
        id: "MUT-1017B-04",
        operator: "AzimuthSnapBypassInDrag",
        from: "let snapped = (azimuth_deg / step).round() * step;",
        to: "let snapped = 0.0_f32;",
      },
      {
        id: "MUT-1017B-05",
        operator: "SqlitePersistSkipOnEndDrag",
        from: "store.upsert_position(&row)?;",
        to: "let _ = &row;",
      },
    ];

    const mutants = mutationSpecs.map((spec) => {
      assert.ok(
        originalRust.includes(spec.from),
        `Целевая строка для мутации ${spec.id} должна присутствовать в spatial_radar.rs: ${spec.from}`,
      );
      return {
        id: spec.id,
        moduleName: "spatial_radar.rs",
        operator: spec.operator,
        mutatedSource: originalRust.replace(spec.from, spec.to),
      };
    });

    const gate = evaluateMutationGate(mutants, (mutant, idx) => {
      const mutantSrcPath = join(workDir, `mutant_${idx + 1}.rs`);
      const mutantBinPath = join(workDir, `mutant_${idx + 1}_bin`);
      writeFileSync(mutantSrcPath, mutant.mutatedSource, "utf8");

      const compiled = compileRustModuleTests(mutantSrcPath, mutantBinPath);
      if (compiled.status !== 0) {
        return { killed: true, reason: "compilation_error" };
      }
      const run = runCompiledRustTests(mutantBinPath);
      if (run.status !== 0) {
        return { killed: true, reason: "test_assertion_failed" };
      }
      return { killed: false, reason: "mutant_survived" };
    });

    const afterMd5 = createHash("md5")
      .update(readFileSync(RUST_MODULE_PATH, "utf8"))
      .digest("hex");
    assert.equal(
      afterMd5,
      originalMd5,
      "MD5 исходного файла spatial_radar.rs не должен измениться",
    );

    assert.equal(
      gate.passed,
      true,
      `Мутационный гейт не пройден (выжили: ${gate.survivors.join(", ")})`,
    );
    assert.equal(gate.killed, 5);
    assert.equal(gate.total, 5);
    assert.equal(gate.killRate, 1.0);
  });
});
