//! W39j: пилот моста BFF → `discord-core` (`state::Message` + `history::History`).
//!
//! Цель — доказать, что страница сообщений BFF может собираться из ядра, а не
//! только из JSON фикстур. Здесь нет сети, Tauri-рантайма и нового формата
//! ответа: строки канала читает тот же загрузчик, что `data::messages_page`
//! (`data::channel_rows`), затем они конвертируются в
//! [`discord_core::state::Message`] и складываются в ограниченную
//! [`History`]. Страница выбирается из истории ядра; курсор проверяется общей с
//! BFF функцией `data::cursor_start`, поэтому семантика курсора одна.
//!
//! Ограничения пилота честно типизированы:
//! - `id`/`channel_id`/`author.id`/`guild_id`/упоминания обязаны быть
//!   `u64`-снежинками, иначе [`DataError::Parse`] (`kind = "Parse"`), без
//!   паники и без молчаливого пропуска строки. Санитизированные отрицательные
//!   id дампа ядро не представляет — это ожидаемый `Parse`, а не регрессия;
//! - `Message` ядра не несёт embeds/attachments/reactions: мост отдаёт их
//!   счётчики (`embed_count`/`attachment_count`), полные объекты остаются в BFF
//!   (`MessageRow`) — формат ответа BFF не меняется;
//! - `settings`/`voice` в этой итерации не подключены (следующий шаг).
//!
//! Порядок страницы — как у селекторов ядра (возрастание числового id). На
//! боевых snowflake это совпадает с порядком BFF (`timestamp`, затем `id`); на
//! санитизированных отрицательных id сравнение неприменимо (см. выше).

use std::path::Path;

use discord_core::history::{Cursor, History, HistoryConfig, MessagePage, PageRequest};
use discord_core::ready::User;
use discord_core::state::Message;

use crate::data::{self, DataError, MessageRow};

/// Синтетический источник для `DataError::Parse`: строка уже прочитана из
/// `by-type`, но путь в `MessageRow` не сохраняется. Ошибка всё равно остаётся
/// типизированной (`{kind, message, path}`), а не паникой.
const BRIDGE_SOURCE: &str = "<fixtures/gateway/by-type>";

/// Страница сообщений, собранная ядром. Поля повторяют контракт
/// [`crate::data::MessagesPage`] там, где ядро может их дать: id/порядок/курсор,
/// `has_more`, `total`, `limit`. Элементы — типизированные `Message` ядра.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreMessagesPage {
    /// Канал (нормализованный id).
    pub channel_id: String,
    /// Сообщения страницы в порядке ядра (возрастание id).
    pub items: Vec<Message>,
    /// id последней строки, если есть следующая страница.
    pub next_cursor: Option<String>,
    /// Есть ли ещё строки после этой страницы.
    pub has_more: bool,
    /// Всего сообщений канала (до пагинации).
    pub total: usize,
    /// Запрошенный размер страницы.
    pub limit: usize,
}

/// Разбор снежинки: нечисловое/отрицательное/переполненное значение — `Parse`.
fn parse_snowflake(value: &str, what: &str) -> Result<u64, DataError> {
    value.parse::<u64>().map_err(|_| {
        DataError::parse(
            Path::new(BRIDGE_SOURCE),
            format!("{what}: {value:?} не является снежинкой u64"),
        )
    })
}

/// Конверсия строки BFF в сообщение ядра.
///
/// Переносятся поля контракта списка чата: `id`/`channel_id`/`guild_id`,
/// автор, `content`, `timestamp`, `edited_timestamp`, `pinned`, `message_type`,
/// упоминания; счётчики embeds/attachments берутся из длин списков.
/// Флаги/`tts` в строке BFF отсутствуют и остаются `0`/`false` — это не
/// выдуманные значения, а отсутствие данных в источнике.
pub fn message_from_row(row: &MessageRow) -> Result<Message, DataError> {
    let id = parse_snowflake(&row.id, "сообщение")?;
    let channel_id = parse_snowflake(&row.channel_id, "канал сообщения")?;
    let author = User {
        id: parse_snowflake(&row.author.id, "автор")?,
        username: Some(row.author.username.clone()),
        global_name: row.author.global_name.clone(),
        display_name: None,
        discriminator: None,
        avatar: row.author.avatar.clone(),
        bot: row.author.bot,
        public_flags: None,
    };
    let guild_id = row
        .guild_id
        .as_deref()
        .map(|value| parse_snowflake(value, "гильдия"))
        .transpose()?;
    let message_type = i64::try_from(row.message_type).map_err(|_| {
        DataError::parse(
            Path::new(BRIDGE_SOURCE),
            format!(
                "сообщение {}: type {} не помещается в i64",
                row.id, row.message_type
            ),
        )
    })?;
    let mention_user_ids = row
        .mentions
        .iter()
        .map(|value| parse_snowflake(value, "упоминание"))
        .collect::<Result<Vec<u64>, DataError>>()?;

    Ok(Message {
        id,
        channel_id,
        guild_id,
        author: Some(author),
        content: row.content.clone(),
        timestamp: row.timestamp.clone(),
        edited_timestamp: row.edited_at.clone(),
        message_type,
        flags: 0,
        pinned: row.pinned,
        tts: false,
        mention_everyone: row.mention_everyone,
        mention_user_ids,
        mention_role_ids: Vec::new(),
        attachment_count: row.attachments.len(),
        embed_count: row.embeds.len(),
    })
}

/// Ошибки `History` в `DataError`: конфигурация/применение страницы здесь —
/// внутренняя инвариантная операция, поэтому `Parse` с источником `history`.
fn history_error(error: impl std::fmt::Display) -> DataError {
    DataError::parse(Path::new(BRIDGE_SOURCE), format!("history: {error}"))
}

/// Страница канала через ядро: загрузка строк BFF, конверсия в `Message`,
/// окно [`History`] и выборка страницы по тому же курсору, что у BFF.
///
/// `limit` — `1..=MAX_PAGE_LIMIT` (как `data::messages_page`), `cursor` — id
/// последней строки предыдущей страницы; чужой/неизвестный курсор —
/// `InvalidArg`. Пустая страница (тихий канал или хвост выборки) — валидный
/// ответ: `items: []`, `has_more: false`, `next_cursor: None`.
pub fn core_messages_page(
    root: &Path,
    channel_id: &str,
    cursor: Option<&str>,
    limit: Option<usize>,
) -> Result<CoreMessagesPage, DataError> {
    let limit = data::page_limit(limit)?;
    let rows = data::channel_rows(root, channel_id)?;
    let channel_id = channel_id.trim();
    let channel = parse_snowflake(channel_id, "канал")?;

    let messages = rows
        .iter()
        .map(message_from_row)
        .collect::<Result<Vec<Message>, DataError>>()?;
    let total = messages.len();

    // Окно обязано вместить весь канал: пилот сравнивает страницы, а не
    // вытеснение. Верхняя граница — `u16` из `HistoryConfig`; больше — честная
    // ошибка аргумента, а не тихое усечение.
    let capacity = total.max(1);
    let capacity_u16 = u16::try_from(capacity).map_err(|_| {
        DataError::invalid_arg(format!(
            "канал {channel_id}: {total} сообщений не помещаются в окно ядра (максимум {})",
            u16::MAX
        ))
    })?;
    let config = HistoryConfig {
        channel_window: capacity,
        page_limit: capacity_u16,
    };
    let mut history = History::new(config).map_err(history_error)?;

    if !messages.is_empty() {
        let request = PageRequest::new(channel, Cursor::Latest, capacity_u16);
        let token = history.begin_page(request).map_err(history_error)?;
        let page = MessagePage {
            messages,
            has_older: None,
            has_newer: None,
        };
        history.apply_page(token, page).map_err(history_error)?;
    }

    // Селектор ядра: возрастание числового id; окно равно числу сообщений
    // канала, поэтому вытеснения нет.
    let all: Vec<Message> = history.messages(channel).into_iter().cloned().collect();
    let ids: Vec<String> = all.iter().map(|message| message.id.to_string()).collect();
    let start = data::cursor_start(&ids, cursor, "сообщений")?;
    let end = (start + limit).min(all.len());
    let items: Vec<Message> = all[start..end].to_vec();
    let has_more = end < all.len();
    let next_cursor = if has_more {
        items.last().map(|message| message.id.to_string())
    } else {
        None
    };

    Ok(CoreMessagesPage {
        channel_id: channel_id.to_string(),
        items,
        next_cursor,
        has_more,
        total: all.len(),
        limit,
    })
}
