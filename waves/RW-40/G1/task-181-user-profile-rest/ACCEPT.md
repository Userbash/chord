# Приёмка PM: task-181 (BE-P2-10) — Профиль пользователя `GET /users/{id}/profile`

- **PR:** PR-66 (`feature/task-181-user-profile-rest -> unstable`)
- **Статус:** `Ready for Technical Review` (передано Техническому ревьюеру в Review 95A)
- **Дата:** 2026-10-31
- **Автор ревью:** Product Manager (PM)

## Проверка соответствия ТЗ и DoD
1. **Маршрут и query-параметры по оракулу `reference-oracle/rest-static.json` (`USER_PROFILE`):**
   - `USER_PROFILE_PATH_TEMPLATE = "/users/{id}/profile"` и `DEFAULT_POPOUT_QUERY = "type=popout&with_mutual_guilds=true&with_mutual_friends=true&with_mutual_friends_count=false"`.
   - `build_user_profile_path` проверяет снежинки `user_id` и `guild_id` (`u64`, ASCII-цифры) и защищает `type` от query-инъекций до обращения к сети.
2. **Типизированный ответ и обработка ошибок:**
   - `UserProfileResponse`, `ProfileUser`, `ProfileBadge`, `ConnectedAccount`, `GuildMemberProfile`, `MutualGuild` с обязательными непустыми `user.id` и `user.username`.
   - `ProfileError` с маппингом `401/403/404/429/4xx/5xx` и разбором `retry_after` из JSON и заголовков.
3. **Тесты и чистота:**
   - Red -> Green подтверждён (`26/26` в `profile_contract` + `17/17` lib).
   - Clippy (`-D warnings`) и `rustfmt --check` чистые, write-scope соблюдён.

## Решение PM
**ПРИНЯТО (Accepted for Technical Review).** Передаётся на Техническое ревью (Review 95A) с проверкой 5/5 мутаций.
