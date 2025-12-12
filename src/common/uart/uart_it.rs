//! UART interrupt implementation

use super::*;
use crate::common::{
    embedded_io::{ErrorType, Read, Write},
    ringbuf::*,
};

// TX -------------------------------------------------------------------------

pub struct UartInterruptTx<U, OS: OsInterface> {
    uart: U,
    timeout: MicrosDurationU32,
    flush_timeout: MicrosDurationU32,
    w: Producer<u8>,
    waiter: OS::NotifyWaiter,
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
    ) -> (Self, UartInterruptTxHandler<U, OS>) {
        let (notifier, waiter) = OS::notify();
        let [uart, u2] = uart;
        let (w, r) = RingBuffer::<u8>::new(buf_size);
        (
            Self {
                uart,
                timeout,
                flush_timeout: calculate_timeout(baudrate, buf_size + 10),
                w,
                waiter,
            },
            UartInterruptTxHandler::new(u2, r, notifier),
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

        self.waiter
            .wait_with(OS::O, self.timeout, 2, || {
                if let n @ 1.. = self.w.push_slice(buf) {
                    self.uart.set_interrupt(Event::TxEmpty, true);
                    return Some(n);
                } else if !self.uart.is_interrupt_enable(Event::TxEmpty) {
                    self.uart.set_interrupt(Event::TxEmpty, true);
                }
                None
            })
            .ok_or(Error::Busy)
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        self.waiter
            .wait_with(OS::O, self.flush_timeout, 4, || {
                if self.uart.is_tx_complete() && self.w.slots() == self.w.buffer().capacity() {
                    return Some(());
                } else if !self.uart.is_interrupt_enable(Event::TxEmpty) {
                    self.uart.set_interrupt(Event::TxEmpty, true);
                }
                None
            })
            .ok_or(Error::Other)
    }
}

// TX interrupt -----------------

pub struct UartInterruptTxHandler<U, OS: OsInterface> {
    uart: U,
    r: Consumer<u8>,
    notifier: OS::Notifier,
}

impl<U, OS> UartInterruptTxHandler<U, OS>
where
    U: UartPeriph,
    OS: OsInterface,
{
    pub fn new(uart: U, r: Consumer<u8>, notifier: OS::Notifier) -> Self {
        Self { uart, r, notifier }
    }
}

impl<U, OS> UartInterruptTxHandler<U, OS>
where
    U: UartPeriph,
    OS: OsInterface,
{
    pub fn handler(&mut self) {
        if let Some(has_data) = self.uart.write_with(|| {
            let data = self.r.pop();
            data.map_or(None, |d| Some(d as u16))
        }) {
            if has_data {
                self.notifier.notify();
            } else if self.uart.is_interrupt_enable(Event::TxEmpty) {
                self.uart.set_interrupt(Event::TxEmpty, false);
            }
        }
    }
}

// RX -------------------------------------------------------------------------

pub struct UartInterruptRx<U, OS: OsInterface> {
    uart: U,
    timeout: MicrosDurationU32,
    r: Consumer<u8>,
    waiter: OS::NotifyWaiter,
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
    ) -> (Self, UartInterruptRxHandler<U, OS>) {
        let (notifier, waiter) = OS::notify();
        let [uart, u2] = uart;
        let (w, r) = RingBuffer::<u8>::new(buf_size);
        (
            Self {
                uart,
                timeout,
                r,
                waiter,
            },
            UartInterruptRxHandler::new(u2, w, notifier),
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

        self.waiter
            .wait_with(OS::O, self.timeout, 2, || {
                if let n @ 1.. = self.r.pop_slice(buf) {
                    return Some(n);
                } else if !self.uart.is_interrupt_enable(Event::RxNotEmpty) {
                    self.uart.set_interrupt(Event::RxNotEmpty, true);
                }
                None
            })
            .ok_or(Error::Other)
    }
}

// RX interrupt -----------------

pub struct UartInterruptRxHandler<U, OS: OsInterface> {
    uart: U,
    w: Producer<u8>,
    notifier: OS::Notifier,
    // count: [u32; 10],
}

impl<U, OS> UartInterruptRxHandler<U, OS>
where
    U: UartPeriph,
    OS: OsInterface,
{
    pub fn new(mut uart: U, w: Producer<u8>, notifier: OS::Notifier) -> Self {
        uart.set_interrupt(Event::RxNotEmpty, true);
        Self {
            uart,
            w,
            notifier,
            // count: [0; 10],
        }
    }

    pub fn handler(&mut self) {
        if let Ok(data) = self.uart.read() {
            self.w.push(data as u8).ok();
            self.notifier.notify();
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
