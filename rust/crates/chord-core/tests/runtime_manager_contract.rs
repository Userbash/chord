//! Contract tests for RuntimeManager and ThreadRegistry (STAGE-02.1.2).
//!
//! Verifies:
//! - Multi-threaded Tokio Runtime initialization via `tokio::runtime::Builder::new_multi_thread()`.
//! - Worker pool thread limitation according to `RuntimeConfig`.
//! - Thread registration hooks (`on_thread_start` and `on_thread_stop`).
//! - ThreadRegistry inspection (active thread counts, thread names, thread IDs).
//! - Task spawning (`spawn`, `spawn_blocking`, `block_on`) and `Handle` retrieval.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use chord_core::runtime::{
    DEFAULT_THREAD_NAME_PREFIX, RuntimeConfig, RuntimeManager, ThreadRegistry,
};

#[test]
fn test_thread_registry_direct_operations() {
    let registry = ThreadRegistry::new();
    assert_eq!(registry.active_thread_count(), 0);
    assert_eq!(registry.total_spawned_count(), 0);
    assert!(registry.registered_threads().is_empty());

    registry.register_current_thread();
    assert_eq!(registry.active_thread_count(), 1);
    assert_eq!(registry.total_spawned_count(), 1);
    assert_eq!(registry.registered_threads().len(), 1);
    assert!(registry.registered_threads()[0].is_active);

    registry.unregister_current_thread();
    assert_eq!(registry.active_thread_count(), 0);
    assert_eq!(registry.total_spawned_count(), 1);
    assert_eq!(registry.registered_threads().len(), 1);
    assert!(!registry.registered_threads()[0].is_active);
}

#[test]
fn test_runtime_manager_initialization_default() {
    let manager = RuntimeManager::new().expect("default runtime manager should initialize");
    assert!(manager.config().worker_threads >= 1);
    assert_eq!(
        manager.config().thread_name_prefix,
        DEFAULT_THREAD_NAME_PREFIX
    );

    let res = manager.block_on(async { 100 + 23 });
    assert_eq!(res, 123);
}

#[test]
fn test_runtime_manager_pool_limits_and_configuration() {
    let config = RuntimeConfig::default()
        .with_worker_threads(3)
        .with_max_blocking_threads(8)
        .with_thread_name_prefix("test-rt-")
        .with_stack_size(2 * 1024 * 1024);

    let manager = RuntimeManager::from_config(config.clone())
        .expect("runtime manager from config should initialize");

    assert_eq!(manager.config().worker_threads, 3);
    assert_eq!(manager.config().max_blocking_threads, 8);
    assert_eq!(manager.config().thread_name_prefix, "test-rt-");

    // Spawn 10 async tasks to engage the worker pool
    let counter = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::new();

    for _ in 0..10 {
        let c = Arc::clone(&counter);
        handles.push(manager.spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            c.fetch_add(1, Ordering::SeqCst);
        }));
    }

    manager.block_on(async {
        for h in handles {
            h.await.unwrap();
        }
    });

    assert_eq!(counter.load(Ordering::SeqCst), 10);
}

#[test]
fn test_runtime_manager_thread_registration_hooks() {
    let config = RuntimeConfig::default()
        .with_worker_threads(2)
        .with_thread_name_prefix("reg-test-");

    let manager = RuntimeManager::from_config(config).expect("runtime manager should initialize");

    let registry = manager.registry();

    // Trigger work so worker threads start
    let thread_names = manager.block_on(async {
        let mut names = Vec::new();
        for _ in 0..6 {
            let name = tokio::spawn(async {
                std::thread::current()
                    .name()
                    .unwrap_or("unknown")
                    .to_string()
            })
            .await
            .unwrap();
            names.push(name);
        }
        names
    });

    // Verify worker threads are named according to prefix
    for name in &thread_names {
        assert!(
            name.starts_with("reg-test-"),
            "expected thread name starting with 'reg-test-', got: {}",
            name
        );
    }

    // Verify registry tracked the threads
    let registered = registry.registered_threads();
    assert!(
        !registered.is_empty(),
        "registry should have registered threads on start"
    );

    for thread_info in registered {
        assert!(
            thread_info.name.starts_with("reg-test-"),
            "registered thread info should have 'reg-test-' prefix, got: {}",
            thread_info.name
        );
    }
}

#[test]
fn test_runtime_manager_spawn_blocking_and_handle() {
    let manager = RuntimeManager::new().expect("runtime manager should initialize");
    let handle = manager.handle();

    // Spawn blocking computation
    let blocking_res = manager.block_on(async {
        let jh = manager.spawn_blocking(|| {
            std::thread::sleep(Duration::from_millis(5));
            42 * 2
        });
        jh.await.unwrap()
    });
    assert_eq!(blocking_res, 84);

    // Spawn via cloned Handle
    let handle_res = handle.block_on(async {
        let jh = handle.spawn(async { "tokio-handle-ok" });
        jh.await.unwrap()
    });
    assert_eq!(handle_res, "tokio-handle-ok");
}

#[test]
fn test_runtime_manager_shutdown_unregisters_threads() {
    let config = RuntimeConfig::default()
        .with_worker_threads(2)
        .with_thread_name_prefix("shut-test-");

    let manager = RuntimeManager::from_config(config).expect("runtime manager should initialize");

    let registry = manager.registry();

    // Run task to spin up workers
    manager.block_on(async {
        tokio::time::sleep(Duration::from_millis(5)).await;
    });

    let active_before = registry.active_thread_count();
    assert!(active_before > 0, "should have active worker threads");

    // Perform shutdown
    manager.shutdown_timeout(Duration::from_secs(1));

    // After shutdown, all worker threads should have stopped and called on_thread_stop hook
    let active_after = registry.active_thread_count();
    assert_eq!(
        active_after, 0,
        "active worker threads count should drop to 0 after shutdown"
    );
}
