//! Search overlay model (P3-09, headless).
//!
//! Контракт — `docs/re/frontend-d03.md` §1: 7 секций фильтров (От, В, Содержит,
//! Упоминания, Дата, Тип автора, Закреплено), 9 значений «Содержит», 3 сортировки,
//! REST `GET /api/v9/guilds/<gid>/messages/search`, панель результатов и группы
//! по каналам. Модуль не ходит в сеть — он собирает параметры запроса и модель
//! результатов.

use serde::{Deserialize, Serialize};

/// Значение фильтра «Содержит» (REST-параметр `has`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SearchContentKind {
    /// Изображение.
    Image,
    /// Видео.
    Video,
    /// Ссылка.
    Link,
    /// Файл.
    File,
    /// Вложение/эмбед.
    Embed,
    /// Аудио.
    Sound,
    /// Опрос.
    Poll,
    /// Стикер.
    Sticker,
    /// Пересланное сообщение.
    Snapshot,
}

impl SearchContentKind {
    /// REST-значение параметра `has`.
    pub fn as_has_value(self) -> &'static str {
        match self {
            SearchContentKind::Image => "image",
            SearchContentKind::Video => "video",
            SearchContentKind::Link => "link",
            SearchContentKind::File => "file",
            SearchContentKind::Embed => "embed",
            SearchContentKind::Sound => "sound",
            SearchContentKind::Poll => "poll",
            SearchContentKind::Sticker => "sticker",
            SearchContentKind::Snapshot => "snapshot",
        }
    }

    /// Все 9 опций в порядке UI.
    pub fn all() -> [SearchContentKind; 9] {
        [
            SearchContentKind::Image,
            SearchContentKind::Video,
            SearchContentKind::Link,
            SearchContentKind::File,
            SearchContentKind::Embed,
            SearchContentKind::Sound,
            SearchContentKind::Poll,
            SearchContentKind::Sticker,
            SearchContentKind::Snapshot,
        ]
    }
}

/// Тип автора (секция «Тип автора»).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SearchAuthorType {
    /// Пользователь.
    User,
    /// Бот.
    Bot,
    /// Вебхук.
    Webhook,
}

impl SearchAuthorType {
    /// REST-значение параметра `author_type`.
    pub fn as_param(self) -> &'static str {
        match self {
            SearchAuthorType::User => "user",
            SearchAuthorType::Bot => "bot",
            SearchAuthorType::Webhook => "webhook",
        }
    }
}

/// Сортировка результатов (3 опции из RE).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SearchSort {
    /// «По новизне» — `sort_by=timestamp&sort_order=desc`.
    Newest,
    /// «Самое старое» — `sort_by=timestamp&sort_order=asc`.
    Oldest,
    /// «Самое актуальное» — `sort_by=relevance`.
    Relevance,
}

impl SearchSort {
    /// Параметры REST для сортировки.
    pub fn params(self) -> Vec<(&'static str, &'static str)> {
        match self {
            SearchSort::Newest => vec![("sort_by", "timestamp"), ("sort_order", "desc")],
            SearchSort::Oldest => vec![("sort_by", "timestamp"), ("sort_order", "asc")],
            SearchSort::Relevance => vec![("sort_by", "relevance")],
        }
    }

    /// Все 3 опции.
    pub fn all() -> [SearchSort; 3] {
        [
            SearchSort::Newest,
            SearchSort::Oldest,
            SearchSort::Relevance,
        ]
    }
}

/// Фильтры поиска (секции панели).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchFilters {
    /// «От» — id автора.
    pub author_id: Option<u64>,
    /// «В» — id канала.
    pub channel_id: Option<u64>,
    /// «Содержит» — набор значений `has`.
    pub contains: std::collections::BTreeSet<SearchContentKind>,
    /// «Упоминания» — id упомянутого пользователя.
    pub mentions: Option<u64>,
    /// «Дата» — не раньше (ms since epoch).
    pub date_after_ms: Option<u64>,
    /// «Дата» — не позже.
    pub date_before_ms: Option<u64>,
    /// «Тип автора».
    pub author_type: Option<SearchAuthorType>,
    /// «Закреплено».
    pub pinned: Option<bool>,
}

impl SearchFilters {
    /// Пустые фильтры.
    pub fn new() -> Self {
        Self::default()
    }

    /// Есть ли хотя бы один активный фильтр.
    pub fn is_empty(&self) -> bool {
        self.author_id.is_none()
            && self.channel_id.is_none()
            && self.contains.is_empty()
            && self.mentions.is_none()
            && self.date_after_ms.is_none()
            && self.date_before_ms.is_none()
            && self.author_type.is_none()
            && self.pinned.is_none()
    }

    /// Сбрасывает все фильтры (кнопка «Очистить фильтры»).
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Переключает значение «Содержит».
    pub fn toggle_contains(&mut self, kind: SearchContentKind) {
        if !self.contains.remove(&kind) {
            self.contains.insert(kind);
        }
    }

    /// Параметры REST для активных фильтров (детерминированный порядок).
    pub fn to_params(&self) -> Vec<(String, String)> {
        let mut params = Vec::new();
        if let Some(id) = self.author_id {
            params.push(("author_id".to_string(), id.to_string()));
        }
        if let Some(id) = self.channel_id {
            params.push(("channel_id".to_string(), id.to_string()));
        }
        for kind in &self.contains {
            params.push(("has".to_string(), kind.as_has_value().to_string()));
        }
        if let Some(id) = self.mentions {
            params.push(("mentions".to_string(), id.to_string()));
        }
        if let Some(ms) = self.date_after_ms {
            params.push(("min_id".to_string(), ms.to_string()));
        }
        if let Some(ms) = self.date_before_ms {
            params.push(("max_id".to_string(), ms.to_string()));
        }
        if let Some(author_type) = self.author_type {
            params.push((
                "author_type".to_string(),
                author_type.as_param().to_string(),
            ));
        }
        if let Some(pinned) = self.pinned {
            params.push(("pinned".to_string(), pinned.to_string()));
        }
        params
    }
}

/// Запрос поиска.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchQuery {
    /// Текст поиска (`content`).
    pub text: String,
    /// Фильтры.
    pub filters: SearchFilters,
    /// Сортировка.
    pub sort: SearchSort,
    /// Смещение страницы.
    pub offset: usize,
    /// Размер страницы (Discord: 25 по умолчанию).
    pub limit: usize,
}

impl Default for SearchQuery {
    fn default() -> Self {
        Self {
            text: String::new(),
            filters: SearchFilters::new(),
            sort: SearchSort::Newest,
            offset: 0,
            limit: 25,
        }
    }
}

impl SearchQuery {
    /// Новый запрос по тексту.
    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
            ..Self::default()
        }
    }

    /// Можно ли выполнять поиск (непустой текст или хотя бы один фильтр).
    pub fn is_executable(&self) -> bool {
        !self.text.trim().is_empty() || !self.filters.is_empty()
    }

    /// Параметры REST (`content`, фильтры, сортировка, offset, limit).
    pub fn to_params(&self) -> Vec<(String, String)> {
        let mut params = Vec::new();
        let text = self.text.trim();
        if !text.is_empty() {
            params.push(("content".to_string(), text.to_string()));
        }
        params.extend(self.filters.to_params());
        for (key, value) in self.sort.params() {
            params.push((key.to_string(), value.to_string()));
        }
        params.push(("offset".to_string(), self.offset.to_string()));
        params.push(("limit".to_string(), self.limit.to_string()));
        params
    }
}

/// Один результат поиска.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResult {
    /// Id сообщения.
    pub message_id: u64,
    /// Id канала.
    pub channel_id: u64,
    /// Имя канала.
    pub channel_name: String,
    /// Id автора.
    pub author_id: u64,
    /// Отображаемое имя автора.
    pub author_name: String,
    /// Текст сообщения.
    pub content: String,
    /// Время отправки (ms since epoch).
    pub timestamp_ms: u64,
    /// Системное сообщение (иконка «Текст» и т.п.).
    pub system: bool,
}

/// Группа результатов по каналу.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResultGroup {
    /// Id канала.
    pub channel_id: u64,
    /// Имя канала.
    pub channel_name: String,
    /// Результаты группы.
    pub results: Vec<SearchResult>,
}

impl SearchResultGroup {
    /// Заголовок группы.
    pub fn title(&self) -> String {
        format!("{} — {}", self.channel_name, self.results.len())
    }
}

/// Состояние оверлея поиска.
#[derive(Debug, Clone)]
pub struct SearchOverlay {
    open: bool,
    query: SearchQuery,
    groups: Vec<SearchResultGroup>,
    total_results: u64,
    loading: bool,
    error: Option<String>,
    selected_index: Option<usize>,
}

impl SearchOverlay {
    /// Создаёт закрытый оверлей.
    pub fn new() -> Self {
        Self {
            open: false,
            query: SearchQuery::default(),
            groups: Vec::new(),
            total_results: 0,
            loading: false,
            error: None,
            selected_index: None,
        }
    }

    /// Открывает оверлей.
    pub fn open(&mut self) {
        self.open = true;
    }

    /// Закрывает оверлей.
    pub fn close(&mut self) {
        self.open = false;
    }

    /// Открыт ли оверлей.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Текущий запрос.
    pub fn query(&self) -> &SearchQuery {
        &self.query
    }

    /// Изменяемый запрос.
    pub fn query_mut(&mut self) -> &mut SearchQuery {
        &mut self.query
    }

    /// Устанавливает текст поиска.
    pub fn set_text(&mut self, text: &str) {
        self.query.text = text.to_string();
    }

    /// Устанавливает сортировку.
    pub fn set_sort(&mut self, sort: SearchSort) {
        self.query.sort = sort;
    }

    /// Переключает значение «Содержит».
    pub fn toggle_contains(&mut self, kind: SearchContentKind) {
        self.query.filters.toggle_contains(kind);
    }

    /// Начинает загрузку (сбрасывает ошибку).
    pub fn begin_loading(&mut self) {
        self.loading = true;
        self.error = None;
    }

    /// Применяет ответ: `total_results` и плоский список результатов.
    pub fn apply_results(&mut self, total_results: u64, results: Vec<SearchResult>) {
        self.loading = false;
        self.error = None;
        self.total_results = total_results;
        self.groups = Self::group_results(results);
        self.selected_index = None;
    }

    /// Применяет ошибку поиска.
    pub fn apply_error(&mut self, message: &str) {
        self.loading = false;
        self.error = Some(message.to_string());
        self.groups.clear();
        self.total_results = 0;
    }

    /// Группирует плоский список по каналам с сохранением порядка первого появления.
    pub fn group_results(results: Vec<SearchResult>) -> Vec<SearchResultGroup> {
        let mut groups: Vec<SearchResultGroup> = Vec::new();
        for result in results {
            if let Some(group) = groups
                .iter_mut()
                .find(|group| group.channel_id == result.channel_id)
            {
                group.results.push(result);
            } else {
                groups.push(SearchResultGroup {
                    channel_id: result.channel_id,
                    channel_name: result.channel_name.clone(),
                    results: vec![result],
                });
            }
        }
        groups
    }

    /// Группы результатов.
    pub fn groups(&self) -> &[SearchResultGroup] {
        &self.groups
    }

    /// Общее число результатов из ответа.
    pub fn total_results(&self) -> u64 {
        self.total_results
    }

    /// Идёт ли загрузка.
    pub fn is_loading(&self) -> bool {
        self.loading
    }

    /// Ошибка поиска.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Количество загруженных строк.
    pub fn loaded_count(&self) -> usize {
        self.groups.iter().map(|group| group.results.len()).sum()
    }

    /// Заголовок «N результатов» по фактическому total.
    pub fn result_heading(&self) -> String {
        format!("{} результатов", self.total_results)
    }

    /// Выбирает строку по индексу (0..loaded_count).
    pub fn select(&mut self, index: usize) {
        if index < self.loaded_count() {
            self.selected_index = Some(index);
        }
    }

    /// Выбранный индекс.
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    /// Перемещает выбор с зажимом в границах.
    pub fn move_selection(&mut self, delta: isize) {
        let total = self.loaded_count();
        if total == 0 {
            return;
        }
        let current = self.selected_index.unwrap_or(0) as isize;
        let next = (current + delta).clamp(0, total as isize - 1);
        self.selected_index = Some(next as usize);
    }

    /// Возвращает выбранный результат (для прыжка к сообщению).
    pub fn selected_result(&self) -> Option<&SearchResult> {
        let index = self.selected_index?;
        self.groups
            .iter()
            .flat_map(|group| group.results.iter())
            .nth(index)
    }

    /// Следующая страница (offset += limit).
    pub fn next_page(&mut self) {
        self.query.offset = self.query.offset.saturating_add(self.query.limit);
    }

    /// Сброс страницы на первую.
    pub fn reset_page(&mut self) {
        self.query.offset = 0;
    }
}

impl Default for SearchOverlay {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(message_id: u64, channel_id: u64, timestamp_ms: u64) -> SearchResult {
        SearchResult {
            message_id,
            channel_id,
            channel_name: format!("channel-{channel_id}"),
            author_id: 1,
            author_name: "author".to_string(),
            content: format!("message {message_id}"),
            timestamp_ms,
            system: false,
        }
    }

    #[test]
    fn test_contains_values_match_rest() {
        assert_eq!(SearchContentKind::Image.as_has_value(), "image");
        assert_eq!(SearchContentKind::Sound.as_has_value(), "sound");
        assert_eq!(SearchContentKind::Snapshot.as_has_value(), "snapshot");
        assert_eq!(SearchContentKind::all().len(), 9);
    }

    #[test]
    fn test_sort_params() {
        assert_eq!(
            SearchSort::Newest.params(),
            vec![("sort_by", "timestamp"), ("sort_order", "desc")]
        );
        assert_eq!(
            SearchSort::Oldest.params(),
            vec![("sort_by", "timestamp"), ("sort_order", "asc")]
        );
        assert_eq!(
            SearchSort::Relevance.params(),
            vec![("sort_by", "relevance")]
        );
        assert_eq!(SearchSort::all().len(), 3);
    }

    #[test]
    fn test_author_type_params() {
        assert_eq!(SearchAuthorType::User.as_param(), "user");
        assert_eq!(SearchAuthorType::Bot.as_param(), "bot");
        assert_eq!(SearchAuthorType::Webhook.as_param(), "webhook");
    }

    #[test]
    fn test_filters_empty_and_clear() {
        let mut filters = SearchFilters::new();
        assert!(filters.is_empty());
        filters.author_id = Some(5);
        assert!(!filters.is_empty());
        filters.clear();
        assert!(filters.is_empty());
    }

    #[test]
    fn test_toggle_contains() {
        let mut filters = SearchFilters::new();
        filters.toggle_contains(SearchContentKind::Image);
        assert!(filters.contains.contains(&SearchContentKind::Image));
        filters.toggle_contains(SearchContentKind::Image);
        assert!(!filters.contains.contains(&SearchContentKind::Image));
    }

    #[test]
    fn test_query_params_full() {
        let mut query = SearchQuery::new("  ProBot  ");
        query.filters.author_id = Some(10);
        query.filters.channel_id = Some(20);
        query.filters.contains.insert(SearchContentKind::Link);
        query.filters.mentions = Some(30);
        query.filters.pinned = Some(true);
        query.filters.author_type = Some(SearchAuthorType::Bot);
        query.sort = SearchSort::Oldest;
        query.offset = 25;
        let params = query.to_params();
        assert!(params.contains(&("content".to_string(), "ProBot".to_string())));
        assert!(params.contains(&("author_id".to_string(), "10".to_string())));
        assert!(params.contains(&("channel_id".to_string(), "20".to_string())));
        assert!(params.contains(&("has".to_string(), "link".to_string())));
        assert!(params.contains(&("mentions".to_string(), "30".to_string())));
        assert!(params.contains(&("pinned".to_string(), "true".to_string())));
        assert!(params.contains(&("author_type".to_string(), "bot".to_string())));
        assert!(params.contains(&("sort_order".to_string(), "asc".to_string())));
        assert!(params.contains(&("offset".to_string(), "25".to_string())));
        assert!(params.contains(&("limit".to_string(), "25".to_string())));
    }

    #[test]
    fn test_query_params_skips_empty_text() {
        let query = SearchQuery::default();
        let params = query.to_params();
        assert!(!params.iter().any(|(key, _)| key == "content"));
    }

    #[test]
    fn test_is_executable() {
        assert!(!SearchQuery::default().is_executable());
        assert!(!SearchQuery::new("   ").is_executable());
        assert!(SearchQuery::new("x").is_executable());
        let mut query = SearchQuery::default();
        query.filters.pinned = Some(true);
        assert!(query.is_executable());
    }

    #[test]
    fn test_group_results_preserves_order() {
        let groups = SearchOverlay::group_results(vec![
            result(1, 10, 100),
            result(2, 20, 101),
            result(3, 10, 102),
        ]);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].channel_id, 10);
        assert_eq!(groups[0].results.len(), 2);
        assert_eq!(groups[1].channel_id, 20);
        assert_eq!(groups[0].title(), "channel-10 — 2");
    }

    #[test]
    fn test_overlay_open_close_and_results() {
        let mut overlay = SearchOverlay::new();
        assert!(!overlay.is_open());
        overlay.open();
        assert!(overlay.is_open());
        overlay.begin_loading();
        assert!(overlay.is_loading());
        overlay.apply_results(3302, vec![result(1, 10, 100), result(2, 10, 101)]);
        assert!(!overlay.is_loading());
        assert_eq!(overlay.total_results(), 3302);
        assert_eq!(overlay.loaded_count(), 2);
        assert_eq!(overlay.result_heading(), "3302 результатов");
        overlay.close();
        assert!(!overlay.is_open());
    }

    #[test]
    fn test_apply_error_clears() {
        let mut overlay = SearchOverlay::new();
        overlay.apply_results(5, vec![result(1, 10, 100)]);
        overlay.apply_error("Сеть недоступна");
        assert_eq!(overlay.error(), Some("Сеть недоступна"));
        assert_eq!(overlay.total_results(), 0);
        assert_eq!(overlay.loaded_count(), 0);
    }

    #[test]
    fn test_selection_clamped_and_returns_result() {
        let mut overlay = SearchOverlay::new();
        overlay.apply_results(2, vec![result(1, 10, 100), result(2, 10, 101)]);
        overlay.select(5);
        assert_eq!(overlay.selected_index(), None);
        overlay.select(1);
        assert_eq!(overlay.selected_result().unwrap().message_id, 2);
        overlay.move_selection(10);
        assert_eq!(overlay.selected_index(), Some(1));
        overlay.move_selection(-10);
        assert_eq!(overlay.selected_index(), Some(0));
    }

    #[test]
    fn test_move_selection_empty_noop() {
        let mut overlay = SearchOverlay::new();
        overlay.move_selection(1);
        assert_eq!(overlay.selected_index(), None);
    }

    #[test]
    fn test_pagination() {
        let mut overlay = SearchOverlay::new();
        overlay.query_mut().limit = 25;
        overlay.next_page();
        assert_eq!(overlay.query().offset, 25);
        overlay.next_page();
        assert_eq!(overlay.query().offset, 50);
        overlay.reset_page();
        assert_eq!(overlay.query().offset, 0);
    }
}
