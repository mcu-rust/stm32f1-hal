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
    fn transfer(&mut self, read: &mut [WD], write: &[WD]) -> Result<(), BusError> {
        self.transaction(&mut [Operation::Transfer(read, write)])
    }

    /// Write data out while reading data into the provided buffer.
    fn transfer_in_place(&mut self, buf: &mut [WD]) -> Result<(), BusError> {
        self.transaction(&mut [Operation::TransferInPlace(buf)])
    }
}

pub use super::i2c::Address;
pub trait BusDeviceAddress<WD: Word>: BusDevice<WD> {
    fn set_address(&mut self, address: Address);
}

pub trait Word: Copy + 'static {}
impl Word for u8 {}
impl Word for u16 {}

pub trait IntoI2cOperation {
    fn get_read_buf(&mut self) -> Option<&mut [u8]>;
    fn get_write_buf(&self) -> Option<&[u8]>;
}

impl<'a> IntoI2cOperation for Operation<'a, u8> {
    #[inline]
    fn get_read_buf(&mut self) -> Option<&mut [u8]> {
        match self {
            Operation::Read(buf) => Some(buf),
            _ => None,
        }
    }

    #[inline]
    fn get_write_buf(&self) -> Option<&[u8]> {
        match self {
            Operation::Write(buf) => Some(buf),
            _ => None,
        }
    }
}

impl<'a> IntoI2cOperation for i2c::Operation<'a> {
    #[inline]
    fn get_read_buf(&mut self) -> Option<&mut [u8]> {
        match self {
            i2c::Operation::Read(buf) => Some(buf),
            _ => None,
        }
    }

    #[inline]
    fn get_write_buf(&self) -> Option<&[u8]> {
        match self {
            i2c::Operation::Write(buf) => Some(buf),
            _ => None,
        }
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

impl<WD: Word, T: BusDevice<WD> + ?Sized> BusDevice<WD> for &mut T {
    #[inline]
    fn transaction(&mut self, operations: &mut [Operation<'_, WD>]) -> Result<(), BusError> {
        T::transaction(self, operations)
    }

    #[inline]
    fn write_read(&mut self, write: &[WD], read: &mut [WD]) -> Result<(), BusError> {
        T::write_read(self, write, read)
    }

    #[inline]
    fn read(&mut self, buf: &mut [WD]) -> Result<(), BusError> {
        T::read(self, buf)
    }

    #[inline]
    fn write(&mut self, buf: &[WD]) -> Result<(), BusError> {
        T::write(self, buf)
    }
}
