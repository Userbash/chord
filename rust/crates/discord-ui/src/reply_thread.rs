//! Replies and threads model (P3-07, headless).
//!
//! Контракт — `docs/rust-client/PLAN.md` P3-07: reply-цитаты, панель треда,
//! `THREAD_LIST_SYNC`. Модель не ходит в сеть и не рисует UI.

use serde::{Deserialize, Serialize};

/// Ссылка на сообщение, на которое отвечают (`message_reference`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageReference {
    /// Id исходного сообщения.
    pub message_id: u64,
    /// Id канала исходного сообщения.
    pub channel_id: u64,
    /// Id гильдии, если сообщение в гильдии.
    pub guild_id: Option<u64>,
    /// Падать ли отправке, если исходное сообщение удалено.
    pub fail_if_not_exists: bool,
}

/// Цитата ответа в ленте (reply-превью).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplyQuote {
    /// Автор исходного сообщения.
    pub author_id: u64,
    /// Имя автора.
    pub author_name: String,
    /// Превью текста (уже обрезанное).
    pub content_preview: String,
    /// Количество вложений в исходном сообщении.
    pub attachment_count: usize,
    /// Исходное сообщение удалено.
    pub deleted: bool,
    /// Исходное сообщение содержит стикер.
    pub has_sticker: bool,
}

impl ReplyQuote {
    /// Строка «Ответить <author>» либо «Исходное сообщение удалено».
    pub fn label(&self) -> String {
        if self.deleted {
            "Исходное сообщение удалено".to_string()
        } else {
            format!("Ответить {}", self.author_name)
        }
    }

    /// Превью с учётом вложений и стикеров.
    pub fn preview(&self) -> String {
        if self.deleted {
            return String::new();
        }
        if !self.content_preview.is_empty() {
            return self.content_preview.clone();
        }
        if self.attachment_count > 0 {
            return format!("Вложение ({})", self.attachment_count);
        }
        if self.has_sticker {
            return "Стикер".to_string();
        }
        String::new()
    }
}

/// Метаданные треда (`thread_metadata`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreadMetadata {
    /// Архивный ли тред.
    pub archived: bool,
    /// Автоархивация в минутах (60/1440/4320/10080).
    pub auto_archive_duration: u32,
    /// Время архивации (ms since epoch).
    pub archive_timestamp_ms: u64,
    /// Заблокирован ли тред.
    pub locked: bool,
    /// Можно ли приглашать.
    pub invitable: bool,
}

impl ThreadMetadata {
    /// Допустимые значения автоархивации.
    pub const ALLOWED_DURATIONS: [u32; 4] = [60, 1440, 4320, 10080];

    /// Проверяет и создаёт метаданные.
    pub fn new(
        archived: bool,
        auto_archive_duration: u32,
        archive_timestamp_ms: u64,
        locked: bool,
        invitable: bool,
    ) -> Result<Self, String> {
        if !Self::ALLOWED_DURATIONS.contains(&auto_archive_duration) {
            return Err(format!(
                "Недопустимая длительность автоархивации: {auto_archive_duration}"
            ));
        }
        Ok(Self {
            archived,
            auto_archive_duration,
            archive_timestamp_ms,
            locked,
            invitable,
        })
    }
}

/// Участник треда (`thread_member`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreadMember {
    /// Id пользователя.
    pub user_id: u64,
    /// Время присоединения (ms since epoch).
    pub join_timestamp_ms: u64,
    /// Флаги участника.
    pub flags: u64,
}

/// Тред в модели.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Thread {
    /// Id треда (он же id канала).
    pub id: u64,
    /// Id родительского канала.
    pub parent_id: u64,
    /// Id гильдии.
    pub guild_id: Option<u64>,
    /// Имя треда.
    pub name: String,
    /// Метаданные.
    pub metadata: ThreadMetadata,
    /// Количество сообщений.
    pub message_count: u32,
    /// Количество участников.
    pub member_count: u32,
}

/// Событие `THREAD_LIST_SYNC`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreadListSync {
    /// Id гильдии.
    pub guild_id: u64,
    /// Каналы, к которым относится синхронизация.
    pub channel_ids: Vec<u64>,
    /// Активные треды.
    pub threads: Vec<Thread>,
    /// Участники (для текущего пользователя).
    pub members: Vec<ThreadMember>,
}

/// Панель треда.
#[derive(Debug, Clone)]
pub struct ThreadPanel {
    open: bool,
    current: Option<u64>,
}

impl ThreadPanel {
    /// Создаёт закрытую панель.
    pub fn new() -> Self {
        Self {
            open: false,
            current: None,
        }
    }

    /// Открывает тред.
    pub fn open(&mut self, thread_id: u64) {
        self.open = true;
        self.current = Some(thread_id);
    }

    /// Закрывает панель.
    pub fn close(&mut self) {
        self.open = false;
        self.current = None;
    }

    /// Открыта ли панель.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Текущий тред.
    pub fn current(&self) -> Option<u64> {
        self.current
    }

    /// Переключается на другой тред, если панель открыта.
    pub fn switch(&mut self, thread_id: u64) {
        if self.open {
            self.current = Some(thread_id);
        }
    }
}

impl Default for ThreadPanel {
    fn default() -> Self {
        Self::new()
    }
}

/// Сервис тредов: хранит треды по id и применяет `THREAD_LIST_SYNC`.
#[derive(Debug, Clone, Default)]
pub struct ThreadService {
    threads: std::collections::BTreeMap<u64, Thread>,
    panel: ThreadPanel,
}

impl ThreadService {
    /// Пустой сервис.
    pub fn new() -> Self {
        Self::default()
    }

    /// Применяет `THREAD_LIST_SYNC`: треды чужих каналов не затрагиваются.
    pub fn apply_sync(&mut self, sync: &ThreadListSync) {
        for thread in &sync.threads {
            if sync.channel_ids.contains(&thread.parent_id) {
                self.threads.insert(thread.id, thread.clone());
            }
        }
        // Убираем из модели треды этих каналов, которых больше нет в списке.
        let incoming: std::collections::BTreeSet<u64> =
            sync.threads.iter().map(|thread| thread.id).collect();
        let channels = &sync.channel_ids;
        let stale: Vec<u64> = self
            .threads
            .values()
            .filter(|thread| channels.contains(&thread.parent_id) && !incoming.contains(&thread.id))
            .map(|thread| thread.id)
            .collect();
        for id in stale {
            self.threads.remove(&id);
        }
    }

    /// Добавляет/обновляет тред.
    pub fn upsert(&mut self, thread: Thread) {
        self.threads.insert(thread.id, thread);
    }

    /// Получает тред по id.
    pub fn get(&self, id: u64) -> Option<&Thread> {
        self.threads.get(&id)
    }

    /// Все треды канала.
    pub fn by_parent(&self, parent_id: u64) -> Vec<&Thread> {
        self.threads
            .values()
            .filter(|thread| thread.parent_id == parent_id)
            .collect()
    }

    /// Количество тредов.
    pub fn len(&self) -> usize {
        self.threads.len()
    }

    /// Пусто ли хранилище.
    pub fn is_empty(&self) -> bool {
        self.threads.is_empty()
    }

    /// Панель треда.
    pub fn panel(&self) -> &ThreadPanel {
        &self.panel
    }

    /// Изменяемая панель.
    pub fn panel_mut(&mut self) -> &mut ThreadPanel {
        &mut self.panel
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata() -> ThreadMetadata {
        ThreadMetadata::new(false, 1440, 1_700_000_000_000, false, true).unwrap()
    }

    fn thread(id: u64, parent_id: u64) -> Thread {
        Thread {
            id,
            parent_id,
            guild_id: Some(1),
            name: format!("thread-{id}"),
            metadata: metadata(),
            message_count: 0,
            member_count: 1,
        }
    }

    #[test]
    fn test_reply_quote_label_alive() {
        let quote = ReplyQuote {
            author_id: 1,
            author_name: "Alice".to_string(),
            content_preview: "привет".to_string(),
            attachment_count: 0,
            deleted: false,
            has_sticker: false,
        };
        assert_eq!(quote.label(), "Ответить Alice");
        assert_eq!(quote.preview(), "привет");
    }

    #[test]
    fn test_reply_quote_deleted() {
        let quote = ReplyQuote {
            author_id: 1,
            author_name: "Alice".to_string(),
            content_preview: "привет".to_string(),
            attachment_count: 0,
            deleted: true,
            has_sticker: false,
        };
        assert_eq!(quote.label(), "Исходное сообщение удалено");
        assert!(quote.preview().is_empty());
    }

    #[test]
    fn test_reply_quote_attachment_and_sticker_fallbacks() {
        let mut quote = ReplyQuote {
            author_id: 1,
            author_name: "A".to_string(),
            content_preview: String::new(),
            attachment_count: 2,
            deleted: false,
            has_sticker: false,
        };
        assert_eq!(quote.preview(), "Вложение (2)");
        quote.attachment_count = 0;
        quote.has_sticker = true;
        assert_eq!(quote.preview(), "Стикер");
    }

    #[test]
    fn test_message_reference_fields() {
        let reference = MessageReference {
            message_id: 10,
            channel_id: 20,
            guild_id: Some(30),
            fail_if_not_exists: true,
        };
        assert_eq!(reference.message_id, 10);
        assert!(reference.fail_if_not_exists);
    }

    #[test]
    fn test_thread_metadata_duration_validation() {
        assert!(ThreadMetadata::new(false, 60, 0, false, true).is_ok());
        assert!(ThreadMetadata::new(false, 1440, 0, false, true).is_ok());
        assert!(ThreadMetadata::new(false, 4320, 0, false, true).is_ok());
        assert!(ThreadMetadata::new(false, 10080, 0, false, true).is_ok());
        assert!(ThreadMetadata::new(false, 90, 0, false, true).is_err());
    }

    #[test]
    fn test_panel_open_close_switch() {
        let mut panel = ThreadPanel::new();
        assert!(!panel.is_open());
        panel.open(5);
        assert!(panel.is_open());
        assert_eq!(panel.current(), Some(5));
        panel.switch(6);
        assert_eq!(panel.current(), Some(6));
        panel.close();
        assert!(!panel.is_open());
        assert_eq!(panel.current(), None);
        panel.switch(7);
        assert_eq!(panel.current(), None);
    }

    #[test]
    fn test_apply_sync_only_touches_listed_channels() {
        let mut service = ThreadService::new();
        service.upsert(thread(1, 100));
        service.upsert(thread(2, 200));

        let sync = ThreadListSync {
            guild_id: 1,
            channel_ids: vec![100],
            threads: vec![thread(1, 100), thread(3, 100)],
            members: vec![],
        };
        service.apply_sync(&sync);
        assert!(service.get(1).is_some());
        assert!(service.get(3).is_some());
        assert!(service.get(2).is_some(), "чужой канал не затрагивается");
    }

    #[test]
    fn test_apply_sync_removes_stale_threads_of_listed_channel() {
        let mut service = ThreadService::new();
        service.upsert(thread(1, 100));
        service.upsert(thread(2, 100));
        let sync = ThreadListSync {
            guild_id: 1,
            channel_ids: vec![100],
            threads: vec![thread(1, 100)],
            members: vec![],
        };
        service.apply_sync(&sync);
        assert!(service.get(1).is_some());
        assert!(service.get(2).is_none(), "устаревший тред удалён");
    }

    #[test]
    fn test_sync_ignores_thread_with_unlisted_parent() {
        let mut service = ThreadService::new();
        let sync = ThreadListSync {
            guild_id: 1,
            channel_ids: vec![100],
            threads: vec![thread(9, 999)],
            members: vec![],
        };
        service.apply_sync(&sync);
        assert!(service.is_empty());
    }

    #[test]
    fn test_by_parent_and_len() {
        let mut service = ThreadService::new();
        service.upsert(thread(1, 100));
        service.upsert(thread(2, 100));
        service.upsert(thread(3, 200));
        assert_eq!(service.len(), 3);
        assert_eq!(service.by_parent(100).len(), 2);
        assert_eq!(service.by_parent(200).len(), 1);
        assert_eq!(service.by_parent(300).len(), 0);
    }
}
