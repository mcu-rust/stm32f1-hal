use crate::{embedded_hal::digital::StatefulOutputPin, os::*};

pub struct LedTask<P> {
    led: P,
    interval: Timeout,
}

impl<P: StatefulOutputPin> LedTask<P> {
    pub fn new(led: P) -> Self {
        Self {
            led,
            interval: Timeout::from_millis(500),
        }
    }

    pub fn poll(&mut self) {
        if self.interval.timeout() {
            self.led.toggle().ok();
        }
    }
}
