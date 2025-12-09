//! See [`OsInterface`]

pub mod atomic_mutex;
pub mod mutex_impls;
pub mod notifier;
pub mod notifier_impls;
pub mod os_impls;
pub mod timeout;

pub use mutex_impls::*;
pub use notifier::*;
pub use notifier_impls::*;
pub use os_impls::*;
pub use timeout::*;

pub use fugit::{ExtU32, MicrosDurationU32};

use mutex_traits::{ConstInit, RawMutex};

/// Adapter for different operating systems.
///
/// We use the [`mutex-traits`](https://crates.io/crates/mutex-traits) crate to provide mutex functionality.
/// You need to select an appropriate mutex implementation based on your needs.
/// And you can implement your own mutex by implementing the `RawMutex` trait from the `mutex-traits` crate.
///
/// ```
/// use stm32f1_hal::common::os::*;
///
/// fn os_interface<OS: OsInterface>() {
///     let mutex = OS::mutex(2);
///
///     let mut guard = mutex.try_lock().unwrap();
///     assert_eq!(*guard, 2);
///
///     OS::yield_thread();
///     OS::sleep(1.millis());
/// }
///
/// fn select_os() {
///     os_interface::<FakeOs>();
///     os_interface::<StdOs>();
/// }
/// ```
pub trait OsInterface: Send + Sync {
    type RawMutex: ConstInit + RawMutex;

    #[inline]
    fn mutex<T>(d: T) -> BlockingMutex<Self::RawMutex, T> {
        BlockingMutex::new(d)
    }

    fn yield_thread();
    fn sleep(dur: MicrosDurationU32);
    fn start_timeout(dur: MicrosDurationU32) -> impl TimeoutStatus;
    fn notifier_isr() -> (impl NotifierIsr, impl NotifyWaiter);
    fn notifier() -> (impl Notifier, impl NotifyWaiter);
}
