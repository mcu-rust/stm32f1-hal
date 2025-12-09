//! It doesn't depend on DMA or interrupts, relying instead on continuous polling.

use super::*;
use crate::common::os::*;
use embedded_hal_nb as e_nb;
use embedded_io as e_io;

// TX -------------------------------------------------------------------------

pub struct UartPollTx<U, W> {
    uart: U,
    timeout: W,
    flush_timeout: W,
}

impl<U: UartPeriph, W: Waiter> UartPollTx<U, W> {
    pub fn new(uart: U, timeout: W, flush_timeout: W) -> Self {
        Self {
            uart,
            timeout,
            flush_timeout,
        }
    }
}

impl<U: UartPeriph, W: Waiter> e_nb::serial::ErrorType for UartPollTx<U, W> {
    type Error = Error;
}
impl<U: UartPeriph, W: Waiter> e_io::ErrorType for UartPollTx<U, W> {
    type Error = Error;
}

// NB Write ----

impl<U: UartPeriph, W: Waiter> e_nb::serial::Write<u16> for UartPollTx<U, W> {
    #[inline]
    fn write(&mut self, word: u16) -> nb::Result<(), Self::Error> {
        self.uart.write(word)
    }

    #[inline]
    fn flush(&mut self) -> nb::Result<(), Self::Error> {
        if self.uart.is_tx_complete() {
            return Ok(());
        }
        Err(nb::Error::WouldBlock)
    }
}

// IO Write ----

impl<U: UartPeriph, W: Waiter> e_io::Write for UartPollTx<U, W> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        if buf.is_empty() {
            return Err(Error::Other);
        }

        // try first data
        let mut t = self.timeout.start();
        let rst = loop {
            let rst = self.uart.write(buf[0] as u16);
            if let Err(nb::Error::WouldBlock) = rst {
                if t.timeout() {
                    break rst;
                }
            } else {
                break rst;
            }
        };

        match rst {
            Ok(()) => (),
            Err(nb::Error::WouldBlock) => return Err(Error::Busy),
            Err(nb::Error::Other(_)) => return Err(Error::Other),
        }

        // write rest data
        for (i, &data) in buf[1..buf.len()].iter().enumerate() {
            if self.uart.write(data as u16).is_err() {
                return Ok(i + 1);
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        let mut t = self.flush_timeout.start();
        loop {
            if self.uart.is_tx_complete() {
                return Ok(());
            }

            if t.timeout() {
                break;
            }
        }
        Err(Error::Other)
    }
}

// RX -------------------------------------------------------------------------

pub struct UartPollRx<U, W> {
    uart: U,
    timeout: W,
    continue_timeout: W,
}

impl<U: UartPeriph, W: Waiter> UartPollRx<U, W> {
    pub fn new(uart: U, timeout: W, continue_timeout: W) -> Self {
        Self {
            uart,
            timeout,
            continue_timeout,
        }
    }
}

impl<U: UartPeriph, W: Waiter> e_nb::serial::ErrorType for UartPollRx<U, W> {
    type Error = Error;
}
impl<U: UartPeriph, W: Waiter> e_io::ErrorType for UartPollRx<U, W> {
    type Error = Error;
}

// NB Read ----

impl<U: UartPeriph, W: Waiter> e_nb::serial::Read<u16> for UartPollRx<U, W> {
    #[inline]
    fn read(&mut self) -> nb::Result<u16, Self::Error> {
        self.uart.read()
    }
}

// IO Read ----

impl<U: UartPeriph, W: Waiter> e_io::Read for UartPollRx<U, W> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        if buf.is_empty() {
            return Err(Error::Other);
        }

        // try first data
        let mut t = self.timeout.start();
        let rst = loop {
            let rst = self.uart.read();
            if let Err(nb::Error::WouldBlock) = rst {
                if t.timeout() {
                    break rst;
                }
            } else {
                break rst;
            }
        };

        match rst {
            Ok(data) => buf[0] = data as u8,
            _ => return Err(Error::Other),
        }

        let mut t = self.continue_timeout.start();
        let mut n = 1;
        while n < buf.len() {
            match self.uart.read() {
                Ok(data) => {
                    buf[n] = data as u8;
                    n += 1;
                    t.restart();
                }
                Err(nb::Error::Other(_)) => return Ok(n),
                Err(nb::Error::WouldBlock) => {
                    if t.timeout() {
                        return Ok(n);
                    }
                }
            }
        }
        Ok(buf.len())
    }
}
