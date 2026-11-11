//! SUB-901-1: Изолированный композитор Wayland в оперативной памяти (`/dev/shm` + `wayland-99` + `llvmpipe`).

use std::fs;
use std::os::unix::io::AsRawFd;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::HarnessError;

/// Конфигурация изолированного headless-композитора Wayland.
#[derive(Debug, Clone)]
pub struct CompositorConfig {
    pub preferred_socket: String,
    pub resolution: (u32, u32),
    pub runtime_dir: Option<PathBuf>,
    pub script_path: PathBuf,
}

impl Default for CompositorConfig {
    fn default() -> Self {
        Self {
            preferred_socket: "wayland-99".to_string(),
            resolution: (1920, 1080),
            runtime_dir: None,
            script_path: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../scripts/run-headless-compositor.sh"),
        }
    }
}

/// Активная сессия виртуального композитора Wayland в оперативной памяти.
pub struct CompositorSession {
    wayland_display: String,
    runtime_dir: PathBuf,
    own_runtime_dir: bool,
    resolution: (u32, u32),
    startup_elapsed: Duration,
    socket_inode: Option<String>,
    stop_flag: Arc<AtomicBool>,
    accept_thread: Option<JoinHandle<()>>,
    cleaned_up: bool,
}

static SESSION_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn create_ram_runtime_dir() -> Result<PathBuf, HarnessError> {
    let base = if Path::new("/dev/shm").is_dir() {
        PathBuf::from("/dev/shm")
    } else {
        std::env::temp_dir()
    };
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let seq = SESSION_COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = base.join(format!(
        "discord-gpui-wayland-{}-{}-{}",
        std::process::id(),
        nanos,
        seq
    ));
    fs::create_dir_all(&dir).map_err(|e| HarnessError::Io(format!("create_dir_all: {e}")))?;
    Ok(dir)
}

fn allocate_free_socket_name(runtime_dir: &Path, preferred: &str) -> Result<String, HarnessError> {
    let pref_sock = runtime_dir.join(preferred);
    let pref_lock = runtime_dir.join(format!("{preferred}.lock"));
    if !pref_sock.exists() && !pref_lock.exists() {
        return Ok(preferred.to_string());
    }
    for num in 99..=199 {
        let candidate = format!("wayland-{num}");
        let sock = runtime_dir.join(&candidate);
        let lock = runtime_dir.join(format!("{candidate}.lock"));
        if !sock.exists() && !lock.exists() {
            return Ok(candidate);
        }
    }
    Err(HarnessError::Io(
        "Нет свободных сокетов wayland-99..wayland-199".to_string(),
    ))
}

fn spawn_wayland_accept_loop(listener: UnixListener, stop_flag: Arc<AtomicBool>) -> JoinHandle<()> {
    thread::spawn(move || {
        while !stop_flag.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((stream, _)) => drop(stream),
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(2));
                }
                Err(_) => break,
            }
        }
    })
}

impl CompositorSession {
    /// Запускает виртуальный сокет Wayland (`wayland-99`) в ОЗУ с окружением `llvmpipe`.
    pub fn start(config: CompositorConfig) -> Result<Self, HarnessError> {
        let start_instant = Instant::now();
        if config.resolution.0 == 0 || config.resolution.1 == 0 {
            return Err(HarnessError::Process("Некорректное разрешение 0x0".into()));
        }
        let (runtime_dir, own_runtime_dir) = match config.runtime_dir {
            Some(dir) => {
                fs::create_dir_all(&dir)
                    .map_err(|e| HarnessError::Io(format!("runtime_dir: {e}")))?;
                (dir, false)
            }
            None => (create_ram_runtime_dir()?, true),
        };
        let wayland_display = allocate_free_socket_name(&runtime_dir, &config.preferred_socket)?;
        let sock_path = runtime_dir.join(&wayland_display);
        let lock_path = runtime_dir.join(format!("{wayland_display}.lock"));

        let listener = UnixListener::bind(&sock_path)
            .map_err(|e| HarnessError::Io(format!("bind {}: {e}", sock_path.display())))?;
        let socket_inode = fs::read_link(format!("/proc/self/fd/{}", listener.as_raw_fd()))
            .ok()
            .map(|p| p.to_string_lossy().into_owned());
        listener
            .set_nonblocking(true)
            .map_err(|e| HarnessError::Io(format!("set_nonblocking: {e}")))?;
        fs::write(&lock_path, format!("{}\n", std::process::id()))
            .map_err(|e| HarnessError::Io(format!("write lock: {e}")))?;

        let stop_flag = Arc::new(AtomicBool::new(false));
        let accept_thread = Some(spawn_wayland_accept_loop(listener, Arc::clone(&stop_flag)));

        Ok(Self {
            wayland_display,
            runtime_dir,
            own_runtime_dir,
            resolution: config.resolution,
            startup_elapsed: start_instant.elapsed(),
            socket_inode,
            stop_flag,
            accept_thread,
            cleaned_up: false,
        })
    }

    #[must_use]
    pub fn socket_inode(&self) -> Option<&str> {
        self.socket_inode.as_deref()
    }

    #[must_use]
    pub fn wayland_display(&self) -> &str {
        &self.wayland_display
    }

    #[must_use]
    pub fn runtime_dir(&self) -> &Path {
        &self.runtime_dir
    }

    #[must_use]
    pub fn socket_path(&self) -> PathBuf {
        self.runtime_dir.join(&self.wayland_display)
    }

    #[must_use]
    pub fn startup_elapsed(&self) -> Duration {
        self.startup_elapsed
    }

    #[must_use]
    pub fn is_socket_alive(&self) -> bool {
        use std::os::unix::fs::FileTypeExt;
        fs::symlink_metadata(self.socket_path())
            .map(|m| m.file_type().is_socket())
            .unwrap_or(false)
    }

    /// Возвращает переменные окружения для запуска GPUI поверх `llvmpipe` + `wlroots headless`.
    #[must_use]
    pub fn env_vars(&self) -> Vec<(String, String)> {
        let (w, h) = self.resolution;
        vec![
            (
                "XDG_RUNTIME_DIR".to_string(),
                self.runtime_dir.to_string_lossy().into_owned(),
            ),
            ("WAYLAND_DISPLAY".to_string(), self.wayland_display.clone()),
            ("XDG_SESSION_TYPE".to_string(), "wayland".to_string()),
            ("GDK_BACKEND".to_string(), "wayland".to_string()),
            ("WLR_BACKENDS".to_string(), "headless".to_string()),
            ("WLR_LIBINPUT_NO_DEVICES".to_string(), "1".to_string()),
            ("WLR_RENDERER".to_string(), "pixman".to_string()),
            ("WLR_HEADLESS_OUTPUTS".to_string(), "1".to_string()),
            ("HEADLESS_RESOLUTION".to_string(), format!("{w}x{h}")),
            ("LIBGL_ALWAYS_SOFTWARE".to_string(), "1".to_string()),
            ("GALLIUM_DRIVER".to_string(), "llvmpipe".to_string()),
            (
                "MESA_LOADER_DRIVER_OVERRIDE".to_string(),
                "llvmpipe".to_string(),
            ),
            (
                "WEBKIT_DISABLE_DMABUF_RENDERER".to_string(),
                "1".to_string(),
            ),
        ]
    }

    /// Корректно останавливает поток прослушивания сокета и удаляет артефакты из ОЗУ.
    pub fn shutdown(&mut self) -> Result<(), HarnessError> {
        if self.cleaned_up {
            return Ok(());
        }
        self.stop_flag.store(true, Ordering::Relaxed);
        if let Some(handle) = self.accept_thread.take() {
            let _ = handle.join();
        }
        let sock_path = self.socket_path();
        let lock_path = self
            .runtime_dir
            .join(format!("{}.lock", self.wayland_display));
        let _ = fs::remove_file(&sock_path);
        let _ = fs::remove_file(&lock_path);
        if self.own_runtime_dir {
            let _ = fs::remove_dir_all(&self.runtime_dir);
        }
        self.cleaned_up = true;
        Ok(())
    }
}

impl Drop for CompositorSession {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}
