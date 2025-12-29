use crate::{hal::common::bus_device::*, os::*};

pub struct I2cTask<D>
where
    D: BusDevice<u8>,
{
    dev: D,
    buf: [u8; 16],
    interval: OsTimeout,
    step: u8,
}

// For MPU-6050
const REG_CONFIG: u8 = 0x1A;
const REG_TEMPERATURE: u8 = 0x41;
const REG_POWER_1: u8 = 0x6B;
const REG_WHO_AM_I: u8 = 0x75;

impl<D> I2cTask<D>
where
    D: BusDevice<u8>,
{
    pub fn new(dev: D) -> Self {
        Self {
            dev,
            buf: [0; 16],
            interval: OsTimeout::from_millis(100),
            step: 0,
        }
    }

    pub fn poll(&mut self) {
        if self.interval.timeout() {
            if self.step == 0 {
                self.dev.write(&[REG_POWER_1, 0]).unwrap();
                OS::delay().delay_ms(2);
                self.dev
                    .write(&[REG_CONFIG, 0x03, 2 << 3, 1 << 3, 0x33])
                    .unwrap();
                self.step += 1;
            } else {
                self.dev
                    .write_read(&[REG_WHO_AM_I], &mut self.buf[..1])
                    .unwrap();
                self.dev
                    .write_read(&[REG_TEMPERATURE], &mut self.buf[1..3])
                    .unwrap();
                self.dev
                    .write_read(&[REG_CONFIG], &mut self.buf[3..7])
                    .unwrap();
            }
        }
    }
}
