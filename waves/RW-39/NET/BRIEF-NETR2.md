# BRIEF NET-R2 — RE окна входа, captcha и 2FA по дампу

Владелец: NET-R2 (RE). PM: DeepSeek. Read-only, без сети/окна.
Вход: `/home/sanya/re-deep/app-asar/`, `asar-extract/`, `ui/`, `backend-ipc.json/md`,
`docs/re/main-arch.md`; `NET/AUTH-RE.md` (A-02…A-08); REA при необходимости.

## Задачи
1. Найти login-окно в бандле: разметка/стили/компоненты, поля (email/phone), password,
   «Забыли пароль?», QR-вкладка, регистрация; размеры/цвета/шрифты/состояния ошибок.
2. Captcha: сервис (hCaptcha?), sitekey, поля ответа (`captcha_key`, `captcha_rqtoken`,
   `captcha_service`), где рендерится, коды ошибок; интерактивность (пользователь решает).
3. 2FA-экран: методы (`totp`, `sms`, `backup`, `webauthn`, `password`), поля, SMS-send,
   тексты/ошибки, переходы.
4. Скриншоты/HTML/JSON из дампа, если есть; если нет — честный gap, не выдумывать.

## Выход
- `NET/LOGIN-UI-RE.md` — таблица `id | факт | [D]/[M]/[A] | источник (файл/строка) | метод`;
- `NET/login-ui-re.json` — машиночитаемо;
- логи `NET/PM/netr2/`.

## Запрет
Копировать ассеты, сеть, окно, cargo, commit; любые рекомендации по обходу captcha.

## Отчёт
Сколько фактов, что найдено по captcha/2FA, чего нет, md5 выходов.
