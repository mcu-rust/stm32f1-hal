use super::OsInterface;
cfg_if::cfg_if! {
    if #[cfg(all(feature = "std", not(feature = "std-custom-mutex")))] {
        pub use std_impl::BlockingMutex;
    } else {
        pub use mutex::BlockingMutex;
    }
}

pub type Mutex<OS, T> = BlockingMutex<<OS as OsInterface>::RawMutex, T>;

use mutex_traits::{ConstInit, RawMutex};

/// A fake mutex is for testing.
/// It does not provide any synchronization between threads,
pub struct FakeRawMutex {}

impl FakeRawMutex {
    /// Create a new `FakeRawMutex`.
    pub const fn new() -> Self {
        Self {}
    }
}

unsafe impl Send for FakeRawMutex {}

impl ConstInit for FakeRawMutex {
    const INIT: Self = Self::new();
}

unsafe impl RawMutex for FakeRawMutex {
    type GuardMarker = *mut ();

    #[inline]
    fn lock(&self) {}

    #[inline]
    fn try_lock(&self) -> bool {
        true
    }

    #[inline]
    unsafe fn unlock(&self) {}

    #[inline]
    fn is_locked(&self) -> bool {
        true
    }
}

#[cfg(feature = "std")]
mod std_impl {
    use core::marker::PhantomData;
    use mutex_traits::RawMutex;
    use std::sync::MutexGuard;

    /// A wrap of [`std::sync::Mutex`]. This type's purpose is to allow choosing
    /// between using [`std::sync::Mutex`] or `BlockingMutex` through feature flags.
    pub struct BlockingMutex<R: RawMutex, T> {
        mutex: std::sync::Mutex<T>,
        _marker: PhantomData<R>,
    }

    impl<R: RawMutex, T> BlockingMutex<R, T> {
        /// Creates a new `Mutex`.
        #[inline]
        pub const fn new(val: T) -> BlockingMutex<R, T> {
            Self {
                mutex: std::sync::Mutex::new(val),
                _marker: PhantomData,
            }
        }

        /// lock
        #[inline]
        pub fn lock(&self) -> MutexGuard<'_, T> {
            self.mutex.lock().unwrap()
        }

        /// try_lock
        #[inline]
        pub fn try_lock(&self) -> Option<MutexGuard<'_, T>> {
            self.mutex.try_lock().ok()
        }

        /// try_with_lock
        #[must_use]
        #[inline]
        pub fn with_lock<U>(&self, f: impl FnOnce(&mut T) -> U) -> U {
            let mut guard = self.mutex.lock().unwrap();
            f(&mut *guard)
        }

        /// try_with_lock
        #[must_use]
        #[inline]
        pub fn try_with_lock<U>(&self, f: impl FnOnce(&mut T) -> U) -> Option<U> {
            let mut guard = self.try_lock()?;
            Some(f(&mut *guard))
        }
    }
}
