//! Contract tests for isolated rayon::ThreadPool for CPU media tasks (STAGE-02.1.3).
//!
//! Verifies:
//! - Creation of dedicated `MediaThreadPool` using `RuntimeConfig`.
//! - Isolation of CPU media tasks (Opus/video encoding) from Tokio async reactor.
//! - Naming of CPU worker threads with prefix `"chord-cpu-"`.
//! - Synchronous execution with `install()` and async non-blocking execution via `spawn_task()`.
//! - Seamless integration with `RuntimeManager`.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use chord_core::runtime::{
    MediaThreadPool, RuntimeConfig, RuntimeManager, DEFAULT_CPU_THREAD_PREFIX,
};

#[test]
fn test_media_thread_pool_creation_from_config() {
    let config = RuntimeConfig::default()
        .with_cpu_threads(4)
        .with_stack_size(2 * 1024 * 1024);

    let media_pool = MediaThreadPool::new(&config).expect("MediaThreadPool should be created");
    assert_eq!(media_pool.current_num_threads(), 4);
}

#[test]
fn test_media_thread_pool_thread_names() {
    let config = RuntimeConfig::default()
        .with_cpu_threads(2)
        .with_stack_size(2 * 1024 * 1024);

    let media_pool = MediaThreadPool::new(&config).expect("MediaThreadPool should be created");

    let thread_name = media_pool.install(|| {
        std::thread::current()
            .name()
            .unwrap_or("unknown")
            .to_string()
    });

    assert!(
        thread_name.starts_with(DEFAULT_CPU_THREAD_PREFIX),
        "Expected thread name to start with '{}', got '{}'",
        DEFAULT_CPU_THREAD_PREFIX,
        thread_name
    );
}

#[test]
fn test_media_thread_pool_install_synchronous_execution() {
    let config = RuntimeConfig::default().with_cpu_threads(2);
    let media_pool = MediaThreadPool::new(&config).expect("MediaThreadPool should be created");

    let result = media_pool.install(|| {
        let mut sum = 0u64;
        for i in 1..=1000 {
            sum += i;
        }
        sum
    });

    assert_eq!(result, 500500);
}

#[test]
fn test_media_thread_pool_spawn_async_bridge_without_blocking_tokio() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Tokio runtime should initialize");

    let config = RuntimeConfig::default().with_cpu_threads(2);
    let media_pool = MediaThreadPool::new(&config).expect("MediaThreadPool should be created");

    rt.block_on(async {
        // Heavy computation offloaded to rayon MediaThreadPool
        let rx = media_pool.spawn_task(|| {
            let name = std::thread::current()
                .name()
                .unwrap_or("unknown")
                .to_string();
            // Simulate CPU Opus encode
            let mut hash = 0u64;
            for i in 0..10_000 {
                hash = hash.wrapping_add(i * 31);
            }
            (name, hash)
        });

        let (name, hash) = rx.await.expect("task result should be received");
        assert!(name.starts_with(DEFAULT_CPU_THREAD_PREFIX));
        assert_ne!(hash, 0);
    });
}

#[test]
fn test_media_thread_pool_parallel_opus_simulated_workload() {
    let config = RuntimeConfig::default().with_cpu_threads(4);
    let media_pool = MediaThreadPool::new(&config).expect("MediaThreadPool should be created");

    let processed = Arc::new(AtomicUsize::new(0));

    // Parallel processing using rayon inside the isolated pool
    media_pool.install(|| {
        use rayon::prelude::*;

        let frames: Vec<Vec<u8>> = (0..32).map(|i| vec![i as u8; 960]).collect();
        frames.par_iter().for_each(|frame| {
            // Simulate Opus 20ms frame encode
            let checksum = frame.iter().fold(0u32, |acc, &b| acc.wrapping_add(b as u32));
            assert!(checksum < 1000000);
            processed.fetch_add(1, Ordering::SeqCst);
        });
    });

    assert_eq!(processed.load(Ordering::SeqCst), 32);
}

#[test]
fn test_media_thread_pool_isolation_from_tokio() {
    let config = RuntimeConfig::default()
        .with_worker_threads(2)
        .with_cpu_threads(2);

    let manager = RuntimeManager::from_config(config).expect("RuntimeManager should initialize");

    // Spawn async timer task on Tokio and CPU task on MediaThreadPool
    let media_pool = manager.media_pool();

    manager.block_on(async {
        let (tx_start, rx_start) = tokio::sync::oneshot::channel::<()>();

        // CPU heavy job on MediaThreadPool
        let media_task = media_pool.spawn_task(move || {
            let _ = tx_start.send(());
            std::thread::sleep(Duration::from_millis(50));
            std::thread::current().name().unwrap_or("unknown").to_string()
        });

        // Tokio worker task should execute concurrently without being blocked by CPU task
        rx_start.await.unwrap();
        let tokio_handle = manager.spawn(async {
            std::thread::current().name().unwrap_or("unknown").to_string()
        });
        let tokio_name = tokio_handle.await.unwrap();

        let cpu_name = media_task.await.unwrap();

        assert!(
            cpu_name.starts_with(DEFAULT_CPU_THREAD_PREFIX),
            "CPU task must run on cpu thread: {}",
            cpu_name
        );
        assert!(
            tokio_name.starts_with("chord-rt-"),
            "Tokio task must run on rt thread: {}",
            tokio_name
        );
    });
}

#[test]
fn test_runtime_manager_spawn_media_convenience() {
    let manager = RuntimeManager::new().expect("RuntimeManager should initialize");

    let rx = manager.spawn_media(|| {
        std::thread::current().name().unwrap_or("unknown").to_string()
    });

    let name = manager.block_on(async { rx.await.unwrap() });
    assert!(name.starts_with(DEFAULT_CPU_THREAD_PREFIX));
}
