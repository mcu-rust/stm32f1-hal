use super::*;
use crate::common::{
    dma::*,
    embedded_io::{ErrorType, Read, Write},
};
use core::marker::PhantomData;

// TX -------------------------------------------------------------------------

pub struct UartDmaBufTx<U, CH, OS> {
    _uart: U,
    w: DmaRingbufTxWriter<u8, CH>,
    timeout: MicrosDurationU32,
    flush_timeout: MicrosDurationU32,
    _os: PhantomData<OS>,
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
    ) -> (Self, DmaRingbufTxLoader<u8, CH>) {
        uart.enable_dma_tx(true);
        let (w, l) = DmaRingbufTx::new(dma_ch, uart.get_tx_data_reg_addr(), buf_size);
        (
            Self {
                _uart: uart,
                w,
                timeout,
                flush_timeout: calculate_timeout(baudrate, buf_size + 10),
                _os: PhantomData,
            },
            l,
        )
    }
}

impl<U, CH, OS> ErrorType for UartDmaBufTx<U, CH, OS>
where
    U: UartPeriph,
    CH: DmaChannel,
    OS: OsInterface,
{
    type Error = Error;
}

impl<U, CH, OS> Write for UartDmaBufTx<U, CH, OS>
where
    U: UartPeriph,
    CH: DmaChannel,
    OS: OsInterface,
{
    #[inline(always)]
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        if buf.is_empty() {
            return Err(Error::Other);
        }

        let mut t = OS::Timeout::start_us(self.timeout.to_micros());
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
        let mut t = OS::Timeout::start_us(self.flush_timeout.to_micros());
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

pub struct UartDmaRx<U, CH, OS> {
    _uart: U,
    ch: DmaCircularBufferRx<u8, CH>,
    timeout: MicrosDurationU32,
    _os: PhantomData<OS>,
}

impl<U, CH, OS> UartDmaRx<U, CH, OS>
where
    U: UartPeriphWithDma,
    CH: DmaChannel,
    OS: OsInterface,
{
    pub fn new(mut uart: U, dma_ch: CH, buf_size: usize, timeout: MicrosDurationU32) -> Self {
        let ch = DmaCircularBufferRx::<u8, CH>::new(dma_ch, uart.get_rx_data_reg_addr(), buf_size);
        uart.enable_dma_rx(true);
        Self {
            _uart: uart,
            ch,
            timeout,
            _os: PhantomData,
        }
    }
}

impl<U, CH, OS> ErrorType for UartDmaRx<U, CH, OS>
where
    U: UartPeriph,
    CH: DmaChannel,
    OS: OsInterface,
{
    type Error = Error;
}

impl<U, CH, OS> Read for UartDmaRx<U, CH, OS>
where
    U: UartPeriph,
    CH: DmaChannel,
    OS: OsInterface,
{
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        if buf.is_empty() {
            return Err(Error::Other);
        }

        let mut t = OS::Timeout::start_us(self.timeout.to_micros());
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
