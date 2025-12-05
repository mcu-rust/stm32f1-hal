use core::marker::PhantomData;
use mutex_traits::{ConstInit, RawMutex};

/// Wrap `BlockingMutex`` and only provide try_lock and try_with_lock methods.
pub struct Mutex<R, T: ?Sized>(mutex::BlockingMutex<R, T>);

impl<R: ConstInit, T> Mutex<R, T> {
    #[inline(always)]
    pub const fn new(data: T) -> Self {
        Self(mutex::BlockingMutex::<R, T>::new(data))
    }
}

impl<R: RawMutex, T: ?Sized> Mutex<R, T> {
    #[inline(always)]
    pub fn try_lock(&self) -> Option<mutex::MutexGuard<'_, R, T>> {
        self.0.try_lock()
    }

    #[inline(always)]
    pub fn try_with_lock<U>(&self, f: impl FnOnce(&mut T) -> U) -> Option<U> {
        self.0.try_with_lock(f)
    }
}

// ----------------------------------------------------

/// A fake mutex that allows borrowing data in local context.
///
/// Which means it does not provide any synchronization between threads,
pub struct FakeRawMutex {
    /// Prevent this from being sync
    _phantom: PhantomData<*mut ()>,
}

impl FakeRawMutex {
    /// Create a new `FakeRawMutex`.
    pub const fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
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
