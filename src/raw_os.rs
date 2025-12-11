use crate::os_trait::{
    AtomicNotifier, AtomicNotifyWaiter, FakeRawMutex, TickDelay, TickTimeoutNs, prelude::*,
};
use crate::timer::SysTickInstant;

/// RawOs implementation
///
/// # Safety
///
/// The sys_tick device should be setup before you use timeout or delay.
/// The FakeRawMutex does not provide any synchronization between threads.
pub struct RawOs;
impl OsInterface for RawOs {
    type RawMutex = FakeRawMutex;
    type Notifier = AtomicNotifier<RawOs>;
    type NotifyWaiter = AtomicNotifyWaiter<RawOs>;
    type Timeout = TickTimeoutNs<SysTickInstant>;

    fn os() -> Self {
        Self {}
    }

    fn yield_thread() {}

    fn delay() -> impl DelayNs {
        TickDelay::<SysTickInstant>::default()
    }

    fn notify() -> (Self::Notifier, Self::NotifyWaiter) {
        AtomicNotifier::<RawOs>::new()
    }
}
