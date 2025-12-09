use stm32f1_hal::os::TimeoutStatus;

use crate::embedded_hal::digital::StatefulOutputPin;

pub struct LedTask<P, T> {
    led: P,
    timeout: T,
}

impl<P: StatefulOutputPin, T: TimeoutStatus> LedTask<P, T> {
    pub fn new(led: P, timeout: T) -> Self {
        Self { led, timeout }
    }

    pub fn poll(&mut self) {
        if self.timeout.timeout() {
            self.led.toggle().ok();
        }
    }
}
