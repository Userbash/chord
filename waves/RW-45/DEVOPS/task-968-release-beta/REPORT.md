# Отчёт о выполнении TASK-968 — GitHub Actions CI/CD матричный релизный пайплайн

| Поле | Значение |
| :--- | :--- |
| **Волна / Batch** | `Wave RW-45` (`DevOps / Integration`) |
| **Карточка** | `TASK-968` — GitHub Actions CI/CD матричный релизный пайплайн (`.github/workflows/release-beta.yml`) |
| **Статус** | `Ready for Product Review` / `Ready for Technical Review` |
| **Дата** | `2026-11-11` |

---

## 1. Сделано

1. **По методологии Strict TDD (`RED -> GREEN`) разработан контрактный и модульный тест [`tests/unit/github-actions-release-968.test.mjs`](file:///home/sanya/orca/project/tests/unit/github-actions-release-968.test.mjs):**
   - Независимый системный оракул парсинга YAML через `python3` (`PyYAML` `yaml.safe_load`) с проверкой отсутствия `CRLF` и табуляций.
   - Экспортируемый валидатор `validateReleaseWorkflow(wf)` с проверкой принципа наименьших привилегий (`permissions.contents: read` на верхнем уровне и `contents: write` только в `publish-release`), блокировки параллельной отмены релиза (`cancel-in-progress: false`), пиннинга экшенов и защиты от утечки токенов.
   - Проверка полноты матрицы `package-matrix` (`5` дистрибутивов / форматов: `tar.gz`, `appimage`, `deb`, `rpm`, `arch`), существования всех вызываемых скриптов (`scripts/release.sh`, `scripts/build-appimage.sh`, `packaging/deb/deb-package.mjs`, `packaging/rpm/discord-gpui.spec`, `packaging/arch/PKGBUILD`) и контрактных тестов на диске.
   - Изолированный поведенческий тест генерации и проверки `SHA256SUMS` через `sha256sum -c SHA256SUMS` (включая негативный сценарий повреждения байта в `.deb` пакете).
   - Проверка публикации GitHub Release (`softprops/action-gh-release@v2`) с `body_path: RELEASE-NOTES-v1.0.0-beta.1.md`, `fail_on_unmatched_files: true` и полным набором релизных артефактов.
2. **Разработан релизный CI/CD workflow [`.github/workflows/release-beta.yml`](file:///home/sanya/orca/project/.github/workflows/release-beta.yml):**
   - **`preflight-gate`**: сканирование секретов (`scripts/secret-scan.mjs`), валидация SBOM SPDX 2.3 (`scripts/sbom.mjs --check`), проверка `RELEASE-NOTES-v1.0.0-beta.1.md`, `docs/BETA-TESTING-GUIDE.md` и `tests/unit/beta-release-package-958.test.mjs`.
   - **`package-matrix`**: матричная сборка и контрактная верификация 5 Linux-пакетов (`.tar.gz`, `.AppImage`, `.deb`, `.rpm`, `.pkg.tar.zst`) с `fail-fast: false` и выгрузкой артефактов + `.sha256`.
   - **`publish-release`**: сборка единого манифеста `SHA256SUMS`, самопроверка `sha256sum -c SHA256SUMS` и публикация GitHub Release с `RELEASE-NOTES-v1.0.0-beta.1.md`.

---

## 2. Доказательства TDD (Red / Green / Targeted / Typecheck)

- **RED-прогон** ([`waves/RW-45/DEVOPS/task-968-release-beta/red.log`](file:///home/sanya/orca/project/waves/RW-45/DEVOPS/task-968-release-beta/red.log)):
  - Команда: `node --test --test-concurrency=1 tests/unit/github-actions-release-968.test.mjs`
  - Результат до создания `.github/workflows/release-beta.yml`: `tests 5 | pass 0 | fail 5` (`AssertionError: Файл workflow должен существовать: .../.github/workflows/release-beta.yml`).
- **GREEN-прогон** ([`waves/RW-45/DEVOPS/task-968-release-beta/green.log`](file:///home/sanya/orca/project/waves/RW-45/DEVOPS/task-968-release-beta/green.log)):
  - Команда: `node --test --test-concurrency=1 tests/unit/github-actions-release-968.test.mjs`
  - Результат после создания `.github/workflows/release-beta.yml`: `tests 5 | pass 5 | fail 0` (`148.36ms`).
- **Targeted & Meta Gate (`T0` + `tsc`)**:
  - `npx tsc -p tsconfig.json` -> `0 errors`.
  - `node --test --test-concurrency=1 tests/unit/github-actions-release-968.test.mjs tests/unit/beta-release-package-958.test.mjs tests/meta/test-policy.test.mjs tests/meta/ci-browser.test.mjs` -> `tests 36 | pass 36 | fail 0`.

---

## 3. Проверка 5/5 мутаций (Mutation Gate)

| № | Мутация | Ожидаемый перехват | Статус |
| :-: | :--- | :--- | :---: |
| `M1` | Повышение глобальных прав `permissions.contents: write` на верхнем уровне | Шаг 1 / Шаг 5 (`validateReleaseWorkflow`) | **KILLED** |
| `M2` | Удаление формата `rpm` из `strategy.matrix.include` | Шаг 2 / Шаг 5 (`REQUIRED_MATRIX_FORMATS`) | **KILLED** |
| `M3` | Сокращение числа дистрибутивов в матрице до `< 4` | Шаг 2 / Шаг 5 (`uniqueDistros.size >= 4`) | **KILLED** |
| `M4` | Замена пиннинга `actions/checkout@v4` на плавающий тег `@main` | Шаг 1 / Шаг 5 (`/@(main\|master\|latest\|HEAD)$/i`) | **KILLED** |
| `M5` | Повреждение байта пакета перед проверкой `sha256sum -c SHA256SUMS` | Шаг 3 (`tamperCheck.status !== 0`) | **KILLED** |

---

## 4. Файлы и контрольные суммы (MD5 / SHA-256)

| Файл | MD5 | SHA-256 |
| :--- | :--- | :--- |
| [`.github/workflows/release-beta.yml`](file:///home/sanya/orca/project/.github/workflows/release-beta.yml) | `2b2ea9f37c36408909abd1b9ac4fd9f0` | `43797c625116d3c6fb6d6fb6080c26777ac75fcd2b6fb5314970958b8847a88c` |
| [`tests/unit/github-actions-release-968.test.mjs`](file:///home/sanya/orca/project/tests/unit/github-actions-release-968.test.mjs) | `f065ba5968fb7423d5a2385efee90df9` | `4c8717a959d0584b5372e85af6db86270ad5f6c5e7905e5cf267cd7fa0357919` |

---

## 5. Риски и ограничения

- Сетевая публикация в GitHub Releases выполняется только в джобе `publish-release` с `permissions: contents: write` после успешного прохождения `preflight-gate` и всех 5 матричных сборок `package-matrix` и самопроверки `sha256sum -c SHA256SUMS`.
- При несовпадении хотя бы одного пакета или отсутствии файла релизных заметок `fail_on_unmatched_files: true` и preflight-проверки аварийно останавливают публикацию.
