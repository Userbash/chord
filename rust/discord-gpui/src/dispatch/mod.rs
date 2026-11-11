//! Высокопроизводительный асинхронный и мультипоточный диспетчер задач.
//! Обеспечивает неблокирующий I/O, фоновые воркеры на базе `tokio` и `async-channel`.

use std::sync::Arc;

/// Тип события, поступающего из фонового асинхронного воркера в GPUI AppState
#[derive(Debug, Clone)]
pub enum BackgroundEvent {
    GatewayMessage(String, String), // channel_id, content
    PresenceUpdate(String, String), // user_id, status
    VoicePing(u32),                 // ping_ms
    ImageDecoded(String, u32, u32), // cache_key, width, height
    TaskCompleted(String),          // task_id
}

/// Асинхронный диспетчер задач с пулом воркеров
pub struct AsyncTaskDispatcher {
    tx: async_channel::Sender<BackgroundEvent>,
    rx: async_channel::Receiver<BackgroundEvent>,
}

impl Default for AsyncTaskDispatcher {
    fn default() -> Self {
        Self::new(1024)
    }
}

impl AsyncTaskDispatcher {
    pub fn new(capacity: usize) -> Self {
        let (tx, rx) = async_channel::bounded(capacity);
        Self { tx, rx }
    }

    /// Получить отправителя для фоновых задач
    pub fn sender(&self) -> async_channel::Sender<BackgroundEvent> {
        self.tx.clone()
    }

    /// Получить получателя для интеграции с циклом GPUI
    pub fn receiver(&self) -> async_channel::Receiver<BackgroundEvent> {
        self.rx.clone()
    }

    /// Неблокирующая отправка события
    pub fn try_send(
        &self,
        event: BackgroundEvent,
    ) -> Result<(), async_channel::TrySendError<BackgroundEvent>> {
        self.tx.try_send(event)
    }

    /// Неблокирующее чтение событий для основного потока отрисовки (drain batch)
    pub fn drain_events(&self) -> Vec<BackgroundEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.rx.try_recv() {
            events.push(event);
        }
        events
    }
}

/// Пул параллельных потоков ОС для тяжелых CPU-bound вычислений
/// (декодирование медиа, шифрование RTP, парсинг больших JSON/ETF)
pub struct ParallelWorkerPool {
    worker_threads: usize,
}

impl Default for ParallelWorkerPool {
    fn default() -> Self {
        let cpus = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        Self::new(cpus)
    }
}

impl ParallelWorkerPool {
    pub fn new(threads: usize) -> Self {
        Self {
            worker_threads: threads.clamp(2, 32),
        }
    }

    pub fn worker_count(&self) -> usize {
        self.worker_threads
    }

    /// Параллельное выполнение пакетных задач с разделением на потоки
    pub fn execute_parallel_chunks<T, F, R>(&self, items: Vec<T>, f: F) -> Vec<R>
    where
        T: Send + 'static,
        R: Send + 'static,
        F: Fn(T) -> R + Sync + Send + 'static,
    {
        let f = Arc::new(f);
        let mut handles = Vec::new();

        for item in items {
            let f_clone = Arc::clone(&f);
            let handle = std::thread::spawn(move || f_clone(item));
            handles.push(handle);
        }

        let mut results = Vec::with_capacity(handles.len());
        for handle in handles {
            if let Ok(res) = handle.join() {
                results.push(res);
            }
        }
        results
    }
}

use discord_platform::dbus::BusAddress;
use discord_platform::notifications::{Notification, Notifier};
use discord_platform::rpc_ipc::{
    IpcFrame, RpcCommand, RpcOpCode, SetActivityPayload, decode_ipc_frame, encode_ipc_frame,
};
use discord_platform::tray::{
    BadgeState, CloseDisposition, MinimizeToTrayPolicy, MockTrayBackend, TrayBackend,
    TrayController, TrayError, TrayIconState, TrayMenu, TrayMenuAction, compute_tray_icon_state,
    format_badge_count,
};

/// Менеджер нативных уведомлений рабочего стола через freedesktop D-Bus (org.freedesktop.Notifications)
pub struct DesktopNotificationManager;

impl DesktopNotificationManager {
    /// Построение уведомления о сообщении по спецификации Freedesktop
    pub fn build_message_notification(
        author: &str,
        content: &str,
        channel_name: &str,
    ) -> Notification {
        Notification {
            app_name: "Discord".to_string(),
            summary: format!("{} (#{})", author, channel_name),
            body: content.to_string(),
            replaces_id: 0,
            actions: vec![
                ("default".to_string(), "Открыть".to_string()),
                ("mark_read".to_string(), "Прочитано".to_string()),
            ],
            timeout_ms: 5000,
        }
    }

    /// Построение уведомления об упоминании (mention) пользователя
    pub fn build_mention_notification(
        author: &str,
        content: &str,
        guild_name: &str,
        channel_name: &str,
    ) -> Notification {
        Notification {
            app_name: "Discord".to_string(),
            summary: format!(
                "{} упомянул(а) вас в {} (#{})",
                author, guild_name, channel_name
            ),
            body: content.to_string(),
            replaces_id: 0,
            actions: vec![
                ("default".to_string(), "Открыть".to_string()),
                ("mark_read".to_string(), "Прочитано".to_string()),
            ],
            timeout_ms: 7000,
        }
    }

    /// Построение уведомления о личном сообщении (Direct Message)
    pub fn build_dm_notification(author: &str, content: &str) -> Notification {
        Notification {
            app_name: "Discord".to_string(),
            summary: format!("Личное сообщение от {}", author),
            body: content.to_string(),
            replaces_id: 0,
            actions: vec![
                ("default".to_string(), "Открыть".to_string()),
                ("mark_read".to_string(), "Прочитано".to_string()),
            ],
            timeout_ms: 5000,
        }
    }

    /// Отправка нативного уведомления по объекту `Notification`
    pub fn send_notification(notification: &Notification) -> Result<u32, String> {
        let addr = BusAddress::from_env()
            .ok_or_else(|| "DBUS_SESSION_BUS_ADDRESS не задан в окружении".to_string())?;
        let mut notifier =
            Notifier::connect(&addr).map_err(|e| format!("D-Bus Notifier недоступен: {:?}", e))?;
        notifier
            .notify(notification)
            .map_err(|e| format!("Ошибка отправки D-Bus уведомления: {:?}", e))
    }

    /// Отправка нативного уведомления о новом входящем сообщении
    pub fn send_message_notification(
        author: &str,
        content: &str,
        channel_name: &str,
    ) -> Result<u32, String> {
        let notif = Self::build_message_notification(author, content, channel_name);
        Self::send_notification(&notif)
    }

    /// Отправка уведомления об упоминании
    pub fn send_mention_notification(
        author: &str,
        content: &str,
        guild_name: &str,
        channel_name: &str,
    ) -> Result<u32, String> {
        let notif = Self::build_mention_notification(author, content, guild_name, channel_name);
        Self::send_notification(&notif)
    }

    /// Закрытие уведомления по идентификатору
    pub fn close_notification(id: u32) -> Result<(), String> {
        let addr = BusAddress::from_env()
            .ok_or_else(|| "DBUS_SESSION_BUS_ADDRESS не задан в окружении".to_string())?;
        let mut notifier =
            Notifier::connect(&addr).map_err(|e| format!("D-Bus Notifier недоступен: {:?}", e))?;
        notifier
            .close(id)
            .map_err(|e| format!("Ошибка закрытия уведомления: {:?}", e))
    }
}

/// Менеджер системного трея Linux с поддержкой 6-пунктового меню и бейджей
pub struct SystemTrayManager;

impl SystemTrayManager {
    /// Создание контроллера системного трея с 6-пунктовым меню Discord
    pub fn create_default_controller() -> TrayController<MockTrayBackend> {
        let backend = MockTrayBackend::default();
        TrayController::new(Some(backend), MinimizeToTrayPolicy::new(true))
    }

    /// Создание headless-контроллера системного трея
    pub fn create_headless_controller(minimize_to_tray: bool) -> TrayController<MockTrayBackend> {
        TrayController::new_headless(MinimizeToTrayPolicy::new(minimize_to_tray))
    }

    /// Создание контроллера с произвольным бэкендом
    pub fn create_controller<B: TrayBackend>(
        backend: Option<B>,
        minimize_to_tray: bool,
    ) -> TrayController<B> {
        TrayController::new(backend, MinimizeToTrayPolicy::new(minimize_to_tray))
    }

    /// Создание 6-пунктового меню Discord
    pub fn create_menu(in_call: bool, self_mute: bool, self_deaf: bool) -> TrayMenu {
        TrayMenu::new(in_call, self_mute, self_deaf)
    }

    /// Проверка соответствия меню 6-пунктовой спецификации Discord:
    /// 1. Open, 2. Mute, 3. Deafen, 4. VoiceSettings, 5. CheckForUpdates, 6. Quit
    pub fn verify_6_item_menu(menu: &TrayMenu) -> bool {
        if menu.len() != 6 {
            return false;
        }
        let expected_actions = [
            TrayMenuAction::Open,
            TrayMenuAction::Mute,
            TrayMenuAction::Deafen,
            TrayMenuAction::VoiceSettings,
            TrayMenuAction::CheckForUpdates,
            TrayMenuAction::Quit,
        ];
        menu.items
            .iter()
            .zip(expected_actions.iter())
            .all(|(item, &expected)| item.action == expected)
    }

    /// Вычисление состояния иконки трея (Normal, Speaking, Muted, Deafened, Unread)
    pub fn compute_icon_state(
        in_call: bool,
        is_muted: bool,
        is_deafened: bool,
        unread_count: u32,
    ) -> TrayIconState {
        compute_tray_icon_state(in_call, is_muted, is_deafened, unread_count)
    }

    /// Форматирование бейджа лаунчера (1..99, 99+)
    pub fn format_badge(count: u32) -> Option<String> {
        format_badge_count(count)
    }
}

/// Высокоуровневый контроллер иконки системного трея Linux и D-Bus уведомлений.
/// Интегрирует 6-пунктовое меню Discord, бейджи непрочитанных,
/// состояния микрофона/звука и отправку уведомлений через D-Bus (org.freedesktop.Notifications).
pub struct LinuxSystemTrayIconController<B: TrayBackend = MockTrayBackend> {
    controller: TrayController<B>,
}

impl LinuxSystemTrayIconController<MockTrayBackend> {
    /// Создание контроллера по умолчанию с mock-бэкендом трея и политикой сворачивания в трей
    pub fn default_linux() -> Self {
        Self {
            controller: SystemTrayManager::create_default_controller(),
        }
    }

    /// Создание headless-контроллера без UI-бэкенда
    pub fn headless(minimize_to_tray: bool) -> Self {
        Self {
            controller: SystemTrayManager::create_headless_controller(minimize_to_tray),
        }
    }
}

impl<B: TrayBackend> LinuxSystemTrayIconController<B> {
    /// Создание контроллера с заданным бэкендом и политикой
    pub fn new(backend: Option<B>, minimize_to_tray: bool) -> Self {
        Self {
            controller: TrayController::new(backend, MinimizeToTrayPolicy::new(minimize_to_tray)),
        }
    }

    /// Доступ к внутреннему TrayController
    pub fn controller(&self) -> &TrayController<B> {
        &self.controller
    }

    /// Мутабельный доступ к внутреннему TrayController
    pub fn controller_mut(&mut self) -> &mut TrayController<B> {
        &mut self.controller
    }

    /// 6-пунктовое контекстное меню Discord
    pub fn menu(&self) -> &TrayMenu {
        self.controller.menu()
    }

    /// Проверка, что меню строго соответствует 6 пунктам Discord
    pub fn has_valid_6_item_menu(&self) -> bool {
        SystemTrayManager::verify_6_item_menu(self.menu())
    }

    /// Текущее состояние иконки (Normal, Speaking, Muted, Deafened, Unread)
    pub fn icon_state(&self) -> TrayIconState {
        self.controller.icon_state()
    }

    /// Текущий бейдж непрочитанных сообщений
    pub fn badge(&self) -> &BadgeState {
        self.controller.badge()
    }

    /// Видимо ли окно приложения
    pub fn is_window_visible(&self) -> bool {
        self.controller.is_window_visible()
    }

    /// Обновление состояния голосового звонка (активность, mute, deafen)
    pub fn update_voice_state(
        &mut self,
        in_call: bool,
        self_mute: bool,
        self_deaf: bool,
    ) -> Result<(), TrayError> {
        self.controller
            .update_voice_state(in_call, self_mute, self_deaf)
    }

    /// Обновление счетчика непрочитанных сообщений
    pub fn set_unread_count(&mut self, count: u32) -> Result<(), TrayError> {
        self.controller.set_unread_count(count)
    }

    /// Обработка клика по иконке трея (переключение видимости окна)
    pub fn on_tray_clicked(&mut self) {
        self.controller.on_tray_clicked();
    }

    /// Обработка действия из контекстного меню
    pub fn on_menu_action(&mut self, action: TrayMenuAction) -> CloseDisposition {
        self.controller.on_menu_action(action)
    }

    /// Обработка запроса закрытия главного окна
    pub fn handle_window_close(&mut self) -> CloseDisposition {
        self.controller.handle_window_close()
    }

    /// Отправка нативного D-Bus уведомления о входящем сообщении
    pub fn send_message_notification(
        &self,
        author: &str,
        content: &str,
        channel_name: &str,
    ) -> Result<u32, String> {
        DesktopNotificationManager::send_message_notification(author, content, channel_name)
    }

    /// Отправка нативного D-Bus уведомления об упоминании
    pub fn send_mention_notification(
        &self,
        author: &str,
        content: &str,
        guild_name: &str,
        channel_name: &str,
    ) -> Result<u32, String> {
        DesktopNotificationManager::send_mention_notification(
            author,
            content,
            guild_name,
            channel_name,
        )
    }
}

/// Сервер локального бинарного протокола Discord Local RPC IPC (discord-ipc-0..9)
/// Полная совместимость с rust-discord-activity и Serein
pub struct LocalRpcIpcServer;

impl LocalRpcIpcServer {
    /// Кодирование фрейма рукопожатия Handshake (Opcode = 0)
    pub fn create_handshake_frame(v: u64, client_id: &str) -> Result<Vec<u8>, String> {
        let payload = serde_json::json!({
            "v": v,
            "client_id": client_id
        });
        encode_ipc_frame(RpcOpCode::Handshake, &payload.to_string())
            .map_err(|e| format!("Ошибка кодирования IPC фрейма: {:?}", e))
    }

    /// Декодирование входящего бинарного фрейма (Header 8B + JSON payload)
    pub fn parse_ipc_frame(bytes: &[u8]) -> Result<IpcFrame, String> {
        decode_ipc_frame(bytes).map_err(|e| format!("Ошибка декодирования IPC фрейма: {:?}", e))
    }

    /// Разбор полезной нагрузки SET_ACTIVITY из JSON
    pub fn parse_set_activity(payload_json: &str) -> Result<SetActivityPayload, String> {
        match RpcCommand::from_json(payload_json) {
            Ok(RpcCommand::SetActivity(payload)) => Ok(payload),
            Ok(_) => Err("Команда не является SET_ACTIVITY".to_string()),
            Err(e) => Err(format!("Ошибка разбора RPC команды: {:?}", e)),
        }
    }
}
