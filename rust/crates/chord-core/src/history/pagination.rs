//! P2-06b: курсоры, страницы и слияние (RW-14 / W14b).
//!
//! `MessagePage` — единственный вход уже типизированной страницы. JSON/REST/сеть/таймауты
//! остаются за адаптером; здесь только детерминированное слияние и generation-токены.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::state::{Effect, Message};

use super::types::{History, WindowDirection};

/// Предел накопленных id в `HistoryDiagnostics::duplicates`; счётчик страницы не ограничен.
const MAX_DIAGNOSTIC_DUPLICATES: usize = 256;

/// Направление/курсор страницы.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cursor {
    /// Самые новые.
    Latest,
    /// Старше указанного id.
    Before(u64),
    /// Новее указанного id (контракт; capture не подтверждает серверную семантику).
    After(u64),
    /// Вокруг указанного id (локальная модель; серверная семантика не подтверждена).
    Around(u64),
}

/// Запрос страницы.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageRequest {
    /// Канал.
    pub channel_id: u64,
    /// Курсор.
    pub cursor: Cursor,
    /// Лимит (1..=`HistoryConfig::page_limit`).
    pub limit: u16,
}

impl PageRequest {
    /// Создать запрос без валидации (валидирует `History::begin_page`).
    #[must_use]
    pub fn new(channel_id: u64, cursor: Cursor, limit: u16) -> Self {
        Self {
            channel_id,
            cursor,
            limit,
        }
    }

    /// Запрос из эффекта `FetchMessages`; другие эффекты не относятся к истории.
    ///
    /// `FetchMessages` умеет только `Latest`/`Before` — это принятый интерфейс P2-02.
    /// `after`/`around` доступны только через прямой `PageRequest`.
    pub fn from_effect(effect: &Effect, max_limit: u16) -> Result<Option<Self>, PageError> {
        let Effect::FetchMessages {
            channel_id,
            before,
            limit,
        } = effect
        else {
            return Ok(None);
        };
        let cursor = before.map_or(Cursor::Latest, Cursor::Before);
        let request = Self::new(*channel_id, cursor, *limit);
        request.validate(max_limit)?;
        Ok(Some(request))
    }

    /// Проверить лимит.
    pub fn validate(&self, max_limit: u16) -> Result<(), PageError> {
        if self.limit == 0 {
            return Err(PageError::ZeroLimit);
        }
        if self.limit > max_limit {
            return Err(PageError::LimitTooLarge {
                limit: self.limit,
                max: max_limit,
            });
        }
        Ok(())
    }

    /// Query-форма. Порядок `limit`/`before`/`around` совпадает с наблюдёнными запросами;
    /// `after` — только контракт.
    #[must_use]
    pub fn query(&self) -> String {
        match self.cursor {
            Cursor::Latest => format!("limit={}", self.limit),
            Cursor::Before(id) => format!("before={id}&limit={}", self.limit),
            Cursor::After(id) => format!("after={id}&limit={}", self.limit),
            Cursor::Around(id) => format!("limit={}&around={id}", self.limit),
        }
    }
}

/// Страница уже разобранных сообщений.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MessagePage {
    /// Сообщения в порядке ответа; history сама сортирует по id.
    pub messages: Vec<Message>,
    /// Есть ли более старые; `None` — неизвестно.
    pub has_older: Option<bool>,
    /// Есть ли более новые; `None` — неизвестно.
    pub has_newer: Option<bool>,
}

/// Токен страницы, защищающий от поздних ответов.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageToken {
    channel_id: u64,
    generation: u64,
    cursor: Cursor,
    limit: u16,
}

impl PageToken {
    /// Канал запроса.
    #[must_use]
    pub fn channel_id(&self) -> u64 {
        self.channel_id
    }

    /// Курсор запроса.
    #[must_use]
    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    /// Лимит запроса.
    #[must_use]
    pub fn limit(&self) -> u16 {
        self.limit
    }
}

/// Результат применения страницы.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PageApply {
    /// Новых сообщений.
    pub inserted: usize,
    /// Заменённых сообщений.
    pub replaced: usize,
    /// Вытесненных сообщений.
    pub evicted: usize,
    /// Повторных id внутри страницы.
    pub duplicates: usize,
}

/// Ошибка страницы/курсора.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageError {
    /// `limit == 0`.
    ZeroLimit,
    /// `limit > max`.
    LimitTooLarge {
        /// Запрошенный лимит.
        limit: u16,
        /// Локальный максимум.
        max: u16,
    },
    /// Токен устарел или уже применён.
    StaleToken {
        /// Текущее поколение.
        expected: u64,
        /// Поколение токена.
        got: u64,
    },
    /// Страница содержит сообщение другого канала.
    ChannelMismatch {
        /// Канал токена.
        expected: u64,
        /// Канал сообщения.
        actual: u64,
        /// id сообщения.
        message_id: u64,
    },
}

impl fmt::Display for PageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroLimit => formatter.write_str("history: page limit must be > 0"),
            Self::LimitTooLarge { limit, max } => {
                write!(formatter, "history: page limit {limit} > max {max}")
            }
            Self::StaleToken { expected, got } => {
                write!(
                    formatter,
                    "history: stale page token {got} (current {expected})"
                )
            }
            Self::ChannelMismatch {
                expected,
                actual,
                message_id,
            } => write!(
                formatter,
                "history: message {message_id} belongs to channel {actual}, expected {expected}"
            ),
        }
    }
}

impl std::error::Error for PageError {}

impl History {
    /// Начать страницу: валидировать лимит, создать канал и выдать generation-токен.
    pub fn begin_page(&mut self, request: PageRequest) -> Result<PageToken, PageError> {
        request.validate(self.config.page_limit)?;
        let channel = self.ensure_channel(request.channel_id);
        channel.generation = channel.generation.wrapping_add(1);
        Ok(PageToken {
            channel_id: request.channel_id,
            generation: channel.generation,
            cursor: request.cursor,
            limit: request.limit,
        })
    }

    /// Применить страницу. При `ChannelMismatch` данные не меняются атомарно.
    ///
    /// Успешное применение увеличивает generation, поэтому повтор токена — `StaleToken`.
    pub fn apply_page(
        &mut self,
        token: PageToken,
        page: MessagePage,
    ) -> Result<PageApply, PageError> {
        let Some(channel) = self.channel(token.channel_id) else {
            self.diagnostics.stale_pages += 1;
            return Err(PageError::StaleToken {
                expected: 0,
                got: token.generation,
            });
        };
        let expected = channel.generation;
        if expected != token.generation {
            self.diagnostics.stale_pages += 1;
            return Err(PageError::StaleToken {
                expected,
                got: token.generation,
            });
        }

        for message in &page.messages {
            if message.channel_id != token.channel_id {
                self.diagnostics.channel_mismatches += 1;
                return Err(PageError::ChannelMismatch {
                    expected: token.channel_id,
                    actual: message.channel_id,
                    message_id: message.id,
                });
            }
        }

        let mut duplicates = Vec::new();
        let mut seen = BTreeSet::new();
        for message in &page.messages {
            if !seen.insert(message.id) {
                duplicates.push(message.id);
            }
        }

        // Дедуп до счётчиков: повторный id внутри страницы — last-wins, но не «замена».
        let has_older = page.has_older;
        let has_newer = page.has_newer;
        let mut unique = BTreeMap::new();
        for message in page.messages {
            unique.insert(message.id, message);
        }

        let (inserted, replaced) = {
            let Some(channel) = self.channel_mut(token.channel_id) else {
                // Канал не удаляется публично; ветка защищает от будущих изменений API.
                self.diagnostics.stale_pages += 1;
                return Err(PageError::StaleToken {
                    expected: 0,
                    got: token.generation,
                });
            };
            channel.direction = match token.cursor {
                Cursor::Latest => WindowDirection::Latest,
                Cursor::Before(_) => WindowDirection::Before,
                Cursor::After(_) => WindowDirection::After,
                Cursor::Around(_) => WindowDirection::Around,
            };
            channel.anchor = match token.cursor {
                Cursor::Around(anchor) => Some(anchor),
                _ => None,
            };

            let mut inserted = 0;
            let mut replaced = 0;
            for (id, message) in unique {
                // Live-версия в текущем окне неприкосновенна; страница её не перезаписывает.
                if channel.live_ids.contains(&id) {
                    continue;
                }
                if channel.messages.insert(id, message).is_some() {
                    replaced += 1;
                } else {
                    inserted += 1;
                }
            }
            if let Some(value) = has_older {
                channel.has_older = Some(value);
            }
            if let Some(value) = has_newer {
                channel.has_newer = Some(value);
            }
            channel.generation = token.generation.wrapping_add(1);
            (inserted, replaced)
        };

        let remaining = MAX_DIAGNOSTIC_DUPLICATES.saturating_sub(self.diagnostics.duplicates.len());
        self.diagnostics
            .duplicates
            .extend(duplicates.iter().copied().take(remaining));
        let evicted = self.trim_channel(token.channel_id);
        Ok(PageApply {
            inserted,
            replaced,
            evicted,
            duplicates: duplicates.len(),
        })
    }
}
