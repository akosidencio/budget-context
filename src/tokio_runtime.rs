use std::future::Future;

use crate::{Budget, BudgetError};

impl Budget {
    /// Requests cancellation of this budget and all existing or future children.
    pub fn cancel(&self) {
        self.node.cancellation.cancel();
        #[cfg(feature = "tracing")]
        tracing::event!(
            tracing::Level::DEBUG,
            budget.id = self.id().get(),
            "budget.cancelled"
        );
    }

    /// Returns whether cancellation has been requested for this node.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.node.cancellation.is_cancelled()
    }

    /// Runs a future until it completes, cancellation is requested, or the
    /// effective deadline expires.
    ///
    /// If several branches are ready simultaneously, cancellation wins over
    /// the deadline, which wins over future completion. Dropping the future is
    /// only safe when the future itself is cancellation-safe.
    ///
    /// # Errors
    ///
    /// Returns [`BudgetError::Cancelled`] or [`BudgetError::DeadlineExceeded`]
    /// when the corresponding terminal condition wins.
    pub async fn run<F>(&self, future: F) -> Result<F::Output, BudgetError>
    where
        F: Future,
    {
        self.ensure_active()?;
        tokio::pin!(future);

        if let Some(deadline) = self.node.deadline {
            tokio::select! {
                biased;
                () = self.node.cancellation.cancelled() => Err(BudgetError::Cancelled),
                () = tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)) => {
                    Err(BudgetError::DeadlineExceeded)
                }
                output = &mut future => Ok(output),
            }
        } else {
            tokio::select! {
                biased;
                () = self.node.cancellation.cancelled() => Err(BudgetError::Cancelled),
                output = &mut future => Ok(output),
            }
        }
    }
}
