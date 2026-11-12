# EVIDENCE — G6 RW-40 (эталон ↔ приложение)

Каталог содержит **выдержки и хеши**, а не копии эталона. Секретов, токенов, паролей,
cookie и ключей нет; в цитатах нет пользовательских данных, кроме публичных имён,
уже присутствовавших в RE-дампе (`ShadowByte`, `ITSKIDON`).

| Файл | Что |
|---|---|
| `hashes.txt` | SHA-256 asar/ELF/дампов/нормативных отчётов; REA evidence id |
| `ref-166-excerpts.md` | E1 из установленного 1.0.166: deep links, autostart, 203 IPC-события, нативные модули |
| `ref-162-ui-excerpts.md` | E1/E3 из дампа 1.0.162: меню сервера, каналы, голосовая панель, i18n-строки, gateway/REST |

## Уровни (вместо `confirmed`)

`[L]` live-захват · `[S]` call-site в коде эталона · `[D]` только Enum/путь · `[M]` измерение
(DOM/скрин/каталог) · `[A]` нет подтверждения. Поле `confirmed` из `rest-static.json`
не используется: оно `false` у всех 904 записей.

## Границы

1. `app.asar` 1.0.166 содержит desktop main + preload, **не** renderer. UI-функции
   подтверждены дампом 1.0.162; это явный `version-gap`, а не сглаживание.
2. Live-сессия 1.0.166 **не запускалась** (запрет окна PM). Никаких live-утверждений
   о 1.0.166 нет; E3 относится только к дампу 1.0.162.
3. `IPCEvents` — статические константы. Наличие константы не доказывает runtime-поведение.
4. `attachments` в `MESSAGE_CREATE` наблюдались только как `null`; схема вложения — unknown.
5. `STREAM_*`/Go Live сигнализация в дампе отсутствует; экранный шаринг подтверждён только
   UI-строкой и capture-IPC, не полным потоком.
6. Цитаты 1.0.166 — из распакованных в `/tmp/opencode/ref166` копий; исходные asar
   не изменялись (read-only).

## Воспроизведение

```
# хеши
sha256sum /home/user/.config/discord/app-1.0.166/resources/app.asar
# REA
analyze_javascript_application(input_path=app.asar, format=asar, detail=summary)
# смещения 1.0.166
python3 - <<'PY'
s=open('/tmp/opencode/ref166/app/bundle.js',encoding='utf-8',errors='replace').read()
print(s.find('open-url'))
PY
```
