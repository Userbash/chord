//! RW-40 / TASK-626 (`G1 Backend`): контрактные тесты модуля Nitro Gifts & Entitlements
//! (`rust/gui-spike/src/gifts.rs` и Tauri IPC-команды в `rust/gui-spike/src/main.rs`).
//!
//! Оракулы (`/home/sanya/re-deep/` и `waves/RW-40/`):
//! - `waves/RW-40/RE-DEEP-REA.md` §5 (`STUB-12 — кнопка Gift 🎁`, маршруты `@290386..@290799`,
//!   `createGiftCode` `@613587`, `gift_style` `0..=2`);
//! - `waves/RW-40/DESIGN/ATTACHMENTS-SPEC.md` §4.1–§4.5 (`#gift-modal`, лимит `200` символов,
//!   расчёт `gift_style`: `2` при непустом сообщении, иначе `1`);
//! - `/home/sanya/re-deep/decoded/4d2a64cb5df5fc4e_0`:
//!   - модуль `869038` (`@613060`): `resolveGiftCode`, `fetchUserGiftCodesForSKU`,
//!     `createGiftCode`, `revokeGiftCode`, `USER_GIFT_CODE_DELIVERIES`;
//!   - модуль `75255` (`@735306`): `redeemGiftCode` (`POST /entitlements/gift-codes/{code}/redeem`,
//!     тело `{channel_id, payment_source_id, gateway_checkout_context}`);
//!   - модуль `339048` (`@596307`): `fetchEntitlementsForApplication`, `fetchEntitlementsForUser`,
//!     `fetchGiftableEntitlements`;
//!   - модуль `45938` (`@9711598`): `GiftStyle` (`0 = DEFAULT`, `1 = CUSTOM_STYLE`,
//!     `2 = CUSTOM_MESSAGE_EMOJI_SOUNDBOARD`), `resolveGiftCode` (`?with_application=...&with_subscription_plan=...`),
//!     извлечение подарочных ссылок (`discord.gift`, `discord.com/gifts`, `discordapp.com/gifts`,
//!     `promos.discord.gg`, `discord.com/billing/promotions`, не более 3 кодов);
//!   - модуль `7133` (`@8906800`): `GiftCodeRecord` (`createFromServer`, флаги `PAYMENT_SOURCE_REQUIRED = 1`,
//!     `EXISTING_PREMIUM_SUBSCRIPTION_DISALLOWED = 2`, `NOT_SELF_REDEEMABLE = 4`);
//!   - модуль `557009` (`@8904412`): `EntitlementRecord` (`createFromServer`, `isGiftable`:
//!     `type === USER_GIFT (6) && gifterId == null`);
//!   - модуль `202541` (`@6789126..@6797000`): `NITRO_APPLICATION_ID = "521842831262875670"`,
//!     SKU (`TIER_0 = "978380684370378762"`, `TIER_1 = "521846918637420545"`,
//!     `TIER_2 = "521847234246082599"`, `GUILD = "590663762298667008"`,
//!     `NONE = "628379670982688768"`, `LEGACY = "521842865731534868"`) и планы подписок;
//!   - коды ошибок Discord (`@320207`, `@325314`): `UNKNOWN_GIFT_CODE = 10038`,
//!     `INVALID_FORM_BODY = 50035`, `INVALID_GIFT_REDEMPTION_EXHAUSTED = 50050`,
//!     `INVALID_GIFT_REDEMPTION_OWNED = 50051`, `INVALID_GIFT_SELF_REDEMPTION = 50054`,
//!     `INVALID_GIFT_REDEMPTION_PREVIOUSLY_OWNED = 50092`,
//!     `INVALID_COUNTRY_CODE = 50095`, `INVALID_GIFT_REDEMPTION_FRAUD_REJECTED = 50097`.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use discord_core::rest::transport::HttpResponse;
pub use chord_client::rest_failure;

#[path = "../src/gifts.rs"]
pub mod gifts;

use gifts::{
    create_gift_code, extract_gift_codes_from_text, fetch_application_entitlements,
    fetch_giftable_entitlements, fetch_user_entitlements, fetch_user_gift_codes,
    list_nitro_gift_plans, redeem_gift_code, resolve_gift_code, resolve_gift_style,
    revoke_gift_code, CreateGiftCodeRequest, DiscordGifts, EntitlementQuery, GiftError,
    GiftRedeemRequest, GiftResolveOptions, GiftTransport, DISCORD_ERR_INVALID_COUNTRY_CODE,
    DISCORD_ERR_INVALID_FORM_BODY, DISCORD_ERR_INVALID_GIFT_REDEMPTION_EXHAUSTED,
    DISCORD_ERR_INVALID_GIFT_REDEMPTION_FRAUD_REJECTED, DISCORD_ERR_INVALID_GIFT_REDEMPTION_OWNED,
    DISCORD_ERR_INVALID_GIFT_REDEMPTION_PREVIOUSLY_OWNED, DISCORD_ERR_INVALID_GIFT_SELF_REDEMPTION,
    DISCORD_ERR_UNKNOWN_GIFT_CODE, ENTITLEMENT_TYPE_APPLICATION_SUBSCRIPTION,
    ENTITLEMENT_TYPE_DEVELOPER_GIFT, ENTITLEMENT_TYPE_FRACTIONAL_REDEMPTION,
    ENTITLEMENT_TYPE_FREE_PURCHASE, ENTITLEMENT_TYPE_FREE_STAFF_PURCHASE,
    ENTITLEMENT_TYPE_PREMIUM_PURCHASE, ENTITLEMENT_TYPE_PREMIUM_SUBSCRIPTION,
    ENTITLEMENT_TYPE_PURCHASE, ENTITLEMENT_TYPE_PURCHASE_REWARD, ENTITLEMENT_TYPE_QUEST_REWARD,
    ENTITLEMENT_TYPE_TEST_MODE_PURCHASE, ENTITLEMENT_TYPE_USER_GIFT,
    ENTITLEMENT_TYPE_VIRTUAL_CURRENCY_REDEMPTION,
    GIFT_FLAG_EXISTING_PREMIUM_SUBSCRIPTION_DISALLOWED, GIFT_FLAG_NOT_SELF_REDEEMABLE,
    GIFT_FLAG_PAYMENT_SOURCE_REQUIRED, GIFT_IPC_COMMANDS,
    GIFT_STYLE_CUSTOM_MESSAGE_EMOJI_SOUNDBOARD, GIFT_STYLE_CUSTOM_STYLE, GIFT_STYLE_DEFAULT,
    MAX_EXTRACTED_GIFT_CODES, MAX_GIFT_CODE_CHARS, MAX_GIFT_CUSTOM_MESSAGE_CHARS,
    MIN_GIFT_CODE_CHARS, NITRO_APPLICATION_ID, PLAN_GUILD_BOOST_MONTH, PLAN_GUILD_BOOST_YEAR,
    PLAN_NITRO_3_MONTH, PLAN_NITRO_6_MONTH, PLAN_NITRO_BASIC_MONTH, PLAN_NITRO_BASIC_YEAR,
    PLAN_NITRO_CLASSIC_MONTH, PLAN_NITRO_CLASSIC_YEAR, PLAN_NITRO_MONTH, PLAN_NITRO_YEAR,
    SKU_GUILD_BOOST, SKU_NITRO_BASIC_TIER_0, SKU_NITRO_CLASSIC_TIER_1, SKU_NITRO_LEGACY,
    SKU_NITRO_STANDARD_TIER_2, SKU_NONE,
};

fn run<F: Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("tokio runtime")
        .block_on(future)
}

type RequestLog = Arc<Mutex<Vec<(String, String, Vec<u8>)>>>;

#[derive(Clone, Default)]
struct FakeGiftTransport {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    error: Option<GiftError>,
    requests: RequestLog,
}

impl FakeGiftTransport {
    fn answering(status: u16, body: &[u8]) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: body.to_vec(),
            error: None,
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn with_headers(mut self, headers: &[(&str, &str)]) -> Self {
        self.headers = headers
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self
    }

    fn failing(error: GiftError) -> Self {
        Self {
            status: 0,
            headers: Vec::new(),
            body: Vec::new(),
            error: Some(error),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn calls(&self) -> Vec<(String, String, Vec<u8>)> {
        self.requests.lock().expect("requests lock").clone()
    }
}

impl GiftTransport for FakeGiftTransport {
    fn get<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, GiftError>> + Send + 'a>> {
        self.requests.lock().expect("requests lock").push((
            "GET".to_string(),
            target.to_string(),
            Vec::new(),
        ));
        let error = self.error.clone();
        let response = HttpResponse {
            status: self.status,
            headers: self.headers.clone(),
            body: self.body.clone(),
        };
        Box::pin(async move {
            if let Some(err) = error {
                Err(err)
            } else {
                Ok(response)
            }
        })
    }

    fn post<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, GiftError>> + Send + 'a>> {
        self.requests.lock().expect("requests lock").push((
            "POST".to_string(),
            target.to_string(),
            body.to_vec(),
        ));
        let error = self.error.clone();
        let response = HttpResponse {
            status: self.status,
            headers: self.headers.clone(),
            body: self.body.clone(),
        };
        Box::pin(async move {
            if let Some(err) = error {
                Err(err)
            } else {
                Ok(response)
            }
        })
    }

    fn delete<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, GiftError>> + Send + 'a>> {
        self.requests.lock().expect("requests lock").push((
            "DELETE".to_string(),
            target.to_string(),
            Vec::new(),
        ));
        let error = self.error.clone();
        let response = HttpResponse {
            status: self.status,
            headers: self.headers.clone(),
            body: self.body.clone(),
        };
        Box::pin(async move {
            if let Some(err) = error {
                Err(err)
            } else {
                Ok(response)
            }
        })
    }
}

// ---------------------------------------------------------------------------
// 1. Константы RE-оракула (модули 202541, 45938, 7133, 557009, 652215)
// ---------------------------------------------------------------------------

#[test]
fn re_constants_match_discord_bundle_oracles() {
    assert_eq!(NITRO_APPLICATION_ID, "521842831262875670");
    assert_eq!(SKU_NONE, "628379670982688768");
    assert_eq!(SKU_NITRO_BASIC_TIER_0, "978380684370378762");
    assert_eq!(SKU_NITRO_CLASSIC_TIER_1, "521846918637420545");
    assert_eq!(SKU_NITRO_STANDARD_TIER_2, "521847234246082599");
    assert_eq!(SKU_GUILD_BOOST, "590663762298667008");
    assert_eq!(SKU_NITRO_LEGACY, "521842865731534868");

    assert_eq!(PLAN_NITRO_BASIC_MONTH, "978380692553465866");
    assert_eq!(PLAN_NITRO_BASIC_YEAR, "1024422698568122368");
    assert_eq!(PLAN_NITRO_CLASSIC_MONTH, "511651871736201216");
    assert_eq!(PLAN_NITRO_CLASSIC_YEAR, "511651876987469824");
    assert_eq!(PLAN_NITRO_MONTH, "511651880837840896");
    assert_eq!(PLAN_NITRO_YEAR, "511651885459963904");
    assert_eq!(PLAN_NITRO_3_MONTH, "642251038925127690");
    assert_eq!(PLAN_NITRO_6_MONTH, "944037208325619722");
    assert_eq!(PLAN_GUILD_BOOST_MONTH, "590665532894740483");
    assert_eq!(PLAN_GUILD_BOOST_YEAR, "590665538238152709");

    assert_eq!(GIFT_STYLE_DEFAULT, 0);
    assert_eq!(GIFT_STYLE_CUSTOM_STYLE, 1);
    assert_eq!(GIFT_STYLE_CUSTOM_MESSAGE_EMOJI_SOUNDBOARD, 2);
    assert_eq!(MAX_GIFT_CUSTOM_MESSAGE_CHARS, 200);
    assert_eq!(MIN_GIFT_CODE_CHARS, 2);
    assert_eq!(MAX_GIFT_CODE_CHARS, 64);
    assert_eq!(MAX_EXTRACTED_GIFT_CODES, 3);

    assert_eq!(GIFT_FLAG_PAYMENT_SOURCE_REQUIRED, 1);
    assert_eq!(GIFT_FLAG_EXISTING_PREMIUM_SUBSCRIPTION_DISALLOWED, 2);
    assert_eq!(GIFT_FLAG_NOT_SELF_REDEEMABLE, 4);

    assert_eq!(ENTITLEMENT_TYPE_PURCHASE, 1);
    assert_eq!(ENTITLEMENT_TYPE_PREMIUM_SUBSCRIPTION, 2);
    assert_eq!(ENTITLEMENT_TYPE_DEVELOPER_GIFT, 3);
    assert_eq!(ENTITLEMENT_TYPE_TEST_MODE_PURCHASE, 4);
    assert_eq!(ENTITLEMENT_TYPE_FREE_PURCHASE, 5);
    assert_eq!(ENTITLEMENT_TYPE_USER_GIFT, 6);
    assert_eq!(ENTITLEMENT_TYPE_PREMIUM_PURCHASE, 7);
    assert_eq!(ENTITLEMENT_TYPE_APPLICATION_SUBSCRIPTION, 8);
    assert_eq!(ENTITLEMENT_TYPE_FREE_STAFF_PURCHASE, 9);
    assert_eq!(ENTITLEMENT_TYPE_QUEST_REWARD, 10);
    assert_eq!(ENTITLEMENT_TYPE_FRACTIONAL_REDEMPTION, 11);
    assert_eq!(ENTITLEMENT_TYPE_VIRTUAL_CURRENCY_REDEMPTION, 12);
    assert_eq!(ENTITLEMENT_TYPE_PURCHASE_REWARD, 16);

    assert_eq!(DISCORD_ERR_UNKNOWN_GIFT_CODE, 10038);
    assert_eq!(DISCORD_ERR_INVALID_FORM_BODY, 50035);
    assert_eq!(DISCORD_ERR_INVALID_GIFT_REDEMPTION_EXHAUSTED, 50050);
    assert_eq!(DISCORD_ERR_INVALID_GIFT_REDEMPTION_OWNED, 50051);
    assert_eq!(DISCORD_ERR_INVALID_GIFT_SELF_REDEMPTION, 50054);
    assert_eq!(DISCORD_ERR_INVALID_GIFT_REDEMPTION_PREVIOUSLY_OWNED, 50092);
    assert_eq!(DISCORD_ERR_INVALID_COUNTRY_CODE, 50095);
    assert_eq!(DISCORD_ERR_INVALID_GIFT_REDEMPTION_FRAUD_REJECTED, 50097);
}

#[test]
fn nitro_gift_plans_catalog_matches_module_202541() {
    let plans = list_nitro_gift_plans();
    assert_eq!(plans.len(), 10);

    let nitro_month = plans
        .iter()
        .find(|p| p.id == PLAN_NITRO_MONTH)
        .expect("Nitro Monthly plan");
    assert_eq!(nitro_month.name, "Nitro Monthly");
    assert_eq!(nitro_month.sku_id, SKU_NITRO_STANDARD_TIER_2);
    assert_eq!(nitro_month.interval, 1);
    assert_eq!(nitro_month.interval_count, 1);
    assert_eq!(nitro_month.premium_type, Some(2));

    let basic_year = plans
        .iter()
        .find(|p| p.id == PLAN_NITRO_BASIC_YEAR)
        .expect("Nitro Basic Yearly plan");
    assert_eq!(basic_year.name, "Nitro Basic Yearly");
    assert_eq!(basic_year.sku_id, SKU_NITRO_BASIC_TIER_0);
    assert_eq!(basic_year.interval, 2);
    assert_eq!(basic_year.interval_count, 1);
    assert_eq!(basic_year.premium_type, Some(3));

    let boost_month = plans
        .iter()
        .find(|p| p.id == PLAN_GUILD_BOOST_MONTH)
        .expect("Server Boost Monthly plan");
    assert_eq!(boost_month.sku_id, SKU_GUILD_BOOST);
    assert_eq!(boost_month.premium_type, None);
}

// ---------------------------------------------------------------------------
// 2. Расчёт gift_style и извлечение кодов подарков из текста (модуль 45938)
// ---------------------------------------------------------------------------

#[test]
fn resolve_gift_style_follows_attachment_spec_and_module_45938() {
    // Пустое/пробельное сообщение без эмодзи/звука -> CUSTOM_STYLE (1)
    assert_eq!(resolve_gift_style(None, false), Ok(GIFT_STYLE_CUSTOM_STYLE));
    assert_eq!(
        resolve_gift_style(Some(""), false),
        Ok(GIFT_STYLE_CUSTOM_STYLE)
    );
    assert_eq!(
        resolve_gift_style(Some("   \n\t "), false),
        Ok(GIFT_STYLE_CUSTOM_STYLE)
    );

    // Непустое сообщение или наличие эмодзи/звука -> CUSTOM_MESSAGE_EMOJI_SOUNDBOARD (2)
    assert_eq!(
        resolve_gift_style(Some("С днём рождения!"), false),
        Ok(GIFT_STYLE_CUSTOM_MESSAGE_EMOJI_SOUNDBOARD)
    );
    assert_eq!(
        resolve_gift_style(None, true),
        Ok(GIFT_STYLE_CUSTOM_MESSAGE_EMOJI_SOUNDBOARD)
    );

    // Граница 200 UTF-8 символов (включительно) и отказ на 201 символе:
    let msg_200 = "я".repeat(MAX_GIFT_CUSTOM_MESSAGE_CHARS);
    assert_eq!(
        resolve_gift_style(Some(&msg_200), false),
        Ok(GIFT_STYLE_CUSTOM_MESSAGE_EMOJI_SOUNDBOARD)
    );
    let msg_201 = "я".repeat(MAX_GIFT_CUSTOM_MESSAGE_CHARS + 1);
    assert_eq!(
        resolve_gift_style(Some(&msg_201), false),
        Err(GiftError::CustomMessageTooLong {
            len: 201,
            max: MAX_GIFT_CUSTOM_MESSAGE_CHARS,
        })
    );
}

#[test]
fn extract_gift_codes_from_text_normalizes_and_caps_at_three() {
    let text = "Лови подарок https://discord.gift/Wump-Ab12-Cd34 и ещё https://discord.com/gifts/Nitro2026 \
                плюс https://promos.discord.gg/?code=PROMO-9999 и четвёртый https://discordapp.com/gifts/EXTRA444";
    let codes = extract_gift_codes_from_text(text);
    assert_eq!(
        codes,
        vec![
            "WumpAb12Cd34".to_string(),
            "Nitro2026".to_string(),
            "PROMO9999".to_string()
        ]
    );

    // Дубликаты дедуплицируются с сохранением порядка:
    let dup = "discord.gift/AbCd-1234 https://discord.com/gifts/AbCd1234 https://discord.com/billing/promotions/XyZ987";
    assert_eq!(
        extract_gift_codes_from_text(dup),
        vec!["AbCd1234".to_string(), "XyZ987".to_string()]
    );

    assert!(extract_gift_codes_from_text("обычное сообщение без подарков").is_empty());
}

// ---------------------------------------------------------------------------
// 3. POST /users/@me/entitlements/gift-codes (create_gift_code)
// ---------------------------------------------------------------------------

#[test]
fn create_gift_code_posts_canonical_payload_and_parses_gift_record() {
    let response_json = br#"{
        "code": "WUMP-2026-GIFT-CODE",
        "sku_id": "521847234246082599",
        "application_id": "521842831262875670",
        "uses": 0,
        "max_uses": 1,
        "expires_at": "2026-12-01T00:00:00.000000+00:00",
        "redeemed": false,
        "subscription_plan_id": "511651880837840896",
        "gift_style": 2,
        "flags": 4,
        "user": {"id": "1014537851418980372", "username": "testuser"}
    }"#;
    let transport = FakeGiftTransport::answering(200, response_json);
    let req = CreateGiftCodeRequest {
        sku_id: SKU_NITRO_STANDARD_TIER_2.to_string(),
        subscription_plan_id: Some(PLAN_NITRO_MONTH.to_string()),
        gift_style: None,
        custom_message: Some("Поздравляю!".to_string()),
    };

    let created = run(create_gift_code(&transport, &req)).expect("create_gift_code ok");
    assert_eq!(created.code, "WUMP-2026-GIFT-CODE");
    assert_eq!(created.sku_id, SKU_NITRO_STANDARD_TIER_2);
    assert_eq!(
        created.application_id.as_deref(),
        Some(NITRO_APPLICATION_ID)
    );
    assert_eq!(created.user_id.as_deref(), Some("1014537851418980372"));
    assert_eq!(created.uses, 0);
    assert_eq!(created.max_uses, 1);
    assert!(!created.is_claimed());
    assert_eq!(created.remaining_uses(), 1);
    assert!(!created.has_multiple_copies());
    assert!(created.is_subscription());
    assert_eq!(created.premium_subscription_type(), Some(2));
    assert!(!created.is_self_redeemable());
    assert!(!created.is_existing_premium_subscription_disallowed());
    assert!(!created.is_payment_source_required());
    assert_eq!(
        created.gift_style,
        Some(GIFT_STYLE_CUSTOM_MESSAGE_EMOJI_SOUNDBOARD)
    );

    let calls = transport.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "POST");
    assert_eq!(calls[0].1, "/users/@me/entitlements/gift-codes");
    let wire_body: serde_json::Value = serde_json::from_slice(&calls[0].2).expect("POST body JSON");
    assert_eq!(
        wire_body,
        serde_json::json!({
            "sku_id": SKU_NITRO_STANDARD_TIER_2,
            "subscription_plan_id": PLAN_NITRO_MONTH,
            "gift_style": 2
        })
    );
}

#[test]
fn create_gift_code_validates_snowflakes_gift_style_and_custom_message_before_network() {
    let transport = FakeGiftTransport::answering(200, b"{}");

    // Невалидный sku_id (0, ведущий ноль, переполнение u64, нечисловой)
    for bad_sku in ["", "0", "01234", "18446744073709551616", "sku-abc"] {
        let req = CreateGiftCodeRequest {
            sku_id: bad_sku.to_string(),
            subscription_plan_id: Some(PLAN_NITRO_MONTH.to_string()),
            gift_style: Some(1),
            custom_message: None,
        };
        assert_eq!(
            run(create_gift_code(&transport, &req)),
            Err(GiftError::InvalidSnowflake { field: "sku_id" }),
            "sku_id={bad_sku:?} должен отклоняться до сети"
        );
    }

    // Невалидный subscription_plan_id
    let bad_plan_req = CreateGiftCodeRequest {
        sku_id: SKU_NITRO_STANDARD_TIER_2.to_string(),
        subscription_plan_id: Some("0".to_string()),
        gift_style: Some(1),
        custom_message: None,
    };
    assert_eq!(
        run(create_gift_code(&transport, &bad_plan_req)),
        Err(GiftError::InvalidSnowflake {
            field: "subscription_plan_id"
        })
    );

    // Невалидный gift_style (> 2)
    let bad_style_req = CreateGiftCodeRequest {
        sku_id: SKU_NITRO_STANDARD_TIER_2.to_string(),
        subscription_plan_id: None,
        gift_style: Some(3),
        custom_message: None,
    };
    assert_eq!(
        run(create_gift_code(&transport, &bad_style_req)),
        Err(GiftError::InvalidGiftStyle { style: 3 })
    );

    // Слишком длинное сообщение (> 200 символов)
    let long_msg_req = CreateGiftCodeRequest {
        sku_id: SKU_NITRO_STANDARD_TIER_2.to_string(),
        subscription_plan_id: None,
        gift_style: None,
        custom_message: Some("a".repeat(201)),
    };
    assert_eq!(
        run(create_gift_code(&transport, &long_msg_req)),
        Err(GiftError::CustomMessageTooLong { len: 201, max: 200 })
    );

    assert!(
        transport.calls().is_empty(),
        "предсетевая валидация не должна обращаться к транспорту"
    );
}

// ---------------------------------------------------------------------------
// 4. GET /entitlements/gift-codes/{code} (resolve_gift_code)
//    POST /entitlements/gift-codes/{code}/redeem (redeem_gift_code)
//    GET /users/@me/entitlements/gift-codes (fetch_user_gift_codes)
//    DELETE /users/@me/entitlements/gift-codes/{code} (revoke_gift_code)
// ---------------------------------------------------------------------------

#[test]
fn resolve_gift_code_normalizes_url_or_code_and_builds_query() {
    let body = br#"{
        "code": "AbCd1234EfGh5678",
        "sku_id": "978380684370378762",
        "application_id": "521842831262875670",
        "uses": 1,
        "max_uses": 5,
        "redeemed": false,
        "store_listing": {
            "id": "777888999000111222",
            "sku": {
                "id": "978380684370378762",
                "type": 5,
                "application_id": "521842831262875670",
                "name": "Nitro Basic"
            }
        },
        "subscription_plan": {
            "id": "978380692553465866",
            "name": "Nitro Basic Monthly",
            "interval": 1,
            "interval_count": 1
        },
        "flags": 3
    }"#;
    let transport = FakeGiftTransport::answering(200, body);
    let opts = GiftResolveOptions {
        with_application: true,
        with_subscription_plan: true,
    };

    let resolved = run(resolve_gift_code(
        &transport,
        "https://discord.gift/AbCd-1234-EfGh-5678",
        opts,
    ))
    .expect("resolve_gift_code ok");

    assert_eq!(resolved.code, "AbCd1234EfGh5678");
    assert_eq!(resolved.sku_id, SKU_NITRO_BASIC_TIER_0);
    assert_eq!(
        resolved.store_listing_id.as_deref(),
        Some("777888999000111222")
    );
    assert_eq!(
        resolved.subscription_plan_id.as_deref(),
        Some(PLAN_NITRO_BASIC_MONTH)
    );
    assert_eq!(resolved.uses, 1);
    assert_eq!(resolved.max_uses, 5);
    assert!(!resolved.is_claimed());
    assert_eq!(resolved.remaining_uses(), 4);
    assert!(resolved.has_multiple_copies());
    assert_eq!(resolved.premium_subscription_type(), Some(3));
    assert!(resolved.is_self_redeemable());
    assert!(resolved.is_payment_source_required());
    assert!(resolved.is_existing_premium_subscription_disallowed());

    let calls = transport.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "GET");
    assert_eq!(
        calls[0].1,
        "/entitlements/gift-codes/AbCd1234EfGh5678?with_application=true&with_subscription_plan=true"
    );
}

#[test]
fn redeem_gift_code_posts_three_nullable_fields_and_parses_entitlement() {
    let entitlement_body = br#"{
        "id": "999888777666555444",
        "sku_id": "521847234246082599",
        "application_id": "521842831262875670",
        "user_id": "1014537851418980372",
        "gifter_user_id": "2024537851418980372",
        "type": 6,
        "branches": ["521842831262875670"],
        "starts_at": "2026-11-07T00:00:00+00:00",
        "ends_at": "2026-12-07T00:00:00+00:00",
        "subscription_id": "3034537851418980372",
        "consumed": false,
        "deleted": false,
        "gift_style": 2
    }"#;
    let transport = FakeGiftTransport::answering(200, entitlement_body);
    let req = GiftRedeemRequest {
        channel_id: Some("1163408591315812392".to_string()),
        payment_source_id: None,
        gateway_checkout_context: None,
    };

    let outcome =
        run(redeem_gift_code(&transport, "WUMP-2026-GIFT", &req)).expect("redeem_gift_code ok");
    assert_eq!(outcome.code, "WUMP2026GIFT");
    assert_eq!(outcome.entitlement.id, "999888777666555444");
    assert_eq!(outcome.entitlement.sku_id, SKU_NITRO_STANDARD_TIER_2);
    assert_eq!(outcome.entitlement.kind, ENTITLEMENT_TYPE_USER_GIFT);
    assert!(!outcome.entitlement.is_giftable());
    assert!(outcome.entitlement.is_active_unconsumed());

    let calls = transport.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "POST");
    assert_eq!(calls[0].1, "/entitlements/gift-codes/WUMP2026GIFT/redeem");
    let wire_body: serde_json::Value =
        serde_json::from_slice(&calls[0].2).expect("redeem body JSON");
    assert_eq!(
        wire_body,
        serde_json::json!({
            "channel_id": "1163408591315812392",
            "payment_source_id": null,
            "gateway_checkout_context": null
        })
    );
}

#[test]
fn fetch_user_gift_codes_and_revoke_gift_code_match_module_869038() {
    let list_body = br#"[
        {
            "code": "CODE1111",
            "sku_id": "521847234246082599",
            "uses": 0,
            "max_uses": 1,
            "redeemed": false,
            "subscription_plan_id": "511651880837840896"
        },
        {
            "code": "CODE2222",
            "sku_id": "521847234246082599",
            "uses": 1,
            "max_uses": 1,
            "redeemed": true,
            "subscription_plan_id": "511651880837840896"
        }
    ]"#;
    let transport = FakeGiftTransport::answering(200, list_body);
    let codes = run(fetch_user_gift_codes(
        &transport,
        Some(SKU_NITRO_STANDARD_TIER_2),
        Some(PLAN_NITRO_MONTH),
    ))
    .expect("fetch_user_gift_codes ok");
    assert_eq!(codes.len(), 2);
    assert!(!codes[0].is_claimed());
    assert!(codes[1].is_claimed());

    let calls = transport.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "GET");
    assert_eq!(
        calls[0].1,
        format!(
            "/users/@me/entitlements/gift-codes?sku_id={SKU_NITRO_STANDARD_TIER_2}&subscription_plan_id={PLAN_NITRO_MONTH}"
        )
    );

    // DELETE /users/@me/entitlements/gift-codes/{code} (204 No Content)
    let del_transport = FakeGiftTransport::answering(204, b"");
    run(revoke_gift_code(&del_transport, "CODE-1111")).expect("revoke_gift_code 204 ok");
    let del_calls = del_transport.calls();
    assert_eq!(del_calls.len(), 1);
    assert_eq!(del_calls[0].0, "DELETE");
    assert_eq!(
        del_calls[0].1,
        "/users/@me/entitlements/gift-codes/CODE1111"
    );
}

// ---------------------------------------------------------------------------
// 5. GET /users/@me/entitlements, /users/@me/applications/{id}/entitlements,
//    /users/@me/entitlements/gifts (модуль 339048 и live D-02 оракул)
// ---------------------------------------------------------------------------

#[test]
fn fetch_entitlements_routes_match_d02_live_captures_and_module_339048() {
    let entitlements_json = br#"[
        {
            "id": "1333912750274904001",
            "sku_id": "1271583180566036480",
            "application_id": "521842831262875670",
            "user_id": "1014537851418980372",
            "type": 11,
            "consumed": false,
            "deleted": false,
            "source_type": 1
        },
        {
            "id": "1333912750274904002",
            "sku_id": "521847234246082599",
            "application_id": "521842831262875670",
            "user_id": "1014537851418980372",
            "gifter_user_id": null,
            "type": 6,
            "consumed": false,
            "deleted": false
        }
    ]"#;

    // 1) GET /users/@me/entitlements?with_sku=false&with_application=false&entitlement_type=11&exclude_ended=true
    // (точный URL из capture-ready.snap.jsonl:4004)
    let user_transport = FakeGiftTransport::answering(200, entitlements_json);
    let query = EntitlementQuery {
        with_sku: false,
        with_application: false,
        entitlement_type: Some(ENTITLEMENT_TYPE_FRACTIONAL_REDEMPTION),
        exclude_ended: true,
    };
    let user_ents =
        run(fetch_user_entitlements(&user_transport, &query)).expect("fetch_user_entitlements ok");
    assert_eq!(user_ents.len(), 2);
    assert!(user_ents[0].is_fractional_redemption());
    assert!(!user_ents[0].is_giftable());
    assert!(user_ents[0].is_active_unconsumed());
    assert!(user_ents[1].is_giftable());
    assert!(!user_ents[1].is_active_unconsumed());
    assert_eq!(
        user_transport.calls()[0].1,
        "/users/@me/entitlements?with_sku=false&with_application=false&entitlement_type=11&exclude_ended=true"
    );

    // 2) GET /users/@me/applications/521842831262875670/entitlements?exclude_consumed=true
    // (точный URL из capture-run2.snap.jsonl:8350)
    let app_transport = FakeGiftTransport::answering(200, entitlements_json);
    let app_ents = run(fetch_application_entitlements(
        &app_transport,
        NITRO_APPLICATION_ID,
        true,
    ))
    .expect("fetch_application_entitlements ok");
    assert_eq!(app_ents.len(), 2);
    assert_eq!(
        app_transport.calls()[0].1,
        "/users/@me/applications/521842831262875670/entitlements?exclude_consumed=true"
    );

    // 3) GET /users/@me/entitlements/gifts?country_code=LV
    // (точный URL из rest-static.json:6940)
    let giftable_transport = FakeGiftTransport::answering(200, entitlements_json);
    let giftable = run(fetch_giftable_entitlements(&giftable_transport, Some("lv")))
        .expect("fetch_giftable_entitlements ok");
    assert_eq!(giftable.len(), 2);
    assert_eq!(
        giftable_transport.calls()[0].1,
        "/users/@me/entitlements/gifts?country_code=LV"
    );
}

// ---------------------------------------------------------------------------
// 6. Классификация HTTP-ошибок и кодов Discord (10038, 50035, 50050, 50051,
//    50054, 50092, 50097, 401, 403, 404, 429 retry_after, 5xx)
// ---------------------------------------------------------------------------

#[test]
fn gift_error_classification_covers_all_discord_codes_and_http_statuses() {
    let opts = GiftResolveOptions::default();

    // 400 + 10038 UNKNOWN_GIFT_CODE -> NotFound
    let t_10038 =
        FakeGiftTransport::answering(400, br#"{"code": 10038, "message": "Unknown Gift Code"}"#);
    assert_eq!(
        run(resolve_gift_code(&t_10038, "ABCD1234", opts)),
        Err(GiftError::NotFound)
    );

    // 400 + 50035 INVALID_FORM_BODY -> InvalidFormBody
    let t_50035 =
        FakeGiftTransport::answering(400, br#"{"code": 50035, "message": "Invalid Form Body"}"#);
    assert_eq!(
        run(resolve_gift_code(&t_50035, "ABCD1234", opts)),
        Err(GiftError::InvalidFormBody)
    );

    // 400 + 50050 INVALID_GIFT_REDEMPTION_EXHAUSTED -> GiftRedemptionExhausted
    let t_50050 = FakeGiftTransport::answering(
        400,
        br#"{"code": 50050, "message": "This gift has been redeemed already."}"#,
    );
    assert_eq!(
        run(redeem_gift_code(
            &t_50050,
            "ABCD1234",
            &GiftRedeemRequest::default()
        )),
        Err(GiftError::GiftRedemptionExhausted)
    );

    // 400 + 50051 / 50092 -> GiftRedemptionOwned
    for code in [50051u32, 50092u32] {
        let body = format!(r#"{{"code": {code}, "message": "Already owned"}}"#);
        let t = FakeGiftTransport::answering(400, body.as_bytes());
        assert_eq!(
            run(redeem_gift_code(
                &t,
                "ABCD1234",
                &GiftRedeemRequest::default()
            )),
            Err(GiftError::GiftRedemptionOwned)
        );
    }

    // 400 + 50054 INVALID_GIFT_SELF_REDEMPTION -> GiftSelfRedemption
    let t_50054 = FakeGiftTransport::answering(
        400,
        br#"{"code": 50054, "message": "Cannot redeem your own gift"}"#,
    );
    assert_eq!(
        run(redeem_gift_code(
            &t_50054,
            "ABCD1234",
            &GiftRedeemRequest::default()
        )),
        Err(GiftError::GiftSelfRedemption)
    );

    // 400 + 50097 INVALID_GIFT_REDEMPTION_FRAUD_REJECTED -> GiftRedemptionFraudRejected
    let t_50097 =
        FakeGiftTransport::answering(400, br#"{"code": 50097, "message": "Fraud rejected"}"#);
    assert_eq!(
        run(redeem_gift_code(
            &t_50097,
            "ABCD1234",
            &GiftRedeemRequest::default()
        )),
        Err(GiftError::GiftRedemptionFraudRejected)
    );

    // 401, 403, 404, 429 (JSON retry_after и заголовок Retry-After), 502
    assert_eq!(
        run(resolve_gift_code(
            &FakeGiftTransport::answering(401, b"{}"),
            "ABCD1234",
            opts
        )),
        Err(GiftError::Unauthorized)
    );
    assert_eq!(
        run(resolve_gift_code(
            &FakeGiftTransport::answering(403, b"{}"),
            "ABCD1234",
            opts
        )),
        Err(GiftError::Forbidden)
    );
    assert_eq!(
        run(resolve_gift_code(
            &FakeGiftTransport::answering(404, b"{}"),
            "ABCD1234",
            opts
        )),
        Err(GiftError::NotFound)
    );

    let t_429_json = FakeGiftTransport::answering(429, br#"{"retry_after": 2.5}"#);
    assert_eq!(
        run(resolve_gift_code(&t_429_json, "ABCD1234", opts)),
        Err(GiftError::RateLimited {
            retry_after_ms: 2500
        })
    );

    let t_429_hdr = FakeGiftTransport::answering(429, b"").with_headers(&[("Retry-After", "4")]);
    assert_eq!(
        run(resolve_gift_code(&t_429_hdr, "ABCD1234", opts)),
        Err(GiftError::RateLimited {
            retry_after_ms: 4000
        })
    );

    assert_eq!(
        run(resolve_gift_code(
            &FakeGiftTransport::answering(502, b"Bad Gateway"),
            "ABCD1234",
            opts
        )),
        Err(GiftError::Server { status: 502 })
    );

    // 200 OK с битым JSON или пустым code -> InvalidResponse
    assert_eq!(
        run(resolve_gift_code(
            &FakeGiftTransport::answering(200, b"not-json"),
            "ABCD1234",
            opts
        )),
        Err(GiftError::InvalidResponse)
    );
    assert_eq!(
        run(resolve_gift_code(
            &FakeGiftTransport::answering(200, br#"{"code": "", "sku_id": "521847234246082599"}"#),
            "ABCD1234",
            opts
        )),
        Err(GiftError::InvalidResponse)
    );

    // Отказ транспорта -> Network
    assert_eq!(
        run(resolve_gift_code(
            &FakeGiftTransport::failing(GiftError::Network),
            "ABCD1234",
            opts
        )),
        Err(GiftError::Network)
    );
}

// ---------------------------------------------------------------------------
// 7. Граничные условия валидации кодов, Snowflake и безопасности (маскирование токена)
// ---------------------------------------------------------------------------

#[test]
fn boundary_validation_and_token_redaction() {
    let transport = FakeGiftTransport::answering(204, b"");

    // Границы длины кода подарка: 2 и 64 символа — валидны, 1 и 65 — InvalidArg
    let min_code = "Ab";
    let max_code = "A".repeat(MAX_GIFT_CODE_CHARS);
    let too_short = "A";
    let too_long = "A".repeat(MAX_GIFT_CODE_CHARS + 1);

    assert_eq!(run(revoke_gift_code(&transport, min_code)), Ok(()));
    assert_eq!(run(revoke_gift_code(&transport, &max_code)), Ok(()));
    assert_eq!(
        run(revoke_gift_code(&transport, too_short)),
        Err(GiftError::InvalidArg)
    );
    assert_eq!(
        run(revoke_gift_code(&transport, &too_long)),
        Err(GiftError::InvalidArg)
    );

    // Границы Snowflake: "1" и u64::MAX — валидны
    let app_transport = FakeGiftTransport::answering(200, b"[]");
    let u64_max = u64::MAX.to_string();
    assert!(run(fetch_application_entitlements(&app_transport, "1", false)).is_ok());
    assert!(run(fetch_application_entitlements(
        &app_transport,
        &u64_max,
        false
    ))
    .is_ok());
    assert_eq!(
        run(fetch_application_entitlements(&app_transport, "0", false)),
        Err(GiftError::InvalidSnowflake {
            field: "application_id"
        })
    );

    // Невалидный country_code в fetch_giftable_entitlements
    for bad_cc in ["", "L", "LVA", "12", "L$"] {
        assert_eq!(
            run(fetch_giftable_entitlements(&app_transport, Some(bad_cc))),
            Err(GiftError::InvalidArg),
            "country_code={bad_cc:?} должен отклоняться"
        );
    }

    // Токен никогда не попадает в Debug DiscordGifts
    let secret = "mfa.super-secret-gift-bearer-token-999";
    let client = DiscordGifts::new(secret).expect("DiscordGifts::new");
    let dbg = format!("{client:?}");
    assert!(
        !dbg.contains(secret),
        "Debug DiscordGifts не должен раскрывать токен: {dbg}"
    );
}

// ---------------------------------------------------------------------------
// 8. Контракт регистрации и проводки Tauri IPC-команд в `rust/gui-spike/src/main.rs`
// ---------------------------------------------------------------------------

fn main_head_source() -> &'static str {
    include_str!("../src/main.rs")
        .split("mod wiring_tests")
        .next()
        .expect("голова main.rs")
}

fn registered_commands(source: &str) -> Vec<&str> {
    let open = "generate_handler![";
    let start = source
        .find(open)
        .expect("main.rs объявляет invoke_handler!")
        + open.len();
    let block = &source[start..start + source[start..].find(']').expect("закрывающая ]")];
    block
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .filter(|name| !name.is_empty())
        .collect()
}

fn command_body<'a>(source: &'a str, name: &str) -> &'a str {
    let async_marker = format!("async fn {name}(");
    let sync_marker = format!("fn {name}(");
    let start = source
        .find(&async_marker)
        .or_else(|| source.find(&sync_marker))
        .unwrap_or_else(|| panic!("main.rs должен объявлять команду {name}"));
    let rest = &source[start..];
    let end = rest[1..]
        .find("#[tauri::command]")
        .or_else(|| rest[1..].find("const SYSTEM_TRAY_PY"))
        .or_else(|| rest[1..].find("fn main()"))
        .map(|idx| idx + 1)
        .unwrap_or(rest.len());
    &rest[..end]
}

#[test]
fn tauri_main_registers_and_wires_all_gift_and_entitlement_ipc_commands() {
    assert_eq!(
        GIFT_IPC_COMMANDS,
        [
            "resolve_gift_code",
            "create_gift_code",
            "fetch_user_gift_codes",
            "revoke_gift_code",
            "redeem_gift_code",
            "fetch_user_entitlements",
            "fetch_application_entitlements",
            "fetch_giftable_entitlements",
            "list_nitro_gift_plans",
        ]
    );

    let head = main_head_source();
    let registered = registered_commands(head);

    for cmd in GIFT_IPC_COMMANDS {
        assert_eq!(
            registered.iter().filter(|name| **name == cmd).count(),
            1,
            "{cmd} должен быть зарегистрирован в generate_handler! ровно один раз"
        );
        let body = command_body(head, cmd);
        assert!(
            !body.contains("token: String") && !body.contains("bearer: String"),
            "{cmd}: токен никогда не передаётся из фронтенда: {body}"
        );
        assert!(
            !body.contains("println!") && !body.contains("eprintln!") && !body.contains("log::"),
            "{cmd}: токен и ошибки не логируются: {body}"
        );

        if cmd != "list_nitro_gift_plans" {
            assert!(
                body.contains("token_or::<GiftError, _>("),
                "{cmd}: обязан читать токен через token_or::<GiftError, _>: {body}"
            );
            assert_eq!(
                body.matches("backend.token()").count(),
                1,
                "{cmd}: ровно одно чтение backend.token(): {body}"
            );
            assert!(
                body.contains("GiftError::Storage") && body.contains("GiftError::MissingToken"),
                "{cmd}: обязан сохранять GiftError::Storage и GiftError::MissingToken: {body}"
            );
            assert!(
                body.contains("DiscordGifts::new(&token)?"),
                "{cmd}: обязан создавать DiscordGifts::new(&token)?: {body}"
            );
        }
    }
}
