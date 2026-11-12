use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PresenceStatus {
    #[default]
    Online,
    Idle,
    Dnd,
    Offline,
}

impl PresenceStatus {
    pub fn label(&self) -> &'static str {
        match self {
            PresenceStatus::Online => "В сети",
            PresenceStatus::Idle => "Не активен",
            PresenceStatus::Dnd => "Не беспокоить",
            PresenceStatus::Offline => "Не в сети",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentUser {
    pub id: String,
    pub username: String,
    pub discriminator: String,
    pub global_name: Option<String>,
    pub avatar: Option<String>,
    pub status: PresenceStatus,
    pub custom_status: Option<String>,
}

/// Тип отношений между пользователями (re-deep/gateway-schema.md §d.relationships)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationshipType {
    None = 0,
    Friend = 1,          // В друзьях
    Blocked = 2,         // Заблокирован
    IncomingRequest = 3, // Входящая заявка
    OutgoingRequest = 4, // Исходящая заявка
}

/// Отношения / Друзья (re-deep/gateway-schema.md §d.relationships[])
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    pub id: String,
    pub user_id: String,
    pub relationship_type: RelationshipType,
    pub nickname: Option<String>,
    pub user: UserSummary,
    pub status: PresenceStatus,
    pub activity: Option<String>,
    pub banner_color: Option<String>,
    pub user_tag: Option<String>,
    pub member_since: Option<String>,
    pub mutual_guilds_count: u32,
    pub note: Option<String>,
}

/// Учетная запись пользователя для менеджера мульти-аккаунтов
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserAccount {
    pub user: CurrentUser,
    pub token: String,
    pub is_active: bool,
}

impl Default for CurrentUser {
    fn default() -> Self {
        Self {
            id: "123456789012345678".into(),
            username: "Rustery".into(),
            discriminator: "0001".into(),
            global_name: Some("Rustery Developer".into()),
            avatar: None,
            status: PresenceStatus::Online,
            custom_status: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ChannelType {
    #[default]
    GuildText,
    GuildVoice,
    GuildAnnouncement,
    Dm,
    GroupDm,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    pub id: String,
    pub guild_id: Option<String>,
    pub category_id: Option<String>,
    pub name: String,
    pub channel_type: ChannelType,
    pub topic: Option<String>,
    pub position: i32,
    pub unread: bool,
    pub is_locked: bool,
    pub permissions_bitmask: u64,
}

/// Источник для демонстрации экрана (окно приложения или рабочий стол)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenSource {
    pub id: String,
    pub name: String,
    pub is_screen: bool,
    pub preview_icon: String,
}

/// Видеокамера устройства
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelCategory {
    pub id: String,
    pub name: String,
    pub position: i32,
    pub is_collapsed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Guild {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub owner_id: Option<String>,
    pub categories: Vec<ChannelCategory>,
    pub channels: Vec<Channel>,
    pub unread_count: u32,
    pub mention_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSummary {
    pub id: String,
    pub username: String,
    pub global_name: Option<String>,
    pub avatar: Option<String>,
    pub is_bot: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageAttachment {
    pub id: String,
    pub filename: String,
    pub url: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageReaction {
    pub emoji: String,
    pub count: u32,
    pub me: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MessageKind {
    #[default]
    Default,
    CallStarted,
    CallMissed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub channel_id: String,
    pub author: UserSummary,
    pub content: String,
    pub timestamp: String,
    pub edited_timestamp: Option<String>,
    pub attachments: Vec<MessageAttachment>,
    pub reactions: Vec<MessageReaction>,
    pub is_pinned: bool,
    #[serde(default)]
    pub message_kind: MessageKind,
}

/// Состояние голосового/видео вызова в личном сообщении (Direct Call)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CallStatus {
    #[default]
    Idle,
    Ringing {
        recipient_id: String,
        recipient_name: String,
    },
    Connected {
        channel_id: String,
        recipient_name: String,
        duration_secs: u32,
        is_camera_on: bool,
        is_screen_sharing: bool,
    },
}

impl CallStatus {
    pub fn is_in_call(&self) -> bool {
        !matches!(self, CallStatus::Idle)
    }

    pub fn is_video(&self) -> bool {
        match self {
            CallStatus::Connected { is_camera_on, .. } => *is_camera_on,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuildMember {
    pub id: String,
    pub username: String,
    pub global_name: Option<String>,
    pub avatar: Option<String>,
    pub status: PresenceStatus,
    pub activity: Option<String>,
    pub roles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceSessionState {
    pub channel_id: String,
    pub channel_name: String,
    pub guild_name: String,
    pub endpoint: String,
    pub ping_ms: u32,
    pub is_speaking: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptchaChallenge {
    pub sitekey: String,
    pub service: String,
    pub rqdata: Option<String>,
}

/// Этап процесса авторизации Discord
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoginStep {
    Credentials,
    Mfa { ticket: String },
    Loading,
}

/// Состояние формы входа пользователя (Email/Password, Token, MFA, QR)
/// Точно соответствует поведению официального клиента из /home/sanya/re-deep/rest-static.md §1
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginFormState {
    pub email_or_phone: String,
    pub password: String,
    pub token: String,
    pub is_token_mode: bool,
    pub mfa_code: String,
    pub mfa_ticket: Option<String>,
    pub step: LoginStep,
    pub error_message: Option<String>,
    pub is_password_visible: bool,
    pub qr_fingerprint: String,
}

impl Default for LoginFormState {
    fn default() -> Self {
        Self {
            email_or_phone: String::new(),
            password: String::new(),
            token: String::new(),
            is_token_mode: false,
            mfa_code: String::new(),
            mfa_ticket: None,
            step: LoginStep::Credentials,
            error_message: None,
            is_password_visible: false,
            qr_fingerprint: "discord-remote-auth-7f9a2b8c4d1e".into(),
        }
    }
}

/// Тип элемента для быстрого перехода (Quick Switcher)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuickSwitcherItemKind {
    Server,  // '*' - серверы / гильдии
    Channel, // '#' - текстовые / голосовые каналы
    User,    // '@' - пользователи / ЛС / друзья
}

impl QuickSwitcherItemKind {
    pub fn prefix(&self) -> char {
        match self {
            QuickSwitcherItemKind::Server => '*',
            QuickSwitcherItemKind::Channel => '#',
            QuickSwitcherItemKind::User => '@',
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            QuickSwitcherItemKind::Server => "Сервер",
            QuickSwitcherItemKind::Channel => "Канал",
            QuickSwitcherItemKind::User => "Пользователь",
        }
    }
}

/// Элемент быстрого перехода Quick Switcher (Ctrl+K)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickSwitcherItem {
    pub id: String,
    pub kind: QuickSwitcherItemKind,
    pub name: String,
    pub subtitle: String,
    pub extra_id: Option<String>,
}

/// Вкладки окна настроек пользователя (Discord Settings Tabs)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SettingsTab {
    #[default]
    VoiceAndVideo,
    Appearance,
    MyAccount,
    Profile,
    Keybinds,
    Notifications,
    TextAndImages,
    Language,
}

impl SettingsTab {
    pub fn title(&self) -> &'static str {
        match self {
            Self::VoiceAndVideo => "Голос и видео (CPAL)",
            Self::Appearance => "Внешний вид",
            Self::MyAccount => "Моя учётная запись",
            Self::Profile => "Профиль пользователя",
            Self::Keybinds => "Горячие клавиши",
            Self::Notifications => "Уведомления",
            Self::TextAndImages => "Текст и изображения",
            Self::Language => "Язык интерфейса",
        }
    }

    pub fn category(&self) -> &'static str {
        match self {
            Self::MyAccount | Self::Profile => "НАСТРОЙКИ ПОЛЬЗОВАТЕЛЯ",
            Self::VoiceAndVideo
            | Self::Appearance
            | Self::Keybinds
            | Self::Notifications
            | Self::TextAndImages
            | Self::Language => "НАСТРОЙКИ ПРИЛОЖЕНИЯ",
        }
    }
}

/// Выбор темы оформления (Theme Switcher)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ThemeKind {
    #[default]
    Dark,
    Light,
    Midnight,
    System,
}

impl ThemeKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Dark => "Тёмная",
            Self::Light => "Светлая",
            Self::Midnight => "Полночь (AMOLED)",
            Self::System => "Синхронизация с системой",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Dark => "Классическая тёмная тема Discord (Dark Theme)",
            Self::Light => "Светлая тема с повышенной контрастностью текста",
            Self::Midnight => "Глубокий чёрный цвет для OLED экранов (#000000)",
            Self::System => "Автоматически подстраивается под настройки вашей ОС",
        }
    }
}

/// Плотность отображения сообщений (Message Display Density Switcher)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MessageDensity {
    #[default]
    Cozy,
    Compact,
}

impl MessageDensity {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Cozy => "Уютный (с аватарками)",
            Self::Compact => "Компактный (в одну строку)",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Cozy => {
                "Современный красивый вид: аватарки 40x40px, увеличенный межстрочный интервал"
            }
            Self::Compact => {
                "Компактный IRC-стиль: сообщения в одну строку без аватарок для максимальной плотности"
            }
        }
    }
}
