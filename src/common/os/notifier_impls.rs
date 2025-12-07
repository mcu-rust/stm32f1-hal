use super::*;

#[derive(Default)]
pub struct FakeNotifier;

impl FakeNotifier {
    pub fn new() -> (Self, Self) {
        (Self {}, Self {})
    }
}

impl Notifier for FakeNotifier {
    fn notify(&mut self) {}
}

impl NotifierIsr for FakeNotifier {
    fn notify_from_isr(&mut self) {}
}

impl NotifyReceiver for FakeNotifier {
    fn take(&mut self, _timeout: MicrosDurationU32) -> bool {
        true
    }
}

#[cfg(feature = "std")]
pub use std_impl::*;
#[cfg(feature = "std")]
mod std_impl {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use std::time::Instant;

    /// This implementation is only for unit testing.
    pub struct StdNotifier {
        flag: Arc<AtomicBool>,
    }

    impl StdNotifier {
        pub fn new() -> (Self, StdNotifyReceiver) {
            let s = Self {
                flag: Arc::new(AtomicBool::new(false)),
            };
            let r = StdNotifyReceiver {
                flag: Arc::clone(&s.flag),
            };
            (s, r)
        }
    }

    impl Notifier for StdNotifier {
        fn notify(&mut self) {
            self.flag.store(true, Ordering::Release)
        }
    }

    impl NotifierIsr for StdNotifier {
        fn notify_from_isr(&mut self) {
            self.flag.store(true, Ordering::Release)
        }
    }

    /// This implementation is only for unit testing.
    pub struct StdNotifyReceiver {
        flag: Arc<AtomicBool>,
    }

    impl NotifyReceiver for StdNotifyReceiver {
        fn take(&mut self, timeout: MicrosDurationU32) -> bool {
            let now = Instant::now();
            while now.elapsed().as_micros() < timeout.ticks().into() {
                if self
                    .flag
                    .compare_exchange(true, false, Ordering::SeqCst, Ordering::Acquire)
                    .is_ok()
                {
                    return true;
                }
                std::thread::yield_now();
            }
            false
        }
    }
}
