//! # discord-updater
//!
//! Карточка P8-01: собственный апдейтер клиента.
//!
//! Контур: подписанный манифест релиза (Ed25519) → проверка подписи и версии →
//! скачивание во временный каталог → staged-замена бинаря → откат при неудачном старте.
//!
//! Сеть и запись в каталог установки в этой карточке не выполняются: модуль только
//! проверяет манифест, вычисляет хэши артефактов и планирует шаги обновления. Фактическая
//! загрузка — отдельный слой (P8-01 следующий подэтап).
//!
//! Контракт заморожен `waves/RW-25/TZ-W26c.md`; тесты — `tests/updater_contract.rs`.
//!
//! Карточка BE-P3-07 (task-179) добавляет сетевой контур автообновления по
//! `main-arch.md` §5: константы эндпоинтов, выбор апдейтера, Linux-URL скачивания,
//! feed и каталога модулей, расписание `ScheduledBackoff` и состояния сплэш-экрана.

pub mod manifest;
pub mod plan;

/// Эндпоинт нового апдейтера: `NEW_UPDATE_ENDPOINT`
/// (`app #19192 @3231`, переопределяется настройкой).
pub const NEW_UPDATE_ENDPOINT: &str = "https://updates.discord.com/";

/// API-эндпоинт по умолчанию: `UPDATE_ENDPOINT` = `API_ENDPOINT`
/// (`app #19192 @3231`, переопределяется настройкой).
pub const DEFAULT_UPDATE_ENDPOINT: &str = "https://discord.com/api";

/// Верхняя граница задержки повтора: `DESKTOP_TTI_UPDATE_BACKOFF_MAX_MS`
/// (по умолчанию `3e4`, `app #53006 @97270`).
pub const DEFAULT_UPDATE_BACKOFF_MAX_MS: u64 = 30_000;

/// Расписание `ScheduledBackoff([2e3,3e3,3e3,4e3,5e3,7e3,1e4,12e3,15e3], !0)`
/// (`app #53006 @97270`).
pub const SCHEDULED_UPDATE_BACKOFF_MS: [u64; 9] = [
    2_000, 3_000, 3_000, 4_000, 5_000, 7_000, 10_000, 12_000, 15_000,
];

/// Канал выпуска (`releaseChannel` в `build_info`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReleaseChannel {
    /// Стабильный канал.
    Stable,
    /// Публичная бета (PTB).
    Ptb,
    /// Canary.
    Canary,
    /// Канал разработки.
    Development,
}

impl ReleaseChannel {
    /// Значение `releaseChannel` как в артефактах Discord (`stable`, `ptb`, …).
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Ptb => "ptb",
            Self::Canary => "canary",
            Self::Development => "development",
        }
    }
}

/// Отключён ли апдейтер (`app #55405 @26169`):
/// `disableUpdater || ("development" === releaseChannel && (standaloneModules || localModulesRoot))`.
pub fn should_disable_updater(
    release_channel: ReleaseChannel,
    disable_flag: bool,
    standalone_or_local_modules: bool,
) -> bool {
    disable_flag || (release_channel == ReleaseChannel::Development && standalone_or_local_modules)
}

/// Формат Linux-пакета: кнопки «Ubuntu (deb)» и «Linux (tar.gz)» в
/// `splashScreenPreload.js`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LinuxPackageFormat {
    /// `.deb` для Ubuntu.
    Deb,
    /// `.tar.gz` для остальных дистрибутивов.
    TarGz,
}

impl LinuxPackageFormat {
    /// Значение query-параметра `format`.
    pub fn as_query_value(&self) -> &'static str {
        match self {
            Self::Deb => "deb",
            Self::TarGz => "tar.gz",
        }
    }
}

/// База эндпоинта без хвостовых слэшей; пустой или невалидный URL ⇒
/// [`manifest::ManifestError::InvalidEndpoint`].
fn endpoint_base(endpoint: &str) -> Result<&str, manifest::ManifestError> {
    let invalid = || manifest::ManifestError::InvalidEndpoint {
        value: endpoint.to_string(),
    };

    let trimmed = endpoint.trim();
    let rest = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .ok_or_else(invalid)?;
    let authority_and_path = rest.trim_end_matches('/');
    if authority_and_path.is_empty()
        || authority_and_path
            .bytes()
            .any(|byte| byte.is_ascii_whitespace())
        || authority_and_path.contains(['?', '#'])
    {
        return Err(invalid());
    }

    let base_len = trimmed.len() - (rest.len() - authority_and_path.len());
    Ok(&trimmed[..base_len])
}

/// URL скачивания Linux-пакета:
/// `https://discord.com/api/download/{channel}?platform=linux&format={deb|tar.gz}`
/// (`LINUX_DOWNLOAD_URL_BASE` в `app/splashScreenPreload.js`).
pub fn build_linux_download_url(channel: ReleaseChannel, format: LinuxPackageFormat) -> String {
    format!(
        "{DEFAULT_UPDATE_ENDPOINT}/download/{}?platform=linux&format={}",
        channel.as_str(),
        format.as_query_value()
    )
}

/// URL legacy-feed обновлений:
/// `{endpoint}/updates/{channel}?platform=linux&version={major}.{minor}.{patch}`
/// (`app #47924 @62552`).
pub fn build_linux_feed_url(
    endpoint: &str,
    channel: ReleaseChannel,
    current_version: &manifest::Version,
) -> Result<String, manifest::ManifestError> {
    let base = endpoint_base(endpoint)?;
    Ok(format!(
        "{base}/updates/{}?platform=linux&version={}.{}.{}",
        channel.as_str(),
        current_version.major,
        current_version.minor,
        current_version.patch
    ))
}

/// Базовый URL каталога модулей:
/// `{endpoint}/modules/{channel}` (`remoteBaseURL`, `app #47924 @61321`).
pub fn build_modules_base_url(
    endpoint: &str,
    channel: ReleaseChannel,
) -> Result<String, manifest::ManifestError> {
    let base = endpoint_base(endpoint)?;
    Ok(format!("{base}/modules/{}", channel.as_str()))
}

/// Расписание повторов `ScheduledBackoff` из сплэш-экрана (`app #53006 @97270`).
///
/// Каждый `next_delay_ms` регистрирует неудачу и возвращает очередную задержку;
/// после последнего элемента расписания задержка остаётся на нём и усекается по
/// `max_ms`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledUpdateBackoff {
    /// Сколько неудач зарегистрировано с последнего `succeed()`.
    failures: u64,
    /// Верхняя граница задержки (`updateBackoffMaxMs`).
    max_ms: u64,
}

impl Default for ScheduledUpdateBackoff {
    fn default() -> Self {
        Self::new()
    }
}

impl ScheduledUpdateBackoff {
    /// Расписание с границей [`DEFAULT_UPDATE_BACKOFF_MAX_MS`].
    pub fn new() -> Self {
        Self::with_max_ms(DEFAULT_UPDATE_BACKOFF_MAX_MS)
    }

    /// Расписание с явной границей `max_ms` (значение настройки
    /// `DESKTOP_TTI_UPDATE_BACKOFF_MAX_MS`).
    pub fn with_max_ms(max_ms: u64) -> Self {
        Self {
            failures: 0,
            max_ms,
        }
    }

    /// Сколько неудач зарегистрировано с последнего [`Self::succeed`].
    pub fn failures(&self) -> u64 {
        self.failures
    }

    /// Регистрирует неудачу и возвращает задержку до следующего повтора.
    pub fn next_delay_ms(&mut self) -> u64 {
        let last = SCHEDULED_UPDATE_BACKOFF_MS.len() - 1;
        let index = usize::try_from(self.failures).map_or(last, |value| value.min(last));
        self.failures = self.failures.saturating_add(1);
        SCHEDULED_UPDATE_BACKOFF_MS[index].min(self.max_ms)
    }

    /// Регистрирует неудачу и возвращает задержку в секундах для сплэша:
    /// `Math.round(delayMs / 1e3)` (`app #53006 @98275`).
    pub fn retry_seconds(&mut self) -> u32 {
        let delay_ms = self.next_delay_ms();
        (delay_ms as f64 / 1_000.0).round() as u32
    }

    /// Успешное обновление: счётчик неудач сбрасывается на начало расписания.
    pub fn succeed(&mut self) {
        self.failures = 0;
    }
}

/// Состояния сплэш-экрана при обновлении (`app #53006`, `app #47924`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SplashUpdateState {
    /// Проверка обновлений (начало каждой итерации `updateUntilCurrent`).
    CheckingForUpdates,
    /// Идёт скачивание (host или module delta).
    DownloadingUpdates,
    /// Идёт установка.
    InstallingUpdates,
    /// Ошибка обновления; `seconds` — задержка до повтора.
    UpdateFailure {
        /// Задержка до следующего повтора в секундах.
        seconds: u32,
    },
    /// Linux: обновление ставится вручную; `latest_version` — версия с сервера.
    UpdateManually {
        /// Версия, которую предлагает апдейтер.
        latest_version: String,
    },
    /// Запуск основной версии.
    Launching,
}

impl SplashUpdateState {
    /// Значение `status` события сплэш-экрана (`SPLASH_UPDATE_STATE` в `bundle.js` и
    /// `splashScreenPreload.js`).
    pub fn event_name(&self) -> &'static str {
        match self {
            Self::CheckingForUpdates => "checking-for-updates",
            Self::DownloadingUpdates => "downloading-updates",
            Self::InstallingUpdates => "installing-updates",
            Self::UpdateFailure { .. } => "update-failure",
            Self::UpdateManually { .. } => "update-manually",
            Self::Launching => "launching",
        }
    }
}
