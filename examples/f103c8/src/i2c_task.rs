use crate::{hal::common::bus_device::*, os::*};

pub struct I2cTask<D>
where
    D: BusDevice<u8>,
{
    dev: D,
    buf: [u8; 8],
    interval: OsTimeoutState,
}

impl<D> I2cTask<D>
where
    D: BusDevice<u8>,
{
    pub fn new(dev: D) -> Self {
        Self {
            dev,
            buf: [0; 8],
            interval: OsTimeout::start_ms(100),
        }
    }

    pub fn poll(&mut self) {
        if self.interval.timeout() {
            self.dev.write_read(&[0x75], &mut self.buf[..1]).ok();
            // self.dev.write(&[0x1B, 0]).ok();
        }
    }
}
