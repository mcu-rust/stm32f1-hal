mod utils;

pub mod bus_it;
pub mod device;

pub use crate::fugit::{HertzU32, KilohertzU32};
pub use device::*;
pub use embedded_hal::spi::{Mode, Phase, Polarity};

use crate::common::prelude::*;
use embedded_hal::spi::{ErrorKind, ErrorType, Operation};

pub trait SpiPeriph {
    /// master mode only
    /// # Return
    /// - `true`: changed
    /// - `false`: no changes
    fn config<W: Word>(&mut self, mode: Mode, freq: KilohertzU32) -> bool;

    fn is_tx_empty(&self) -> bool;
    fn uncheck_write<W: Word>(&mut self, data: W);
    fn read<W: Word>(&mut self) -> Option<W>;
    fn is_busy(&self) -> bool;
    fn get_and_clean_error(&mut self) -> Option<Error>;

    fn set_interrupt(&mut self, event: Event, enable: bool);
    /// Disable all interrupt
    fn disable_all_interrupt(&mut self);
}

pub trait SpiBusInterface {
    fn transaction<W: Word>(&mut self, operations: &mut [Operation<'_, W>]) -> Result<(), Error>;
    /// config mode and frequency
    fn config<W: Word>(&mut self, mode: Mode, freq: KilohertzU32);
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    TxEmpty,
    RxNotEmpty,
    Error,
}

/// SPI error
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum Error {
    /// Overrun occurred
    Overrun,
    /// Underrun occurred. I2S only.
    Underrun,
    /// Mode fault occurred
    ModeFault,
    /// CRC error
    Crc,
    ChipSelectFault,
    Busy,
    Buffer,
    Timeout,
    Other,
}

impl embedded_hal::spi::Error for Error {
    fn kind(&self) -> ErrorKind {
        match *self {
            Self::Overrun => ErrorKind::Overrun,
            Self::ModeFault => ErrorKind::ModeFault,
            Self::Crc => ErrorKind::FrameFormat,
            Self::ChipSelectFault => ErrorKind::ChipSelectFault,
            Self::Busy | Self::Buffer | Self::Underrun | Self::Other | Self::Timeout => {
                ErrorKind::Other
            }
        }
    }
}

pub trait Word: Copy + Default + Sized + 'static {
    fn into_u32(self) -> u32;
    fn from_u32(v: u32) -> Self;
}
impl Word for u8 {
    fn into_u32(self) -> u32 {
        self as u32
    }
    fn from_u32(v: u32) -> Self {
        v as Self
    }
}
impl Word for u16 {
    fn into_u32(self) -> u32 {
        self as u32
    }
    fn from_u32(v: u32) -> Self {
        v as Self
    }
}
impl Word for u32 {
    fn into_u32(self) -> u32 {
        self
    }
    fn from_u32(v: u32) -> Self {
        v as Self
    }
}
