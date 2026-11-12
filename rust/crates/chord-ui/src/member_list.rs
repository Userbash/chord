//! Member list model (P3-08, headless).
//!
//! Данные контракта — `docs/re/frontend-d03.md` §3: правый сайдбар участников,
//! группы «Server Booster — 2», «В сети — 735», строки member/name/subText/tag,
//! виртуализация списка (`scroller_affa7e` + `listItems_affa7e`).
//! Модуль не рисует UI: это чистая модель групп, фильтра и окна виртуализации.

use serde::{Deserialize, Serialize};

/// Присутствие участника.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PresenceStatus {
    /// В сети.
    Online,
    /// Неактивен.
    Idle,
    /// Не беспокоить.
    Dnd,
    /// Не в сети.
    Offline,
    /// Невидимка (в списке выглядит как offline).
    Invisible,
}

impl PresenceStatus {
    /// Сортировочный ранг для групп (online/idle/dnd выше offline).
    pub fn rank(self) -> u8 {
        match self {
            PresenceStatus::Online => 0,
            PresenceStatus::Idle => 1,
            PresenceStatus::Dnd => 2,
            PresenceStatus::Offline => 3,
            PresenceStatus::Invisible => 4,
        }
    }

    /// Входит ли статус в группу «В сети».
    pub fn is_online_group(self) -> bool {
        !matches!(self, PresenceStatus::Offline | PresenceStatus::Invisible)
    }
}

/// Вид группы участников.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemberGroupKind {
    /// Роль гильдии (`Server Booster`, `Администратор`, ...).
    Role {
        /// Идентификатор роли.
        role_id: u64,
        /// Отображаемое имя роли.
        name: String,
    },
    /// Все присутствующие вне ролевых групп.
    Online,
    /// Все отсутствующие.
    Offline,
}

impl MemberGroupKind {
    /// Стабильный ключ группы.
    pub fn key(&self) -> String {
        match self {
            MemberGroupKind::Role { role_id, .. } => format!("role:{role_id}"),
            MemberGroupKind::Online => "online".to_string(),
            MemberGroupKind::Offline => "offline".to_string(),
        }
    }
}

/// Строка списка участников.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemberRow {
    /// Идентификатор пользователя.
    pub user_id: u64,
    /// Ник на сервере (может отсутствовать).
    pub display_name: Option<String>,
    /// Глобальное имя пользователя (tag).
    pub username: String,
    /// Статус.
    pub status: PresenceStatus,
    /// Ссылка на аватар (уже разрешённая), если есть.
    pub avatar_url: Option<String>,
    /// Цвет роли (CSS-цвет), если есть.
    pub role_color: Option<String>,
    /// Дополнительная активность (`subText__91a9d`).
    pub activity: Option<String>,
}

impl MemberRow {
    /// Имя для отображения: display_name, иначе username.
    pub fn name(&self) -> &str {
        self.display_name.as_deref().unwrap_or(&self.username)
    }

    /// Совпадает ли строка с поисковым запросом (регистронезависимо).
    pub fn matches(&self, query: &str) -> bool {
        if query.is_empty() {
            return true;
        }
        let needle = query.to_lowercase();
        self.name().to_lowercase().contains(&needle)
            || self.username.to_lowercase().contains(&needle)
    }
}

/// Группа участников с заголовком и количеством.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemberGroup {
    /// Вид группы.
    pub kind: MemberGroupKind,
    /// Участники группы в порядке отображения.
    pub members: Vec<MemberRow>,
}

impl MemberGroup {
    /// Заголовок группы вида «В сети — 735» / «Server Booster — 2».
    pub fn title(&self) -> String {
        let base = match &self.kind {
            MemberGroupKind::Role { name, .. } => name.clone(),
            MemberGroupKind::Online => "В сети".to_string(),
            MemberGroupKind::Offline => "Не в сети".to_string(),
        };
        format!("{base} — {}", self.members.len())
    }
}

/// Окно виртуализации списка фиксированной высоты строки.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtualWindow {
    /// Смещение прокрутки в пикселях.
    pub scroll_top: u32,
    /// Высота видимой области в пикселях.
    pub viewport_height: u32,
    /// Высота одной строки в пикселях.
    pub row_height: u32,
}

impl VirtualWindow {
    /// Создаёт окно с проверкой ненулевой высоты строки.
    pub fn new(scroll_top: u32, viewport_height: u32, row_height: u32) -> Result<Self, String> {
        if row_height == 0 {
            return Err("Высота строки должна быть больше нуля".to_string());
        }
        Ok(Self {
            scroll_top,
            viewport_height,
            row_height,
        })
    }

    /// Диапазон индексов видимых строк (полуоткрытый, с запасом в одну строку).
    pub fn visible_range(&self, total: usize) -> std::ops::Range<usize> {
        if total == 0 {
            return 0..0;
        }
        let first = (self.scroll_top / self.row_height) as usize;
        let visible = (self.viewport_height / self.row_height) as usize;
        let last = (first + visible + 1).min(total);
        first.min(total)..last
    }

    /// Полная высота контента.
    pub fn total_height(&self, total: usize) -> u32 {
        self.row_height.saturating_mul(total as u32)
    }

    /// Максимально допустимое смещение прокрутки.
    pub fn max_scroll(&self, total: usize) -> u32 {
        self.total_height(total)
            .saturating_sub(self.viewport_height)
    }

    /// Прокрутка с ограничением.
    pub fn scroll_by(&mut self, delta: i32, total: usize) {
        let next = self.scroll_top as i64 + delta as i64;
        let clamped = next.clamp(0, self.max_scroll(total) as i64);
        self.scroll_top = clamped as u32;
    }
}

/// Модель правого списка участников.
#[derive(Debug, Clone)]
pub struct MemberListModel {
    groups: Vec<MemberGroup>,
    filter: String,
    collapsed: std::collections::BTreeSet<String>,
    window: VirtualWindow,
}

impl MemberListModel {
    /// Создаёт модель; группы сортируются: роли по порядку входа, затем Online, Offline.
    pub fn new(groups: Vec<MemberGroup>) -> Self {
        Self {
            groups,
            filter: String::new(),
            collapsed: std::collections::BTreeSet::new(),
            window: VirtualWindow {
                scroll_top: 0,
                viewport_height: 600,
                row_height: 40,
            },
        }
    }

    /// Создаёт модель из плоского списка участников (группировка по статусу).
    pub fn from_members(members: Vec<MemberRow>) -> Self {
        let mut online = Vec::new();
        let mut offline = Vec::new();
        for member in members {
            if member.status.is_online_group() {
                online.push(member);
            } else {
                offline.push(member);
            }
        }
        online.sort_by(|a, b| {
            a.status
                .rank()
                .cmp(&b.status.rank())
                .then_with(|| a.name().to_lowercase().cmp(&b.name().to_lowercase()))
        });
        offline.sort_by_key(|member| member.name().to_lowercase());
        let mut groups = Vec::new();
        if !online.is_empty() {
            groups.push(MemberGroup {
                kind: MemberGroupKind::Online,
                members: online,
            });
        }
        if !offline.is_empty() {
            groups.push(MemberGroup {
                kind: MemberGroupKind::Offline,
                members: offline,
            });
        }
        Self::new(groups)
    }

    /// Все группы.
    pub fn groups(&self) -> &[MemberGroup] {
        &self.groups
    }

    /// Устанавливает фильтр поиска по имени/тегу.
    pub fn set_filter(&mut self, query: &str) {
        self.filter = query.to_string();
        self.window.scroll_top = 0;
    }

    /// Текущий фильтр.
    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// Свёрнута ли группа.
    pub fn is_collapsed(&self, kind: &MemberGroupKind) -> bool {
        self.collapsed.contains(&kind.key())
    }

    /// Сворачивает/разворачивает группу.
    pub fn toggle_collapsed(&mut self, kind: &MemberGroupKind) {
        let key = kind.key();
        if !self.collapsed.remove(&key) {
            self.collapsed.insert(key);
        }
    }

    /// Видимые строки с учётом фильтра и свёрнутых групп (плоский список).
    pub fn visible_rows(&self) -> Vec<&MemberRow> {
        let mut rows = Vec::new();
        for group in &self.groups {
            if !self.filter.is_empty() {
                for member in &group.members {
                    if member.matches(&self.filter) {
                        rows.push(member);
                    }
                }
                continue;
            }
            if self.is_collapsed(&group.kind) {
                continue;
            }
            rows.extend(group.members.iter());
        }
        rows
    }

    /// Заголовки, которые видны при текущем фильтре: при активном поиске заголовков нет.
    pub fn visible_group_headers(&self) -> Vec<(&MemberGroupKind, usize)> {
        if !self.filter.is_empty() {
            return Vec::new();
        }
        self.groups
            .iter()
            .filter(|group| !self.is_collapsed(&group.kind) && !group.members.is_empty())
            .map(|group| (&group.kind, group.members.len()))
            .collect()
    }

    /// Общее количество строк после фильтра.
    pub fn visible_count(&self) -> usize {
        self.visible_rows().len()
    }

    /// Окно виртуализации.
    pub fn window(&self) -> &VirtualWindow {
        &self.window
    }

    /// Изменяет окно виртуализации.
    pub fn set_window(&mut self, window: VirtualWindow) {
        self.window = window;
    }

    /// Индексы видимых строк для рендера.
    pub fn visible_range(&self) -> std::ops::Range<usize> {
        self.window.visible_range(self.visible_count())
    }

    /// Прокрутка списка.
    pub fn scroll_by(&mut self, delta: i32) {
        let total = self.visible_count();
        self.window.scroll_by(delta, total);
    }

    /// Находит участника по id.
    pub fn find_member(&self, user_id: u64) -> Option<&MemberRow> {
        self.groups
            .iter()
            .flat_map(|group| group.members.iter())
            .find(|member| member.user_id == user_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(id: u64, name: &str, status: PresenceStatus) -> MemberRow {
        MemberRow {
            user_id: id,
            display_name: Some(name.to_string()),
            username: name.to_lowercase(),
            status,
            avatar_url: None,
            role_color: None,
            activity: None,
        }
    }

    #[test]
    fn test_presence_rank_order() {
        assert!(PresenceStatus::Online.rank() < PresenceStatus::Idle.rank());
        assert!(PresenceStatus::Idle.rank() < PresenceStatus::Dnd.rank());
        assert!(PresenceStatus::Dnd.rank() < PresenceStatus::Offline.rank());
    }

    #[test]
    fn test_is_online_group() {
        assert!(PresenceStatus::Online.is_online_group());
        assert!(PresenceStatus::Idle.is_online_group());
        assert!(PresenceStatus::Dnd.is_online_group());
        assert!(!PresenceStatus::Offline.is_online_group());
        assert!(!PresenceStatus::Invisible.is_online_group());
    }

    #[test]
    fn test_group_title_matches_re() {
        let group = MemberGroup {
            kind: MemberGroupKind::Online,
            members: vec![member(1, "A", PresenceStatus::Online)],
        };
        assert_eq!(group.title(), "В сети — 1");

        let role = MemberGroup {
            kind: MemberGroupKind::Role {
                role_id: 7,
                name: "Server Booster".to_string(),
            },
            members: vec![
                member(1, "A", PresenceStatus::Online),
                member(2, "B", PresenceStatus::Online),
            ],
        };
        assert_eq!(role.title(), "Server Booster — 2");
    }

    #[test]
    fn test_group_key_stable() {
        assert_eq!(
            MemberGroupKind::Role {
                role_id: 42,
                name: "X".to_string()
            }
            .key(),
            "role:42"
        );
        assert_eq!(MemberGroupKind::Online.key(), "online");
        assert_eq!(MemberGroupKind::Offline.key(), "offline");
    }

    #[test]
    fn test_from_members_groups_and_sorts() {
        let model = MemberListModel::from_members(vec![
            member(1, "Zed", PresenceStatus::Offline),
            member(2, "bob", PresenceStatus::Online),
            member(3, "Alice", PresenceStatus::Online),
        ]);
        assert_eq!(model.groups().len(), 2);
        assert_eq!(model.groups()[0].kind, MemberGroupKind::Online);
        assert_eq!(
            model.groups()[0].members[0].display_name.as_deref(),
            Some("Alice")
        );
        assert_eq!(model.groups()[1].kind, MemberGroupKind::Offline);
    }

    #[test]
    fn test_filter_matches_name_and_username() {
        let model = MemberListModel::from_members(vec![
            member(1, "Alice", PresenceStatus::Online),
            member(2, "Bob", PresenceStatus::Offline),
        ]);
        let mut model = model;
        model.set_filter("ali");
        assert_eq!(model.visible_count(), 1);
        assert_eq!(model.visible_rows()[0].user_id, 1);
        model.set_filter("BOB");
        assert_eq!(model.visible_count(), 1);
        assert_eq!(model.visible_rows()[0].user_id, 2);
        model.set_filter("zzz");
        assert_eq!(model.visible_count(), 0);
    }

    #[test]
    fn test_filter_suppresses_group_headers() {
        let mut model =
            MemberListModel::from_members(vec![member(1, "Alice", PresenceStatus::Online)]);
        assert_eq!(model.visible_group_headers().len(), 1);
        model.set_filter("a");
        assert!(model.visible_group_headers().is_empty());
    }

    #[test]
    fn test_collapse_hides_group_rows() {
        let mut model = MemberListModel::from_members(vec![
            member(1, "Alice", PresenceStatus::Online),
            member(2, "Bob", PresenceStatus::Offline),
        ]);
        assert_eq!(model.visible_count(), 2);
        model.toggle_collapsed(&MemberGroupKind::Online);
        assert!(model.is_collapsed(&MemberGroupKind::Online));
        assert_eq!(model.visible_count(), 1);
        model.toggle_collapsed(&MemberGroupKind::Online);
        assert_eq!(model.visible_count(), 2);
    }

    #[test]
    fn test_virtual_window_range() {
        let window = VirtualWindow::new(0, 200, 40).unwrap();
        assert_eq!(window.visible_range(100), 0..6); // 5 видимых + 1 запас
        let window = VirtualWindow::new(400, 200, 40).unwrap();
        assert_eq!(window.visible_range(100), 10..16);
        assert_eq!(window.visible_range(0), 0..0);
    }

    #[test]
    fn test_virtual_window_zero_row_height_rejected() {
        assert!(VirtualWindow::new(0, 100, 0).is_err());
    }

    #[test]
    fn test_virtual_window_scroll_clamped() {
        let mut window = VirtualWindow::new(0, 200, 40).unwrap();
        assert_eq!(window.total_height(10), 400);
        assert_eq!(window.max_scroll(10), 200);
        window.scroll_by(10_000, 10);
        assert_eq!(window.scroll_top, 200);
        window.scroll_by(-10_000, 10);
        assert_eq!(window.scroll_top, 0);
    }

    #[test]
    fn test_find_member() {
        let model = MemberListModel::from_members(vec![member(7, "Alice", PresenceStatus::Online)]);
        assert_eq!(model.find_member(7).unwrap().user_id, 7);
        assert!(model.find_member(8).is_none());
    }

    #[test]
    fn test_name_falls_back_to_username() {
        let row = MemberRow {
            user_id: 1,
            display_name: None,
            username: "global".to_string(),
            status: PresenceStatus::Online,
            avatar_url: None,
            role_color: None,
            activity: None,
        };
        assert_eq!(row.name(), "global");
    }
}
