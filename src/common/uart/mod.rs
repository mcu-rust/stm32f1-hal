mod uart_dma;
mod uart_it;
mod uart_poll;

pub use uart_dma::*;
pub use uart_it::*;
pub use uart_poll::*;

use crate::common::{
    embedded_hal_nb as e_nb, embedded_io as e_io, fugit::MicrosDurationU32, prelude::*,
};
use core::fmt::Display;

// ------------------------------------------------------------------------------------------------

// UART idle interrupt handler
pub struct UartIdleInterrupt<U: UartPeriph> {
    uart: U,
}

impl<U: UartPeriph> UartIdleInterrupt<U> {
    pub fn new(uart: U) -> Self {
        Self { uart }
    }

    #[inline]
    pub fn is_interrupted(&mut self) -> bool {
        self.uart.is_interrupted(Event::Idle)
    }

    #[inline]
    pub fn listen(&mut self) {
        self.uart.set_interrupt(Event::Idle, true);
    }

    #[inline]
    pub fn unlisten(&mut self) {
        self.uart.set_interrupt(Event::Idle, false);
    }
}

// Peripheral Trait -----------------------------------------------------------

pub trait UartPeriph {
    fn write(&mut self, word: u16) -> nb::Result<(), Error>;
    /// # Returns
    /// - `None`: need to wait
    /// - `Some(true)`: Wrote a data
    /// - `Some(false)`: No new data
    fn write_with(&mut self, f: impl FnOnce() -> Option<u16>) -> Option<bool>;
    /// Transfer is empty and completed
    fn is_tx_complete(&self) -> bool;

    fn read(&mut self) -> nb::Result<u16, Error>;

    fn set_interrupt(&mut self, event: Event, enable: bool);
    fn is_interrupt_enable(&mut self, event: Event) -> bool;
    /// Read and clean the flag
    fn is_interrupted(&mut self, event: Event) -> bool;

    fn clear_err_flag(&self);
}

pub trait UartPeriphWithDma: UartPeriph {
    fn get_tx_data_reg_addr(&self) -> usize;
    fn get_rx_data_reg_addr(&self) -> usize;
    fn enable_dma_tx(&mut self, enable: bool);
    fn enable_dma_rx(&mut self, enable: bool);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// New data can be sent
    TxEmpty,
    /// New data has been received
    RxNotEmpty,
    /// Idle line state detected
    Idle,
}

/// UART error
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// The peripheral receive buffer was overrun.
    Overrun,
    /// Received data does not conform to the peripheral configuration.
    /// Can be caused by a misconfigured device on either end of the serial line.
    FrameFormat,
    /// Parity check failed.
    Parity,
    /// UART line is too noisy to read valid data.
    Noise,
    /// UART is busy and cannot accept new data.
    Busy,
    /// A different error occurred. The original error may contain more information.
    Other,
}

impl Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Overrun => write!(f, "UART overrun error"),
            Self::FrameFormat => write!(f, "UART frame format error"),
            Self::Parity => write!(f, "UART parity error"),
            Self::Noise => write!(f, "UART noise error"),
            Self::Busy => write!(f, "UART busy"),
            Self::Other => write!(f, "UART other error"),
        }
    }
}

impl core::error::Error for Error {}

impl embedded_io::Error for Error {
    #[inline]
    fn kind(&self) -> e_io::ErrorKind {
        match self {
            Self::Overrun => e_io::ErrorKind::InvalidData,
            Self::FrameFormat => e_io::ErrorKind::InvalidData,
            Self::Parity => e_io::ErrorKind::InvalidData,
            Self::Noise => e_io::ErrorKind::InvalidData,
            Self::Busy => e_io::ErrorKind::WriteZero,
            Self::Other => e_io::ErrorKind::Other,
        }
    }
}

impl e_nb::serial::Error for Error {
    #[inline]
    fn kind(&self) -> e_nb::serial::ErrorKind {
        match self {
            Self::Overrun => e_nb::serial::ErrorKind::Overrun,
            Self::FrameFormat => e_nb::serial::ErrorKind::FrameFormat,
            Self::Parity => e_nb::serial::ErrorKind::Parity,
            Self::Noise => e_nb::serial::ErrorKind::Noise,
            Self::Busy => e_nb::serial::ErrorKind::Other,
            Self::Other => e_nb::serial::ErrorKind::Other,
        }
    }
}

pub enum WordLength {
    /// When parity is enabled, a word has 7 data bits + 1 parity bit,
    /// otherwise 8 data bits.
    Bits8,
    /// When parity is enabled, a word has 8 data bits + 1 parity bit,
    /// otherwise 9 data bits.
    Bits9,
}

pub enum Parity {
    ParityNone,
    ParityEven,
    ParityOdd,
}

pub enum StopBits {
    /// 1 stop bit
    STOP1,
    /// 0.5 stop bits
    STOP0P5,
    /// 2 stop bits
    STOP2,
    /// 1.5 stop bits
    STOP1P5,
}

pub struct Config {
    pub baudrate: u32,
    pub word_length: WordLength,
    pub parity: Parity,
    pub stop_bits: StopBits,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            baudrate: 115_200,
            word_length: WordLength::Bits8,
            parity: Parity::ParityNone,
            stop_bits: StopBits::STOP1,
        }
    }
}

impl Config {
    pub fn baudrate(mut self, baudrate: u32) -> Self {
        self.baudrate = baudrate;
        self
    }

    pub fn word_length(mut self, wordlength: WordLength) -> Self {
        self.word_length = wordlength;
        self
    }

    pub fn word_length_8bits(mut self) -> Self {
        self.word_length = WordLength::Bits8;
        self
    }

    pub fn word_length_9bits(mut self) -> Self {
        self.word_length = WordLength::Bits9;
        self
    }

    pub fn parity(mut self, parity: Parity) -> Self {
        self.parity = parity;
        self
    }

    pub fn parity_none(mut self) -> Self {
        self.parity = Parity::ParityNone;
        self
    }

    pub fn parity_even(mut self) -> Self {
        self.parity = Parity::ParityEven;
        self
    }

    pub fn parity_odd(mut self) -> Self {
        self.parity = Parity::ParityOdd;
        self
    }

    pub fn stop_bits(mut self, stop_bits: StopBits) -> Self {
        self.stop_bits = stop_bits;
        self
    }
}

#[inline]
const fn calculate_timeout(baudrate: u32, data_len: usize) -> MicrosDurationU32 {
    let bytes_per_sec = baudrate / 12;
    MicrosDurationU32::micros((data_len * 1_000_000) as u32 / bytes_per_sec)
}
