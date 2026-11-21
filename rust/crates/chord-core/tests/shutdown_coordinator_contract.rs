#![forbid(unsafe_code)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use chord_core::ShutdownCoordinator;

#[tokio::test]
async fn test_shutdown_coordinator_initial_state() {
    let coordinator = ShutdownCoordinator::new();
    assert!(!coordinator.is_cancelled());
    assert!(!coordinator.token().is_cancelled());
}

#[tokio::test]
async fn test_shutdown_coordinator_cascading_child_cancellation() {
    let coordinator = ShutdownCoordinator::new();
    let child_a = coordinator.child_token();
    let child_b = coordinator.child_token();
    let grandchild = child_a.child_token();

    assert!(!child_a.is_cancelled());
    assert!(!child_b.is_cancelled());
    assert!(!grandchild.is_cancelled());

    coordinator.shutdown();

    assert!(coordinator.is_cancelled());
    assert!(child_a.is_cancelled());
    assert!(child_b.is_cancelled());
    assert!(grandchild.is_cancelled());
}

#[tokio::test]
async fn test_shutdown_coordinator_child_cancelled_independently_does_not_cancel_parent() {
    let coordinator = ShutdownCoordinator::new();
    let child = coordinator.child_token();

    child.cancel();
    assert!(child.is_cancelled());
    assert!(!coordinator.is_cancelled());
}

#[tokio::test]
async fn test_shutdown_coordinator_wakes_background_tasks() {
    let coordinator = ShutdownCoordinator::new();
    let completed = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::new();
    for _ in 0..4 {
        let token = coordinator.child_token();
        let counter = Arc::clone(&completed);
        handles.push(tokio::spawn(async move {
            token.cancelled().await;
            counter.fetch_add(1, Ordering::SeqCst);
        }));
    }

    tokio::time::sleep(Duration::from_millis(10)).await;
    assert_eq!(completed.load(Ordering::SeqCst), 0);

    coordinator.shutdown();

    for handle in handles {
        tokio::time::timeout(Duration::from_secs(1), handle)
            .await
            .expect("task should complete within timeout")
            .expect("task should not panic");
    }

    assert_eq!(completed.load(Ordering::SeqCst), 4);
}

#[tokio::test]
async fn test_shutdown_coordinator_cancelled_future_resolves_after_shutdown() {
    let coordinator = ShutdownCoordinator::new();
    let clone = coordinator.clone();

    let task = tokio::spawn(async move {
        clone.cancelled().await;
        true
    });

    coordinator.shutdown();

    let result = tokio::time::timeout(Duration::from_secs(1), task)
        .await
        .expect("cancelled future should resolve")
        .expect("task should not panic");
    assert!(result);
}
