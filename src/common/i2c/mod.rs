mod i2c_bus_it;
mod i2c_device;
mod utils;

pub use crate::common::embedded_hal::i2c::{
    self, NoAcknowledgeSource, SevenBitAddress, TenBitAddress,
};
pub use i2c_bus_it::*;
pub use i2c_device::*;

use crate::common::{embedded_hal::i2c::ErrorKind, os_trait::prelude::*};

pub trait I2cPeriph {
    fn it_reset(&mut self);
    fn it_send_start(&mut self);
    fn it_start_write_data(&mut self) -> bool;
    fn it_start_read_data(&mut self, total_len: usize) -> bool;
    fn it_write(&mut self, data: u8) -> bool;
    /// # Returns
    /// - `None`: need to wait
    /// - `Some(true)`: Wrote a data
    /// - `Some(false)`: No new data
    fn it_write_with(&mut self, f: impl FnOnce() -> Option<u8>) -> Option<bool>;
    fn it_read(&mut self, left_len: usize) -> Option<u8>;

    fn send_stop(&mut self);
    fn is_stopped(&mut self, master_mode: bool) -> bool;

    /// Read and clean the flag
    fn get_and_clean_error(&mut self) -> Option<Error>;
    fn get_flag(&mut self, flag: Flag) -> bool;
}

pub trait I2cPeriphAddress<A: AddressMode>: I2cPeriph {
    fn it_send_slave_addr(&mut self, address: A, read: bool) -> bool;
}

pub trait I2cBusInterface<A: AddressMode> {
    fn write_read(
        &mut self,
        slave_addr: A,
        write: &[&[u8]],
        read: &mut [&mut [u8]],
    ) -> Result<(), Error>;
}

pub trait AddressMode: Copy + PartialEq {
    fn from_u16(v: u16) -> Self;
    fn into_u16(self) -> u16;
}
impl AddressMode for SevenBitAddress {
    #[inline]
    fn from_u16(v: u16) -> Self {
        v as Self
    }
    #[inline]
    fn into_u16(self) -> u16 {
        self as u16
    }
}
impl AddressMode for TenBitAddress {
    #[inline]
    fn from_u16(v: u16) -> Self {
        v as Self
    }
    #[inline]
    fn into_u16(self) -> u16 {
        self as u16
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Flag {
    /// Start condition generated
    Started,
    /// Busy
    Busy,
    /// Address is sent in master mode or received and matches in slave mode
    AddressSent,
    /// Byte transfer finished
    ByteTransferFinished,
    /// 10-bit header sent
    Address10,
    /// Data register not empty
    RxNotEmpty,
    /// Data register empty
    TxEmpty,
    /// SMBus alert
    MasterSlave,
    /// Master/Slave
    Transmitter,
    /// General call address (Slave mode)
    GeneralCall,
    /// SMBus device default address (Slave mode)
    SMBusDefault,
    /// SMBus host header (Slave mode)
    SMBusHost,
    /// Dual flag (Slave mode)
    Dual,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Address {
    Seven(u8),
    Ten(u16),
}

impl From<u8> for Address {
    fn from(value: u8) -> Self {
        Self::Seven(value)
    }
}

impl From<u16> for Address {
    fn from(value: u16) -> Self {
        Self::Ten(value)
    }
}

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
#[non_exhaustive]
pub enum Error {
    Busy,
    /// Overrun/underrun
    Overrun,
    /// No ack received
    NoAcknowledge(NoAcknowledgeSource),
    Timeout,
    /// Bus error
    Bus,
    Crc,
    /// Arbitration was lost
    ArbitrationLoss,
    /// SMBus alert
    SMBusAlert,
    /// SMBus PEC Error in reception
    Pec,
    /// SMBus timeout
    SMBusTimeout,
    Buffer,
    Other,
}

impl Error {
    pub(crate) fn nack_addr(self) -> Self {
        match self {
            Self::NoAcknowledge(NoAcknowledgeSource::Unknown) => {
                Self::NoAcknowledge(NoAcknowledgeSource::Address)
            }
            e => e,
        }
    }
    pub(crate) fn nack_data(self) -> Self {
        match self {
            Self::NoAcknowledge(NoAcknowledgeSource::Unknown) => {
                Self::NoAcknowledge(NoAcknowledgeSource::Data)
            }
            e => e,
        }
    }
}

impl embedded_hal::i2c::Error for Error {
    fn kind(&self) -> ErrorKind {
        match *self {
            Self::Overrun => ErrorKind::Overrun,
            Self::Bus => ErrorKind::Bus,
            Self::ArbitrationLoss => ErrorKind::ArbitrationLoss,
            Self::NoAcknowledge(nack) => ErrorKind::NoAcknowledge(nack),
            Self::Crc
            | Self::Timeout
            | Self::SMBusAlert
            | Self::SMBusTimeout
            | Self::Pec
            | Self::Other
            | Self::Busy
            | Self::Buffer => ErrorKind::Other,
        }
    }
}
