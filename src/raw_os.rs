use crate::common::os::*;
use crate::timer::syst::TIMEOUT;

pub struct RawOs;
impl OsInterface for RawOs {
    type RawMutex = FakeRawMutex;
    fn yield_thread() {}

    fn sleep(dur: MicrosDurationU32) {
        let mut t = TIMEOUT.start(dur);
        while !t.timeout() {}
    }

    fn start_timeout(dur: MicrosDurationU32) -> impl TimeoutStatus {
        TIMEOUT.start(dur)
    }

    fn notifier_isr() -> (impl NotifierIsr, impl NotifyWaiter) {
        AtomicNotifier::<RawOs>::new()
    }

    fn notifier() -> (impl Notifier, impl NotifyWaiter) {
        AtomicNotifier::<RawOs>::new()
    }
}
