// Ревью RW-03: мутационное тестирование в КОПИИ (mut/); источники волны не трогаются.
// Для каждой мутации: восстановить оригинал → заменить фрагмент → прогнать
// `cargo test --offline -p discord-core --test gateway_session` → зафиксировать упавшие тесты.
import { execFileSync } from "node:child_process";
import fs from "node:fs";

const MUT_DIR = "/tmp/opencode/rw-03/review/mut";
const SRC = `${MUT_DIR}/crates/discord-core/src/gateway.rs`;
const ORIG = fs.readFileSync(`${MUT_DIR}/gateway.rs.orig`, "utf8");
const TARGET = "/tmp/opencode/rw-03/review/mut-target";

const mutations = [
  {
    id: "a",
    name: "jitter всегда 0 (игнор seed)",
    from: "self.next_beat_ms = Some(now_ms.saturating_add(self.jitter_seed % interval));",
    to: "self.next_beat_ms = Some(now_ms);",
  },
  {
    id: "b",
    name: "дедлайн зомби sent + interval*2",
    from: "Some(sent.saturating_add(self.interval_ms()?))",
    to: "Some(sent.saturating_add(self.interval_ms()?.saturating_mul(2)))",
  },
  {
    id: "c",
    name: "после ACK next_beat = now (без +interval)",
    from: "self.next_beat_ms = Some(now_ms.saturating_add(interval));",
    to: "self.next_beat_ms = Some(now_ms);",
  },
  {
    id: "d",
    name: "в ударе всегда d: None",
    from: "return vec![Effect::SendHeartbeat { d: self.last_seq }];",
    to: "return vec![Effect::SendHeartbeat { d: None }];",
  },
  {
    id: "e",
    name: "RTT = now вместо now - sent",
    from: "self.rtt_ms = Some(now_ms.saturating_sub(sent));",
    to: "self.rtt_ms = Some(now_ms);",
  },
  {
    id: "f",
    name: "повторный HELLO после первого удара не игнорируется",
    from: `        if self.beats_emitted > 0 {
            return Ok(Vec::new());
        }`,
    to: `        if false {
            return Ok(Vec::new());
        }`,
  },
  {
    id: "g",
    name: "нулевой интервал не отвергается (деление на 0)",
    from: "            .filter(|interval| *interval > 0)",
    to: "            .filter(|interval| *interval >= 0)",
  },
  {
    id: "h",
    name: "«лишний» ACK без pending переносит расписание",
    from: `        let (Some(sent), Some(interval)) = (self.pending_beat_ms, self.interval_ms()) else {
            return;
        };
        self.pending_beat_ms = None;
        self.rtt_ms = Some(now_ms.saturating_sub(sent));
        self.next_beat_ms = Some(now_ms.saturating_add(interval));`,
    to: `        let Some(interval) = self.interval_ms() else {
            return;
        };
        if let Some(sent) = self.pending_beat_ms {
            self.rtt_ms = Some(now_ms.saturating_sub(sent));
        }
        self.pending_beat_ms = None;
        self.next_beat_ms = Some(now_ms.saturating_add(interval));`,
  },
  {
    id: "i",
    name: "on_value мёртвой сессии не молчит",
    from: `        if self.dead {
            // Мёртвая сессия молчит: транспорт уже переподключается.
            return Ok(Vec::new());
        }`,
    to: `        if false {
            return Ok(Vec::new());
        }`,
  },
];

const results = [];
for (const m of mutations) {
  const count = ORIG.split(m.from).length - 1;
  if (count !== 1) {
    results.push({ ...m, status: `ОШИБКА ПОДГОТОВКИ: фрагмент встречается ${count} раз` });
    continue;
  }
  fs.writeFileSync(SRC, ORIG.replace(m.from, m.to));
  let out = "";
  let exit = 0;
  try {
    out = execFileSync(
      "cargo",
      ["test", "--offline", "-p", "discord-core", "--test", "gateway_session"],
      { cwd: MUT_DIR, env: { ...process.env, CARGO_TARGET_DIR: TARGET }, encoding: "utf8" },
    );
  } catch (e) {
    exit = e.status ?? -1;
    out = (e.stdout ?? "") + (e.stderr ?? "");
  }
  const failed = [...out.matchAll(/^test ([^\s]+) \.\.\. FAILED$/gm)].map((x) => x[1]);
  const passed = /test result: ok\./.test(out);
  const compileErr = /error\[|error: could not compile/.test(out) && !/panicked at/.test(out);
  results.push({
    ...m,
    status: passed ? "НЕ ПОЙМАНА (тесты зелёные)" : compileErr ? "ОШИБКА КОМПИЛЯЦИИ (не засчитывается)" : "ПОЙМАНА",
    exit,
    failed,
    panics: [...out.matchAll(/panicked at ([^\n]+)/g)].map((x) => x[1]).slice(0, 2),
  });
  fs.writeFileSync(SRC, ORIG); // вернуть оригинал
}

console.log(JSON.stringify(results, null, 2));
fs.writeFileSync("/tmp/opencode/rw-03/review/mutations.json", JSON.stringify(results, null, 2));
