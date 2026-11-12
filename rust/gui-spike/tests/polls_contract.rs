//! RW-40 / task-216 (BE-P1-18): контрактные тесты BFF-модуля голосования и результатов опросов (`polls`).
//!
//! Проверяет:
//! 1. Константы `MAX_POLL_ANSWERS = 10`, `MAX_POLL_QUESTION_CHARS = 300`, `MAX_POLL_ANSWER_CHARS = 55`.
//! 2. Структуры `PollAnswerOption`, `PollVoteRequest`, `PollResultEntry` и их serde round-trip.
//! 3. `validate_poll_vote_request(req: &PollVoteRequest) -> Result<(), String>`:
//!    - валидация `channel_id` и `message_id` как канонических положительных `u64` Snowflake ID (`1..=u64::MAX`, без ведущих нулей);
//!    - пустой `answer_ids` (снятие голоса) и `1..=MAX_POLL_ANSWERS` при `allow_multiselect = true`;
//!    - отклонение `answer_ids.len() > MAX_POLL_ANSWERS`;
//!    - отклонение `answer_ids.len() > 1` при `allow_multiselect == false`;
//!    - отклонение `answer_id == 0` и дублирующихся `answer_id`.
//! 4. `build_poll_answer_put_route(channel_id: &str, message_id: &str, answer_id: u32) -> Result<String, String>`:
//!    - валидация `channel_id`, `message_id` и `answer_id >= 1`;
//!    - построение канонического маршрута `"/channels/{channel_id}/polls/{message_id}/answers/{answer_id}"`
//!      (сверка с RE-оракулом `mod-652215.pretty.js:97` / `rest-static.json:10552`).
//! 5. `compute_poll_results(options: &[PollAnswerOption], is_expired: bool) -> (u64, Vec<PollResultEntry>)`:
//!    - вычисление `total_votes = sum(votes)`;
//!    - округление `percentage = ((votes as f64 / total_votes as f64) * 100.0).round() as u8` (и `0` при `total_votes == 0`);
//!    - вычисление `is_winner = is_expired && max_votes > 0 && votes == max_votes` (включая ничью при `is_expired == true`).

use chord_client::polls::{
    build_poll_answer_put_route, compute_poll_results, validate_poll_vote_request,
    PollAnswerOption, PollResultEntry, PollVoteRequest, MAX_POLL_ANSWERS, MAX_POLL_ANSWER_CHARS,
    MAX_POLL_QUESTION_CHARS,
};

const VALID_CHANNEL_ID: &str = "100000000000000001";
const VALID_MESSAGE_ID: &str = "100000000000000002";

// ---------------------------------------------------------------------------
// 1. Константы и serde round-trip структур
// ---------------------------------------------------------------------------

#[test]
fn poll_constants_match_discord_contract() {
    assert_eq!(MAX_POLL_ANSWERS, 10);
    assert_eq!(MAX_POLL_QUESTION_CHARS, 300);
    assert_eq!(MAX_POLL_ANSWER_CHARS, 55);
}

#[test]
fn poll_structs_serde_roundtrip() {
    let opt = PollAnswerOption {
        answer_id: 1,
        text: "Rust".to_string(),
        votes: 42,
        me_voted: true,
    };
    let opt_json = serde_json::to_value(&opt).expect("serialize PollAnswerOption");
    let opt_back: PollAnswerOption =
        serde_json::from_value(opt_json).expect("deserialize PollAnswerOption");
    assert_eq!(opt_back, opt);

    let req = PollVoteRequest {
        channel_id: VALID_CHANNEL_ID.to_string(),
        message_id: VALID_MESSAGE_ID.to_string(),
        answer_ids: vec![1, 3, 5],
        allow_multiselect: true,
    };
    let req_json = serde_json::to_value(&req).expect("serialize PollVoteRequest");
    let req_back: PollVoteRequest =
        serde_json::from_value(req_json).expect("deserialize PollVoteRequest");
    assert_eq!(req_back, req);

    let entry = PollResultEntry {
        answer_id: 2,
        votes: 15,
        percentage: 75,
        is_winner: true,
    };
    let entry_json = serde_json::to_value(&entry).expect("serialize PollResultEntry");
    let entry_back: PollResultEntry =
        serde_json::from_value(entry_json).expect("deserialize PollResultEntry");
    assert_eq!(entry_back, entry);
}

// ---------------------------------------------------------------------------
// 2. Валидация PollVoteRequest: Snowflake границы (1..=u64::MAX, без ведущих нулей)
// ---------------------------------------------------------------------------

#[test]
fn validate_poll_vote_request_checks_channel_and_message_snowflake_bounds() {
    let max_u64 = u64::MAX.to_string();
    let min_req = PollVoteRequest {
        channel_id: "1".to_string(),
        message_id: max_u64.clone(),
        answer_ids: vec![1],
        allow_multiselect: false,
    };
    assert!(validate_poll_vote_request(&min_req).is_ok());

    let max_req = PollVoteRequest {
        channel_id: max_u64,
        message_id: "1".to_string(),
        answer_ids: vec![u32::MAX],
        allow_multiselect: false,
    };
    assert!(validate_poll_vote_request(&max_req).is_ok());

    let invalid_snowflakes = [
        "",
        "0",
        "000",
        "0123456789",
        "abc",
        "123a456",
        "-1",
        " 100000000000000001",
        "100000000000000001 ",
        "18446744073709551616", // u64::MAX + 1
    ];

    for bad in invalid_snowflakes {
        let bad_channel = PollVoteRequest {
            channel_id: bad.to_string(),
            message_id: VALID_MESSAGE_ID.to_string(),
            answer_ids: vec![1],
            allow_multiselect: false,
        };
        assert!(
            validate_poll_vote_request(&bad_channel).is_err(),
            "channel_id={bad:?} must be rejected"
        );

        let bad_message = PollVoteRequest {
            channel_id: VALID_CHANNEL_ID.to_string(),
            message_id: bad.to_string(),
            answer_ids: vec![1],
            allow_multiselect: false,
        };
        assert!(
            validate_poll_vote_request(&bad_message).is_err(),
            "message_id={bad:?} must be rejected"
        );
    }
}

// ---------------------------------------------------------------------------
// 3. Валидация PollVoteRequest: лимит MAX_POLL_ANSWERS, multiselect, answer_id=0 и дубликаты
// ---------------------------------------------------------------------------

#[test]
fn validate_poll_vote_request_enforces_multiselect_and_max_answers_bounds() {
    // Снятие голоса (пустой список) допустимо и для single-, и для multi-select
    let clear_single = PollVoteRequest {
        channel_id: VALID_CHANNEL_ID.to_string(),
        message_id: VALID_MESSAGE_ID.to_string(),
        answer_ids: vec![],
        allow_multiselect: false,
    };
    assert!(validate_poll_vote_request(&clear_single).is_ok());

    let clear_multi = PollVoteRequest {
        channel_id: VALID_CHANNEL_ID.to_string(),
        message_id: VALID_MESSAGE_ID.to_string(),
        answer_ids: vec![],
        allow_multiselect: true,
    };
    assert!(validate_poll_vote_request(&clear_multi).is_ok());

    // Single-select: ровно 1 ответ -> Ok, 2 ответа -> Err
    let single_one = PollVoteRequest {
        channel_id: VALID_CHANNEL_ID.to_string(),
        message_id: VALID_MESSAGE_ID.to_string(),
        answer_ids: vec![1],
        allow_multiselect: false,
    };
    assert!(validate_poll_vote_request(&single_one).is_ok());

    let single_two = PollVoteRequest {
        channel_id: VALID_CHANNEL_ID.to_string(),
        message_id: VALID_MESSAGE_ID.to_string(),
        answer_ids: vec![1, 2],
        allow_multiselect: false,
    };
    assert!(validate_poll_vote_request(&single_two).is_err());

    // Multi-select: ровно 10 ответов (MAX_POLL_ANSWERS) -> Ok, 11 ответов -> Err
    let multi_ten = PollVoteRequest {
        channel_id: VALID_CHANNEL_ID.to_string(),
        message_id: VALID_MESSAGE_ID.to_string(),
        answer_ids: (1..=10).collect(),
        allow_multiselect: true,
    };
    assert!(validate_poll_vote_request(&multi_ten).is_ok());

    let multi_eleven = PollVoteRequest {
        channel_id: VALID_CHANNEL_ID.to_string(),
        message_id: VALID_MESSAGE_ID.to_string(),
        answer_ids: (1..=11).collect(),
        allow_multiselect: true,
    };
    assert!(validate_poll_vote_request(&multi_eleven).is_err());
}

#[test]
fn validate_poll_vote_request_rejects_zero_and_duplicate_answer_ids() {
    let zero_single = PollVoteRequest {
        channel_id: VALID_CHANNEL_ID.to_string(),
        message_id: VALID_MESSAGE_ID.to_string(),
        answer_ids: vec![0],
        allow_multiselect: false,
    };
    assert!(validate_poll_vote_request(&zero_single).is_err());

    let zero_in_multi = PollVoteRequest {
        channel_id: VALID_CHANNEL_ID.to_string(),
        message_id: VALID_MESSAGE_ID.to_string(),
        answer_ids: vec![1, 2, 0, 4],
        allow_multiselect: true,
    };
    assert!(validate_poll_vote_request(&zero_in_multi).is_err());

    let duplicate_multi = PollVoteRequest {
        channel_id: VALID_CHANNEL_ID.to_string(),
        message_id: VALID_MESSAGE_ID.to_string(),
        answer_ids: vec![1, 3, 2, 3],
        allow_multiselect: true,
    };
    assert!(validate_poll_vote_request(&duplicate_multi).is_err());
}

// ---------------------------------------------------------------------------
// 4. build_poll_answer_put_route: канонический маршрут и валидация аргументов
// ---------------------------------------------------------------------------

#[test]
fn build_poll_answer_put_route_returns_canonical_path_and_validates_inputs() {
    let route = build_poll_answer_put_route(VALID_CHANNEL_ID, VALID_MESSAGE_ID, 1)
        .expect("valid poll answer route");
    assert_eq!(
        route,
        "/channels/100000000000000001/polls/100000000000000002/answers/1"
    );

    let max_u64 = u64::MAX.to_string();
    let boundary_route =
        build_poll_answer_put_route("1", &max_u64, u32::MAX).expect("boundary route");
    assert_eq!(
        boundary_route,
        format!("/channels/1/polls/{max_u64}/answers/{}", u32::MAX)
    );

    // Невалидный answer_id == 0
    assert!(build_poll_answer_put_route(VALID_CHANNEL_ID, VALID_MESSAGE_ID, 0).is_err());

    // Невалидные Snowflake для channel_id и message_id
    for bad in ["", "0", "0123", "abc", "18446744073709551616"] {
        assert!(
            build_poll_answer_put_route(bad, VALID_MESSAGE_ID, 1).is_err(),
            "channel_id={bad:?} must fail"
        );
        assert!(
            build_poll_answer_put_route(VALID_CHANNEL_ID, bad, 1).is_err(),
            "message_id={bad:?} must fail"
        );
    }
}

// ---------------------------------------------------------------------------
// 5. compute_poll_results: пустые/нулевые голоса, округление процентов, is_winner (active/expired/tie)
// ---------------------------------------------------------------------------

#[test]
fn compute_poll_results_handles_empty_and_zero_vote_polls() {
    let (total_empty, entries_empty) = compute_poll_results(&[], true);
    assert_eq!(total_empty, 0);
    assert!(entries_empty.is_empty());

    let zero_options = vec![
        PollAnswerOption {
            answer_id: 1,
            text: "Вариант А".to_string(),
            votes: 0,
            me_voted: false,
        },
        PollAnswerOption {
            answer_id: 2,
            text: "Вариант Б".to_string(),
            votes: 0,
            me_voted: false,
        },
    ];

    for is_expired in [false, true] {
        let (total, entries) = compute_poll_results(&zero_options, is_expired);
        assert_eq!(total, 0);
        assert_eq!(
            entries,
            vec![
                PollResultEntry {
                    answer_id: 1,
                    votes: 0,
                    percentage: 0,
                    is_winner: false,
                },
                PollResultEntry {
                    answer_id: 2,
                    votes: 0,
                    percentage: 0,
                    is_winner: false,
                },
            ]
        );
    }
}

#[test]
fn compute_poll_results_calculates_rounded_percentages_and_winners_when_expired() {
    // 1 голос из 3 -> 33.33% -> 33%, 2 голоса из 3 -> 66.67% -> 67%
    let options = vec![
        PollAnswerOption {
            answer_id: 1,
            text: "Первый".to_string(),
            votes: 1,
            me_voted: false,
        },
        PollAnswerOption {
            answer_id: 2,
            text: "Второй".to_string(),
            votes: 2,
            me_voted: true,
        },
        PollAnswerOption {
            answer_id: 3,
            text: "Третий".to_string(),
            votes: 0,
            me_voted: false,
        },
    ];

    // Пока опрос не завершён (is_expired == false), победителей нет (is_winner == false)
    let (total_active, entries_active) = compute_poll_results(&options, false);
    assert_eq!(total_active, 3);
    assert_eq!(
        entries_active,
        vec![
            PollResultEntry {
                answer_id: 1,
                votes: 1,
                percentage: 33,
                is_winner: false,
            },
            PollResultEntry {
                answer_id: 2,
                votes: 2,
                percentage: 67,
                is_winner: false,
            },
            PollResultEntry {
                answer_id: 3,
                votes: 0,
                percentage: 0,
                is_winner: false,
            },
        ]
    );

    // После завершения опроса (is_expired == true), вариант с max_votes > 0 получает is_winner == true
    let (total_expired, entries_expired) = compute_poll_results(&options, true);
    assert_eq!(total_expired, 3);
    assert_eq!(
        entries_expired,
        vec![
            PollResultEntry {
                answer_id: 1,
                votes: 1,
                percentage: 33,
                is_winner: false,
            },
            PollResultEntry {
                answer_id: 2,
                votes: 2,
                percentage: 67,
                is_winner: true,
            },
            PollResultEntry {
                answer_id: 3,
                votes: 0,
                percentage: 0,
                is_winner: false,
            },
        ]
    );
}

#[test]
fn compute_poll_results_marks_all_tied_leaders_as_winners_when_expired() {
    // 1 голос из 8 = 12.5% -> round() = 13%, 3 голоса из 8 = 37.5% -> round() = 38%
    let tied_options = vec![
        PollAnswerOption {
            answer_id: 10,
            text: "Лидер 1".to_string(),
            votes: 3,
            me_voted: true,
        },
        PollAnswerOption {
            answer_id: 20,
            text: "Лидер 2".to_string(),
            votes: 3,
            me_voted: false,
        },
        PollAnswerOption {
            answer_id: 30,
            text: "Отстающий 1".to_string(),
            votes: 1,
            me_voted: false,
        },
        PollAnswerOption {
            answer_id: 40,
            text: "Отстающий 2".to_string(),
            votes: 1,
            me_voted: false,
        },
    ];

    let (total, entries) = compute_poll_results(&tied_options, true);
    assert_eq!(total, 8);
    assert_eq!(
        entries,
        vec![
            PollResultEntry {
                answer_id: 10,
                votes: 3,
                percentage: 38,
                is_winner: true,
            },
            PollResultEntry {
                answer_id: 20,
                votes: 3,
                percentage: 38,
                is_winner: true,
            },
            PollResultEntry {
                answer_id: 30,
                votes: 1,
                percentage: 13,
                is_winner: false,
            },
            PollResultEntry {
                answer_id: 40,
                votes: 1,
                percentage: 13,
                is_winner: false,
            },
        ]
    );
}
