//! Семантический драйвер GUI и автоматизация тестирования через D-Bus AT-SPI (`org.a11y.Bus`).
//! Реализует TASK-902 (Подзадачи 902.1, 902.2, 902.3).

pub mod driver;
pub mod tree;
pub mod wcag;

pub use driver::AppDriver;
pub use tree::AccessibilityTree;
pub use wcag::{FocusTrapAuditReport, Wcag22Auditor, Wcag22ContrastReport};

use chord_i18n::resolve_locale;
use gpui::{Role as GpuiRole, accesskit};

/// Роли элементов в семантическом дереве доступности AT-SPI (`org.a11y.atspi.Role`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessibleRole {
    PushButton,
    Text,
    List,
    ListItem,
    Dialog,
    Window,
    Tab,
    CheckBox,
    Region,
}

pub type Role = AccessibleRole;

impl AccessibleRole {
    /// Разбор строкового имени роли (поддерживает канонические имена AT-SPI и удобные алиасы DSL).
    pub fn parse_role(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "button" | "push_button" | "push-button" | "push button" | "pushbutton" => {
                Some(Self::PushButton)
            }
            "text" | "input" | "entry" | "textbox" | "text_input" => Some(Self::Text),
            "list" => Some(Self::List),
            "listitem" | "list_item" | "list-item" | "list item" => Some(Self::ListItem),
            "dialog" | "modal" | "alert_dialog" => Some(Self::Dialog),
            "window" | "frame" | "application" => Some(Self::Window),
            "tab" | "page_tab" | "page tab" => Some(Self::Tab),
            "checkbox" | "check_box" | "check box" | "switch" => Some(Self::CheckBox),
            "region" | "panel" | "section" => Some(Self::Region),
            _ => None,
        }
    }

    /// Числовой идентификатор роли по спецификации Linux AT-SPI2 (`AtspiRole`).
    pub fn atspi_role_id(self) -> u32 {
        match self {
            Self::PushButton => 43,
            Self::Text => 61,
            Self::List => 31,
            Self::ListItem => 32,
            Self::Dialog => 16,
            Self::Window => 23,
            Self::Tab => 38,
            Self::CheckBox => 7,
            Self::Region => 74,
        }
    }

    /// Каноническое имя роли AT-SPI2 (`GetRoleName`).
    pub fn atspi_role_name(self) -> &'static str {
        match self {
            Self::PushButton => "push button",
            Self::Text => "text",
            Self::List => "list",
            Self::ListItem => "list item",
            Self::Dialog => "dialog",
            Self::Window => "frame",
            Self::Tab => "page tab",
            Self::CheckBox => "check box",
            Self::Region => "section",
        }
    }

    /// Конвертация роли AT-SPI в нативную роль GPUI / AccessKit.
    pub fn to_gpui_role(self) -> GpuiRole {
        match self {
            Self::PushButton => GpuiRole::Button,
            Self::Text => GpuiRole::TextInput,
            Self::List => GpuiRole::List,
            Self::ListItem => GpuiRole::ListItem,
            Self::Dialog => GpuiRole::Dialog,
            Self::Window => GpuiRole::Window,
            Self::Tab => GpuiRole::Tab,
            Self::CheckBox => GpuiRole::CheckBox,
            Self::Region => GpuiRole::Region,
        }
    }

    /// Обратная конвертация из роли GPUI / AccessKit в `AccessibleRole`.
    pub fn from_gpui_role(role: GpuiRole) -> Self {
        match role {
            GpuiRole::Button => Self::PushButton,
            GpuiRole::TextInput => Self::Text,
            GpuiRole::List => Self::List,
            GpuiRole::ListItem => Self::ListItem,
            GpuiRole::Dialog | GpuiRole::AlertDialog => Self::Dialog,
            GpuiRole::Window => Self::Window,
            GpuiRole::Tab => Self::Tab,
            GpuiRole::CheckBox | GpuiRole::Switch => Self::CheckBox,
            _ => Self::Region,
        }
    }
}

/// Состояния элементов доступности (`org.a11y.atspi.StateType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessibleState {
    Focused,
    Selected,
    Checked,
    Disabled,
    Focusable,
    Editable,
    Modal,
    Expanded,
    Showing,
}

pub type State = AccessibleState;

impl AccessibleState {
    /// Индекс бита в 64-битной маске `AtspiStateSet`.
    pub fn bit_index(self) -> u32 {
        match self {
            Self::Disabled => 0,
            Self::Checked => 4,
            Self::Editable => 7,
            Self::Expanded => 9,
            Self::Focusable => 10,
            Self::Focused => 11,
            Self::Modal => 16,
            Self::Selected => 23,
            Self::Showing => 24,
        }
    }
}

/// 64-битная маска состояний AT-SPI (`StateSet`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AtSpiStateSet(pub u64);

impl AtSpiStateSet {
    pub fn empty() -> Self {
        Self(0)
    }

    pub fn from_states(states: &[AccessibleState]) -> Self {
        let mut set = Self::empty();
        for s in states {
            set.insert(*s);
        }
        set
    }

    pub fn insert(&mut self, state: AccessibleState) {
        self.0 |= 1u64 << state.bit_index();
    }

    pub fn remove(&mut self, state: AccessibleState) {
        self.0 &= !(1u64 << state.bit_index());
    }

    pub fn contains(self, state: AccessibleState) -> bool {
        (self.0 & (1u64 << state.bit_index())) != 0
    }

    pub fn to_atspi_u32_pair(self) -> [u32; 2] {
        [(self.0 & 0xFFFF_FFFF) as u32, (self.0 >> 32) as u32]
    }
}

/// Ключи локализации из эталонного каталога `/home/sanya/re-deep/ru-catalog.json` и `en-catalog.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum A11yI18nKey {
    LoginButton,
    UserSettings,
    MuteMicrophone,
    UnmuteMicrophone,
    DeafenAudio,
    UndeafenAudio,
    Search,
    QuickSwitcher,
    EmojiPicker,
    Close,
    Friends,
    DirectMessages,
    Channels,
    Servers,
    Members,
    ShareScreen,
    AddServer,
    JoinServer,
    EmailOrPhone,
    Password,
    SendMessage,
    Cancel,
    Back,
    Submit,
}

impl A11yI18nKey {
    /// 6-символьный хеш ключа в каталоге Discord (`xxh64 -> base64[0..6]`).
    pub fn catalog_hash(self) -> &'static str {
        match self {
            Self::LoginButton => "2jvQ6K",
            Self::UserSettings => "cduTBL",
            Self::MuteMicrophone => "HITUcR",
            Self::UnmuteMicrophone => "6QK4H+",
            Self::DeafenAudio => "wjcRFX",
            Self::UndeafenAudio => "2US872",
            Self::Search => "pw+r5b",
            Self::QuickSwitcher => "ZvKwYa",
            Self::EmojiPicker => "sMOuuS",
            Self::Close => "cpT0Cq",
            Self::Friends => "TdEu5X",
            Self::DirectMessages => "/U8Iwa",
            Self::Channels => "OGiMXJ",
            Self::Servers => "+Anbp9",
            Self::Members => "QZql7O",
            Self::ShareScreen => "fjBNo1",
            Self::AddServer => "l5WIbf",
            Self::JoinServer => "riOUtB",
            Self::EmailOrPhone => "tUjnxr",
            Self::Password => "CIGa+7",
            Self::SendMessage => "QuPY4s",
            Self::Cancel => "4gTnU0",
            Self::Back => "13/7kX",
            Self::Submit => "uBFuok",
        }
    }

    /// Локализованное `Accessible Name` элемента без хардкода в UI.
    pub fn resolve_name(self, locale: &str) -> &'static str {
        let resolved = resolve_locale(locale, &["ru", "en-US"]);
        if resolved == "ru" {
            self.name_ru()
        } else {
            self.name_en()
        }
    }

    fn name_ru(self) -> &'static str {
        match self {
            Self::LoginButton => "Войти",
            Self::UserSettings => "Настройки пользователя",
            Self::MuteMicrophone => "Заглушить",
            Self::UnmuteMicrophone => "Вкл. микрофон",
            Self::DeafenAudio => "Откл. звук",
            Self::UndeafenAudio => "Вкл. звук",
            Self::Search => "Поиск",
            Self::QuickSwitcher => "Quick Switcher",
            Self::EmojiPicker => "Эмодзи",
            Self::Close => "Закрыть",
            Self::Friends => "Друзья",
            Self::DirectMessages => "Личные сообщения",
            Self::Channels => "Каналы",
            Self::Servers => "Серверы",
            Self::Members => "Участники",
            Self::ShareScreen => "Продемонстрируйте свой экран",
            Self::AddServer => "Добавить сервер",
            Self::JoinServer => "Присоединиться к серверу",
            Self::EmailOrPhone => "Адрес электронной почты или номер телефона",
            Self::Password => "Пароль",
            Self::SendMessage => "Отправить сообщение",
            Self::Cancel => "Отмена",
            Self::Back => "Назад",
            Self::Submit => "Отправить",
        }
    }

    fn name_en(self) -> &'static str {
        match self {
            Self::LoginButton => "Log In",
            Self::UserSettings => "User Settings",
            Self::MuteMicrophone => "Mute",
            Self::UnmuteMicrophone => "Unmute",
            Self::DeafenAudio => "Deafen",
            Self::UndeafenAudio => "Undeafen",
            Self::Search => "Search",
            Self::QuickSwitcher => "Quick Switcher",
            Self::EmojiPicker => "Emoji",
            Self::Close => "Close",
            Self::Friends => "Friends",
            Self::DirectMessages => "Direct Messages",
            Self::Channels => "Channels",
            Self::Servers => "Servers",
            Self::Members => "Members",
            Self::ShareScreen => "Share Your Screen",
            Self::AddServer => "Add a Server",
            Self::JoinServer => "Join a Server",
            Self::EmailOrPhone => "Email or Phone Number",
            Self::Password => "Password",
            Self::SendMessage => "Send a message",
            Self::Cancel => "Cancel",
            Self::Back => "Back",
            Self::Submit => "Submit",
        }
    }

    /// Локализованное `Accessible Description` элемента из каталога.
    pub fn resolve_description(self, locale: &str) -> &'static str {
        let resolved = resolve_locale(locale, &["ru", "en-US"]);
        if resolved == "ru" {
            self.desc_ru()
        } else {
            self.desc_en()
        }
    }

    fn desc_ru(self) -> &'static str {
        match self {
            Self::LoginButton => "Войти в учётную запись Discord",
            Self::UserSettings => "Открыть диалог настроек пользователя и приложения",
            Self::MuteMicrophone | Self::UnmuteMicrophone => {
                "Переключить состояние микрофона в голосовом канале"
            }
            Self::DeafenAudio | Self::UndeafenAudio => "Переключить звук наушников и динамиков",
            Self::Search => "Поиск сообщений в текущем канале или на сервере",
            Self::QuickSwitcher => "Быстрый переход к серверам, каналам и личным сообщениям",
            Self::EmojiPicker => "Выбрать эмодзи для вставки в сообщение",
            Self::Close => "Закрыть текущее модальное окно или панель",
            Self::Friends => "Список друзей и входящих запросов дружбы",
            Self::DirectMessages => "Перейти к личным сообщениям",
            Self::Channels => "Дерево текстовых и голосовых каналов сервера",
            Self::Servers => "Список серверов пользователя",
            Self::Members => "Список участников текущего сервера",
            Self::ShareScreen => "Выбрать экран или окно приложения для трансляции",
            Self::AddServer => "Создать новый сервер или присоединиться по приглашению",
            Self::JoinServer => "Ввести код приглашения для вступления на сервер",
            Self::EmailOrPhone => "Поле ввода адреса электронной почты или телефона",
            Self::Password => "Поле ввода пароля учётной записи",
            Self::SendMessage => "Поле ввода и отправка сообщения в активный канал",
            Self::Cancel => "Отменить действие и закрыть диалог",
            Self::Back => "Вернуться к предыдущему шагу",
            Self::Submit => "Подтвердить и отправить форму",
        }
    }

    fn desc_en(self) -> &'static str {
        match self {
            Self::LoginButton => "Sign in to your Discord account",
            Self::UserSettings => "Open user and application settings dialog",
            Self::MuteMicrophone | Self::UnmuteMicrophone => {
                "Toggle microphone mute state in voice channel"
            }
            Self::DeafenAudio | Self::UndeafenAudio => "Toggle audio output deafen state",
            Self::Search => "Search messages in current channel or server",
            Self::QuickSwitcher => "Quickly jump to servers, channels, or direct messages",
            Self::EmojiPicker => "Pick an emoji to insert into message composer",
            Self::Close => "Close active modal dialog or panel",
            Self::Friends => "Friends list and pending relationship requests",
            Self::DirectMessages => "Navigate to direct messages",
            Self::Channels => "Server text and voice channels list",
            Self::Servers => "User joined servers list",
            Self::Members => "Current server members list",
            Self::ShareScreen => "Select screen or application window to stream",
            Self::AddServer => "Create a new server or join via invite link",
            Self::JoinServer => "Enter an invite link to join an existing server",
            Self::EmailOrPhone => "Input field for account email or phone number",
            Self::Password => "Input field for account password",
            Self::SendMessage => "Message composer input and send action",
            Self::Cancel => "Cancel current operation and close modal",
            Self::Back => "Return to previous step",
            Self::Submit => "Confirm and submit form",
        }
    }
}

/// Узел дерева доступности GPUI / AT-SPI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessibleNode {
    pub id: String,
    pub object_path: String,
    pub role: AccessibleRole,
    pub name: String,
    pub description: String,
    pub i18n_key: String,
    pub states: Vec<AccessibleState>,
    pub state_set: AtSpiStateSet,
    pub actions: Vec<String>,
    pub value_text: String,
    pub modal_scope: Option<String>,
    pub children_ids: Vec<String>,
}

impl AccessibleNode {
    pub fn has_state(&self, state: AccessibleState) -> bool {
        self.state_set.contains(state)
    }

    /// Экспорт узла в нативный `gpui::accesskit::Node` для интеграции с деревом GPUI.
    pub fn to_accesskit_node(&self) -> accesskit::Node {
        let mut node = accesskit::Node::new(self.role.to_gpui_role());
        node.set_label(self.name.clone());
        node.set_description(self.description.clone());
        if !self.value_text.is_empty() {
            node.set_value(self.value_text.clone());
        }
        if self.has_state(AccessibleState::Disabled) {
            node.set_disabled();
        }
        if self.has_state(AccessibleState::Modal) {
            node.set_modal();
        }
        node
    }
}

/// Дескриптор найденного элемента для `AppDriver`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessibleElementHandle {
    pub id: String,
    pub object_path: String,
    pub role: AccessibleRole,
    pub name: String,
}

/// Квитанция выполнения D-Bus метода `org.a11y.atspi.Action.DoAction`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtSpiActionReceipt {
    pub object_path: String,
    pub interface: String,
    pub member: String,
    pub action_index: i32,
    pub dbus_serial: u32,
    pub wire_bytes_len: usize,
    pub handled: bool,
}

/// Ошибки семантического драйвера AT-SPI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum A11yDriverError {
    InvalidRole(String),
    ElementNotFound { role: String, name: String },
    ElementDisabled { id: String, name: String },
    UnsupportedAction { id: String, action_index: i32 },
    NotEditable { id: String, role: AccessibleRole },
    InvalidHotkey(String),
    DbusWireError(String),
}
