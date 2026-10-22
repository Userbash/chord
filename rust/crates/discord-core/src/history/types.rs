//! P2-06a: конфиг, ограниченное окно и live-слияние (RW-14 / W14a).
//!
//! Контракт — `waves/RW-14/DESIGN-P2-06.md` §3. История — отдельный memory-only кэш;
//! `State.messages` не меняется. Никакого I/O, сети, SQLite или UI.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::state::Message;

/// Локальные границы истории. Числа — guard-политика, не подтверждённые лимиты Discord.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistoryConfig {
    /// Максимум сообщений на канал.
    pub channel_window: usize,
    /// Максимальный/локальный лимит страницы.
    pub page_limit: u16,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            channel_window: 100,
            page_limit: 50,
        }
    }
}

/// Ошибка создания истории.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryError {
    /// `channel_window == 0`.
    ZeroWindow,
    /// `page_limit == 0`.
    ZeroPageLimit,
}

impl fmt::Display for HistoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroWindow => formatter.write_str("history: channel_window must be > 0"),
            Self::ZeroPageLimit => formatter.write_str("history: page_limit must be > 0"),
        }
    }
}

impl std::error::Error for HistoryError {}

/// Результат live-вставки.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveOutcome {
    /// Новое сообщение вставлено (возможно, с вытеснением).
    Inserted,
    /// Существующее сообщение заменено live-версией.
    Replaced,
    /// Сообщение новее окна при прокрутке вверх; окно не менялось.
    Deferred,
}

/// Диагностика истории. Содержимое сообщений сюда не попадает.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HistoryDiagnostics {
    /// Повторные id внутри одной страницы (last-wins).
    pub duplicates: Vec<u64>,
    /// Всего вытеснено сообщений.
    pub evicted: usize,
    /// Отклонённые устаревшие токены страниц.
    pub stale_pages: usize,
    /// Страницы с чужим `channel_id`.
    pub channel_mismatches: usize,
    /// Live-сообщения, отложенные при прокрутке вверх.
    pub deferred_live: usize,
}

/// Направление окна, заданное последней страницей.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum WindowDirection {
    #[default]
    Latest,
    Before,
    After,
    Around,
}

/// Внутреннее состояние одного канала.
#[derive(Debug, Clone, Default)]
pub(crate) struct ChannelHistory {
    pub(crate) messages: BTreeMap<u64, Message>,
    pub(crate) has_older: Option<bool>,
    pub(crate) has_newer: Option<bool>,
    pub(crate) direction: WindowDirection,
    pub(crate) anchor: Option<u64>,
    pub(crate) generation: u64,
    /// id, пришедшие live и находящиеся в текущем окне: страница не может их перезаписать.
    pub(crate) live_ids: BTreeSet<u64>,
}

/// Ограниченная история сообщений по каналам.
#[derive(Default)]
pub struct History {
    pub(crate) config: HistoryConfig,
    pub(crate) channels: BTreeMap<u64, ChannelHistory>,
    pub(crate) diagnostics: HistoryDiagnostics,
}

impl fmt::Debug for History {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("History")
            .field("channel_window", &self.config.channel_window)
            .field("page_limit", &self.config.page_limit)
            .field("channels", &self.channels.len())
            .field(
                "messages",
                &self
                    .channels
                    .values()
                    .map(|channel| channel.messages.len())
                    .sum::<usize>(),
            )
            .field("diagnostics", &self.diagnostics)
            .finish()
    }
}

impl History {
    /// Создать историю с проверенной конфигурацией.
    pub fn new(config: HistoryConfig) -> Result<Self, HistoryError> {
        if config.channel_window == 0 {
            return Err(HistoryError::ZeroWindow);
        }
        if config.page_limit == 0 {
            return Err(HistoryError::ZeroPageLimit);
        }
        Ok(Self {
            config,
            channels: BTreeMap::new(),
            diagnostics: HistoryDiagnostics::default(),
        })
    }

    /// Конфигурация.
    #[must_use]
    pub fn config(&self) -> HistoryConfig {
        self.config
    }

    /// Диагностика.
    #[must_use]
    pub fn diagnostics(&self) -> &HistoryDiagnostics {
        &self.diagnostics
    }

    /// Применить полное сообщение из live-события (`MESSAGE_CREATE`/`MESSAGE_UPDATE`).
    ///
    /// Live-версия всегда важнее страницы. При прокрутке вверх (`Before`/`Around`) новое
    /// сообщение не вытесняет старые: оно откладывается, `has_newer` становится `Some(true)`.
    pub fn apply_live(&mut self, message: Message) -> LiveOutcome {
        let channel_id = message.channel_id;
        let message_id = message.id;
        let mut deferred = false;

        {
            let channel = self.channels.entry(channel_id).or_default();
            if let Some(existing) = channel.messages.get_mut(&message_id) {
                *existing = message;
                channel.live_ids.insert(message_id);
                return LiveOutcome::Replaced;
            }

            let newest = channel.messages.keys().next_back().copied();
            let detached = matches!(
                channel.direction,
                WindowDirection::Before | WindowDirection::Around
            );
            if detached && newest.is_some_and(|newest| message_id > newest) {
                channel.has_newer = Some(true);
                deferred = true;
            } else {
                channel.messages.insert(message_id, message);
                channel.live_ids.insert(message_id);
                if matches!(
                    channel.direction,
                    WindowDirection::Latest | WindowDirection::After
                ) && newest.is_none_or(|newest| message_id > newest)
                {
                    channel.has_newer = Some(false);
                }
            }
        }

        if deferred {
            self.diagnostics.deferred_live += 1;
            return LiveOutcome::Deferred;
        }
        self.trim_channel(channel_id);
        LiveOutcome::Inserted
    }

    /// Канал, если он уже создан (read-only).
    #[must_use]
    pub(crate) fn channel(&self, channel_id: u64) -> Option<&ChannelHistory> {
        self.channels.get(&channel_id)
    }

    /// Канал, если он уже создан (mutable).
    pub(crate) fn channel_mut(&mut self, channel_id: u64) -> Option<&mut ChannelHistory> {
        self.channels.get_mut(&channel_id)
    }

    /// Создать канал при необходимости.
    pub(crate) fn ensure_channel(&mut self, channel_id: u64) -> &mut ChannelHistory {
        self.channels.entry(channel_id).or_default()
    }

    /// Обрезать окно канала по его направлению. Возвращает число вытесненных.
    pub(crate) fn trim_channel(&mut self, channel_id: u64) -> usize {
        let window = self.config.channel_window;
        let removed_ids = {
            let Some(channel) = self.channels.get_mut(&channel_id) else {
                return 0;
            };
            if channel.messages.len() <= window {
                return 0;
            }
            let keep = keep_ids(channel, window);
            let mut removed = Vec::new();
            channel.messages.retain(|id, _| {
                let keep_message = keep.contains(id);
                if !keep_message {
                    removed.push(*id);
                }
                keep_message
            });
            for id in &removed {
                channel.live_ids.remove(id);
            }
            removed
        };
        let removed = removed_ids.len();
        self.diagnostics.evicted += removed;
        removed
    }
}

fn keep_ids(channel: &ChannelHistory, window: usize) -> BTreeSet<u64> {
    match channel.direction {
        WindowDirection::Latest | WindowDirection::After => channel
            .messages
            .keys()
            .rev()
            .take(window)
            .copied()
            .collect(),
        WindowDirection::Before => channel.messages.keys().take(window).copied().collect(),
        WindowDirection::Around => {
            let anchor = channel.anchor.unwrap_or(0);
            let mut ids: Vec<u64> = channel.messages.keys().copied().collect();
            ids.sort_by_key(|id| (id.abs_diff(anchor), *id));
            ids.truncate(window);
            ids.into_iter().collect()
        }
    }
}
