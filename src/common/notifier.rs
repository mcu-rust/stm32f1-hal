use fugit::MicrosDurationU32;

pub trait NotifierIsr {
    fn notify_from_isr(&mut self);
}

pub trait Notifier {
    fn notify(&mut self);
}

pub trait NotifyReceiver {
    /// Wait until notified or timeout occurs.
    /// # Returns
    ///   - `true` notified
    ///   - `false` timeout occurred
    fn take(&mut self, timeout: MicrosDurationU32) -> bool;
}

#[derive(Default)]
pub struct FakeNotifier;

impl Notifier for FakeNotifier {
    fn notify(&mut self) {}
}

impl NotifierIsr for FakeNotifier {
    fn notify_from_isr(&mut self) {}
}

#[derive(Default)]
pub struct FakeNotifyReceiver;

impl NotifyReceiver for FakeNotifyReceiver {
    fn take(&mut self, _timeout: MicrosDurationU32) -> bool {
        true
    }
}
