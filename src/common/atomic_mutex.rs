use super::*;
use core::{
    cell::UnsafeCell,
    fmt::{self, Debug, Formatter},
    ops::{Deref, DerefMut},
    sync::atomic::*,
};

/// A simple atomic mutex for `no_std` environment.
/// It can be used in interrupt context.
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
        if self
            .state
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            Some(AtomicMutexGuard { m: &self })
        } else {
            None
        }
    }

    /// # Safety
    ///
    /// Blocking, cannot be used in interrupt context
    // pub fn lock(&self) -> AtomicMutexGuard<'_, T> {
    //     while self
    //         .state
    //         .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
    //         .is_err()
    //     {
    //         // os::yield_thread()
    //     }

    //     AtomicMutexGuard { m: &self }
    // }

    #[inline]
    fn get_data_mut(&self) -> &mut T {
        unsafe { self.data.unsafe_get_mut() }
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

    fn deref<'a>(&'a self) -> &'a T {
        self.m.get_data_mut()
    }
}

impl<'mutex, T> DerefMut for AtomicMutexGuard<'mutex, T> {
    fn deref_mut<'a>(&'a mut self) -> &'a mut T {
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
        // {
        //     let d = l.lock();
        //     assert_eq!(d.a, 2);
        //     assert_eq!(d.b, 2);
        // }
    }
}
