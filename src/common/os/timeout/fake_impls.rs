use super::*;

#[derive(Default)]
pub struct FakeTimeout {}

impl Timeout for FakeTimeout {
    #[inline]
    fn start(&self, timeout: MicrosDurationU32) -> impl TimeoutStatus {
        FakeTimeoutStatus::new(timeout)
    }
}

pub struct FakeTimeoutStatus {
    timeout: MicrosDurationU32,
    count: u32,
}

impl FakeTimeoutStatus {
    pub fn new(timeout: MicrosDurationU32) -> Self {
        Self { timeout, count: 0 }
    }
}

impl TimeoutStatus for FakeTimeoutStatus {
    #[inline]
    fn timeout(&mut self) -> bool {
        self.count += 1;
        self.count >= self.timeout.ticks()
    }

    #[inline(always)]
    fn restart(&mut self) {
        self.count = 0;
    }
}
