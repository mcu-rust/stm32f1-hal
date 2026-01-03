#[cfg(any(all(feature = "f103", feature = "high"), feature = "connectivity"))]
mod uart4;
#[cfg(any(all(feature = "f103", feature = "high"), feature = "connectivity"))]
mod uart5;
mod usart1;
mod usart2;
mod usart3;

pub use crate::common::uart::*;

use crate::{
    Mcu, Steal,
    afio::{RemapMode, uart_remap::*},
    common::prelude::*,
    dma::{DmaBindRx, DmaBindTx, DmaRingbufTxLoader},
    fugit::MicrosDurationU32,
    rcc::{Enable, GetClock, Reset},
};
use core::marker::PhantomData;

pub trait UartInit<U> {
    fn init<OS: OsInterface>(self, mcu: &mut Mcu) -> Uart<OS, U>;
}

pub trait UartPeriphConfig: UartPeriph + GetClock + Enable + Reset + Steal {
    fn config(&mut self, config: Config);
    fn enable_comm(&mut self, tx: bool, rx: bool);
    fn set_stop_bits(&mut self, bits: StopBits);
    fn is_tx_empty(&self) -> bool;
    fn is_rx_not_empty(&self) -> bool;
}

// wrapper
pub struct Uart<OS: OsInterface, U> {
    uart: U,
    _os: PhantomData<OS>,
}

#[allow(clippy::type_complexity)]
impl<OS, U> Uart<OS, U>
where
    OS: OsInterface,
    U: UartPeriphConfig,
{
    pub fn into_tx_rx<REMAP: RemapMode<U>>(
        mut self,
        pins: (impl UartTxPin<REMAP>, impl UartRxPin<REMAP>),
        config: Config,
        mcu: &mut Mcu,
    ) -> (Option<Tx<OS, U>>, Option<Rx<OS, U>>) {
        REMAP::remap(&mut mcu.afio);
        let baudrate = config.baudrate;
        self.uart.config(config);
        self.uart.enable_comm(pins.0.is_pin(), pins.1.is_pin());
        unsafe {
            (
                if pins.0.is_pin() {
                    Some(Tx::new(self.uart.steal(), baudrate))
                } else {
                    None
                },
                if pins.1.is_pin() {
                    Some(Rx::new(self.uart.steal(), baudrate))
                } else {
                    None
                },
            )
        }
    }

    pub fn get_idle_interrupt_handler(&self) -> UartIdleInterrupt<U> {
        UartIdleInterrupt::new(unsafe { self.uart.steal() })
    }
}

// ------------------------------------------------------------------------------------------------

/// UART Transmitter
pub struct Tx<OS: OsInterface, U> {
    uart: U,
    baudrate: u32,
    _os: PhantomData<OS>,
}

impl<OS, U> Tx<OS, U>
where
    OS: OsInterface,
    U: UartPeriphConfig,
{
    pub(crate) fn new(uart: U, baudrate: u32) -> Self {
        Self {
            uart,
            baudrate,
            _os: PhantomData,
        }
    }

    pub fn into_poll(self, timeout: MicrosDurationU32) -> UartPollTx<U, OS> {
        UartPollTx::new(self.uart, self.baudrate, timeout)
    }

    pub fn into_interrupt(
        self,
        buf_size: usize,
        timeout: MicrosDurationU32,
    ) -> (UartInterruptTx<U, OS>, UartInterruptTxHandler<U, OS>) {
        let u2 = unsafe { self.uart.steal() };
        UartInterruptTx::new([self.uart, u2], buf_size, self.baudrate, timeout)
    }
}

impl<OS, U> Tx<OS, U>
where
    OS: OsInterface,
    U: UartPeriphConfig + UartPeriphWithDma,
{
    pub fn into_dma_ringbuf<CH>(
        self,
        dma_ch: CH,
        buf_size: usize,
        timeout: MicrosDurationU32,
    ) -> (UartDmaBufTx<U, CH, OS>, DmaRingbufTxLoader<u8, CH, OS>)
    where
        CH: DmaBindTx<U>,
        OS: OsInterface,
    {
        UartDmaBufTx::new(self.uart, dma_ch, buf_size, self.baudrate, timeout)
    }
}

// ------------------------------------------------------------------------------------------------

/// UART Receiver
pub struct Rx<OS: OsInterface, U> {
    uart: U,
    baudrate: u32,
    _os: PhantomData<OS>,
}

impl<OS, U> Rx<OS, U>
where
    OS: OsInterface,
    U: UartPeriphConfig,
{
    pub(crate) fn new(uart: U, baudrate: u32) -> Self {
        Self {
            uart,
            baudrate,
            _os: PhantomData,
        }
    }

    pub fn into_poll(self, timeout: MicrosDurationU32) -> UartPollRx<U, OS> {
        UartPollRx::new(self.uart, self.baudrate, timeout)
    }

    pub fn into_interrupt(
        self,
        buf_size: usize,
        timeout: MicrosDurationU32,
    ) -> (UartInterruptRx<U, OS>, UartInterruptRxHandler<U, OS>) {
        let u2 = unsafe { self.uart.steal() };
        UartInterruptRx::new([self.uart, u2], buf_size, timeout)
    }
}

impl<OS, U> Rx<OS, U>
where
    OS: OsInterface,
    U: UartPeriphConfig + UartPeriphWithDma,
{
    pub fn into_dma_circle<CH>(
        self,
        dma_ch: CH,
        buf_size: usize,
        timeout: MicrosDurationU32,
    ) -> (
        UartDmaRx<U, CH, OS>,
        UartDmaRxNotify<CH, OS>,
        UartIdleNotify<U, OS>,
    )
    where
        CH: DmaBindRx<U> + Steal,
        OS: OsInterface,
    {
        UartDmaRx::new(self.uart, dma_ch, buf_size, timeout)
    }
}
