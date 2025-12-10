use crate::os_trait::{AtomicNotifier, FakeRawMutex, TickDelay, TickTimeoutNs, prelude::*};
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
    type NotifyBuilder = AtomicNotifier<RawOs>;
    type Timeout = TickTimeoutNs<SysTickInstant>;

    fn yield_thread() {}

    fn delay() -> impl DelayNs {
        TickDelay::<SysTickInstant>::default()
    }
}
