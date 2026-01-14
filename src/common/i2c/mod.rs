pub mod bus_it;
pub mod mutex_device;

mod utils;

pub use crate::common::embedded_hal::i2c::NoAcknowledgeSource;
pub use mutex_device::I2cMutexDevice;

use crate::common::{
    embedded_hal::i2c::{ErrorKind, ErrorType, I2c, Operation, SevenBitAddress, TenBitAddress},
    fugit::HertzU32,
    prelude::*,
};

pub trait I2cPeriph {
    /// Disable all interrupt
    fn disable_all_interrupt(&mut self);

    /// Disable receiving data interrupt
    fn disable_data_interrupt(&mut self);

    fn is_tx_empty(&self) -> bool;
    fn unchecked_write(&mut self, data: u8);

    fn it_send_start(&mut self);

    /// # Returns
    /// - `Ok()`: finished
    /// - `Err(true)`: did something but hasn't finished
    /// - `Err(false)`: did nothing and need to wait
    fn it_prepare_write(&mut self, addr: Address, step: &mut u8) -> Result<(), bool>;

    /// # Returns
    /// - `Ok()`: finished
    /// - `Err(true)`: did something but hasn't finished
    /// - `Err(false)`: did nothing and need to wait
    fn it_prepare_read(
        &mut self,
        addr: Address,
        total_len: usize,
        last_operation: bool,
        step: &mut u8,
    ) -> Result<(), bool>;

    fn it_read(&mut self, left_len: usize, last_operation: bool) -> Option<u8>;

    fn send_stop(&mut self);
    fn is_stopped(&mut self) -> bool;
    fn is_slave_stopped(&mut self) -> bool;

    /// Read and clean the error flag
    fn get_and_clean_error(&mut self) -> Option<Error>;
    fn get_flag(&mut self, flag: Flag) -> bool;

    /// Perform an I2C software reset
    fn soft_reset(&mut self);
    fn handle_error(&mut self, err: Error);
    // fn read_sr(&mut self) -> u32;
}

pub trait I2cBusInterface {
    fn bus_transaction(
        &mut self,
        slave_addr: Address,
        operations: &mut [Operation<'_>],
    ) -> Result<(), Error>;
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
    Address10Sent,
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
