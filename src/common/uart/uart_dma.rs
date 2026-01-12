use super::*;
use crate::{
    Steal,
    common::{
        dma::*,
        embedded_io::{BufRead, ErrorType, Read, ReadReady, Write, WriteReady},
        os_trait::Duration,
    },
};

// TX -------------------------------------------------------------------------

pub struct UartDmaBufTx<U, CH, OS: OsInterface> {
    _uart: U,
    w: DmaRingbufTxWriter<u8, CH>,
    timeout: MicrosDurationU32,
    flush_timeout: MicrosDurationU32,
    waiter: OS::NotifyWaiter,
}

impl<U, CH, OS> UartDmaBufTx<U, CH, OS>
where
    U: UartPeriphWithDma,
    CH: DmaChannel,
    OS: OsInterface,
{
    pub fn new(
        mut uart: U,
        dma_ch: CH,
        buf_size: usize,
        baudrate: u32,
        timeout: MicrosDurationU32,
    ) -> (Self, DmaRingbufTxLoader<u8, CH, OS>) {
        let (notifier, waiter) = OS::notify();

        uart.enable_dma_tx(true);
        let (w, l) = DmaRingbufTx::new(dma_ch, uart.get_tx_data_reg_addr(), buf_size, notifier);
        (
            Self {
                _uart: uart,
                w,
                timeout,
                flush_timeout: calculate_timeout(baudrate, buf_size + 10),
                waiter,
            },
            l,
        )
    }
}

impl<U, CH, OS> ErrorType for UartDmaBufTx<U, CH, OS>
where
    OS: OsInterface,
{
    type Error = Error;
}

impl<U, CH, OS> Write for UartDmaBufTx<U, CH, OS>
where
    CH: DmaChannel,
    OS: OsInterface,
{
    #[inline(always)]
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        if buf.is_empty() {
            return Err(Error::Other);
        }

        let dur = Duration::<OS>::micros(self.timeout.ticks());
        let mut timeout = false;
        loop {
            if let n @ 1.. = self.w.write(buf) {
                return Ok(n);
            } else if timeout {
                return Err(Error::Busy);
            }
            timeout = !self.waiter.wait(&dur);
        }
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        self.waiter
            .wait_with(
                &Duration::<OS>::micros(self.flush_timeout.ticks()),
                1,
                || {
                    if self.w.is_empty() && !self.w.in_progress() {
                        Some(())
                    } else {
                        None
                    }
                },
            )
            .ok_or(Error::Other)
    }
}

impl<U, CH, OS> WriteReady for UartDmaBufTx<U, CH, OS>
where
    CH: DmaChannel,
    OS: OsInterface,
{
    fn write_ready(&mut self) -> Result<bool, Self::Error> {
        Ok(!self.w.is_full())
    }
}

// RX -------------------------------------------------------------------------

pub struct UartDmaRx<U, CH, OS: OsInterface> {
    _uart: U,
    ch: DmaCircularBufferRx<u8, CH>,
    timeout: MicrosDurationU32,
    waiter: OS::NotifyWaiter,
}

impl<U, CH, OS> UartDmaRx<U, CH, OS>
where
    U: UartPeriphWithDma + Steal,
    CH: DmaChannel + Steal,
    OS: OsInterface,
{
    pub fn new(
        mut uart: U,
        mut dma_ch: CH,
        buf_size: usize,
        timeout: MicrosDurationU32,
    ) -> (Self, UartDmaRxNotify<CH, OS>, UartIdleNotify<U, OS>) {
        let (notifier, waiter) = OS::notify();
        let dma_ch2 = unsafe { dma_ch.steal() };
        let ch = DmaCircularBufferRx::<u8, CH>::new(dma_ch2, uart.get_rx_data_reg_addr(), buf_size);
        uart.enable_dma_rx(true);
        uart.set_interrupt(Event::Idle, true);
        dma_ch.set_interrupt(DmaEvent::HalfTransfer, true);
        dma_ch.set_interrupt(DmaEvent::TransferComplete, true);
        let uart2 = unsafe { uart.steal() };
        (
            Self {
                _uart: uart2,
                ch,
                timeout,
                waiter,
            },
            UartDmaRxNotify {
                notifier: notifier.clone(),
                ch: dma_ch,
            },
            UartIdleNotify { uart, notifier },
        )
    }
}

impl<U, CH, OS> ErrorType for UartDmaRx<U, CH, OS>
where
    OS: OsInterface,
{
    type Error = Error;
}

impl<U, CH, OS> Read for UartDmaRx<U, CH, OS>
where
    CH: DmaChannel,
    OS: OsInterface,
{
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        if buf.is_empty() {
            return Err(Error::Other);
        }

        self.waiter
            .wait_with(&Duration::<OS>::micros(self.timeout.ticks()), 1, || {
                if let Some(d) = self.ch.read_slice(buf.len()) {
                    buf[..d.len()].copy_from_slice(d);
                    self.ch.consume(d.len());
                    Some(d.len())
                } else {
                    None
                }
            })
            .ok_or(Error::Other)
    }
}

impl<U, CH, OS> BufRead for UartDmaRx<U, CH, OS>
where
    CH: DmaChannel,
    OS: OsInterface,
{
    fn fill_buf(&mut self) -> Result<&[u8], Self::Error> {
        self.waiter
            .wait_with(&Duration::<OS>::micros(self.timeout.ticks()), 1, || {
                self.ch.read_slice(usize::MAX)
            })
            .ok_or(Error::Other)
    }

    fn consume(&mut self, amt: usize) {
        self.ch.consume(amt);
    }
}

impl<U, CH, OS> ReadReady for UartDmaRx<U, CH, OS>
where
    CH: DmaChannel,
    OS: OsInterface,
{
    fn read_ready(&mut self) -> Result<bool, Self::Error> {
        Ok(self.ch.has_data())
    }
}

pub struct UartDmaRxNotify<CH, OS: OsInterface> {
    notifier: OS::Notifier,
    ch: CH,
}

impl<CH, OS> UartDmaRxNotify<CH, OS>
where
    CH: DmaChannel,
    OS: OsInterface,
{
    pub fn interrupt_notify(&mut self) {
        if self.ch.check_and_clear_interrupt(DmaEvent::HalfTransfer)
            || self
                .ch
                .check_and_clear_interrupt(DmaEvent::TransferComplete)
        {
            self.notifier.notify();
        }
    }
}

pub struct UartIdleNotify<U, OS: OsInterface> {
    uart: U,
    notifier: OS::Notifier,
}

impl<U, OS> UartIdleNotify<U, OS>
where
    U: UartPeriph,
    OS: OsInterface,
{
    pub fn interrupt_notify(&mut self) {
        if self.uart.check_and_clear_interrupt(Event::Idle) {
            self.notifier.notify();
        }
    }
}
