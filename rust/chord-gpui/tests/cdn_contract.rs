#![forbid(unsafe_code)]

#[path = "../src/net/cdn.rs"]
mod cdn;

use std::sync::Arc;

use cdn::{
    CDN_BASE_URL, CachedAsset, CdnClient, CdnError, CdnMemoryCache, clamp_cdn_size,
    custom_emoji_url, default_avatar_index, guild_banner_url, guild_icon_url, parse_snowflake,
    user_avatar_png_url, user_avatar_url, validate_asset_hash,
};

#[test]
fn t01_clamp_cdn_size_power_of_two_bounds() {
    assert_eq!(CDN_BASE_URL, "https://cdn.discordapp.com");

    // Lower bound clamping (<= 16 -> 16)
    assert_eq!(clamp_cdn_size(0), 16);
    assert_eq!(clamp_cdn_size(1), 16);
    assert_eq!(clamp_cdn_size(15), 16);
    assert_eq!(clamp_cdn_size(16), 16);

    // Exact powers of two in 16..=4096 remain unchanged
    for &exact in &[16, 32, 64, 128, 256, 512, 1024, 2048, 4096] {
        assert_eq!(clamp_cdn_size(exact), exact);
    }

    // Intermediate values round up to next power of two
    assert_eq!(clamp_cdn_size(17), 32);
    assert_eq!(clamp_cdn_size(48), 64);
    assert_eq!(clamp_cdn_size(100), 128);
    assert_eq!(clamp_cdn_size(300), 512);
    assert_eq!(clamp_cdn_size(2049), 4096);

    // Upper bound clamping (>= 4096 -> 4096)
    assert_eq!(clamp_cdn_size(4096), 4096);
    assert_eq!(clamp_cdn_size(4097), 4096);
    assert_eq!(clamp_cdn_size(9999), 4096);
    assert_eq!(clamp_cdn_size(u32::MAX), 4096);
}

#[test]
fn t02_validate_asset_hash_and_snowflake_security() {
    // Valid hashes
    assert_eq!(
        validate_asset_hash("8342729096ea3675442027381ff50dfe"),
        Ok("8342729096ea3675442027381ff50dfe")
    );
    assert_eq!(
        validate_asset_hash("a_1269e74af4df7417b13759eae50c83dc"),
        Ok("a_1269e74af4df7417b13759eae50c83dc")
    );
    let max_len_hash = "a".repeat(128);
    assert_eq!(validate_asset_hash(&max_len_hash), Ok(max_len_hash.as_str()));

    // Invalid hashes: empty, path traversal, URL injection, whitespace, > 128 chars
    for bad in [
        "",
        "../admin",
        "..\\admin",
        "foo..bar",
        "hash/evil",
        "hash\\evil",
        "hash?token=x",
        "hash#frag",
        "hash with space",
        "hash\twith_tab",
        "hash\nwith_newline",
        "hash\rwith_cr",
        "hash\0with_nul",
    ] {
        assert_eq!(
            validate_asset_hash(bad),
            Err(CdnError::InvalidHash),
            "expected InvalidHash for {bad:?}"
        );
    }

    let over_max_hash = "a".repeat(129);
    assert_eq!(
        validate_asset_hash(&over_max_hash),
        Err(CdnError::InvalidHash)
    );

    // Snowflake validation (`0..=u64::MAX` ASCII digits)
    assert_eq!(parse_snowflake("0"), Ok(0));
    assert_eq!(
        parse_snowflake("80351110224678912"),
        Ok(80_351_110_224_678_912)
    );
    assert_eq!(parse_snowflake("18446744073709551615"), Ok(u64::MAX));

    for bad_sf in [
        "",
        " ",
        "-1",
        "+123",
        "123abc",
        "18446744073709551616", // u64::MAX + 1
        "80351110224678912/../../avatars",
    ] {
        assert_eq!(
            parse_snowflake(bad_sf),
            Err(CdnError::InvalidSnowflake),
            "expected InvalidSnowflake for {bad_sf:?}"
        );
    }
}

#[test]
fn t03_default_avatar_pomelo_and_legacy_discriminator_fallbacks() {
    // Pomelo accounts (`discriminator` is None or Some(0)): `((user_id >> 22) % 6) as u8`
    assert_eq!(default_avatar_index(0, None), 0);
    assert_eq!(default_avatar_index(0, Some(0)), 0);

    for idx in 0u64..6 {
        let uid = idx << 22;
        assert_eq!(default_avatar_index(uid, None), idx as u8);
        assert_eq!(default_avatar_index(uid, Some(0)), idx as u8);
        let url = user_avatar_url(uid, None, None, 128, true).expect("default avatar url");
        assert_eq!(
            url,
            format!("https://cdn.discordapp.com/embed/avatars/{idx}.png")
        );
    }

    // Real Discord Snowflake & u64::MAX edge boundary
    let real_uid: u64 = 80_351_110_224_678_912;
    let expected_real_idx = ((real_uid >> 22) % 6) as u8;
    assert_eq!(
        user_avatar_url(real_uid, None, Some(0), 64, false).expect("url"),
        format!("https://cdn.discordapp.com/embed/avatars/{expected_real_idx}.png")
    );

    let max_idx = ((u64::MAX >> 22) % 6) as u8;
    assert_eq!(
        user_avatar_url(u64::MAX, None, None, 64, true).expect("url"),
        format!("https://cdn.discordapp.com/embed/avatars/{max_idx}.png")
    );

    // Legacy 4-digit discriminator (`Some(d)` with `d > 0`): `(d % 5) as u8`
    assert_eq!(default_avatar_index(real_uid, Some(1)), 1);
    assert_eq!(default_avatar_index(real_uid, Some(5)), 0);
    assert_eq!(default_avatar_index(real_uid, Some(1337)), (1337 % 5) as u8);
    assert_eq!(
        user_avatar_url(real_uid, None, Some(1337), 256, true).expect("legacy default url"),
        "https://cdn.discordapp.com/embed/avatars/2.png"
    );
}

#[test]
fn t04_user_avatar_guild_icon_banner_and_emoji_urls() {
    let uid = 80_351_110_224_678_912_u64;

    // Static hash -> always .webp regardless of prefer_animated
    let static_hash = "8342729096ea3675442027381ff50dfe";
    assert_eq!(
        user_avatar_url(uid, Some(static_hash), None, 48, true).expect("static avatar"),
        "https://cdn.discordapp.com/avatars/80351110224678912/8342729096ea3675442027381ff50dfe.webp?size=64"
    );
    assert_eq!(
        user_avatar_png_url(uid, static_hash, 48).expect("png helper"),
        "https://cdn.discordapp.com/avatars/80351110224678912/8342729096ea3675442027381ff50dfe.png?size=64"
    );

    // Animated hash `a_...` -> .gif when prefer_animated=true, .webp when prefer_animated=false
    let anim_hash = "a_1269e74af4df7417b13759eae50c83dc";
    assert_eq!(
        user_avatar_url(uid, Some(anim_hash), None, 128, true).expect("animated avatar gif"),
        "https://cdn.discordapp.com/avatars/80351110224678912/a_1269e74af4df7417b13759eae50c83dc.gif?size=128"
    );
    assert_eq!(
        user_avatar_url(uid, Some(anim_hash), None, 128, false).expect("reduced motion webp"),
        "https://cdn.discordapp.com/avatars/80351110224678912/a_1269e74af4df7417b13759eae50c83dc.webp?size=128"
    );

    // Guild icon & banner
    let gid = 197_038_439_483_310_086_u64;
    assert_eq!(
        guild_icon_url(gid, anim_hash, 100, true).expect("guild icon gif"),
        "https://cdn.discordapp.com/icons/197038439483310086/a_1269e74af4df7417b13759eae50c83dc.gif?size=128"
    );
    assert_eq!(
        guild_icon_url(gid, anim_hash, 100, false).expect("guild icon webp"),
        "https://cdn.discordapp.com/icons/197038439483310086/a_1269e74af4df7417b13759eae50c83dc.webp?size=128"
    );
    assert_eq!(
        guild_banner_url(gid, static_hash, 600, true).expect("guild banner"),
        "https://cdn.discordapp.com/banners/197038439483310086/8342729096ea3675442027381ff50dfe.webp?size=1024"
    );

    // Custom emoji with lossless quality
    let eid = 567_890_123_456_789_012_u64;
    assert_eq!(
        custom_emoji_url(eid, true, 40),
        "https://cdn.discordapp.com/emojis/567890123456789012.gif?size=64&quality=lossless"
    );
    assert_eq!(
        custom_emoji_url(eid, false, 0),
        "https://cdn.discordapp.com/emojis/567890123456789012.webp?size=16&quality=lossless"
    );

    // Custom base_url via CdnClient
    let client = CdnClient::with_base_url("https://media.example.test/");
    assert_eq!(client.base_url(), "https://media.example.test");
    assert_eq!(
        client
            .user_avatar_url(uid, Some(static_hash), None, 32, false)
            .expect("custom base url"),
        "https://media.example.test/avatars/80351110224678912/8342729096ea3675442027381ff50dfe.webp?size=32"
    );
    assert_eq!(
        client.custom_emoji_url(eid, false, 32),
        "https://media.example.test/emojis/567890123456789012.webp?size=32&quality=lossless"
    );

    // Path traversal / injection rejected across all hash-taking builders
    assert_eq!(
        user_avatar_url(uid, Some("../admin"), None, 64, false),
        Err(CdnError::InvalidHash)
    );
    assert_eq!(
        guild_icon_url(gid, "hash?token=x", 64, false),
        Err(CdnError::InvalidHash)
    );
    assert_eq!(
        guild_banner_url(gid, "", 64, false),
        Err(CdnError::InvalidHash)
    );
}

#[test]
fn t05_cdn_memory_cache_lru_recency_byte_budget_and_telemetry() {
    let mut cache = CdnMemoryCache::new(3, 100);
    assert_eq!(cache.max_entries(), 3);
    assert_eq!(cache.max_bytes(), 100);
    assert_eq!(cache.len(), 0);
    assert!(cache.is_empty());
    assert_eq!(cache.current_bytes(), 0);

    let make_asset = |url: &str, size: usize| CachedAsset {
        url: url.to_string(),
        mime: "image/webp".to_string(),
        width: 64,
        height: 64,
        bytes: Arc::from(vec![0xAB_u8; size]),
    };

    // Miss increments misses counter
    assert!(cache.get("https://cdn.discordapp.com/avatars/1/a.webp?size=64").is_none());
    assert_eq!(cache.misses(), 1);
    assert_eq!(cache.hits(), 0);

    // Insert 3 items of 30 bytes each (total 90 bytes <= 100, 3 entries <= 3)
    cache.insert(make_asset("url1", 30)).expect("insert 1");
    cache.insert(make_asset("url2", 30)).expect("insert 2");
    cache.insert(make_asset("url3", 30)).expect("insert 3");
    assert_eq!(cache.len(), 3);
    assert_eq!(cache.current_bytes(), 90);
    assert_eq!(cache.evictions(), 0);

    // Promote "url1" via `get` so LRU order becomes: url2 (oldest), url3, url1 (newest)
    let hit1 = cache.get("url1").expect("url1 present");
    assert_eq!(hit1.bytes.len(), 30);
    assert_eq!(cache.hits(), 1);

    // Insert "url4" (10 bytes): total bytes would be 100, but max_entries is 3 -> evicts "url2"
    cache.insert(make_asset("url4", 10)).expect("insert 4");
    assert_eq!(cache.len(), 3);
    assert_eq!(cache.current_bytes(), 70);
    assert_eq!(cache.evictions(), 1);
    assert!(cache.get("url2").is_none(), "url2 should have been evicted");
    assert_eq!(cache.misses(), 2);

    // Insert "url5" (70 bytes): requires evicting "url3" (30B) and "url1" (30B) to fit 70B + 10B (url4) <= 100B
    cache.insert(make_asset("url5", 70)).expect("insert 5");
    assert_eq!(cache.len(), 2);
    assert_eq!(cache.current_bytes(), 80);
    assert_eq!(cache.evictions(), 3);
    assert!(cache.get("url3").is_none());
    assert!(cache.get("url1").is_none());
    assert!(cache.get("url4").is_some());
    assert!(cache.get("url5").is_some());

    // Oversized asset (> max_bytes) returns Err(CdnError::AssetTooLarge) WITHOUT evicting existing cache!
    let evictions_before = cache.evictions();
    let bytes_before = cache.current_bytes();
    let len_before = cache.len();
    let res = cache.insert(make_asset("url_huge", 101));
    assert_eq!(res, Err(CdnError::AssetTooLarge));
    assert_eq!(cache.len(), len_before);
    assert_eq!(cache.current_bytes(), bytes_before);
    assert_eq!(cache.evictions(), evictions_before);
    assert!(cache.get("url4").is_some());
    assert!(cache.get("url5").is_some());

    // Updating an existing key replaces its bytes and promotes recency without counting as eviction
    cache.insert(make_asset("url4", 25)).expect("update url4");
    assert_eq!(cache.len(), 2);
    assert_eq!(cache.current_bytes(), 95);

    cache.clear();
    assert!(cache.is_empty());
    assert_eq!(cache.current_bytes(), 0);
}
