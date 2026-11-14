//! P1-3.2 Contract Tests: Deterministic QR Code Matrix Rendering in `LoginModal` UI.
//!
//! Tests:
//! 1. `build_remote_auth_qr_url`:
//!    - Builds URL-safe base64 / alphanumeric Discord Remote Auth URL `https://discord.com/ra/<fingerprint>`
//!    - Rejects empty, whitespace, newlines, and path traversal attempts (`..`, `/`, `\`)
//! 2. `QrCodeMatrix::from_remote_auth_fingerprint`:
//!    - Deterministic 25x25 boolean grid (`size == 25`, `modules.len() == 25`, rows `len() == 25`)
//!    - Three 7x7 standard finder patterns (outer black ring, 5x5 white ring, 3x3 black center)
//!    - Quiet separator white borders around finders
//!    - Alternating dark/light timing patterns on row 6 and col 6 between separators (cols/rows 8..=16)
//!    - Invariance of function patterns + distinctness of data modules across different fingerprints
//! 3. `QrLoginUiState` state machine transitions:
//!    - `AwaitingFingerprint` -> `Ready { fingerprint, url, matrix }` -> `Scanned { username, user_id }` -> `Expired` -> `Error(msg)`
//! 4. UI resolution and rendering helper contracts for `LoginModal`

#![recursion_limit = "512"]

#[path = "../src/audio/mod.rs"]
mod audio;
#[path = "../src/dispatch/mod.rs"]
mod dispatch;
#[path = "../src/media/mod.rs"]
mod media;
#[path = "../src/net/mod.rs"]
mod net;
#[path = "../src/platform/mod.rs"]
mod platform;
#[path = "../src/state/mod.rs"]
mod state;
#[path = "../src/storage/mod.rs"]
mod storage;
#[path = "../src/theme/mod.rs"]
mod theme;
#[path = "../src/ui/mod.rs"]
mod ui;
#[path = "../src/video/mod.rs"]
mod video;

use state::AppState;
use ui::login_modal::{
    LoginModal, QrCodeMatrix, QrLoginUiState, QrMatrixError, REMOTE_AUTH_QR_URL_PREFIX,
    build_remote_auth_qr_url,
};

// ═══════════════════════════════════════════════════════════════════════════
// 1. build_remote_auth_qr_url contract tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_build_remote_auth_qr_url_valid_fingerprints() {
    assert_eq!(REMOTE_AUTH_QR_URL_PREFIX, "https://discord.com/ra/");

    // Base64url unpadded fingerprint (as produced by Remote Auth gateway v2)
    let fp1 = "ungWv48Bz-pBQUDeXa4iI7ADYaOWF3qctBD_YfIAFa0";
    let url1 = build_remote_auth_qr_url(fp1).expect("valid base64url unpadded");
    assert_eq!(
        url1,
        "https://discord.com/ra/ungWv48Bz-pBQUDeXa4iI7ADYaOWF3qctBD_YfIAFa0"
    );

    // Standard client default fingerprint format
    let fp2 = "discord-remote-auth-7f9a2b8c4d1e";
    let url2 = build_remote_auth_qr_url(fp2).expect("valid alphanumeric-hyphen fingerprint");
    assert_eq!(
        url2,
        "https://discord.com/ra/discord-remote-auth-7f9a2b8c4d1e"
    );

    // Mixed alphanumeric, underscore, hyphen
    let fp3 = "RA_Key-2026_xyz123";
    let url3 = build_remote_auth_qr_url(fp3).expect("valid mixed tokens");
    assert_eq!(url3, "https://discord.com/ra/RA_Key-2026_xyz123");
}

#[test]
fn test_build_remote_auth_qr_url_rejects_empty_and_whitespace() {
    // Empty string
    let res_empty = build_remote_auth_qr_url("");
    assert!(matches!(res_empty, Err(QrMatrixError::EmptyFingerprint)));

    // Whitespace only
    let res_ws = build_remote_auth_qr_url("   \t  \n ");
    assert!(matches!(res_ws, Err(QrMatrixError::EmptyFingerprint)));

    // Whitespace within fingerprint
    let res_inner_ws = build_remote_auth_qr_url("discord remote auth");
    assert!(matches!(
        res_inner_ws,
        Err(QrMatrixError::InvalidCharacters)
    ));

    // Newlines within fingerprint
    let res_nl = build_remote_auth_qr_url("discord\nauth");
    assert!(matches!(res_nl, Err(QrMatrixError::InvalidCharacters)));
}

#[test]
fn test_build_remote_auth_qr_url_rejects_path_traversal_and_malicious_characters() {
    // Traversal attempts
    let res_dotdot = build_remote_auth_qr_url("../admin/token");
    assert!(matches!(
        res_dotdot,
        Err(QrMatrixError::PathTraversalAttempt)
    ));

    let res_slash = build_remote_auth_qr_url("users/me/remote-auth");
    assert!(matches!(
        res_slash,
        Err(QrMatrixError::PathTraversalAttempt)
    ));

    let res_backslash = build_remote_auth_qr_url("..\\windows\\system32");
    assert!(matches!(
        res_backslash,
        Err(QrMatrixError::PathTraversalAttempt)
    ));

    // Special characters outside base64url alphabet
    let res_spec = build_remote_auth_qr_url("discord$auth!");
    assert!(matches!(res_spec, Err(QrMatrixError::InvalidCharacters)));

    let res_eq = build_remote_auth_qr_url("padding==");
    assert!(matches!(res_eq, Err(QrMatrixError::InvalidCharacters)));
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. QrCodeMatrix structure and pattern contracts
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_qr_code_matrix_dimensions_25x25() {
    let fp = "discord-remote-auth-7f9a2b8c4d1e";
    let matrix = QrCodeMatrix::from_remote_auth_fingerprint(fp).expect("matrix generation");

    assert_eq!(
        matrix.size, 25,
        "QR matrix size must be 25x25 (Version 2 standard)"
    );
    assert_eq!(matrix.modules.len(), 25);
    for (r, row) in matrix.modules.iter().enumerate() {
        assert_eq!(row.len(), 25, "Row {r} must have exactly 25 columns");
    }
}

#[test]
fn test_qr_code_matrix_finder_patterns_contract() {
    let fp = "ungWv48Bz-pBQUDeXa4iI7ADYaOWF3qctBD_YfIAFa0";
    let matrix = QrCodeMatrix::from_remote_auth_fingerprint(fp).expect("valid matrix");

    // Helper to check 7x7 finder pattern at top-left corner (r0, c0)
    let assert_finder = |r0: usize, c0: usize, label: &str| {
        for dr in 0..7 {
            for dc in 0..7 {
                let r = r0 + dr;
                let c = c0 + dc;
                let is_dark = matrix.modules[r][c];

                // Outer 7x7 ring is dark
                if dr == 0 || dr == 6 || dc == 0 || dc == 6 {
                    assert!(
                        is_dark,
                        "{label}: Outer ring at (dr={dr}, dc={dc}) -> ({r}, {c}) must be dark"
                    );
                }
                // Inner 5x5 ring is light (white)
                else if dr == 1 || dr == 5 || dc == 1 || dc == 5 {
                    assert!(
                        !is_dark,
                        "{label}: 5x5 ring at (dr={dr}, dc={dc}) -> ({r}, {c}) must be light"
                    );
                }
                // Center 3x3 square is dark
                else {
                    assert!(
                        is_dark,
                        "{label}: 3x3 center at (dr={dr}, dc={dc}) -> ({r}, {c}) must be dark"
                    );
                }
            }
        }
    };

    // 1. Top-Left finder: (0..7, 0..7)
    assert_finder(0, 0, "Top-Left Finder");

    // 2. Top-Right finder: (0..7, 18..25)
    assert_finder(0, 18, "Top-Right Finder");

    // 3. Bottom-Left finder: (18..25, 0..7)
    assert_finder(18, 0, "Bottom-Left Finder");
}

#[test]
fn test_qr_code_matrix_quiet_separators_contract() {
    let fp = "discord-remote-auth-7f9a2b8c4d1e";
    let matrix = QrCodeMatrix::from_remote_auth_fingerprint(fp).expect("valid matrix");

    // Top-left quiet separator:
    // Row 7 (cols 0..=7) and Col 7 (rows 0..=7) must all be light (false)
    for c in 0..=7 {
        assert!(
            !matrix.modules[7][c],
            "Top-left separator row 7 col {c} must be light"
        );
    }
    for r in 0..=7 {
        assert!(
            !matrix.modules[r][7],
            "Top-left separator col 7 row {r} must be light"
        );
    }

    // Top-right quiet separator:
    // Row 7 (cols 17..=24) and Col 17 (rows 0..=7) must all be light (false)
    for c in 17..=24 {
        assert!(
            !matrix.modules[7][c],
            "Top-right separator row 7 col {c} must be light"
        );
    }
    for r in 0..=7 {
        assert!(
            !matrix.modules[r][17],
            "Top-right separator col 17 row {r} must be light"
        );
    }

    // Bottom-left quiet separator:
    // Row 17 (cols 0..=7) and Col 7 (rows 17..=24) must all be light (false)
    for c in 0..=7 {
        assert!(
            !matrix.modules[17][c],
            "Bottom-left separator row 17 col {c} must be light"
        );
    }
    for r in 17..=24 {
        assert!(
            !matrix.modules[r][7],
            "Bottom-left separator col 7 row {r} must be light"
        );
    }
}

#[test]
fn test_qr_code_matrix_timing_patterns_contract() {
    let fp = "discord-remote-auth-7f9a2b8c4d1e";
    let matrix = QrCodeMatrix::from_remote_auth_fingerprint(fp).expect("valid matrix");

    // Horizontal timing pattern on row 6 between separators (cols 8..=16)
    // Must alternate dark/light: col 8 is dark (even), col 9 is light (odd), ...
    for c in 8..=16 {
        let expected = c % 2 == 0;
        assert_eq!(
            matrix.modules[6][c], expected,
            "Row 6 col {c} timing module mismatch: expected {expected}"
        );
    }

    // Vertical timing pattern on col 6 between separators (rows 8..=16)
    // Must alternate dark/light: row 8 is dark (even), row 9 is light (odd), ...
    for r in 8..=16 {
        let expected = r % 2 == 0;
        assert_eq!(
            matrix.modules[r][6], expected,
            "Col 6 row {r} timing module mismatch: expected {expected}"
        );
    }
}

#[test]
fn test_qr_code_matrix_data_distinctness_and_finder_invariance() {
    let fp_a = "discord-remote-auth-fingerprint-AAAAA";
    let fp_b = "discord-remote-auth-fingerprint-BBBBB";

    let matrix_a = QrCodeMatrix::from_remote_auth_fingerprint(fp_a).unwrap();
    let matrix_b = QrCodeMatrix::from_remote_auth_fingerprint(fp_b).unwrap();

    // 1. Finder patterns must be 100% identical in both
    for r in 0..7 {
        for c in 0..7 {
            assert_eq!(matrix_a.modules[r][c], matrix_b.modules[r][c]);
            assert_eq!(matrix_a.modules[r][c + 18], matrix_b.modules[r][c + 18]);
            assert_eq!(matrix_a.modules[r + 18][c], matrix_b.modules[r + 18][c]);
        }
    }

    // 2. Timing patterns must be 100% identical in both
    for i in 8..=16 {
        assert_eq!(matrix_a.modules[6][i], matrix_b.modules[6][i]);
        assert_eq!(matrix_a.modules[i][6], matrix_b.modules[i][6]);
    }

    // 3. Data modules must differ between fp_a and fp_b
    let mut data_differences = 0;
    for r in 0..25 {
        for c in 0..25 {
            // Skip finders & separators
            let in_top_left = r <= 7 && c <= 7;
            let in_top_right = r <= 7 && c >= 17;
            let in_bottom_left = r >= 17 && c <= 7;
            let on_timing = (r == 6 && (8..=16).contains(&c)) || (c == 6 && (8..=16).contains(&r));

            if !in_top_left && !in_top_right && !in_bottom_left && !on_timing {
                if matrix_a.modules[r][c] != matrix_b.modules[r][c] {
                    data_differences += 1;
                }
            }
        }
    }

    assert!(
        data_differences > 50,
        "Distinct fingerprints must produce substantially different data modules, got {data_differences} differences"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. QrLoginUiState transitions contract tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_qr_login_ui_state_transitions() {
    let mut ui_state = QrLoginUiState::new();
    assert_eq!(ui_state, QrLoginUiState::AwaitingFingerprint);
    assert!(ui_state.is_awaiting_fingerprint());

    // 1. AwaitingFingerprint -> Ready
    let fp = "discord-remote-auth-7f9a2b8c4d1e";
    ui_state
        .on_fingerprint_received(fp)
        .expect("transition to ready");

    assert!(ui_state.is_ready());
    if let QrLoginUiState::Ready {
        fingerprint,
        url,
        matrix,
    } = &ui_state
    {
        assert_eq!(fingerprint, fp);
        assert_eq!(
            url,
            "https://discord.com/ra/discord-remote-auth-7f9a2b8c4d1e"
        );
        assert_eq!(matrix.size, 25);
    } else {
        panic!("expected QrLoginUiState::Ready");
    }

    // 2. Ready -> Scanned
    ui_state.on_scanned("DiscordUserTester", "102030405060708090");
    assert!(ui_state.is_scanned());
    if let QrLoginUiState::Scanned { username, user_id } = &ui_state {
        assert_eq!(username, "DiscordUserTester");
        assert_eq!(user_id, "102030405060708090");
    } else {
        panic!("expected QrLoginUiState::Scanned");
    }

    // 3. Scanned -> Expired
    ui_state.on_expired();
    assert!(ui_state.is_expired());
    assert_eq!(ui_state, QrLoginUiState::Expired);

    // 4. Expired -> Error
    ui_state.on_error("Remote Auth Gateway Timeout");
    assert!(ui_state.is_error());
    if let QrLoginUiState::Error(err) = &ui_state {
        assert_eq!(err, "Remote Auth Gateway Timeout");
    } else {
        panic!("expected QrLoginUiState::Error");
    }

    // 5. Error with invalid fingerprint transition
    let mut invalid_state = QrLoginUiState::new();
    let res = invalid_state.on_fingerprint_received("../malicious");
    assert!(res.is_err());
    assert!(invalid_state.is_error());
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. LoginModal state resolution and UI helpers contract tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_login_modal_resolve_qr_ui_state_from_app_state() {
    let mut state = AppState::new();

    // Default AppState has "discord-remote-auth-7f9a2b8c4d1e" -> Ready
    let resolved_default = LoginModal::resolve_qr_ui_state(&state);
    assert!(resolved_default.is_ready());

    // Empty fingerprint -> AwaitingFingerprint
    state.login_form.qr_fingerprint = String::new();
    let resolved_awaiting = LoginModal::resolve_qr_ui_state(&state);
    assert!(resolved_awaiting.is_awaiting_fingerprint());

    // "expired" token -> Expired
    state.login_form.qr_fingerprint = "expired".into();
    let resolved_expired = LoginModal::resolve_qr_ui_state(&state);
    assert!(resolved_expired.is_expired());

    // "scanned:username:user_id" -> Scanned
    state.login_form.qr_fingerprint = "scanned:AliceDev:998877665544".into();
    let resolved_scanned = LoginModal::resolve_qr_ui_state(&state);
    assert!(resolved_scanned.is_scanned());
    if let QrLoginUiState::Scanned { username, user_id } = resolved_scanned {
        assert_eq!(username, "AliceDev");
        assert_eq!(user_id, "998877665544");
    }

    // Invalid path traversal string -> Error
    state.login_form.qr_fingerprint = "../path/traversal".into();
    let resolved_err = LoginModal::resolve_qr_ui_state(&state);
    assert!(resolved_err.is_error());
}

#[test]
fn test_login_modal_render_qr_elements_instantiation() {
    let fp = "discord-remote-auth-7f9a2b8c4d1e";
    let matrix = QrCodeMatrix::from_remote_auth_fingerprint(fp).expect("valid matrix");

    // 1. Render matrix element
    let _matrix_el = LoginModal::render_qr_matrix(&matrix);

    // 2. Render all variants of QrLoginUiState container elements
    let ready_state = QrLoginUiState::Ready {
        fingerprint: fp.to_string(),
        url: format!("https://discord.com/ra/{fp}"),
        matrix: matrix.clone(),
    };
    let _ready_el = LoginModal::render_qr_container(&ready_state);

    let scanned_state = QrLoginUiState::Scanned {
        username: "ScannedTester".into(),
        user_id: "123456".into(),
    };
    let _scanned_el = LoginModal::render_qr_container(&scanned_state);

    let expired_state = QrLoginUiState::Expired;
    let _expired_el = LoginModal::render_qr_container(&expired_state);

    let awaiting_state = QrLoginUiState::AwaitingFingerprint;
    let _awaiting_el = LoginModal::render_qr_container(&awaiting_state);

    let error_state = QrLoginUiState::Error("Simulated error".into());
    let _error_el = LoginModal::render_qr_container(&error_state);
}
