#!/usr/bin/env python3
"""Мутации task-293 (main.rs) для технического ревью 200A."""
import sys

PATH = "rust/gui-spike/src/main.rs"

BASE_FN = """fn invoke_bff_domain(
    action: discord_gui_spike::bff_domains_router::BffDomainAction,
) -> Result<discord_gui_spike::bff_domains_router::BffResolvedAction, String> {
    discord_gui_spike::bff_domains_router::resolve_bff_domain_action(action)
}"""

MUTATIONS = {
    # M1: команда не зарегистрирована в invoke_handler!
    "M1": (None, None),
    # M2: маршрут подменён
    "M2": (
        BASE_FN,
        """fn invoke_bff_domain(
    action: discord_gui_spike::bff_domains_router::BffDomainAction,
) -> Result<discord_gui_spike::bff_domains_router::BffResolvedAction, String> {
    let mut resolved = discord_gui_spike::bff_domains_router::resolve_bff_domain_action(action)?;
    resolved.route = resolved.route.replace("welcome-screen", "onboarding");
    Ok(resolved)
}""",
    ),
    # M3: ошибка глотается — невалидный вход превращается в Ok
    "M3": (
        BASE_FN,
        """fn invoke_bff_domain(
    action: discord_gui_spike::bff_domains_router::BffDomainAction,
) -> Result<discord_gui_spike::bff_domains_router::BffResolvedAction, String> {
    match discord_gui_spike::bff_domains_router::resolve_bff_domain_action(action) {
        Ok(resolved) => Ok(resolved),
        Err(_) => Ok(discord_gui_spike::bff_domains_router::BffResolvedAction {
            method: "PATCH",
            route: String::new(),
            body: serde_json::Value::Null,
        }),
    }
}""",
    ),
    # M4: вход игнорируется, ответ захардкожен под позитивный кейс теста
    "M4": (
        BASE_FN,
        """fn invoke_bff_domain(
    action: discord_gui_spike::bff_domains_router::BffDomainAction,
) -> Result<discord_gui_spike::bff_domains_router::BffResolvedAction, String> {
    let _ = action;
    Ok(discord_gui_spike::bff_domains_router::BffResolvedAction {
        method: "PATCH",
        route: "/api/v9/guilds/123456789012345678/welcome-screen".to_string(),
        body: serde_json::json!({"enabled": true, "description": null, "welcome_channels": []}),
    })
}""",
    ),
    # M5: из тела ответа выпадает поле
    "M5": (
        BASE_FN,
        """fn invoke_bff_domain(
    action: discord_gui_spike::bff_domains_router::BffDomainAction,
) -> Result<discord_gui_spike::bff_domains_router::BffResolvedAction, String> {
    let mut resolved = discord_gui_spike::bff_domains_router::resolve_bff_domain_action(action)?;
    if let Some(map) = resolved.body.as_object_mut() {
        map.remove("description");
    }
    Ok(resolved)
}""",
    ),
}

mut = sys.argv[1]
src = open(PATH, encoding="utf-8").read()

if mut == "M1":
    old = "            settings_save,\n            invoke_bff_domain\n"
    new = "            settings_save\n"
    assert src.count(old) == 1, "M1: якорь регистрации не найден"
    src = src.replace(old, new)
else:
    old, new = MUTATIONS[mut]
    assert src.count(old) == 1, f"{mut}: якорь функции не найден"
    src = src.replace(old, new)

open(PATH, "w", encoding="utf-8").write(src)
print(f"{mut}: применена")
