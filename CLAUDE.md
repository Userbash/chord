# CLAUDE.md — ECC 2.0 Control Plane & Правила разработки `discord-lab`

Этот файл автоматически загружается во всех сессиях и включает **ECC 2.0 (Everything Claude Code)** по умолчанию для всего репозитория `discord-lab`.

## 1. Конфигурация ECC по умолчанию
- **Манифест ECC 2.0 Control Plane:** [`.claude/ecc2.toml`](file:///home/sanya/orca/project/.claude/ecc2.toml)
- **Мост ECC ↔ Orca Orchestration:** [`scripts/ecc-orca-bridge.sh`](file:///home/sanya/orca/project/scripts/ecc-orca-bridge.sh)
- **Правила отделов и P0–P2 интеграции:** [`.agents/rules/ecc-discord-lab-departments.md`](file:///home/sanya/orca/project/.agents/rules/ecc-discord-lab-departments.md)
- **Основные правила репозитория:** [`AGENTS.md`](file:///home/sanya/orca/project/AGENTS.md), [`PROCESS.md`](file:///home/sanya/orca/project/PROCESS.md), [`TEAM.md`](file:///home/sanya/orca/project/TEAM.md), [`TEST-POLICY.md`](file:///home/sanya/orca/project/TEST-POLICY.md)
- **Скиллы и субагенты ECC:** `.agents/skills/*`, `.agents/agents/*`, `.agents/rules/common-*.md`

## 2. Управление воркерами через ECC и Orca
1. **Отделы специалистов (`write-scope` без пересечений):**
   - `g1_backend`: `rust/crates/discord-{core,codec,media,store,i18n}`, `rust/gui-spike/src/*.rs` (кроме `voice_devices.rs`), `rust/gui-spike/tests/*_contract.rs`
   - `g2_frontend`: `rust/gui-spike/ui/**/*.{js,css,html}`, `tests/unit/*.test.mjs`
   - `g3_design`: `docs/specs/*.md`, `waves/RW-*/DESIGN/*.md`
   - `g4_qa`: `tests/{meta,unit,smoke,e2e}/`, `scripts/test-impact.mjs`, `QA-BUILD-*.md`
   - `g5_voice_platform`: `rust/crates/discord-voice/**`, `rust/crates/discord-platform/**`, `rust/gui-spike/src/voice_devices.rs`, опкоды `18..=22`, SRTP/DAVE E2EE
   - `g7_re_rea`: Read-only сверка с `/home/sanya/re-deep/`, `docs/reference-projects/` и MCP `rea`
2. **Маршрутизация моделей и лимит памяти (`WORKER_RAM_LIMIT <= 25 GB`):**
   - **Запрет `codex`:** никогда не использовать `--agent codex`.
   - **Antigravity (`--agent antigravity --model gemini-4-argon-high --effort high`):** первичная разработка новых фич и параллельные воркеры.
   - **DeepSeek (`--agent opencode --model deepseek/deepseek-v4.1-flash`):** техническое ревью (`REVIEW.md`), QA и мутации `5/5` в разрешённом окне по Пекину (`TZ=Asia/Shanghai`: Пн–Пт с 18:00, Сб–Вс круглосуточно).
   - **Пул слотов:** не более `5` параллельных воркеров (`max_parallel_sessions = 5` в `.claude/ecc2.toml`), не более одного процесса `cargo` одновременно на общем `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target`.
3. **Обязательный цикл верификации ECC (`verification-loop` & `code-reviewer`):**
   - Проверка синтаксиса и типов (`node --check`, `npm run typecheck`, `cargo check`/`cargo build`).
   - Сборка чистого бинарного артефакта `dist/discord-client` с обновлением `SHA256SUMS`.
   - Непрерывный сбор live-телеметрии кликов и вызовов функций (`pm_live_telemetry_report.md`) для PM и отделов разработки.
