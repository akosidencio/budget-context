use std::collections::{BTreeMap, BTreeSet};

use crate::{Budget, BudgetError, Resource};

/// A cancellation-safe reservation for one resource.
#[derive(Debug)]
#[must_use = "dropping an active reservation releases its capacity"]
pub struct Reservation {
    budget: Budget,
    resource: Resource,
    amount: u64,
    active: bool,
}

impl Reservation {
    pub(crate) fn new(budget: Budget, resource: Resource, amount: u64) -> Self {
        Self {
            budget,
            resource,
            amount,
            active: amount != 0,
        }
    }

    /// Returns the resource held by this reservation.
    #[must_use]
    pub fn resource(&self) -> &Resource {
        &self.resource
    }

    /// Returns the reserved amount.
    #[must_use]
    pub fn amount(&self) -> u64 {
        self.amount
    }

    /// Reconciles actual usage and releases unused capacity.
    ///
    /// If `actual` exceeds the reservation, the complete reservation is
    /// converted to consumed capacity before the error is returned. The caller
    /// is responsible for externally constraining operations when a hard upper
    /// bound is required.
    ///
    /// # Errors
    ///
    /// Returns [`BudgetError::ReservationExceeded`] after retaining the full
    /// reservation as consumed when `actual` exceeds the reserved amount.
    pub fn commit(mut self, actual: u64) -> Result<(), BudgetError> {
        if self.amount == 0 {
            if actual == 0 {
                return Ok(());
            }
            return Err(BudgetError::ReservationExceeded {
                resource: self.resource.clone(),
                reserved: 0,
                actual,
                unaccounted: actual,
            });
        }
        let reserved = BTreeMap::from([(self.resource.clone(), self.amount)]);
        let accounted = BTreeMap::from([(self.resource.clone(), actual.min(self.amount))]);
        self.budget.commit_reserved(&reserved, &accounted);
        self.active = false;
        self.budget
            .trace_event("commit", &self.resource, actual.min(self.amount));
        if actual > self.amount {
            return Err(BudgetError::ReservationExceeded {
                resource: self.resource.clone(),
                reserved: self.amount,
                actual,
                unaccounted: actual - self.amount,
            });
        }
        Ok(())
    }

    /// Explicitly releases the complete reservation.
    pub fn release(self) {
        drop(self);
    }
}

impl Drop for Reservation {
    fn drop(&mut self) {
        if self.active {
            let reserved = BTreeMap::from([(self.resource.clone(), self.amount)]);
            self.budget.release_reserved(&reserved);
            self.active = false;
            self.budget
                .trace_event("release", &self.resource, self.amount);
        }
    }
}

/// An atomic, cancellation-safe reservation for several resources.
#[derive(Debug)]
#[must_use = "dropping an active reservation set releases its capacity"]
pub struct ReservationSet {
    budget: Budget,
    amounts: BTreeMap<Resource, u64>,
    active: bool,
}

impl ReservationSet {
    pub(crate) fn new(budget: Budget, amounts: BTreeMap<Resource, u64>) -> Self {
        let active = amounts.values().any(|amount| *amount != 0);
        Self {
            budget,
            amounts,
            active,
        }
    }

    /// Returns the canonical, resource-sorted reservation amounts.
    ///
    /// Resources requested with a zero quantity are listed with a zero amount.
    #[must_use]
    pub fn amounts(&self) -> &BTreeMap<Resource, u64> {
        &self.amounts
    }

    /// Reconciles actual usage atomically and releases unused capacity.
    ///
    /// Duplicate actual entries are combined with checked arithmetic. Omitted
    /// resources and zero-quantity entries have zero actual usage. Resources
    /// requested with a zero quantity are part of the set, so usage reported
    /// for them is an overage, as for a zero-quantity [`Reservation`].
    /// Resources never requested with non-zero usage and arithmetic overflow
    /// fail closed by converting the full reservation set to consumed capacity
    /// before returning an error.
    ///
    /// # Errors
    ///
    /// Returns an error for unknown resources, duplicate-entry overflow, or
    /// actual usage beyond a reservation. Accounting is reconciled
    /// conservatively before every error is returned.
    pub fn commit<'a>(
        mut self,
        actual: impl IntoIterator<Item = (&'a Resource, u64)>,
    ) -> Result<(), BudgetError> {
        let mut actuals = BTreeMap::<Resource, u64>::new();
        let mut overflows = BTreeSet::new();
        for (resource, amount) in actual {
            if amount == 0 {
                continue;
            }
            let current = actuals.entry(resource.clone()).or_default();
            if let Some(sum) = current.checked_add(amount) {
                *current = sum;
            } else {
                overflows.insert(resource.clone());
            }
        }
        if let Some(resource) = actuals
            .keys()
            .find(|resource| !self.amounts.contains_key(*resource))
            .cloned()
        {
            self.commit_all_inner();
            return Err(BudgetError::UnknownReservationResource { resource });
        }
        if let Some(resource) = overflows.into_iter().next() {
            self.commit_all_inner();
            return Err(BudgetError::Overflow { resource });
        }

        let accounted = self
            .amounts
            .iter()
            .map(|(resource, reserved)| {
                let actual = actuals.get(resource).copied().unwrap_or(0);
                (resource.clone(), actual.min(*reserved))
            })
            .collect();
        if self.active {
            self.budget.commit_reserved(&self.amounts, &accounted);
            self.active = false;
        }
        for (resource, amount) in &accounted {
            self.budget.trace_event("commit", resource, *amount);
        }

        for (resource, reserved) in &self.amounts {
            let actual = actuals.get(resource).copied().unwrap_or(0);
            if actual > *reserved {
                return Err(BudgetError::ReservationExceeded {
                    resource: resource.clone(),
                    reserved: *reserved,
                    actual,
                    unaccounted: actual - reserved,
                });
            }
        }
        Ok(())
    }

    /// Converts every reserved amount to consumed capacity.
    pub fn commit_all(mut self) {
        self.commit_all_inner();
    }

    /// Explicitly releases the complete reservation set.
    pub fn release(self) {
        drop(self);
    }

    fn commit_all_inner(&mut self) {
        if self.active {
            self.budget.commit_reserved(&self.amounts, &self.amounts);
            self.active = false;
            for (resource, amount) in &self.amounts {
                self.budget.trace_event("commit", resource, *amount);
            }
        }
    }
}

impl Drop for ReservationSet {
    fn drop(&mut self) {
        if self.active {
            self.budget.release_reserved(&self.amounts);
            self.active = false;
            for (resource, amount) in &self.amounts {
                self.budget.trace_event("release", resource, *amount);
            }
        }
    }
}
