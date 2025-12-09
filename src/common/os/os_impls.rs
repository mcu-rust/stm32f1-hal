use super::*;
cfg_if::cfg_if! {
    if #[cfg(feature = "std")] {
        pub use std::sync::Arc;

        use std::{thread, time::Duration};
        use super::timeout::std_impls::*;
    } else {
        pub use alloc::vec::Vec;
        pub use alloc::boxed::Box;
        pub use alloc::sync::Arc;
    }
}

// STD --------------------------------------------------------------

/// This implementation is only for unit testing.
pub struct StdOs;
#[cfg(feature = "std")]
impl OsInterface for StdOs {
    type RawMutex = FakeRawMutex;
    fn yield_thread() {
        thread::yield_now();
    }

    fn sleep(dur: MicrosDurationU32) {
        thread::sleep(Duration::from_micros(dur.ticks().into()))
    }

    fn start_timeout(dur: MicrosDurationU32) -> impl TimeoutStatus {
        StdTimeoutStatus::new(dur)
    }

    fn notifier_isr() -> (impl NotifierIsr, impl NotifyWaiter) {
        StdNotifier::new()
    }

    fn notifier() -> (impl Notifier, impl NotifyWaiter) {
        StdNotifier::new()
    }
}

// Fake -------------------------------------------------------------

pub struct FakeOs;
impl OsInterface for FakeOs {
    type RawMutex = FakeRawMutex;
    fn yield_thread() {}
    fn sleep(_dur: MicrosDurationU32) {}
    fn start_timeout(dur: MicrosDurationU32) -> impl TimeoutStatus {
        FakeTimeoutStatus::new(dur)
    }
    fn notifier_isr() -> (impl NotifierIsr, impl NotifyWaiter) {
        FakeNotifier::new()
    }
    fn notifier() -> (impl Notifier, impl NotifyWaiter) {
        FakeNotifier::new()
    }
}

// Tests ------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn os_interface<OS: OsInterface>() {
        let mutex = OS::mutex(0);

        let mut guard = mutex.try_lock().unwrap();
        assert_eq!(*guard, 0);
        *guard = 4;
        drop(guard);

        mutex
            .try_with_lock(|data| {
                assert_eq!(*data, 4);
                *data = 5;
            })
            .unwrap();

        OS::yield_thread();
        OS::sleep(1.millis());

        let (mut n, mut r) = OS::notifier_isr();
        n.notify_from_isr();
        assert!(r.wait(1.millis()));

        let (mut n, mut r) = OS::notifier();
        n.notify();
        assert!(r.wait(1.millis()));
    }

    #[test]
    fn select_os() {
        os_interface::<FakeOs>();
        os_interface::<StdOs>();
    }
}
