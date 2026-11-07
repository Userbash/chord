//! RW-40 / task-216 (`BE-P1-18`): BFF-модуль голосования и подсчёта результатов опросов (`polls`).
//!
//! Источники контракта:
//! - `/home/sanya/re-deep/rest-static.json:10525-10560` (`POLL_ANSWERS`, `POLL_EXPIRE`, `POLL_ANSWER_VOTERS`);
//! - `/home/sanya/re-deep/pretty/mod-652215.pretty.js:97` (`POLL_ANSWER_VOTERS: (e, t, n) => "/channels/${e}/polls/${t}/answers/${n}"`);
//! - `rust/gui-spike/src/ipc_ext.rs` (`MAX_POLL_ANSWERS = 10`, `VotePoll`);
//! - `rust/gui-spike/ui/features/poll-progress.js` (`computePollResults`).

/// Максимальное количество вариантов ответа в опросе Discord (`1..=10`).
pub const MAX_POLL_ANSWERS: usize = 10;

/// Максимальная длина текста вопроса опроса в символах Unicode (`<= 300`).
pub const MAX_POLL_QUESTION_CHARS: usize = 300;

/// Максимальная длина текста варианта ответа опроса в символах Unicode (`<= 55`).
pub const MAX_POLL_ANSWER_CHARS: usize = 55;

/// Вариант ответа опроса с текущим числом голосов и отметкой выбора текущим пользователем.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PollAnswerOption {
    /// Идентификатор варианта ответа (`>= 1`).
    pub answer_id: u32,
    /// Текст варианта ответа.
    pub text: String,
    /// Количество голосов за данный вариант.
    pub votes: u64,
    /// Проголосовал ли текущий пользователь за этот вариант.
    pub me_voted: bool,
}

/// Запрос на голосование (или снятие голоса) в опросе.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PollVoteRequest {
    /// Snowflake ID канала (`1..=u64::MAX`, без ведущих нулей).
    pub channel_id: String,
    /// Snowflake ID сообщения с опросом (`1..=u64::MAX`, без ведущих нулей).
    pub message_id: String,
    /// Список выбранных `answer_id` (может быть пустым при снятии голоса).
    pub answer_ids: Vec<u32>,
    /// Разрешён ли множественный выбор в опросе.
    pub allow_multiselect: bool,
}

/// Рассчитанный результат по одному варианту ответа опроса.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PollResultEntry {
    /// Идентификатор варианта ответа.
    pub answer_id: u32,
    /// Количество голосов за вариант.
    pub votes: u64,
    /// Округлённая доля голосов в процентах (`0..=100`).
    pub percentage: u8,
    /// Признак победившего варианта (только для завершённого опроса `is_expired && max_votes > 0 && votes == max_votes`).
    pub is_winner: bool,
}

/// Проверяет, что строка является каноническим положительным `u64` Snowflake ID
/// (`1..=u64::MAX`, только ASCII-цифры, без ведущих нулей).
fn validate_snowflake(field: &str, value: &str) -> Result<(), String> {
    if value.is_empty() || value.starts_with('0') || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("некорректный Snowflake ID в поле {field}"));
    }
    match value.parse::<u64>() {
        Ok(n) if n > 0 => Ok(()),
        _ => Err(format!("некорректный Snowflake ID в поле {field}")),
    }
}

/// Валидирует запрос голосования в опросе [`PollVoteRequest`]:
/// - `channel_id` и `message_id` — канонические положительные `u64` Snowflake строки (`1..=u64::MAX`, без ведущих нулей);
/// - `answer_ids.len() <= MAX_POLL_ANSWERS`;
/// - если `!allow_multiselect`, то `answer_ids.len() <= 1`;
/// - каждый `answer_id >= 1` и без дубликатов.
pub fn validate_poll_vote_request(req: &PollVoteRequest) -> Result<(), String> {
    validate_snowflake("channel_id", &req.channel_id)?;
    validate_snowflake("message_id", &req.message_id)?;

    if req.answer_ids.len() > MAX_POLL_ANSWERS {
        return Err(format!(
            "количество выбранных ответов ({}) превышает максимум {MAX_POLL_ANSWERS}",
            req.answer_ids.len()
        ));
    }

    if !req.allow_multiselect && req.answer_ids.len() > 1 {
        return Err(
            "множественный выбор запрещён (allow_multiselect = false), но передано более 1 ответа"
                .to_string(),
        );
    }

    for (idx, &answer_id) in req.answer_ids.iter().enumerate() {
        if answer_id == 0 {
            return Err("answer_id должен быть >= 1 (получен 0)".to_string());
        }
        if req.answer_ids[..idx].contains(&answer_id) {
            return Err(format!("обнаружен дублирующийся answer_id: {answer_id}"));
        }
    }

    Ok(())
}

/// Строит маршрут ответа опроса `/channels/{channel_id}/polls/{message_id}/answers/{answer_id}`
/// после валидации `channel_id`, `message_id` (канонические положительные Snowflake) и `answer_id >= 1`.
pub fn build_poll_answer_put_route(
    channel_id: &str,
    message_id: &str,
    answer_id: u32,
) -> Result<String, String> {
    validate_snowflake("channel_id", channel_id)?;
    validate_snowflake("message_id", message_id)?;
    if answer_id == 0 {
        return Err("answer_id должен быть >= 1 (получен 0)".to_string());
    }
    Ok(format!(
        "/channels/{channel_id}/polls/{message_id}/answers/{answer_id}"
    ))
}

/// Вычисляет агрегированные результаты опроса `(total_votes, entries)`:
/// - `total_votes` — сумма голосов по всем вариантам;
/// - `percentage` — `0`, если `total_votes == 0`, иначе `((votes as f64 / total_votes as f64) * 100.0).round() as u8`;
/// - `is_winner` — `is_expired && max_votes > 0 && votes == max_votes`.
#[must_use]
pub fn compute_poll_results(
    options: &[PollAnswerOption],
    is_expired: bool,
) -> (u64, Vec<PollResultEntry>) {
    let total_votes: u64 = options.iter().map(|opt| opt.votes).sum();
    let max_votes: u64 = options.iter().map(|opt| opt.votes).max().unwrap_or(0);

    let entries = options
        .iter()
        .map(|opt| {
            let percentage = if total_votes == 0 {
                0
            } else {
                #[allow(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    clippy::cast_precision_loss
                )]
                let pct = ((opt.votes as f64 / total_votes as f64) * 100.0).round() as u8;
                pct
            };
            let is_winner = is_expired && max_votes > 0 && opt.votes == max_votes;
            PollResultEntry {
                answer_id: opt.answer_id,
                votes: opt.votes,
                percentage,
                is_winner,
            }
        })
        .collect();

    (total_votes, entries)
}
