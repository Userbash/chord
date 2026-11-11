# Независимое ревью QA-M2-04

- **HEAD:** `4e240481e59ff4d49732ac8bd5ba08a006363748`
- **Режим:** read-only по workflow, meta-тесту и сохранённым логам; новые тесты не запускались.
- **Вердикт:** **ACCEPT** по проверяемой структуре и представленным локальным evidence.

## Сверка

- `.github/workflows/ci.yml` содержит `qa-browser` на `ubuntu-24.04`, Node 24,
  `npm ci`, Chromium `131.0.6778.85`, экспорт `CHROME_BIN` из `chrome-path`, обязательный
  executable preflight и последовательный fixture e2e. Сборка исключает `live-*` и
  снимает `DISCORD_LAB_LIVE_CDP`/`CHROME_PATH`; live вынесен в отключённый `qa-live`.
- Логи `QA-M2-04-red.log` и `QA-M2-04-green.log` подтверждают ожидаемую последовательность:
  до workflow 3/10, после — 10/10. `QA-M2-04-verify-meta.log` также показывает 10/10.
  Сохранённый suite-log сообщает о единственном `plan-sync` fail на не относящихся к этой
  карточке файлах; это не скрыто и не выдано за полностью зелёный meta-suite.
- Сам workflow сохраняет `.gate` через `upload-artifact` с `if: always()`. Файл
  `tests/meta/ci-browser.test.mjs` проверяет перечисленные структурные требования и
  read-only permissions. Заявленные md5 для workflow и meta-теста совпадают с текущими.
- Устаревший в отчёте риск untracked-зависимостей снят на текущем дереве: browser-job,
  meta-тест, e2e и UI fixture-зависимости проверены как tracked через git index.

## Граница evidence

Реальный GitHub Actions запуск `qa-browser` не представлен: локальный отчёт прямо
указывает, что Chromium job не запускался на hosted runner. Поэтому ACCEPT подтверждает
контракт workflow и локальные структурные проверки, но не фактическую установку pinned
Chrome или успешное выполнение полного e2e набора в CI. Это ограничение уже явно указано
в `QUALITY/QA-M2-04.md` и не является скрытым расхождением отчёта.
