use crate::{TimeoutState, embedded_hal::digital::StatefulOutputPin};

pub struct LedTask<P, T> {
    led: P,
    timeout: T,
}

impl<P: StatefulOutputPin, T: TimeoutState> LedTask<P, T> {
    pub fn new(led: P, timeout: T) -> Self {
        Self { led, timeout }
    }

    pub fn poll(&mut self) {
        if self.timeout.timeout() {
            self.led.toggle().ok();
        }
    }
}
