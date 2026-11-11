// @ts-check
/**
 * Контрактные и модульные тесты иерархии ролей сервера, drag-and-drop упорядочивания
 * `PATCH /guilds/{id}/roles` и 64-битного редактора битовой маски прав `permissions` (TASK-1007):
 * - SUB-1007-0: Статический и компиляционный контракт нативного модуля `rust/discord-gpui/src/guild/roles.rs`
 *   и верификатора `scripts/verify-roles.mjs` (`#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()`
 *   в продуктовом коде, лимиты `scripts/quality-gate.mjs`: функции < 50 строк, файл < 800 строк,
 *   `rustfmt --check --edition 2024`, компиляция `rustc --test --edition 2024 -D warnings` и `secret-scan.mjs`).
 * - SUB-1007-1: Сверка с независимыми оракулами репозитория (`waves/kanban-board.json` карточка `TASK-1007`,
 *   `docs/re/gateway-schema.json` и `docs/re/gateway-schema.md` `READY.d.guilds[].roles[]` 132 роли / 13 ключей,
 *   `docs/re/rest-d02.json` метаданные снимков D-02, `rust/crates/discord-core/src/permissions.rs` 41 флаг `0..=40`
 *   и `rust/gui-spike/src/roles.rs`).
 * - SUB-1007-2: Иерархия ролей сервера, проверка потолка наивысшей роли актора (запрет эскалации выше своей роли,
 *   неприкосновенность `@everyone` `position=0` и `managed`-ролей ботов), drag-and-drop перетаскивание позиций
 *   с линией вставки (`Before` / `After`), генерация `PATCH /guilds/{id}/roles` и выбор RGB/HEX цвета роли.
 * - SUB-1007-3: Интерактивный 64-битный редактор прав `PermissionsBitmaskEditor` (41 переключатель `0..=40`
 *   в 5 категориях, запрет выдачи отсутствующих у актора битов, баннер предупреждения `ADMINISTRATOR` `1 << 3`,
 *   плавающая плашка несохранённых изменений `Save / Reset`, компоновка `Roles Manager Dialog` `720px x 540px`
 *   и состояния `role-item`: `row -> reorder icon -> selected role` + клавиатурная навигация).
 * - SUB-1007-4: Покрытие всех 9 обязательных измерений `TEST-POLICY.md` (граничные Snowflake `0`/`u64::MAX`,
 *   имя `1..=100` символов, `2000` символов, `10` файлов/`25 МБ`, `offset=9975`, громкость `0..200%`,
 *   бейдж `99+`, HTTP `400/401/403/404/413/429/5xx`, откат оптимистичного drag-and-drop при сбое сети,
 *   маскирование секретов) и проверка CLI `scripts/verify-roles.mjs` (коды выхода `0 / 1 / 2`).
 * - SUB-1007-5: Мутационный гейт `5/5` (`100%` kill rate) для `rust/discord-gpui/src/guild/roles.rs`
 *   и `scripts/verify-roles.mjs`.
 */
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
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
const RUST_MODULE_REL_PATH = "rust/discord-gpui/src/guild/roles.rs";
const RUST_MODULE_PATH = join(ROOT, RUST_MODULE_REL_PATH);
const VERIFY_SCRIPT_REL_PATH = "scripts/verify-roles.mjs";
const VERIFY_SCRIPT_PATH = join(ROOT, VERIFY_SCRIPT_REL_PATH);
const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const GATEWAY_SCHEMA_JSON_PATH = join(ROOT, "docs", "re", "gateway-schema.json");
const GATEWAY_SCHEMA_MD_PATH = join(ROOT, "docs", "re", "gateway-schema.md");
const REST_D02_JSON_PATH = join(ROOT, "docs", "re", "rest-d02.json");
const CORE_PERMISSIONS_RS_PATH = join(
  ROOT,
  "rust",
  "crates",
  "discord-core",
  "src",
  "permissions.rs",
);
const SPIKE_ROLES_RS_PATH = join(ROOT, "rust", "gui-spike", "src", "roles.rs");

/** @type {string} */
let workDir = "";
/** @type {string} */
let testBinPath = "";

/**
 * Компилирует `roles.rs` через `rustc --test --edition 2024 -D warnings`.
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

describe("TASK-1007: Иерархия ролей сервера, drag-and-drop PATCH /guilds/{id}/roles и редактор 64-битной маски прав", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "guild-roles-1007-"));
    testBinPath = join(workDir, "guild_roles_1007_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-1007-0: проверяет статический контракт rust/discord-gpui/src/guild/roles.rs и scripts/verify-roles.mjs (<50 строк/fn, <800 строк/файл, #![forbid(unsafe_code)], rustfmt 2024 и rustc --test)", () => {
    assert.ok(
      existsSync(RUST_MODULE_PATH),
      `Отсутствует Rust-модуль ${RUST_MODULE_REL_PATH}`,
    );
    assert.ok(
      existsSync(VERIFY_SCRIPT_PATH),
      `Отсутствует скрипт верификации ${VERIFY_SCRIPT_REL_PATH}`,
    );

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    assert.match(
      rustSource,
      /^#!\[forbid\(unsafe_code\)\]/m,
      "В модуле обязательна директива #![forbid(unsafe_code)]",
    );

    const prodSection = rustSource.split("#[cfg(test)]")[0] ?? "";
    assert.equal(
      prodSection.includes(".unwrap()") || prodSection.includes(".expect("),
      false,
      "В продуктовом коде roles.rs запрещены .unwrap() и .expect()",
    );

    const sizeCheck = checkRustSourceSizeLimits(RUST_MODULE_REL_PATH, rustSource);
    assert.equal(
      sizeCheck.passed,
      true,
      `Нарушение лимитов размера Rust-кода: ${sizeCheck.reason}`,
    );
    assert.ok(
      sizeCheck.fileLines >= 250 && sizeCheck.fileLines < 800,
      `Ожидался размер файла 250..799 строк, получено ${sizeCheck.fileLines}`,
    );

    const jsSource = readFileSync(VERIFY_SCRIPT_PATH, "utf8");
    const jsLines = jsSource.split("\n").length;
    assert.ok(
      jsLines >= 200 && jsLines < 800,
      `Ожидался размер JS-скрипта 200..799 строк, получено ${jsLines}`,
    );

    assert.equal(scanTextForSecrets(rustSource, RUST_MODULE_REL_PATH).length, 0);
    assert.equal(scanTextForSecrets(jsSource, VERIFY_SCRIPT_REL_PATH).length, 0);

    const fmtCheck = spawnSync(
      "rustfmt",
      ["--check", "--edition", "2024", RUST_MODULE_PATH],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(
      fmtCheck.status,
      0,
      `rustfmt --check завершился с ошибкой: ${fmtCheck.stderr || fmtCheck.stdout}`,
    );

    const compiled = compileRustModuleTests(RUST_MODULE_PATH, testBinPath);
    assert.equal(
      compiled.status,
      0,
      `Ошибка компиляции rustc --test: ${compiled.stderr}`,
    );
    assert.equal(
      compiled.stderr.includes("warning:"),
      false,
      `Предупреждения компилятора недопустимы: ${compiled.stderr}`,
    );

    const allTests = runCompiledRustTests(testBinPath);
    assert.equal(
      allTests.status,
      0,
      `Ошибка выполнения Rust-тестов: ${allTests.stderr || allTests.stdout}`,
    );
    assert.match(allTests.stdout, /test result: ok\./);
  });

  it("SUB-1007-1: сверяет дизайн-токены, раскладку Roles Manager Dialog, состояния role-item, 41 флаг прав v9 и схему ролей с независимыми оракулами", async () => {
    const {
      ROLES_DESIGN_TOKENS,
      ROLES_DIALOG_LAYOUT,
      ROLE_ITEM_STATES,
      ROLES_PROTOCOL_CONSTANTS,
      DISCORD_V9_PERMISSIONS_CATALOG,
      verifyRolesOracleParity,
    } = await import("../../scripts/verify-roles.mjs");

    // 1. Оракул waves/kanban-board.json (карточка TASK-1007)
    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);
    const kanban = JSON.parse(kanbanRaw);
    const card1007 = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-1007");
    assert.ok(card1007, "Карточка TASK-1007 должна присутствовать в waves/kanban-board.json");
    assert.deepEqual(ROLES_DESIGN_TOKENS, card1007.design_tokens);
    assert.deepEqual(ROLES_DIALOG_LAYOUT, card1007.layout);
    assert.deepEqual(ROLE_ITEM_STATES, card1007.interactive_states[0]);

    // 2. Оракул docs/re/gateway-schema.json и docs/re/gateway-schema.md (READY.d.guilds[].roles[])
    const gwSchemaJson = JSON.parse(readFileSync(GATEWAY_SCHEMA_JSON_PATH, "utf8"));
    assert.equal(gwSchemaJson.stats?.ready?.rolesTotal, 132);
    const gwSchemaMd = readFileSync(GATEWAY_SCHEMA_MD_PATH, "utf8");
    assert.match(gwSchemaMd, /\*\*READY\.d\.guilds\[\]\.roles\[\] \(132 роли\)\*\* \(13 ключей\)/);
    assert.match(gwSchemaMd, /\|\s*`permissions`\s*\|\s*string\s*\|\s*str\s*\|\s*132\/132\s*\|\s*2221977711673344\s*\|/);

    // 3. Оракул docs/re/rest-d02.json и rust/gui-spike/src/roles.rs
    const restD02 = JSON.parse(readFileSync(REST_D02_JSON_PATH, "utf8"));
    assert.equal(restD02.totals?.apiUniqueEndpoints, 47);
    const spikeRolesRs = readFileSync(SPIKE_ROLES_RS_PATH, "utf8");
    assert.match(spikeRolesRs, /pub const MAX_ROLE_NAME_CHARS: usize = 100;/);
    assert.match(spikeRolesRs, /pub const MAX_RGB_COLOR: u32 = 0x00FF_FFFF;/);
    assert.match(spikeRolesRs, /pub const PERM_ADMINISTRATOR: u64 = 1 << 3;/);

    // 4. Оракул rust/crates/discord-core/src/permissions.rs (41 битовый флаг 0..=40)
    const corePermsRs = readFileSync(CORE_PERMISSIONS_RS_PATH, "utf8");
    assert.match(corePermsRs, /pub const ALL_V9_DEFINED_PERMISSIONS: u64 = \(1u64 << 41\) - 1;/);
    assert.match(corePermsRs, /pub const CREATE_INSTANT_INVITE: u64 = 1 << 0;/);
    assert.match(corePermsRs, /pub const MANAGE_ROLES: u64 = 1 << 28;/);
    assert.match(corePermsRs, /pub const MODERATE_MEMBERS: u64 = 1 << 40;/);

    assert.equal(DISCORD_V9_PERMISSIONS_CATALOG.length, 41);
    assert.equal(ROLES_PROTOCOL_CONSTANTS.MAX_ROLE_NAME_CHARS, 100);
    assert.equal(ROLES_PROTOCOL_CONSTANTS.MAX_RGB_COLOR, 0x00ff_ffff);
    assert.equal(ROLES_PROTOCOL_CONSTANTS.MAX_GUILD_ROLES, 250);
    assert.equal(ROLES_PROTOCOL_CONSTANTS.PERM_ADMINISTRATOR, 1n << 3n);
    assert.equal(ROLES_PROTOCOL_CONSTANTS.PERM_MANAGE_ROLES, 1n << 28n);
    assert.equal(ROLES_PROTOCOL_CONSTANTS.ALL_V9_PERMISSIONS_MASK, (1n << 41n) - 1n);

    const parity = verifyRolesOracleParity({
      kanbanPath: KANBAN_BOARD_PATH,
      gatewaySchemaJsonPath: GATEWAY_SCHEMA_JSON_PATH,
      gatewaySchemaMdPath: GATEWAY_SCHEMA_MD_PATH,
      restD02JsonPath: REST_D02_JSON_PATH,
      corePermissionsRsPath: CORE_PERMISSIONS_RS_PATH,
      spikeRolesRsPath: SPIKE_ROLES_RS_PATH,
      rustModulePath: RUST_MODULE_PATH,
    });
    assert.equal(parity.passed, true, `Нарушение паритета с оракулами: ${parity.reasons.join("; ")}`);

    const rustOracleTest = runCompiledRustTests(
      testBinPath,
      "test_sub_1007_1_oracle_tokens_and_permission_constants",
    );
    assert.equal(rustOracleTest.status, 0, rustOracleTest.stderr || rustOracleTest.stdout);
  });

  it("SUB-1007-2: проверяет иерархию ролей, потолок наивысшей роли актора, drag-and-drop PATCH /guilds/{id}/roles и валидацию цвета RGB/HEX", async () => {
    const {
      sortGuildRolesHierarchy,
      computeActorHighestRolePosition,
      reorderGuildRolesDragAndDrop,
      parseRoleHexColor,
      formatRoleHexColor,
    } = await import("../../scripts/verify-roles.mjs");

    const guildId = "100000000000000001";
    const sampleRoles = [
      {
        id: guildId,
        name: "@everyone",
        position: 0,
        color: 0,
        hoist: false,
        managed: false,
        mentionable: false,
        permissions: "66560",
      },
      {
        id: "100000000000000002",
        name: "Developer",
        position: 1,
        color: 0x3498db,
        hoist: true,
        managed: false,
        mentionable: true,
        permissions: "68608",
      },
      {
        id: "100000000000000003",
        name: "Moderator",
        position: 2,
        color: 0x2ecc71,
        hoist: true,
        managed: false,
        mentionable: true,
        permissions: String((1n << 28n) | (1n << 10n) | (1n << 1n) | (1n << 2n)),
      },
      {
        id: "100000000000000004",
        name: "Release Bot",
        position: 3,
        color: 0x9b59b6,
        hoist: false,
        managed: true,
        mentionable: false,
        permissions: "8",
      },
      {
        id: "100000000000000005",
        name: "Server Admin",
        position: 4,
        color: 0xe74c3c,
        hoist: true,
        managed: false,
        mentionable: false,
        permissions: String((1n << 3n) | (1n << 28n)),
      },
    ];

    // 1. Сортировка сверху вниз (по убыванию position, @everyone всегда внизу с position=0)
    const sorted = sortGuildRolesHierarchy(sampleRoles, guildId);
    assert.deepEqual(
      sorted.map((r) => r.id),
      [
        "100000000000000005",
        "100000000000000004",
        "100000000000000003",
        "100000000000000002",
        guildId,
      ],
    );

    // 2. Определение наивысшей позиции роли актора
    assert.equal(
      computeActorHighestRolePosition(sampleRoles, ["100000000000000003"], false),
      2,
    );
    assert.equal(
      computeActorHighestRolePosition(sampleRoles, ["100000000000000005"], false),
      4,
    );

    // 3. Drag-and-drop: Admin (position=4) перетаскивает Developer (pos=1) выше Moderator (pos=2) -> placement: "before"
    const reordered = reorderGuildRolesDragAndDrop(sampleRoles, {
      guildId,
      draggedRoleId: "100000000000000002",
      targetRoleId: "100000000000000003",
      placement: "before",
      actorHighestPosition: 4,
      actorPermissions: (1n << 28n) | (1n << 3n),
      isOwner: false,
    });
    assert.equal(reordered.method, "PATCH");
    assert.equal(reordered.route, `/guilds/${guildId}/roles`);
    assert.equal(reordered.dropIndicatorLine, "before:100000000000000003");
    // Теперь порядок сверху вниз: Admin(4), Release Bot(3), Developer(2), Moderator(1), @everyone(0)
    assert.deepEqual(
      reordered.orderedRoles.map((r) => ({ id: r.id, position: r.position })),
      [
        { id: "100000000000000005", position: 4 },
        { id: "100000000000000004", position: 3 },
        { id: "100000000000000002", position: 2 },
        { id: "100000000000000003", position: 1 },
        { id: guildId, position: 0 },
      ],
    );
    assert.equal(reordered.patchPayload.length, 4);

    // 4. Запрет эскалации: Moderator (position=2) не может перетащить роль выше или на уровень своей позиции (pos >= 2)
    assert.throws(
      () =>
        reorderGuildRolesDragAndDrop(sampleRoles, {
          guildId,
          draggedRoleId: "100000000000000002",
          targetRoleId: "100000000000000004",
          placement: "before",
          actorHighestPosition: 2,
          actorPermissions: 1n << 28n,
          isOwner: false,
        }),
      /HierarchyViolation|выше или равной/,
    );

    // 5. Запрет перетаскивания роли @everyone
    assert.throws(
      () =>
        reorderGuildRolesDragAndDrop(sampleRoles, {
          guildId,
          draggedRoleId: guildId,
          targetRoleId: "100000000000000002",
          placement: "before",
          actorHighestPosition: 4,
          actorPermissions: 1n << 28n,
          isOwner: true,
        }),
      /EveryoneRoleImmutable|@everyone/,
    );

    // 6. Парсер и форматтер HEX/RGB цвета роли
    assert.equal(parseRoleHexColor("#5865f2"), 0x5865f2);
    assert.equal(formatRoleHexColor(0x5865f2), "#5865F2");
    assert.equal(formatRoleHexColor(0), "#99AAB5");
    assert.throws(() => parseRoleHexColor("#12345G"), /InvalidHexColor/);

    const rustHierarchyTest = runCompiledRustTests(
      testBinPath,
      "test_sub_1007_2_hierarchy_reorder_and_anti_escalation",
    );
    assert.equal(rustHierarchyTest.status, 0, rustHierarchyTest.stderr || rustHierarchyTest.stdout);
  });

  it("SUB-1007-3: проверяет 64-битный редактор прав (41 переключатель в 5 категориях, запрет выдачи чужих битов, баннер ADMINISTRATOR, плашку сохранения и раскладку диалога 720x540)", async () => {
    const {
      createRolePermissionsDraft,
      toggleRolePermissionBit,
      buildRoleSavePatchRequest,
      computeRolesManagerDialogView,
      navigateRolesListKeyboard,
    } = await import("../../scripts/verify-roles.mjs");

    const role = {
      id: "100000000000000002",
      guildId: "100000000000000001",
      name: "Developer",
      position: 1,
      color: 0x3498db,
      hoist: true,
      managed: false,
      mentionable: true,
      permissions: String((1n << 10n) | (1n << 11n)), // VIEW_CHANNEL | SEND_MESSAGES
    };

    // 1. Создание черновика и переключение битов в рамках прав актора
    const actorPerms =
      (1n << 28n) | // MANAGE_ROLES
      (1n << 10n) | // VIEW_CHANNEL
      (1n << 11n) | // SEND_MESSAGES
      (1n << 38n) | // SEND_MESSAGES_IN_THREADS
      (1n << 40n); // MODERATE_MEMBERS

    let draft = createRolePermissionsDraft(role);
    assert.equal(draft.dirty, false);
    assert.equal(draft.hasAdministratorWarning, false);

    // Включаем бит 40 (MODERATE_MEMBERS = 1n << 40n, старший бит за пределами 32-битного числа!)
    draft = toggleRolePermissionBit(draft, "MODERATE_MEMBERS", {
      actorEffectivePermissions: actorPerms,
      actorHighestPosition: 3,
      isOwner: false,
    });
    assert.equal(draft.dirty, true);
    assert.equal((BigInt(draft.permissions) & (1n << 40n)) !== 0n, true);
    assert.ok(draft.dangerousBitsEnabled.includes("MODERATE_MEMBERS"));

    // Попытка включить ADMINISTRATOR (1n << 3n), которого нет у актора -> отказ PrivilegeEscalationDenied
    assert.throws(
      () =>
        toggleRolePermissionBit(draft, "ADMINISTRATOR", {
          actorEffectivePermissions: actorPerms,
          actorHighestPosition: 3,
          isOwner: false,
        }),
      /PrivilegeEscalationDenied|не обладает правом/,
    );

    // Владелец сервера может включить ADMINISTRATOR -> появляется предупреждающий баннер
    const adminDraft = toggleRolePermissionBit(draft, "ADMINISTRATOR", {
      actorEffectivePermissions: actorPerms,
      actorHighestPosition: 3,
      isOwner: true,
    });
    assert.equal(adminDraft.hasAdministratorWarning, true);
    assert.match(adminDraft.administratorWarningText, /ADMINISTRATOR/);

    // Формирование PATCH /guilds/{guild_id}/roles/{role_id}
    const patchReq = buildRoleSavePatchRequest(adminDraft);
    assert.equal(patchReq.method, "PATCH");
    assert.equal(
      patchReq.route,
      "/guilds/100000000000000001/roles/100000000000000002",
    );
    assert.equal(patchReq.body.permissions, adminDraft.permissions);

    // 2. Визуальная модель Roles Manager Dialog (720px x 540px, 5 категорий, 41 тумблер, плашка сохранения)
    const dialogView = computeRolesManagerDialogView({
      guildId: "100000000000000001",
      roles: [
        {
          id: "100000000000000001",
          name: "@everyone",
          position: 0,
          color: 0,
          hoist: false,
          managed: false,
          mentionable: false,
          permissions: "66560",
          memberCount: 150,
        },
        {
          ...role,
          memberCount: 12,
        },
      ],
      selectedRoleId: "100000000000000002",
      hoveredRoleId: "100000000000000001",
      draft: adminDraft,
      actorEffectivePermissions: (1n << 41n) - 1n,
      actorHighestPosition: 5,
      isOwner: true,
    });

    assert.equal(dialogView.container, "Roles Manager Dialog");
    assert.equal(dialogView.dimensions, "720px x 540px");
    assert.equal(dialogView.paddingPx, 16);
    assert.equal(dialogView.spacingPx, 12);
    assert.equal(dialogView.sidebarWidthPx, 320);
    assert.equal(dialogView.totalTogglesCount, 41);
    assert.equal(dialogView.categories.length, 5);
    assert.equal(dialogView.unsavedChangesBarVisible, true);
    assert.equal(dialogView.administratorBannerVisible, true);
    assert.equal(dialogView.roleItems[0]?.state, "selected role");
    assert.equal(dialogView.roleItems[1]?.state, "reorder icon");
    assert.equal(dialogView.roleItems[1]?.memberBadgeText, "99+");

    // 3. Клавиатурная навигация по списку ролей
    let nav = { focusedIndex: 0, selectedIndex: 0, totalRoles: 3 };
    nav = navigateRolesListKeyboard(nav, "ArrowDown");
    assert.equal(nav.focusedIndex, 1);
    nav = navigateRolesListKeyboard(nav, "End");
    assert.equal(nav.focusedIndex, 2);
    nav = navigateRolesListKeyboard(nav, "Enter");
    assert.equal(nav.selectedIndex, 2);
    nav = navigateRolesListKeyboard(nav, "Home");
    assert.equal(nav.focusedIndex, 0);

    const rustMatrixTest = runCompiledRustTests(
      testBinPath,
      "test_sub_1007_3_bitmask_editor_and_dialog_spec",
    );
    assert.equal(rustMatrixTest.status, 0, rustMatrixTest.stderr || rustMatrixTest.stdout);
  });

  it("SUB-1007-4: проверяет все 9 обязательных измерений TEST-POLICY.md, откат drag-and-drop при сетевой ошибке и CLI scripts/verify-roles.mjs (коды 0/1/2)", async () => {
    const {
      evaluateRolesNineDimensions,
      redactRolesDiagnostics,
      runCli,
    } = await import("../../scripts/verify-roles.mjs");

    // 1. Покрытие всех 9 измерений TEST-POLICY.md
    const dimReport = evaluateRolesNineDimensions();
    assert.equal(
      dimReport.passed,
      true,
      `Сбой в 9 измерениях TEST-POLICY.md: ${dimReport.failures.join("; ")}`,
    );
    assert.equal(dimReport.dimensionsChecked, 9);

    // 2. Маскирование секретов и токенов в логах диагностики ролей
    const rawDiag =
      'RolePatch PATCH /guilds/100/roles Authorization: Bearer mfa.secret_role_token_987654 token="MTIzNDU2Nzg5MDEyMzQ1Njc4.GhIjKl.MnOpQrStUvWxYz0123456789abcdef"';
    const safeDiag = redactRolesDiagnostics(rawDiag);
    assert.doesNotMatch(safeDiag, /mfa\.secret_role_token/);
    assert.doesNotMatch(safeDiag, /MTIzNDU2Nzg5MDEyMzQ1Njc4/);
    assert.match(safeDiag, /\[REDACTED_TOKEN\]/);

    // 3. Коды выхода CLI: 0 (успех), 1 (неизвестный аргумент), 2 (недоступный оракул)
    const outJsonPath = join(workDir, "roles-report.json");
    const outMdPath = join(workDir, "roles-report.md");
    assert.equal(runCli(["--help"]), 0);
    assert.equal(runCli(["--out-json", outJsonPath, "--out-md", outMdPath]), 0);
    assert.ok(existsSync(outJsonPath));
    assert.ok(existsSync(outMdPath));
    assert.equal(runCli(["--unknown-flag"]), 1);
    assert.equal(runCli(["--oracle", join(workDir, "non-existent-oracle.json")]), 2);

    const rustDimTest = runCompiledRustTests(
      testBinPath,
      "test_sub_1007_4_nine_dimensions_and_rollback",
    );
    assert.equal(rustDimTest.status, 0, rustDimTest.stderr || rustDimTest.stdout);
  });

  it("SUB-1007-5: убивает 5/5 контрольных мутаций в roles.rs и scripts/verify-roles.mjs (100% mutation kill rate)", async () => {
    const { runRolesMutationSuite } = await import("../../scripts/verify-roles.mjs");

    // 1. Мутационный гейт JS-скрипта (5/5)
    const jsMutReport = runRolesMutationSuite();
    assert.equal(jsMutReport.total, 5);
    assert.equal(jsMutReport.killed, 5);
    assert.deepEqual(jsMutReport.survivors, []);

    // 2. Мутационный гейт Rust-модуля (5/5 через перекомпиляцию rustc --test)
    const originalRust = readFileSync(RUST_MODULE_PATH, "utf8");
    const originalMd5 = createHash("md5").update(originalRust).digest("hex");

    const rustMutations = [
      {
        id: "MUT-1007-RS-01",
        moduleName: "guild_roles",
        operator: "AdministratorBitTamper",
        from: "pub const PERM_ADMINISTRATOR: u64 = 1 << 3;",
        to: "pub const PERM_ADMINISTRATOR: u64 = 1 << 0;",
      },
      {
        id: "MUT-1007-RS-02",
        moduleName: "guild_roles",
        operator: "HierarchyEscalationGuardBypass",
        from: "if !actor.is_owner && role_pos >= actor.highest_position {",
        to: "if false && !actor.is_owner && role_pos >= actor.highest_position {",
      },
      {
        id: "MUT-1007-RS-03",
        moduleName: "guild_roles",
        operator: "PrivilegeGrantEscalationBypass",
        from: "if !actor.can_grant_permission(bit) {",
        to: "if false && !actor.can_grant_permission(bit) {",
      },
      {
        id: "MUT-1007-RS-04",
        moduleName: "guild_roles",
        operator: "PositionSortDirectionInversion",
        from: "b.position.cmp(&a.position)",
        to: "a.position.cmp(&b.position)",
      },
      {
        id: "MUT-1007-RS-05",
        moduleName: "guild_roles",
        operator: "MemberBadgeCapTamper",
        from: "pub const MAX_BADGE_CAP: u32 = 99;",
        to: "pub const MAX_BADGE_CAP: u32 = 999;",
      },
    ];

    const mutants = rustMutations.map((m) => {
      assert.ok(
        originalRust.includes(m.from),
        `Целевая строка для ${m.id} должна присутствовать в roles.rs`,
      );
      return {
        id: m.id,
        moduleName: m.moduleName,
        operator: m.operator,
        mutatedSource: originalRust.replace(m.from, m.to),
      };
    });

    const gateResult = evaluateMutationGate(mutants, (mutant, idx) => {
      const mutSrcPath = join(workDir, `mutant_${idx}.rs`);
      const mutBinPath = join(workDir, `mutant_${idx}_bin`);
      spawnSync("sh", ["-c", `cat > "${mutSrcPath}"`], {
        input: mutant.mutatedSource,
        encoding: "utf8",
      });
      const comp = compileRustModuleTests(mutSrcPath, mutBinPath);
      if (comp.status !== 0) {
        return { killed: true, reason: "compile_error" };
      }
      const run = runCompiledRustTests(mutBinPath);
      return {
        killed: run.status !== 0,
        reason: run.status !== 0 ? "test_failed" : "survived",
      };
    });

    const afterMd5 = createHash("md5")
      .update(readFileSync(RUST_MODULE_PATH, "utf8"))
      .digest("hex");
    assert.equal(
      afterMd5,
      originalMd5,
      "MD5 исходного файла roles.rs не должен измениться",
    );
    assert.equal(
      gateResult.passed,
      true,
      `Выжившие мутанты: ${gateResult.survivors.join(", ")}`,
    );
    assert.equal(gateResult.killed, 5);
    assert.equal(gateResult.total, 5);
  });
});
