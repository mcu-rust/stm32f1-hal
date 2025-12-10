use crate::os_trait::{AtomicNotifier, FakeRawMutex, TickDelay, TickTimeoutNs, prelude::*};
use crate::timer::syst::SysTickInstant;

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
