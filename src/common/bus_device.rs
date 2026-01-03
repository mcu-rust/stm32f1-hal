pub use crate::common::embedded_hal::spi::Operation;

use crate::common::embedded_hal::i2c;

pub trait BusDevice<WD: Word>: Send {
    fn transaction(&mut self, operations: &mut [Operation<'_, WD>]) -> Result<(), BusError>;

    #[inline]
    fn write_read(&mut self, write: &[WD], read: &mut [WD]) -> Result<(), BusError> {
        self.transaction(&mut [Operation::Write(write), Operation::Read(read)])
    }

    #[inline]
    fn read(&mut self, buf: &mut [WD]) -> Result<(), BusError> {
        self.transaction(&mut [Operation::Read(buf)])
    }

    #[inline]
    fn write(&mut self, buf: &[WD]) -> Result<(), BusError> {
        self.transaction(&mut [Operation::Write(buf)])
    }
}

pub trait BusDeviceTransfer<WD: Word>: BusDevice<WD> {
    /// Read data into the first buffer, while writing data from the second buffer.
    fn transfer(&mut self, read: &mut [WD], write: &[WD]) -> Result<(), BusError>;
    /// Write data out while reading data into the provided buffer.
    fn transfer_in_place(&mut self, buf: &mut [WD]) -> Result<(), BusError>;
}

pub use super::i2c::Address;
pub trait BusDeviceAddress<WD: Word>: BusDevice<WD> {
    fn set_address(&mut self, address: Address);
}

pub trait Word: Copy + 'static {}
impl Word for u8 {}
impl Word for u16 {}

pub fn from_spi_to_i2c_operation<'a>(value: Operation<'a, u8>) -> i2c::Operation<'a> {
    match value {
        Operation::Write(buf) => i2c::Operation::Write(buf),
        Operation::Read(buf) => i2c::Operation::Read(buf),
        _ => panic!(),
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BusError {
    Busy,
    ArbitrationLoss,
    NoAcknowledge,
    Timeout,
    Other,
}
