//! We use the [`mutex-traits`](https://crates.io/crates/mutex-traits) crate to provide mutex functionality.
//! You need to select an appropriate mutex implementation based on your needs.
//!
//! And you can implement your own mutex by implementing the `RawMutex` trait from the `mutex-traits` crate.
//!
//! ```
//! use stm32f1_hal::mutex;
//! cfg_if::cfg_if! {
//!     if #[cfg(feature = "std")] {
//!         use mutex::StdRawMutex;
//!         type Mutex<T> = mutex::Mutex<StdRawMutex, T>;
//!     } else {
//!         use mutex::FakeRawMutex;
//!         type Mutex<T> = mutex::Mutex<FakeRawMutex, T>;
//!     }
//! }
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

pub mod dma;
pub mod notifier;
pub mod os;
pub mod ringbuf;
pub mod simplest_heap;
pub mod timer;
pub mod uart;
pub mod wrap_trait;

cfg_if::cfg_if! {
    if #[cfg(all(feature = "std", not(feature = "custom-std-mutex")))] {
        pub mod std_mutex;
        pub use std_mutex as mutex;
    } else {
        pub mod mutex;
    }
}
