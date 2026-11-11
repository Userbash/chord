# REPORT — TASK-962: Манифест Flatpak `com.discord_lab.DiscordGpui.yml` и песочница Wayland/PipeWire

- **Волна / Карточка:** Wave RW-45 / `TASK-962` (`task_1b5e08518826`, dispatch `ctx_2922976b5049`)
- **Отдел:** `DevOps / Integration`
- **Приоритет:** `P1`
- **Статус:** Выполнено (все критерии приёмки DoD 1–3 закрыты на 100%)

---

## 1. Сделано

1. **Подзадача 962.1 — Манифест Flatpak [`packaging/flatpak/com.discord_lab.DiscordGpui.yml`](file:///home/sanya/orca/project/packaging/flatpak/com.discord_lab.DiscordGpui.yml):**
   - Задан идентификатор приложения `app-id: com.discord_lab.DiscordGpui`, базовый рантайм `runtime: org.freedesktop.Platform`, версия `runtime-version: "23.08"`, SDK `sdk: org.freedesktop.Sdk` и точка входа `command: discord-gpui`.
   - Сформирована минимальная конфигурация песочницы `finish-args` со строгой сверкой с эталонным RE-дампом `/home/sanya/re-deep/native-re.md` (§2.1–2.2, `$XDG_RUNTIME_DIR/pipewire-0`, `org.freedesktop.portal.ScreenCast`) и модулем системного хранилища [`rust/crates/discord-platform/src/keyring.rs`](file:///home/sanya/orca/project/rust/crates/discord-platform/src/keyring.rs):
     - `--socket=wayland` и `--socket=fallback-x11` (нативный Wayland с резервным X11 без прямого `--socket=x11`);
     - `--filesystem=xdg-run/pipewire-0` и `--socket=pulseaudio` (PipeWire DMA-BUF захват экрана и низколатентное аудио);
     - `--talk-name=org.freedesktop.secrets` и `--talk-name=org.freedesktop.portal.Desktop` (Secret Service portal для хранения токенов и XDG Desktop Portal для ScreenCast/Secret);
     - `--device=dri`, `--share=ipc`, `--share=network`, `--talk-name=org.freedesktop.Notifications`, `--talk-name=org.kde.StatusNotifierWatcher`, `--talk-name=org.a11y.Bus`, `--env=RUST_LOG=info`.
   - Описан сборочный модуль `discord-gpui` (`buildsystem: simple`) с установкой stripped-бинарника, SPDX SBOM и лицензионных файлов и побайтовой привязкой `sha256` к оракулу [`rust/dist/SHA256SUMS`](file:///home/sanya/orca/project/rust/dist/SHA256SUMS).

2. **Подзадача 962.2 — Контрактный и модульный тест [`tests/unit/flatpak-manifest-962.test.mjs`](file:///home/sanya/orca/project/tests/unit/flatpak-manifest-962.test.mjs) (Strict TDD):**
   - Реализована проверка схемы манифеста через независимый системный оракул `python3` (`PyYAML` `yaml.safe_load`) и сверка хешей `sha256` с оракулом `rust/dist/SHA256SUMS` и фактическими файлами в `rust/dist/`.
   - Покрыты все обязательные права песочницы, запрет избыточных разрешений (`--filesystem=host/home`, `--socket=x11`, `--device=all`, `--socket=session-bus/system-bus`, `--allow=devel`) и негативные/граничные сценарии валидатора `validateFlatpakManifest`.

---

## 2. Red-прогон (TDD)

Лог: [`waves/RW-45/DEVOPS/task-962-flatpak-manifest/red.log`](file:///home/sanya/orca/project/waves/RW-45/DEVOPS/task-962-flatpak-manifest/red.log) (запуск до создания `packaging/flatpak/com.discord_lab.DiscordGpui.yml`):

```text
▶ TASK-962: Манифест Flatpak com.discord_lab.DiscordGpui.yml и песочница Wayland/PipeWire
  ✖ SUB-962-1: манифест packaging/flatpak/com.discord_lab.DiscordGpui.yml существует, валиден по YAML-схеме и задаёт runtime org.freedesktop.Platform 23.08 (0.696629ms)
  ✖ SUB-962-2: finish-args включает обязательные права --socket=wayland, --socket=fallback-x11, --filesystem=xdg-run/pipewire-0 и Secret Service portal (30.984097ms)
  ✖ SUB-962-3: права sandboxing строго минимальны (отсутствуют --filesystem=host/home, прямой --socket=x11, --device=all, --socket=session-bus/system-bus) (31.892049ms)
  ✖ SUB-962-4: модуль discord-gpui и его источники ссылаются на реальные артефакты rust/dist/ с точным совпадением SHA-256 по оракулу SHA256SUMS (32.565718ms)
  ✖ SUB-962-5: валидатор validateFlatpakManifest отсекает негативные, граничные и небезопасные мутации манифеста (31.136869ms)
ℹ tests 5 | suites 1 | pass 0 | fail 5
```

---

## 3. Green-прогон (TDD)

Лог: [`waves/RW-45/DEVOPS/task-962-flatpak-manifest/green.log`](file:///home/sanya/orca/project/waves/RW-45/DEVOPS/task-962-flatpak-manifest/green.log):

```text
▶ TASK-962: Манифест Flatpak com.discord_lab.DiscordGpui.yml и песочница Wayland/PipeWire
  ✔ SUB-962-1: манифест packaging/flatpak/com.discord_lab.DiscordGpui.yml существует, валиден по YAML-схеме и задаёт runtime org.freedesktop.Platform 23.08 (33.307014ms)
  ✔ SUB-962-2: finish-args включает обязательные права --socket=wayland, --socket=fallback-x11, --filesystem=xdg-run/pipewire-0 и Secret Service portal (22.54074ms)
  ✔ SUB-962-3: права sandboxing строго минимальны (отсутствуют --filesystem=host/home, прямой --socket=x11, --device=all, --socket=session-bus/system-bus) (23.525753ms)
  ✔ SUB-962-4: модуль discord-gpui и его источники ссылаются на реальные артефакты rust/dist/ с точным совпадением SHA-256 по оракулу SHA256SUMS (68.572501ms)
  ✔ SUB-962-5: валидатор validateFlatpakManifest отсекает негативные, граничные и небезопасные мутации манифеста (22.925815ms)
ℹ tests 5 | suites 1 | pass 5 | fail 0 | skipped 0
```

---

## 4. Targeted & Мутационный гейт (5/5)

- **Targeted (`node --test tests/unit/flatpak-manifest-962.test.mjs`):** `5/5 pass (0 fail, 0 skip)`.
- **Мутационный гейт (`waves/RW-45/DEVOPS/task-962-flatpak-manifest/mutation.log`):**
  1. `M1` (`runtime-version -> 22.08`) — **KILLED**
  2. `M2` (удаление `--socket=wayland`) — **KILLED**
  3. `M3` (замена `--socket=fallback-x11` на `--socket=x11`) — **KILLED**
  4. `M4` (удаление `--filesystem=xdg-run/pipewire-0`) — **KILLED**
  5. `M5` (удаление `--talk-name=org.freedesktop.secrets`) — **KILLED**
  - Итог: **5/5 мутаций убито (100% kill rate)**, оригинал манифеста побайтно восстановлен и проверен по MD5.

---

## 5. Файлы и MD5

| Файл | MD5 |
|---|---|
| `packaging/flatpak/com.discord_lab.DiscordGpui.yml` | `897e1179c36a5ed4c3e78833bb77f5cf` |
| `tests/unit/flatpak-manifest-962.test.mjs` | `85712a7ea97e4b9de4fa97f21e9a2efa` |

---

## 6. Риски и замечания

- Избыточные права доступа к файловой системе хоста (`--filesystem=host`, `--filesystem=home`), прямой `--socket=x11`, `--device=all` и нефильтрованный доступ к шине D-Bus полностью исключены.
- Блокеров нет (`P0 = 0`, `P1 = 0`).
