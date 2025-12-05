use super::*;
use crate::common::{dma::*, os::*};
use embedded_io::{ErrorType, Read, Write};

// TX -------------------------------------------------------------------------

pub struct UartDmaBufTx<U, CH, W> {
    _uart: U,
    w: DmaRingbufTxWriter<u8, CH>,
    timeout: W,
    flush_timeout: W,
}

impl<U, CH, W> UartDmaBufTx<U, CH, W>
where
    U: UartPeriph,
    CH: DmaChannel,
    W: Waiter,
{
    pub fn new(
        mut uart: U,
        dma_ch: CH,
        buf_size: usize,
        timeout: W,
        flush_timeout: W,
    ) -> (Self, DmaRingbufTxLoader<u8, CH>) {
        uart.enable_dma_tx(true);
        let (w, l) = DmaRingbufTx::new(dma_ch, uart.get_tx_data_reg_addr(), buf_size);
        (
            Self {
                _uart: uart,
                w,
                timeout,
                flush_timeout,
            },
            l,
        )
    }
}

impl<U, CH, W> ErrorType for UartDmaBufTx<U, CH, W>
where
    U: UartPeriph,
    CH: DmaChannel,
    W: Waiter,
{
    type Error = Error;
}

impl<U, CH, W> Write for UartDmaBufTx<U, CH, W>
where
    U: UartPeriph,
    CH: DmaChannel,
    W: Waiter,
{
    #[inline(always)]
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        if buf.is_empty() {
            return Err(Error::Other);
        }

        let mut t = self.timeout.start();
        loop {
            if let n @ 1.. = self.w.write(buf) {
                return Ok(n);
            } else if t.timeout() {
                break;
            }
        }
        Err(Error::Busy)
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        let mut t = self.flush_timeout.start();
        loop {
            if !self.w.in_progress() {
                return Ok(());
            } else if t.timeout() {
                break;
            }
        }
        Err(Error::Other)
    }
}

// RX -------------------------------------------------------------------------

pub struct UartDmaRx<U, CH, W> {
    _uart: U,
    ch: DmaCircularBufferRx<u8, CH>,
    timeout: W,
}

impl<U, CH, W> UartDmaRx<U, CH, W>
where
    U: UartPeriph,
    CH: DmaChannel,
    W: Waiter,
{
    pub fn new(mut uart: U, dma_ch: CH, buf_size: usize, timeout: W) -> Self {
        let ch = DmaCircularBufferRx::<u8, CH>::new(dma_ch, uart.get_rx_data_reg_addr(), buf_size);
        uart.enable_dma_rx(true);
        Self {
            _uart: uart,
            ch,
            timeout,
        }
    }
}

impl<U, CH, W> ErrorType for UartDmaRx<U, CH, W>
where
    U: UartPeriph,
    CH: DmaChannel,
    W: Waiter,
{
    type Error = Error;
}

impl<U, CH, W> Read for UartDmaRx<U, CH, W>
where
    U: UartPeriph,
    CH: DmaChannel,
    W: Waiter,
{
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        if buf.is_empty() {
            return Err(Error::Other);
        }

        let mut t = self.timeout.start();
        loop {
            if let Some(d) = self.ch.pop_slice(buf.len()) {
                buf[..d.len()].copy_from_slice(d);
                return Ok(d.len());
            } else if t.timeout() {
                break;
            }
        }
        Err(Error::Other)
    }
}
