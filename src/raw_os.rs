use crate::os_trait::{
    AtomicNotifier, AtomicNotifyWaiter, FakeRawMutex, TickDelay, TickTimeoutNs, TickTimeoutState,
    prelude::*,
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
    type TimeoutState = TickTimeoutState<SysTickInstant>;
    type DelayNs = TickDelay<SysTickInstant>;

    const O: Self = Self {};

    fn yield_thread() {}

    fn delay() -> Self::DelayNs {
        TickDelay::<SysTickInstant>::default()
    }

    fn notify() -> (Self::Notifier, Self::NotifyWaiter) {
        AtomicNotifier::<RawOs>::new()
    }
}
