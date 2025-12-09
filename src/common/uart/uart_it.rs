//! UART interrupt implementation

use super::*;
use crate::common::os::*;
use crate::ringbuf::*;
use core::marker::PhantomData;
use embedded_io::{ErrorType, Read, Write};

// TX -------------------------------------------------------------------------

pub struct UartInterruptTx<U, OS> {
    uart: U,
    timeout: MicrosDurationU32,
    flush_timeout: MicrosDurationU32,
    w: Producer<u8>,
    _os: PhantomData<OS>,
}

impl<U, OS> UartInterruptTx<U, OS>
where
    U: UartPeriph,
    OS: OsInterface,
{
    pub fn new(
        uart: [U; 2],
        buf_size: usize,
        baudrate: u32,
        timeout: MicrosDurationU32,
    ) -> (Self, UartInterruptTxHandler<U>) {
        let [uart, u2] = uart;
        let (w, r) = RingBuffer::<u8>::new(buf_size);
        (
            Self {
                uart,
                timeout,
                flush_timeout: calculate_timeout(baudrate, buf_size + buf_size / 2),
                w,
                _os: PhantomData,
            },
            UartInterruptTxHandler::new(u2, r),
        )
    }
}

impl<U: UartPeriph, OS: OsInterface> ErrorType for UartInterruptTx<U, OS> {
    type Error = Error;
}

impl<U: UartPeriph, OS: OsInterface> Write for UartInterruptTx<U, OS> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        if buf.is_empty() {
            return Err(Error::Other);
        }

        let mut t = OS::start_timeout(self.timeout);
        loop {
            if let n @ 1.. = self.w.push_slice(buf) {
                self.uart.set_interrupt(Event::TxEmpty, true);
                return Ok(n);
            } else if !self.uart.is_interrupt_enable(Event::TxEmpty) {
                self.uart.set_interrupt(Event::TxEmpty, true);
            }

            if t.timeout() {
                break;
            }
        }
        Err(Error::Busy)
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        let mut t = OS::start_timeout(self.flush_timeout);
        loop {
            if self.uart.is_tx_complete() && self.w.slots() == self.w.buffer().capacity() {
                return Ok(());
            } else if t.timeout() {
                break;
            } else if !self.uart.is_interrupt_enable(Event::TxEmpty) {
                self.uart.set_interrupt(Event::TxEmpty, true);
            }
        }
        Err(Error::Other)
    }
}

// TX interrupt -----------------

pub struct UartInterruptTxHandler<U> {
    uart: U,
    r: Consumer<u8>,
}

impl<U> UartInterruptTxHandler<U>
where
    U: UartPeriph,
{
    pub fn new(uart: U, r: Consumer<u8>) -> Self {
        Self { uart, r }
    }
}

impl<U> UartInterruptTxHandler<U>
where
    U: UartPeriph,
{
    pub fn handler(&mut self) {
        if let Ok(data) = self.r.peek() {
            if self.uart.write(*data as u16).is_ok() {
                self.r.pop().ok();
            }
        } else if self.uart.is_interrupt_enable(Event::TxEmpty) {
            self.uart.set_interrupt(Event::TxEmpty, false);
        }
    }
}

// RX -------------------------------------------------------------------------

pub struct UartInterruptRx<U, OS> {
    uart: U,
    timeout: MicrosDurationU32,
    r: Consumer<u8>,
    _os: PhantomData<OS>,
}

impl<U, OS> UartInterruptRx<U, OS>
where
    U: UartPeriph,
    OS: OsInterface,
{
    pub fn new(
        uart: [U; 2],
        buf_size: usize,
        timeout: MicrosDurationU32,
    ) -> (Self, UartInterruptRxHandler<U>) {
        let [uart, u2] = uart;
        let (w, r) = RingBuffer::<u8>::new(buf_size);
        (
            Self {
                uart,
                timeout,
                r,
                _os: PhantomData,
            },
            UartInterruptRxHandler::new(u2, w),
        )
    }
}

impl<U: UartPeriph, OS: OsInterface> ErrorType for UartInterruptRx<U, OS> {
    type Error = Error;
}

impl<U, OS> Read for UartInterruptRx<U, OS>
where
    U: UartPeriph,
    OS: OsInterface,
{
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        if buf.is_empty() {
            return Err(Error::Other);
        }

        let mut t = OS::start_timeout(self.timeout);
        loop {
            if let n @ 1.. = self.r.pop_slice(buf) {
                return Ok(n);
            } else if !self.uart.is_interrupt_enable(Event::RxNotEmpty) {
                self.uart.set_interrupt(Event::RxNotEmpty, true);
            }

            if t.timeout() {
                break;
            }
        }
        Err(Error::Other)
    }
}

// RX interrupt -----------------

pub struct UartInterruptRxHandler<U> {
    uart: U,
    w: Producer<u8>,
    // count: [u32; 10],
}

impl<U> UartInterruptRxHandler<U>
where
    U: UartPeriph,
{
    pub fn new(mut uart: U, w: Producer<u8>) -> Self {
        uart.set_interrupt(Event::RxNotEmpty, true);
        Self {
            uart,
            w,
            // count: [0; 10],
        }
    }

    pub fn handler(&mut self) {
        if let Ok(data) = self.uart.read() {
            self.w.push(data as u8).ok();
        }

        // match self.uart.read() {
        //     Ok(data) => match self.w.push(data as u8) {
        //         Ok(()) => self.count[0] = self.count[0].saturating_add(1),
        //         Err(_) => self.count[1] = self.count[1].saturating_add(1),
        //     },
        //     Err(nb::Error::WouldBlock) => self.count[2] = self.count[2].saturating_add(1),
        //     Err(nb::Error::Other(e)) => match e {
        //         Error::Overrun => self.count[3] = self.count[3].saturating_add(1),
        //         Error::Other => self.count[4] = self.count[4].saturating_add(1),
        //         Error::Noise => self.count[5] = self.count[5].saturating_add(1),
        //         Error::FrameFormat => self.count[6] = self.count[6].saturating_add(1),
        //         Error::Parity => self.count[7] = self.count[7].saturating_add(1),
        //         Error::Busy => self.count[8] = self.count[8].saturating_add(1),
        //     },
        // }
    }
}
