// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Minimal non-reentrant spin lock.

use core::cell::UnsafeCell;
use core::hint::spin_loop;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};

use crate::sync::atomic::{AtomicBool, Ordering};

/// A small non-reentrant lock for state protected by an external IRQ policy.
pub struct SpinLock<T> {
    locked: AtomicBool,
    value: UnsafeCell<T>,
}

struct LockRelease<'lock>(&'lock AtomicBool);

/// A short exclusive borrow obtained without waiting. Must not cross a yield.
pub struct SpinLockGuard<'lock, T> {
    value: &'lock mut T,
    _release: LockRelease<'lock>,
    _local: PhantomData<*mut ()>,
}

impl<T> Deref for SpinLockGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.value
    }
}
impl<T> DerefMut for SpinLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.value
    }
}

impl Drop for LockRelease<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

// SAFETY: Access to the contained value is serialized by `locked`.
unsafe impl<T: Send> Sync for SpinLock<T> {}

impl<T> SpinLock<T> {
    pub const fn new(value: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            value: UnsafeCell::new(value),
        }
    }

    /// Extracts the protected value from an exclusively owned lock.
    pub fn into_inner(self) -> T {
        self.value.into_inner()
    }

    pub fn with<R>(&self, operation: impl FnOnce(&mut T) -> R) -> R {
        self.with_relax(operation, spin_loop)
    }

    /// Runs `operation` after acquiring the lock, invoking `relax` after every
    /// failed acquisition so an enclosing synchronization policy can make
    /// architecture-required progress.
    pub(super) fn with_relax<R>(
        &self,
        operation: impl FnOnce(&mut T) -> R,
        mut relax: impl FnMut(),
    ) -> R {
        while self
            .locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            relax();
        }

        let release = LockRelease(&self.locked);
        // SAFETY: This lock is held exclusively until `release` is dropped.
        let result = operation(unsafe { &mut *self.value.get() });
        drop(release);
        result
    }

    /// Runs `operation` only when the lock can be acquired immediately.
    ///
    /// Fatal and diagnostic paths use this to avoid waiting on a lock held by
    /// the context that they interrupted.
    pub fn try_with<R>(&self, operation: impl FnOnce(&mut T) -> R) -> Option<R> {
        self.try_lock().map(|mut guard| operation(&mut guard))
    }

    /// Acquires without waiting, allowing several locks to be joined before a
    /// transactional update. Drop all guards before allocating, freeing or yielding.
    pub fn try_lock(&self) -> Option<SpinLockGuard<'_, T>> {
        if self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            return None;
        }

        Some(SpinLockGuard {
            // SAFETY: The successful transition above gives this caller exclusive
            // access until the guard's release token is dropped.
            value: unsafe { &mut *self.value.get() },
            _release: LockRelease(&self.locked),
            _local: PhantomData,
        })
    }
}
