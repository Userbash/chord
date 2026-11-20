//! Contract tests for RuntimeConfig (STAGE-02.1.1).
//! Verifies thread worker pool sizing, 2MB stack size, chord-rt- prefix,
//! validation rules, and separation of network and CPU tasks.

use chord_core::runtime::{
    RuntimeConfig, RuntimeConfigError, DEFAULT_STACK_SIZE_BYTES, DEFAULT_THREAD_NAME_PREFIX,
    MIN_STACK_SIZE_BYTES,
};

#[test]
fn test_runtime_config_default_values() {
    let config = RuntimeConfig::default();

    assert!(config.worker_threads >= 1, "worker threads must be at least 1");
    assert!(config.cpu_threads >= 1, "cpu threads must be at least 1");
    assert_eq!(
        config.stack_size,
        DEFAULT_STACK_SIZE_BYTES,
        "default stack size must be 2MB (2097152 bytes)"
    );
    assert_eq!(config.stack_size, 2 * 1024 * 1024);
    assert_eq!(
        config.thread_name_prefix,
        DEFAULT_THREAD_NAME_PREFIX,
        "default thread name prefix must be chord-rt-"
    );
    assert_eq!(config.thread_name_prefix, "chord-rt-");
    assert_eq!(config.max_blocking_threads, 32);

    assert!(config.validate().is_ok(), "default config must be valid");
}

#[test]
fn test_runtime_config_builder_pattern() {
    let config = RuntimeConfig::default()
        .with_worker_threads(8)
        .with_cpu_threads(4)
        .with_stack_size(4 * 1024 * 1024)
        .with_thread_name_prefix("chord-net-")
        .with_max_blocking_threads(64);

    assert_eq!(config.worker_threads, 8);
    assert_eq!(config.cpu_threads, 4);
    assert_eq!(config.stack_size, 4 * 1024 * 1024);
    assert_eq!(config.thread_name_prefix, "chord-net-");
    assert_eq!(config.max_blocking_threads, 64);
    assert!(config.validate().is_ok());
}

#[test]
fn test_runtime_config_validation_zero_worker_threads() {
    let config = RuntimeConfig::default().with_worker_threads(0);
    let err = config.validate().unwrap_err();
    assert_eq!(err, RuntimeConfigError::ZeroWorkerThreads);
}

#[test]
fn test_runtime_config_validation_zero_cpu_threads() {
    let config = RuntimeConfig::default().with_cpu_threads(0);
    let err = config.validate().unwrap_err();
    assert_eq!(err, RuntimeConfigError::ZeroCpuThreads);
}

#[test]
fn test_runtime_config_validation_stack_size_too_small() {
    let config = RuntimeConfig::default().with_stack_size(1024);
    let err = config.validate().unwrap_err();
    assert_eq!(
        err,
        RuntimeConfigError::StackSizeTooSmall {
            min: MIN_STACK_SIZE_BYTES,
            actual: 1024,
        }
    );
}

#[test]
fn test_runtime_config_validation_empty_thread_name_prefix() {
    let config_empty = RuntimeConfig::default().with_thread_name_prefix("");
    assert_eq!(
        config_empty.validate().unwrap_err(),
        RuntimeConfigError::EmptyThreadNamePrefix
    );

    let config_ws = RuntimeConfig::default().with_thread_name_prefix("   ");
    assert_eq!(
        config_ws.validate().unwrap_err(),
        RuntimeConfigError::EmptyThreadNamePrefix
    );
}

#[test]
fn test_runtime_config_thread_naming() {
    let config = RuntimeConfig::default();
    assert_eq!(config.format_worker_thread_name(0), "chord-rt-worker-0");
    assert_eq!(config.format_worker_thread_name(3), "chord-rt-worker-3");
    assert_eq!(config.format_cpu_thread_name(0), "chord-cpu-0");
    assert_eq!(config.format_cpu_thread_name(2), "chord-cpu-2");
}

#[test]
fn test_runtime_config_serde_roundtrip() {
    let config = RuntimeConfig::default()
        .with_worker_threads(6)
        .with_cpu_threads(3)
        .with_stack_size(2 * 1024 * 1024)
        .with_thread_name_prefix("chord-rt-")
        .with_max_blocking_threads(16);

    let json = serde_json::to_string(&config).expect("serialization must succeed");
    let deserialized: RuntimeConfig =
        serde_json::from_str(&json).expect("deserialization must succeed");

    assert_eq!(config, deserialized);
}

#[test]
fn test_runtime_config_tokio_builder_integration() {
    let config = RuntimeConfig::default()
        .with_worker_threads(2)
        .with_stack_size(2 * 1024 * 1024)
        .with_thread_name_prefix("chord-rt-");

    let mut builder = config.to_tokio_builder().expect("builder should be created");
    let rt = builder.build().expect("tokio runtime should build successfully");

    let res = rt.block_on(async {
        let handle = tokio::spawn(async { 42 });
        handle.await.unwrap()
    });

    assert_eq!(res, 42);
}
