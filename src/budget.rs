use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(not(budget_context_loom))]
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

#[cfg(budget_context_loom)]
use loom::sync::{Mutex, MutexGuard};
#[cfg(feature = "tokio")]
use tokio_util::sync::CancellationToken;

use crate::{
    BudgetBuildError, BudgetError, BudgetId, BudgetSnapshot, Remaining, Reservation,
    ReservationSet, Resource, ResourceSnapshot,
};

static NEXT_BUDGET_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Usage {
    pub(crate) consumed: u64,
    pub(crate) reserved: u64,
}

#[derive(Debug, Default)]
pub(crate) struct NodeState {
    pub(crate) usage: BTreeMap<Resource, Usage>,
}

pub(crate) struct Node {
    pub(crate) id: BudgetId,
    pub(crate) name: Option<Arc<str>>,
    pub(crate) parent: Option<Arc<Node>>,
    pub(crate) limits: BTreeMap<Resource, u64>,
    pub(crate) state: Mutex<NodeState>,
    pub(crate) deadline: Option<Instant>,
    #[cfg(feature = "tokio")]
    pub(crate) cancellation: CancellationToken,
}

impl fmt::Debug for Node {
    // Report the parent by identifier; recursing into the full ancestry is
    // unbounded in depth and output size.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = formatter.debug_struct("Node");
        debug
            .field("id", &self.id)
            .field("name", &self.name)
            .field("parent", &self.parent.as_ref().map(|parent| parent.id))
            .field("limits", &self.limits)
            .field("state", &self.state)
            .field("deadline", &self.deadline);
        #[cfg(feature = "tokio")]
        debug.field("cancellation", &self.cancellation);
        debug.finish()
    }
}

impl Drop for Node {
    // Unlink uniquely owned ancestors iteratively. The default recursive drop
    // overflows the stack, aborting the process, for lineages a few thousand
    // nodes deep.
    fn drop(&mut self) {
        let mut parent = self.parent.take();
        while let Some(node) = parent {
            parent = Arc::into_inner(node).and_then(|mut node| node.parent.take());
        }
    }
}

/// A cloneable handle to one node in a hierarchical resource budget.
#[derive(Clone, Debug)]
pub struct Budget {
    pub(crate) node: Arc<Node>,
}

/// Builder for a root or child [`Budget`].
#[derive(Debug)]
pub struct BudgetBuilder {
    parent: Option<Budget>,
    name: Option<Arc<str>>,
    limits: BTreeMap<Resource, u64>,
    duplicate_limit: Option<Resource>,
    deadline: Option<DeadlineRequest>,
    duplicate_deadline: bool,
}

#[derive(Clone, Copy, Debug)]
enum DeadlineRequest {
    At(Instant),
    After(Duration),
}

impl Budget {
    /// Starts a root budget builder.
    #[must_use]
    pub fn builder() -> BudgetBuilder {
        BudgetBuilder::new(None)
    }

    /// Starts a child builder whose operations will also count against this node.
    #[must_use]
    pub fn child(&self) -> BudgetBuilder {
        BudgetBuilder::new(Some(self.clone()))
    }

    /// Returns this node's process-local identifier.
    #[must_use]
    pub fn id(&self) -> BudgetId {
        self.node.id
    }

    /// Immediately and atomically consumes an amount across the entire lineage.
    ///
    /// # Errors
    ///
    /// Returns an error when the request overflows, exceeds a limit, or begins
    /// after the effective deadline or cancellation request.
    pub fn consume(&self, resource: &Resource, amount: u64) -> Result<(), BudgetError> {
        if amount == 0 {
            return Ok(());
        }
        self.ensure_active()?;
        let requests = BTreeMap::from([(resource.clone(), amount)]);
        self.apply(&requests, AccountingKind::Consumed)?;
        self.trace_event("consume", resource, amount);
        Ok(())
    }

    /// Atomically reserves capacity for one resource.
    ///
    /// # Errors
    ///
    /// Returns an error when the request overflows, exceeds a limit, or begins
    /// after the effective deadline or cancellation request.
    pub fn reserve(&self, resource: &Resource, amount: u64) -> Result<Reservation, BudgetError> {
        if amount != 0 {
            self.ensure_active()?;
            let requests = BTreeMap::from([(resource.clone(), amount)]);
            self.apply(&requests, AccountingKind::Reserved)?;
            self.trace_event("reserve", resource, amount);
        }
        Ok(Reservation::new(self.clone(), resource.clone(), amount))
    }

    /// Atomically reserves several resources, combining duplicate entries.
    ///
    /// The input is canonicalized by resource name. Overflow or exhaustion
    /// changes no accounting state. Zero-quantity entries reserve nothing but
    /// remain part of the set, so [`ReservationSet::commit`] treats usage
    /// reported for them as an overage.
    ///
    /// # Errors
    ///
    /// Returns an error when canonicalization overflows, a request exceeds a
    /// limit, or the operation begins after the effective deadline or
    /// cancellation request.
    pub fn reserve_many<'a>(
        &self,
        resources: impl IntoIterator<Item = (&'a Resource, u64)>,
    ) -> Result<ReservationSet, BudgetError> {
        let mut requests = BTreeMap::<Resource, u64>::new();
        let mut overflows = BTreeSet::new();
        for (resource, amount) in resources {
            let current = requests.entry(resource.clone()).or_default();
            if let Some(sum) = current.checked_add(amount) {
                *current = sum;
            } else {
                overflows.insert(resource.clone());
            }
        }
        if let Some(resource) = overflows.into_iter().next() {
            return Err(BudgetError::Overflow { resource });
        }
        if requests.values().any(|amount| *amount != 0) {
            self.ensure_active()?;
            self.apply(&requests, AccountingKind::Reserved)?;
            #[cfg(feature = "tracing")]
            tracing::event!(
                tracing::Level::TRACE,
                budget.id = self.id().get(),
                budget.resources = requests.len(),
                "budget.reserve_many"
            );
        }
        Ok(ReservationSet::new(self.clone(), requests))
    }

    /// Returns effective remaining capacity at one consistent instant.
    ///
    /// The result is observational. Only [`Budget::consume`],
    /// [`Budget::reserve`], and [`Budget::reserve_many`] authorize work.
    #[must_use]
    pub fn remaining(&self, resource: &Resource) -> Remaining {
        let lineage = self.lineage();
        let guards = lock_lineage(&lineage);
        effective_remaining(&lineage, &guards, resource)
    }

    /// Captures a consistent snapshot across this node's complete lineage.
    #[must_use]
    pub fn snapshot(&self) -> BudgetSnapshot {
        let lineage = self.lineage();
        let guards = lock_lineage(&lineage);
        let leaf_index = lineage.len() - 1;
        let leaf = &lineage[leaf_index];
        let leaf_state = &guards[leaf_index];

        let mut resources = BTreeSet::new();
        for node in &lineage {
            resources.extend(node.limits.keys().cloned());
        }
        resources.extend(leaf_state.usage.keys().cloned());

        let resources = resources
            .into_iter()
            .map(|resource| {
                let usage = leaf_state.usage.get(&resource).copied().unwrap_or_default();
                ResourceSnapshot {
                    local_limit: leaf.limits.get(&resource).copied(),
                    effective_remaining: effective_remaining(&lineage, &guards, &resource),
                    resource,
                    consumed: usage.consumed,
                    reserved: usage.reserved,
                }
            })
            .collect();

        BudgetSnapshot {
            id: leaf.id,
            name: leaf.name.clone(),
            resources,
            deadline_remaining: leaf
                .deadline
                .map(|deadline| deadline.saturating_duration_since(Instant::now())),
            #[cfg(feature = "tokio")]
            cancelled: leaf.cancellation.is_cancelled(),
            #[cfg(not(feature = "tokio"))]
            cancelled: false,
        }
    }

    pub(crate) fn release_reserved(&self, resources: &BTreeMap<Resource, u64>) {
        if resources.is_empty() {
            return;
        }
        let lineage = self.lineage();
        let mut guards = lock_lineage(&lineage);
        for guard in &mut guards {
            for (resource, amount) in resources {
                if let Some(usage) = guard.usage.get_mut(resource) {
                    usage.reserved = usage.reserved.saturating_sub(*amount);
                }
            }
        }
    }

    pub(crate) fn commit_reserved(
        &self,
        reserved: &BTreeMap<Resource, u64>,
        actual: &BTreeMap<Resource, u64>,
    ) {
        if reserved.is_empty() {
            return;
        }
        let lineage = self.lineage();
        let mut guards = lock_lineage(&lineage);
        for guard in &mut guards {
            for (resource, reserved_amount) in reserved {
                if let Some(usage) = guard.usage.get_mut(resource) {
                    let actual_amount = actual.get(resource).copied().unwrap_or(0);
                    usage.reserved = usage.reserved.saturating_sub(*reserved_amount);
                    usage.consumed += actual_amount.min(*reserved_amount);
                }
            }
        }
    }

    fn apply(
        &self,
        requests: &BTreeMap<Resource, u64>,
        kind: AccountingKind,
    ) -> Result<(), BudgetError> {
        let lineage = self.lineage();
        let mut guards = lock_lineage(&lineage);

        let nonzero = || requests.iter().filter(|(_, amount)| **amount != 0);
        for (index, node) in lineage.iter().enumerate() {
            for (resource, amount) in nonzero() {
                let usage = guards[index]
                    .usage
                    .get(resource)
                    .copied()
                    .unwrap_or_default();
                let next = usage
                    .consumed
                    .checked_add(usage.reserved)
                    .and_then(|used| used.checked_add(*amount))
                    .ok_or_else(|| BudgetError::Overflow {
                        resource: resource.clone(),
                    })?;
                if let Some(limit) = node.limits.get(resource) {
                    if next > *limit {
                        return Err(BudgetError::Exhausted {
                            resource: resource.clone(),
                            requested: *amount,
                            remaining: limit - usage.consumed - usage.reserved,
                            scope: node.id,
                            scope_name: node.name.clone(),
                        });
                    }
                }
            }
        }

        for guard in &mut guards {
            for (resource, amount) in nonzero() {
                let usage = guard.usage.entry(resource.clone()).or_default();
                match kind {
                    AccountingKind::Consumed => usage.consumed += amount,
                    AccountingKind::Reserved => usage.reserved += amount,
                }
            }
        }
        Ok(())
    }

    pub(crate) fn ensure_active(&self) -> Result<(), BudgetError> {
        #[cfg(feature = "tokio")]
        if self.node.cancellation.is_cancelled() {
            return Err(BudgetError::Cancelled);
        }
        if self
            .node
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            return Err(BudgetError::DeadlineExceeded);
        }
        Ok(())
    }

    pub(crate) fn lineage(&self) -> Vec<Arc<Node>> {
        let mut lineage = Vec::new();
        let mut current = Some(self.node.clone());
        while let Some(node) = current.take() {
            current.clone_from(&node.parent);
            lineage.push(node);
        }
        lineage.reverse();
        lineage
    }

    pub(crate) fn trace_event(&self, operation: &'static str, resource: &Resource, amount: u64) {
        #[cfg(feature = "tracing")]
        tracing::event!(
            tracing::Level::TRACE,
            budget.id = self.id().get(),
            budget.operation = operation,
            budget.resource = resource.as_str(),
            budget.amount = amount,
            "budget accounting operation"
        );
        #[cfg(not(feature = "tracing"))]
        let _ = (self, operation, resource, amount);
    }
}

impl BudgetBuilder {
    fn new(parent: Option<Budget>) -> Self {
        Self {
            parent,
            name: None,
            limits: BTreeMap::new(),
            duplicate_limit: None,
            deadline: None,
            duplicate_deadline: false,
        }
    }

    /// Assigns an optional diagnostic name.
    #[must_use]
    pub fn name(mut self, name: impl Into<Arc<str>>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Adds a local resource limit.
    ///
    /// Configuring the same resource twice makes [`BudgetBuilder::build`] fail.
    #[must_use]
    pub fn limit(mut self, resource: Resource, amount: u64) -> Self {
        if self.limits.insert(resource.clone(), amount).is_some() && self.duplicate_limit.is_none()
        {
            self.duplicate_limit = Some(resource);
        }
        self
    }

    /// Sets an absolute monotonic deadline.
    #[must_use]
    pub fn deadline_at(mut self, deadline: Instant) -> Self {
        self.set_deadline(DeadlineRequest::At(deadline));
        self
    }

    /// Sets a deadline relative to the instant at which the budget is built.
    #[must_use]
    pub fn deadline_after(mut self, duration: Duration) -> Self {
        self.set_deadline(DeadlineRequest::After(duration));
        self
    }

    /// Builds the node and derives its effective deadline and cancellation state.
    ///
    /// # Errors
    ///
    /// Returns an error for duplicate limits, duplicate or unrepresentable
    /// deadlines, or exhaustion of process-local budget identifiers.
    pub fn build(self) -> Result<Budget, BudgetBuildError> {
        if let Some(resource) = self.duplicate_limit {
            return Err(BudgetBuildError::DuplicateLimit { resource });
        }
        if self.duplicate_deadline {
            return Err(BudgetBuildError::DuplicateDeadline);
        }

        let requested_deadline = match self.deadline {
            None => None,
            Some(DeadlineRequest::At(deadline)) => Some(deadline),
            Some(DeadlineRequest::After(duration)) => Some(
                Instant::now()
                    .checked_add(duration)
                    .ok_or(BudgetBuildError::DeadlineOverflow)?,
            ),
        };
        let inherited_deadline = self.parent.as_ref().and_then(|parent| parent.node.deadline);
        let deadline = match (inherited_deadline, requested_deadline) {
            (Some(parent), Some(child)) => Some(parent.min(child)),
            (Some(parent), None) => Some(parent),
            (None, child) => child,
        };

        let id = NEXT_BUDGET_ID
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .map(BudgetId)
            .map_err(|_| BudgetBuildError::BudgetIdExhausted)?;

        #[cfg(feature = "tokio")]
        let cancellation = self
            .parent
            .as_ref()
            .map_or_else(CancellationToken::new, |parent| {
                parent.node.cancellation.child_token()
            });

        Ok(Budget {
            node: Arc::new(Node {
                id,
                name: self.name,
                parent: self.parent.map(|parent| parent.node),
                limits: self.limits,
                state: Mutex::new(NodeState::default()),
                deadline,
                #[cfg(feature = "tokio")]
                cancellation,
            }),
        })
    }

    fn set_deadline(&mut self, deadline: DeadlineRequest) {
        if self.deadline.replace(deadline).is_some() {
            self.duplicate_deadline = true;
        }
    }
}

#[derive(Clone, Copy)]
enum AccountingKind {
    Consumed,
    Reserved,
}

fn lock_lineage(lineage: &[Arc<Node>]) -> Vec<MutexGuard<'_, NodeState>> {
    lineage
        .iter()
        .map(|node| {
            node.state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        })
        .collect()
}

fn effective_remaining(
    lineage: &[Arc<Node>],
    guards: &[MutexGuard<'_, NodeState>],
    resource: &Resource,
) -> Remaining {
    let mut effective = None;
    for (node, state) in lineage.iter().zip(guards) {
        if let Some(limit) = node.limits.get(resource) {
            let usage = state.usage.get(resource).copied().unwrap_or_default();
            let remaining = limit - usage.consumed - usage.reserved;
            effective = Some(effective.map_or(remaining, |current: u64| current.min(remaining)));
        }
    }
    effective.map_or(Remaining::Unlimited, Remaining::Limited)
}
