//! We use the [`mutex-traits`](https://crates.io/crates/mutex-traits) crate to provide mutex functionality.
//! You need to select an appropriate mutex implementation based on your needs.
//!
//! And you can implement your own mutex by implementing the `RawMutex` trait from the `mutex-traits` crate.
//!
//! ```
//! use stm32f1_hal::mutex;
//! type Mutex<T> = mutex::Mutex<mutex::FakeRawMutex, T>;
//!
//! let mutex = Mutex::<u32>::new(0);
//!
//! let mut guard = mutex.try_lock().unwrap();
//! assert_eq!(*guard, 0);
//! *guard = 4;
//! drop(guard);
//!
//! mutex
//!     .try_with_lock(|data| {
//!         assert_eq!(*data, 4);
//!         *data = 5;
//!     })
//!     .unwrap();
//! ```

pub mod atomic_mutex;
#[cfg(feature = "std")]
mod std_mutex;

cfg_if::cfg_if! {
    if #[cfg(all(feature = "std", not(feature = "custom-std-mutex")))] {
        pub use std_mutex::Mutex;
    } else {
        pub use mutex::BlockingMutex as Mutex;
    }
}

use mutex_traits::{ConstInit, RawMutex};

// ----------------------------------------------------

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
