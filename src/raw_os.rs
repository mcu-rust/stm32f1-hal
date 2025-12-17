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
    type Delay = TickDelay<SysTickInstant>;

    const O: Self = Self {};

    fn yield_thread() {}

    #[inline]
    fn timeout() -> Self::Timeout {
        TickTimeoutNs::<SysTickInstant>::new()
    }

    #[inline]
    fn delay() -> Self::Delay {
        TickDelay::<SysTickInstant>::default()
    }

    #[inline]
    fn notify() -> (Self::Notifier, Self::NotifyWaiter) {
        AtomicNotifier::<RawOs>::new()
    }
}
