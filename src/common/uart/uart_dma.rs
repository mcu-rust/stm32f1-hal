use super::*;
use crate::{
    Steal,
    common::{
        dma::*,
        embedded_io::{ErrorType, Read, Write},
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

        self.waiter
            .wait_with(
                &Duration::<OS>::from_micros(self.timeout.ticks()),
                2,
                || {
                    if let n @ 1.. = self.w.write(buf) {
                        Some(n)
                    } else {
                        None
                    }
                },
            )
            .ok_or(Error::Busy)
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        self.waiter
            .wait_with(
                &Duration::<OS>::from_micros(self.flush_timeout.ticks()),
                4,
                || {
                    if !self.w.in_progress() {
                        Some(())
                    } else {
                        None
                    }
                },
            )
            .ok_or(Error::Other)
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
    U: UartPeriphWithDma,
    CH: DmaChannel + Steal,
    OS: OsInterface,
{
    pub fn new(
        mut uart: U,
        mut dma_ch: CH,
        buf_size: usize,
        timeout: MicrosDurationU32,
    ) -> (Self, UartDmaRxNotify<CH, OS>) {
        let (notifier, waiter) = OS::notify();
        let dma_ch2 = unsafe { dma_ch.steal() };
        let ch = DmaCircularBufferRx::<u8, CH>::new(dma_ch2, uart.get_rx_data_reg_addr(), buf_size);
        uart.enable_dma_rx(true);
        dma_ch.set_interrupt(DmaEvent::HalfTransfer, true);
        dma_ch.set_interrupt(DmaEvent::TransferComplete, true);
        (
            Self {
                _uart: uart,
                ch,
                timeout,
                waiter,
            },
            UartDmaRxNotify {
                notifier,
                ch: dma_ch,
            },
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
            .wait_with(
                &Duration::<OS>::from_micros(self.timeout.ticks()),
                2,
                || {
                    if let Some(d) = self.ch.pop_slice(buf.len()) {
                        buf[..d.len()].copy_from_slice(d);
                        Some(d.len())
                    } else {
                        None
                    }
                },
            )
            .ok_or(Error::Other)
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
        if self.ch.is_interrupted(DmaEvent::HalfTransfer)
            || self.ch.is_interrupted(DmaEvent::TransferComplete)
        {
            self.notifier.notify();
        }
    }
}
