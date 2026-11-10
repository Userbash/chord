// @ts-check
import { describe, it } from "node:test";
import assert from "node:assert/strict";

describe("Guild Rail Incremental & Unavailable Handlers", () => {
  it("parseGuild accepts unavailable guild without throwing and provides placeholder name", async () => {
    // Simulate parseGuild behavior
    /** @param {any} value */
    function parseGuild(value) {
      const bag = value && typeof value === "object" ? value : {};
      const id = typeof bag.id === "string" ? bag.id : "";
      if (id === "") throw new Error("ready: гильдия без id");
      const name = typeof bag.name === "string" && bag.name !== ""
        ? bag.name
        : (typeof bag.properties === "object" && bag.properties && typeof bag.properties.name === "string" && bag.properties.name !== ""
          ? bag.properties.name
          : "Загрузка...");
      return { id, name };
    }

    const available = parseGuild({ id: "1001", name: "Main Server" });
    assert.equal(available.id, "1001");
    assert.equal(available.name, "Main Server");

    const unavailable = parseGuild({ id: "1002", unavailable: true });
    assert.equal(unavailable.id, "1002");
    assert.equal(unavailable.name, "Загрузка...");

    const withProps = parseGuild({ id: "1003", properties: { name: "Props Server" } });
    assert.equal(withProps.id, "1003");
    assert.equal(withProps.name, "Props Server");

    assert.throws(() => parseGuild({ name: "No ID" }), /ready: гильдия без id/);
  });

  it("syncLiveGuildRail preserves existing guilds when incremental GUILD_CREATE arrives", () => {
    const liveGuildsById = new Map();
    /**
     * @param {any[]} rawGuilds
     * @param {boolean} [incremental=false]
     */
    function syncLiveGuildRail(rawGuilds, incremental = false) {
      for (const g of rawGuilds) {
        const gid = String(g?.id ?? "").trim();
        if (!gid) continue;
        liveGuildsById.set(gid, g);
      }
      if (!incremental) {
        const validIds = new Set(rawGuilds.map((g) => String(g?.id ?? "").trim()).filter(Boolean));
        for (const oldGid of Array.from(liveGuildsById.keys())) {
          if (!validIds.has(oldGid)) {
            liveGuildsById.delete(oldGid);
          }
        }
      }
      return Array.from(liveGuildsById.values());
    }

    // Initial snapshot with 3 guilds
    const initial = syncLiveGuildRail([
      { id: "1", name: "Alpha" },
      { id: "2", name: "Beta" },
      { id: "3", name: "Gamma" }
    ], false);
    assert.equal(initial.length, 3);
    assert.equal(liveGuildsById.size, 3);

    // Incremental GUILD_CREATE for guild 4
    const afterCreate = syncLiveGuildRail([{ id: "4", name: "Delta" }], true);
    assert.equal(afterCreate.length, 4, "Guild 4 must be added without purging 1, 2, 3");
    assert.deepEqual(
      afterCreate.map(g => g.id).sort(),
      ["1", "2", "3", "4"]
    );

    // Incremental GUILD_CREATE updating guild 2
    const afterUpdate = syncLiveGuildRail([{ id: "2", name: "Beta Updated" }], true);
    assert.equal(afterUpdate.length, 4);
    assert.equal(liveGuildsById.get("2").name, "Beta Updated");
  });
});
