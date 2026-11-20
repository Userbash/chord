#![forbid(unsafe_code)]

//! Global shutdown coordination using `tokio_util::sync::CancellationToken` (STAGE-02.2.1).
//!
//! Provides [`ShutdownCoordinator`] as the root cancellation token for cascading
//! graceful shutdown across all background tasks.

use tokio_util::sync::CancellationToken;

/// Global shutdown coordinator wrapping a root [`CancellationToken`].
///
/// Cloning [`ShutdownCoordinator`] shares the same underlying root token.
/// Use [`ShutdownCoordinator::child_token`] to create scoped child tokens for
/// background tasks so that [`ShutdownCoordinator::shutdown`] cancels the
/// entire task tree in a cascade.
#[derive(Debug, Clone, Default)]
pub struct ShutdownCoordinator {
    root_token: CancellationToken,
}

impl ShutdownCoordinator {
    /// Creates a new [`ShutdownCoordinator`] with a fresh root [`CancellationToken`].
    pub fn new() -> Self {
        Self {
            root_token: CancellationToken::new(),
        }
    }

    /// Returns a reference to the underlying root [`CancellationToken`].
    pub fn token(&self) -> &CancellationToken {
        &self.root_token
    }

    /// Creates a child [`CancellationToken`] linked to the root token.
    ///
    /// Cancelling the parent coordinator cancels all child tokens, while
    /// cancelling a child token does not cancel the parent.
    pub fn child_token(&self) -> CancellationToken {
        self.root_token.child_token()
    }

    /// Triggers cascading shutdown by cancelling the root [`CancellationToken`].
    pub fn shutdown(&self) {
        self.root_token.cancel();
    }

    /// Returns `true` if shutdown has already been triggered.
    pub fn is_cancelled(&self) -> bool {
        self.root_token.is_cancelled()
    }

    /// Completes when shutdown is triggered.
    pub async fn cancelled(&self) {
        self.root_token.cancelled().await;
    }
}
