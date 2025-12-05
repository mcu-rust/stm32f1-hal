//! A `std` `Mutex` based implementation

use core::marker::PhantomData;
use mutex_traits::RawMutex;
use std::sync::MutexGuard;

/// A wrap of [`std::sync::Mutex`]. This type's purpose is to allow choosing
/// between using [`std::sync::Mutex`] or `BlockingMutex` through feature flags.
pub struct Mutex<R, T> {
    mutex: std::sync::Mutex<T>,
    _marker: PhantomData<R>,
}

impl<R, T> Mutex<R, T> {
    /// Creates a new `Mutex`.
    #[inline]
    pub const fn new(val: T) -> Mutex<R, T> {
        Self {
            mutex: std::sync::Mutex::new(val),
            _marker: PhantomData,
        }
    }

    /// try_lock
    #[inline]
    pub fn try_lock(&self) -> Option<MutexGuard<'_, T>> {
        self.mutex.try_lock().ok()
    }

    /// try_with_lock
    #[must_use]
    #[inline]
    pub fn try_with_lock<U>(&self, f: impl FnOnce(&mut T) -> U) -> Option<U> {
        let mut guard = self.try_lock()?;
        Some(f(&mut *guard))
    }
}

/// It only can be used with `StdBlockingMutex`.
/// It's not a real implementation.
pub struct StdRawMutex {}
unsafe impl RawMutex for StdRawMutex {
    type GuardMarker = *mut ();

    fn lock(&self) {}

    fn try_lock(&self) -> bool {
        false
    }

    unsafe fn unlock(&self) {}

    fn is_locked(&self) -> bool {
        true
    }
}
