#![forbid(unsafe_code)]

//! Multi-threaded runtime configuration and central manager (STAGE-02.1.1, STAGE-02.1.2, STAGE-02.1.3).
//!
//! Provides [`RuntimeConfig`] for configuring dedicated worker pools separating
//! network I/O tasks from heavy CPU media and database workloads, [`RuntimeManager`]
//! for initializing the central Tokio runtime with thread pool limits and registration hooks,
//! and [`MediaThreadPool`] for isolated Rayon CPU worker threads executing Opus/video encoding.

use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::thread::ThreadId;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// Default stack size for Tokio runtime worker threads: 2 MB (2,097,152 bytes).
pub const DEFAULT_STACK_SIZE_BYTES: usize = 2 * 1024 * 1024;

/// Default thread name prefix for network & core runtime workers: `"chord-rt-"`.
pub const DEFAULT_THREAD_NAME_PREFIX: &str = "chord-rt-";

/// Default thread name prefix for CPU-bound workers: `"chord-cpu-"`.
pub const DEFAULT_CPU_THREAD_PREFIX: &str = "chord-cpu-";

/// Minimum allowed worker thread stack size: 128 KB (131,072 bytes).
pub const MIN_STACK_SIZE_BYTES: usize = 128 * 1024;

/// Default maximum number of blocking threads for spawn_blocking tasks.
pub const DEFAULT_MAX_BLOCKING_THREADS: usize = 32;

/// Configuration for multi-threaded runtime separating network and CPU tasks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// Number of worker threads for async network and I/O tasks.
    pub worker_threads: usize,
    /// Number of CPU worker threads for heavy computation (e.g. media, encoding).
    pub cpu_threads: usize,
    /// Stack size in bytes for each worker thread (default: 2 MB).
    pub stack_size: usize,
    /// Thread name prefix for Tokio runtime worker threads (default: `"chord-rt-"`).
    pub thread_name_prefix: String,
    /// Maximum number of blocking threads for `spawn_blocking` pool.
    pub max_blocking_threads: usize,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        let detected = match std::thread::available_parallelism() {
            Ok(n) => n.get(),
            Err(_) => 2,
        };
        let worker_threads = if detected < 2 { 2 } else { detected };
        let cpu_threads = if detected < 2 { 2 } else { detected };

        Self {
            worker_threads,
            cpu_threads,
            stack_size: DEFAULT_STACK_SIZE_BYTES,
            thread_name_prefix: DEFAULT_THREAD_NAME_PREFIX.to_string(),
            max_blocking_threads: DEFAULT_MAX_BLOCKING_THREADS,
        }
    }
}

impl RuntimeConfig {
    /// Creates a new builder with default configuration values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the number of async worker threads for network and I/O.
    #[must_use]
    pub fn with_worker_threads(mut self, threads: usize) -> Self {
        self.worker_threads = threads;
        self
    }

    /// Sets the number of CPU worker threads for heavy compute tasks.
    #[must_use]
    pub fn with_cpu_threads(mut self, threads: usize) -> Self {
        self.cpu_threads = threads;
        self
    }

    /// Sets the thread stack size in bytes.
    #[must_use]
    pub fn with_stack_size(mut self, size: usize) -> Self {
        self.stack_size = size;
        self
    }

    /// Sets the thread name prefix for worker threads.
    #[must_use]
    pub fn with_thread_name_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.thread_name_prefix = prefix.into();
        self
    }

    /// Sets the maximum number of blocking threads.
    #[must_use]
    pub fn with_max_blocking_threads(mut self, threads: usize) -> Self {
        self.max_blocking_threads = threads;
        self
    }

    /// Validates runtime configuration parameters.
    pub fn validate(&self) -> Result<(), RuntimeConfigError> {
        if self.worker_threads == 0 {
            return Err(RuntimeConfigError::ZeroWorkerThreads);
        }
        if self.cpu_threads == 0 {
            return Err(RuntimeConfigError::ZeroCpuThreads);
        }
        if self.stack_size < MIN_STACK_SIZE_BYTES {
            return Err(RuntimeConfigError::StackSizeTooSmall {
                min: MIN_STACK_SIZE_BYTES,
                actual: self.stack_size,
            });
        }
        if self.thread_name_prefix.trim().is_empty() {
            return Err(RuntimeConfigError::EmptyThreadNamePrefix);
        }
        Ok(())
    }

    /// Formats the thread name for an async network worker thread.
    pub fn format_worker_thread_name(&self, index: usize) -> String {
        format!("{}worker-{}", self.thread_name_prefix, index)
    }

    /// Formats the thread name for a CPU-bound worker thread.
    pub fn format_cpu_thread_name(&self, index: usize) -> String {
        format!("{}{}", DEFAULT_CPU_THREAD_PREFIX, index)
    }

    /// Applies configuration settings to an existing [`tokio::runtime::Builder`].
    pub fn apply_to_tokio_builder(
        &self,
        builder: &mut tokio::runtime::Builder,
    ) -> Result<(), RuntimeConfigError> {
        self.validate()?;
        builder.worker_threads(self.worker_threads);
        builder.thread_stack_size(self.stack_size);
        builder.thread_name(self.thread_name_prefix.clone());
        builder.max_blocking_threads(self.max_blocking_threads);
        builder.enable_all();
        Ok(())
    }

    /// Creates and configures a multi-threaded [`tokio::runtime::Builder`].
    pub fn to_tokio_builder(&self) -> Result<tokio::runtime::Builder, RuntimeConfigError> {
        self.validate()?;
        let mut builder = tokio::runtime::Builder::new_multi_thread();
        self.apply_to_tokio_builder(&mut builder)?;
        Ok(builder)
    }
}

/// Errors occurring during runtime configuration validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeConfigError {
    /// Worker threads count must be greater than zero.
    ZeroWorkerThreads,
    /// CPU threads count must be greater than zero.
    ZeroCpuThreads,
    /// Stack size is below the minimum allowed threshold.
    StackSizeTooSmall { min: usize, actual: usize },
    /// Thread name prefix cannot be empty or whitespace.
    EmptyThreadNamePrefix,
}

impl std::fmt::Display for RuntimeConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroWorkerThreads => write!(f, "worker_threads must be at least 1"),
            Self::ZeroCpuThreads => write!(f, "cpu_threads must be at least 1"),
            Self::StackSizeTooSmall { min, actual } => {
                write!(
                    f,
                    "stack_size {} is smaller than minimum required {} bytes",
                    actual, min
                )
            }
            Self::EmptyThreadNamePrefix => {
                write!(f, "thread_name_prefix cannot be empty or whitespace")
            }
        }
    }
}

impl std::error::Error for RuntimeConfigError {}

/// Information about a registered runtime thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredThread {
    /// Native thread ID.
    pub id: ThreadId,
    /// Thread name reported by `std::thread::current().name()`.
    pub name: String,
    /// Timestamp when thread started.
    pub started_at: Instant,
    /// Whether the thread is currently active or stopped.
    pub is_active: bool,
}

/// Registry for tracking worker threads managed by the Tokio runtime.
#[derive(Debug, Default)]
pub struct ThreadRegistry {
    active_count: AtomicUsize,
    total_spawned: AtomicUsize,
    threads: RwLock<HashMap<ThreadId, RegisteredThread>>,
}

impl ThreadRegistry {
    /// Creates a new empty thread registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers the current thread when starting.
    pub fn register_current_thread(&self) {
        let cur = std::thread::current();
        let id = cur.id();
        let name = cur.name().unwrap_or("unnamed").to_string();

        self.active_count.fetch_add(1, Ordering::SeqCst);
        self.total_spawned.fetch_add(1, Ordering::SeqCst);

        let info = RegisteredThread {
            id,
            name,
            started_at: Instant::now(),
            is_active: true,
        };

        if let Ok(mut lock) = self.threads.write() {
            lock.insert(id, info);
        }
    }

    /// Unregisters the current thread when stopping.
    pub fn unregister_current_thread(&self) {
        let id = std::thread::current().id();
        self.active_count.fetch_sub(1, Ordering::SeqCst);

        if let Ok(mut lock) = self.threads.write()
            && let Some(info) = lock.get_mut(&id)
        {
            info.is_active = false;
        }
    }

    /// Number of currently active worker threads.
    pub fn active_thread_count(&self) -> usize {
        self.active_count.load(Ordering::SeqCst)
    }

    /// Total count of worker threads spawned over the lifetime of the registry.
    pub fn total_spawned_count(&self) -> usize {
        self.total_spawned.load(Ordering::SeqCst)
    }

    /// Snapshot list of all registered threads.
    pub fn registered_threads(&self) -> Vec<RegisteredThread> {
        match self.threads.read() {
            Ok(lock) => lock.values().cloned().collect(),
            Err(poisoned) => poisoned.into_inner().values().cloned().collect(),
        }
    }
}

/// Error occurring during [`MediaThreadPool`] creation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaThreadPoolError {
    /// Configuration validation failed.
    Config(RuntimeConfigError),
    /// Rayon thread pool builder failed to initialize the pool.
    RayonBuild(String),
}

impl std::fmt::Display for MediaThreadPoolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Config(e) => write!(f, "Media thread pool configuration error: {e}"),
            Self::RayonBuild(msg) => write!(f, "Failed to build rayon media thread pool: {msg}"),
        }
    }
}

impl std::error::Error for MediaThreadPoolError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Config(e) => Some(e),
            Self::RayonBuild(_) => None,
        }
    }
}

impl From<RuntimeConfigError> for MediaThreadPoolError {
    fn from(err: RuntimeConfigError) -> Self {
        Self::Config(err)
    }
}

impl From<rayon::ThreadPoolBuildError> for MediaThreadPoolError {
    fn from(err: rayon::ThreadPoolBuildError) -> Self {
        Self::RayonBuild(err.to_string())
    }
}

/// Dedicated, isolated `rayon::ThreadPool` for background CPU media workloads (STAGE-02.1.3).
///
/// Ensures CPU-heavy computations (such as Opus audio encoding/decoding,
/// video transcoding, soundboard mixing, and cryptography) run on isolated
/// worker threads without blocking Tokio's async reactor.
#[derive(Debug, Clone)]
pub struct MediaThreadPool {
    pool: Arc<rayon::ThreadPool>,
}

impl MediaThreadPool {
    /// Creates a new isolated media thread pool from the provided [`RuntimeConfig`].
    pub fn new(config: &RuntimeConfig) -> Result<Self, MediaThreadPoolError> {
        config.validate()?;

        let cfg = config.clone();
        let builder = rayon::ThreadPoolBuilder::new()
            .num_threads(config.cpu_threads)
            .stack_size(config.stack_size)
            .thread_name(move |idx| cfg.format_cpu_thread_name(idx));

        let pool = builder.build()?;
        Ok(Self {
            pool: Arc::new(pool),
        })
    }

    /// Creates an isolated media thread pool with default stack size and the given number of CPU threads.
    pub fn from_threads(threads: usize) -> Result<Self, MediaThreadPoolError> {
        let config = RuntimeConfig::default().with_cpu_threads(threads);
        Self::new(&config)
    }

    /// Returns a reference to the underlying [`rayon::ThreadPool`].
    #[must_use]
    pub fn raw_pool(&self) -> &rayon::ThreadPool {
        &self.pool
    }

    /// Returns the number of worker threads in the media pool.
    #[must_use]
    pub fn current_num_threads(&self) -> usize {
        self.pool.current_num_threads()
    }

    /// Executes the given operation within the media thread pool synchronously.
    pub fn install<OP, R>(&self, op: OP) -> R
    where
        OP: FnOnce() -> R + Send,
        R: Send,
    {
        self.pool.install(op)
    }

    /// Spawns an asynchronous job into the media thread pool.
    pub fn spawn<OP>(&self, op: OP)
    where
        OP: FnOnce() + Send + 'static,
    {
        self.pool.spawn(op);
    }

    /// Spawns an asynchronous job into the media thread pool with FIFO ordering.
    pub fn spawn_fifo<OP>(&self, op: OP)
    where
        OP: FnOnce() + Send + 'static,
    {
        self.pool.spawn_fifo(op);
    }

    /// Spawns a CPU-bound media task onto the isolated rayon thread pool and returns
    /// a [`tokio::sync::oneshot::Receiver`] to await the result without blocking Tokio.
    pub fn spawn_task<F, R>(&self, func: F) -> tokio::sync::oneshot::Receiver<R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.pool.spawn(move || {
            let res = func();
            let _ = tx.send(res);
        });
        rx
    }
}

/// Errors occurring during runtime creation and management.
#[derive(Debug)]
pub enum RuntimeError {
    /// Configuration validation error.
    Config(RuntimeConfigError),
    /// Tokio runtime initialization error.
    Init(std::io::Error),
    /// Media thread pool initialization error.
    MediaPool(MediaThreadPoolError),
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Config(e) => write!(f, "Runtime configuration error: {e}"),
            Self::Init(e) => write!(f, "Tokio runtime initialization failed: {e}"),
            Self::MediaPool(e) => write!(f, "Media thread pool error: {e}"),
        }
    }
}

impl std::error::Error for RuntimeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Config(e) => Some(e),
            Self::Init(e) => Some(e),
            Self::MediaPool(e) => Some(e),
        }
    }
}

impl From<RuntimeConfigError> for RuntimeError {
    fn from(err: RuntimeConfigError) -> Self {
        Self::Config(err)
    }
}

impl From<std::io::Error> for RuntimeError {
    fn from(err: std::io::Error) -> Self {
        Self::Init(err)
    }
}

impl From<MediaThreadPoolError> for RuntimeError {
    fn from(err: MediaThreadPoolError) -> Self {
        Self::MediaPool(err)
    }
}

/// Central Tokio Runtime manager with pool limiting and thread registration hooks.
#[derive(Debug)]
pub struct RuntimeManager {
    config: RuntimeConfig,
    runtime: tokio::runtime::Runtime,
    registry: Arc<ThreadRegistry>,
    media_pool: Arc<MediaThreadPool>,
}

impl RuntimeManager {
    /// Initializes a central Tokio runtime instance with default `RuntimeConfig`.
    pub fn new() -> Result<Self, RuntimeError> {
        Self::from_config(RuntimeConfig::default())
    }

    /// Initializes a central Tokio runtime instance from the specified `RuntimeConfig`.
    pub fn from_config(config: RuntimeConfig) -> Result<Self, RuntimeError> {
        config.validate()?;

        let registry = Arc::new(ThreadRegistry::new());
        let reg_start = Arc::clone(&registry);
        let reg_stop = Arc::clone(&registry);

        let mut builder = tokio::runtime::Builder::new_multi_thread();
        builder
            .worker_threads(config.worker_threads)
            .thread_stack_size(config.stack_size)
            .thread_name(config.thread_name_prefix.clone())
            .max_blocking_threads(config.max_blocking_threads)
            .enable_all()
            .on_thread_start(move || {
                reg_start.register_current_thread();
            })
            .on_thread_stop(move || {
                reg_stop.unregister_current_thread();
            });

        let runtime = builder.build()?;
        let media_pool = Arc::new(MediaThreadPool::new(&config)?);

        Ok(Self {
            config,
            runtime,
            registry,
            media_pool,
        })
    }

    /// Returns a reference to the configuration.
    #[must_use]
    pub fn config(&self) -> &RuntimeConfig {
        &self.config
    }

    /// Returns a shared reference to the worker thread registry.
    #[must_use]
    pub fn registry(&self) -> Arc<ThreadRegistry> {
        Arc::clone(&self.registry)
    }

    /// Returns a reference to the isolated CPU media thread pool.
    #[must_use]
    pub fn media_pool(&self) -> &MediaThreadPool {
        &self.media_pool
    }

    /// Spawns a CPU-intensive media task on the isolated media thread pool without blocking Tokio.
    pub fn spawn_media<F, R>(&self, func: F) -> tokio::sync::oneshot::Receiver<R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        self.media_pool.spawn_task(func)
    }

    /// Returns the Tokio runtime `Handle` for submitting work without taking ownership.
    #[must_use]
    pub fn handle(&self) -> tokio::runtime::Handle {
        self.runtime.handle().clone()
    }

    /// Runs a future to completion on the Tokio runtime.
    pub fn block_on<F: Future>(&self, future: F) -> F::Output {
        self.runtime.block_on(future)
    }

    /// Spawns a future onto the Tokio runtime.
    pub fn spawn<F>(&self, future: F) -> tokio::task::JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.runtime.spawn(future)
    }

    /// Spawns a blocking task in Tokio's blocking thread pool.
    pub fn spawn_blocking<F, R>(&self, func: F) -> tokio::task::JoinHandle<R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        self.runtime.spawn_blocking(func)
    }

    /// Shuts down the runtime, waiting up to `timeout` for tasks to finish.
    pub fn shutdown_timeout(self, timeout: Duration) {
        self.runtime.shutdown_timeout(timeout);
    }

    /// Shuts down the runtime in the background.
    pub fn shutdown_background(self) {
        self.runtime.shutdown_background();
    }
}

