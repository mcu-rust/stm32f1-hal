//! A `std` `Mutex` based implementation

use core::marker::PhantomData;
use mutex_traits::RawMutex;
use std::sync::MutexGuard;

/// A wrap of [`std::sync::Mutex`]. This type's purpose is to allow choosing
/// between using [`std::sync::Mutex`] or `BlockingMutex` through feature flags.
pub struct Mutex<R: RawMutex, T> {
    mutex: std::sync::Mutex<T>,
    _marker: PhantomData<R>,
}

impl<R: RawMutex, T> Mutex<R, T> {
    /// Creates a new `Mutex`.
    #[inline]
    pub const fn new(val: T) -> Mutex<R, T> {
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

#[cfg(test)]
mod tests {
    use super::super::FakeRawMutex;
    use super::*;

    #[derive(Debug, PartialEq)]
    struct TestData {
        a: u8,
        b: u8,
    }

    #[test]
    fn lock() {
        let l = Mutex::<FakeRawMutex, TestData>::new(TestData { a: 1, b: 2 });
        {
            let mut d = l.try_lock().unwrap();
            assert_eq!(d.a, 1);
            assert_eq!(d.b, 2);
            let d2 = l.try_lock();
            assert!(d2.is_none());
            d.a += 1;
        }
        {
            let d = l.lock();
            assert_eq!(d.a, 2);
            assert_eq!(d.b, 2);
        }
    }
}
