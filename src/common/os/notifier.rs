use super::*;

pub trait NotifierIsr: Send {
    fn notify_from_isr(&mut self);
}

pub trait Notifier: Send {
    fn notify(&mut self);
}

pub trait NotifyWaiter: Send {
    /// Wait until notified or timeout occurs.
    /// # Returns
    ///   - `true` notified
    ///   - `false` timeout occurred
    fn wait(&mut self, timeout: MicrosDurationU32) -> bool;
}
