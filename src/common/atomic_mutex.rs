use core::{
    cell::UnsafeCell,
    fmt::{self, Debug, Formatter},
    ops::{Deref, DerefMut},
    sync::atomic::*,
};

/// A simple atomic mutex for `no_std` environment.
/// It can be used in interrupt context.
/// But Most of the time, [`critical-section::Mutex`] is a better choice.
pub struct AtomicMutex<T> {
    data: UnsafeCell<T>,
    state: AtomicBool,
}

unsafe impl<T> Send for AtomicMutex<T> {}
unsafe impl<T> Sync for AtomicMutex<T> {}

impl<T> Debug for AtomicMutex<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(match self.state.load(Ordering::Relaxed) {
            true => "Locked",
            _ => "Unlocked",
        })
    }
}

impl<T> AtomicMutex<T> {
    /// Create a new populated instance.
    pub const fn new(value: T) -> Self {
        Self {
            data: UnsafeCell::new(value),
            state: AtomicBool::new(false),
        }
    }

    /// Non-blocking, can be used in interrupt context
    pub fn try_lock(&self) -> Option<AtomicMutexGuard<'_, T>> {
        if self.state.swap(true, Ordering::AcqRel) {
            None
        } else {
            Some(AtomicMutexGuard { m: self })
        }
    }

    #[allow(clippy::mut_from_ref)]
    #[inline]
    fn get_data_mut(&self) -> &mut T {
        unsafe { &mut *self.data.get() }
    }
}

// ------------------------------------------------------------------------------------------------

/// Holds the mutex until we are dropped
#[derive(Debug)]
pub struct AtomicMutexGuard<'mutex, T> {
    m: &'mutex AtomicMutex<T>,
}

impl<'mutex, T> Deref for AtomicMutexGuard<'mutex, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.m.get_data_mut()
    }
}

impl<'mutex, T> DerefMut for AtomicMutexGuard<'mutex, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.m.get_data_mut()
    }
}

impl<'mutex, T> Drop for AtomicMutexGuard<'mutex, T> {
    fn drop(&mut self) {
        self.m.state.store(false, Ordering::Release);
    }
}

// ------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct TestData {
        a: u8,
        b: u8,
    }

    #[test]
    fn lock() {
        let l = AtomicMutex::<TestData>::new(TestData { a: 1, b: 2 });
        {
            let mut d = l.try_lock().unwrap();
            assert_eq!(d.a, 1);
            assert_eq!(d.b, 2);
            let d2 = l.try_lock();
            assert!(d2.is_none());
            d.a += 1;
        }
    }
}
