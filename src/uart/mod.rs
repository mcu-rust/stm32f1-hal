#[cfg(any(all(feature = "stm32f103", feature = "high"), feature = "connectivity"))]
mod uart4;
#[cfg(any(all(feature = "stm32f103", feature = "high"), feature = "connectivity"))]
pub mod uart5;
mod usart1;
mod usart2;
mod usart3;
pub use crate::common::uart::*;

use crate::{
    Steal,
    afio::{RemapMode, uart_remap::*},
    dma::{DmaBindRx, DmaBindTx, DmaRingbufTxLoader},
    os_trait::{MicrosDurationU32, prelude::*},
    rcc::{BusClock, Enable, Reset},
};

use crate::Mcu;

pub trait UartInit<U> {
    fn init(self, mcu: &mut Mcu) -> Uart<U>;
}

pub trait UartConfig: UartPeriph + BusClock + Enable + Reset + Steal {
    fn config(&mut self, config: Config, mcu: &mut Mcu);
    fn enable_comm(&mut self, tx: bool, rx: bool);
    fn set_stop_bits(&mut self, bits: StopBits);
    fn is_tx_empty(&self) -> bool;
    fn is_rx_not_empty(&self) -> bool;
}

// wrapper
pub struct Uart<U> {
    uart: U,
}

impl<U: UartConfig> Uart<U> {
    pub fn into_tx_rx<REMAP: RemapMode<U>>(
        mut self,
        pins: (impl UartTxPin<REMAP>, impl UartRxPin<REMAP>),
        config: Config,
        mcu: &mut Mcu,
    ) -> (Option<Tx<U>>, Option<Rx<U>>) {
        REMAP::remap(&mut mcu.afio);
        let baudrate = config.baudrate;
        self.uart.config(config, mcu);
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
pub struct Tx<U> {
    uart: U,
    baudrate: u32,
}

impl<U: UartConfig> Tx<U> {
    pub(crate) fn new(uart: U, baudrate: u32) -> Self {
        Self { uart, baudrate }
    }

    pub fn into_poll<OS: OsInterface>(self, timeout: MicrosDurationU32) -> UartPollTx<U, OS> {
        UartPollTx::new(self.uart, self.baudrate, timeout)
    }

    pub fn into_interrupt<OS: OsInterface>(
        self,
        buf_size: usize,
        timeout: MicrosDurationU32,
    ) -> (UartInterruptTx<U, OS>, UartInterruptTxHandler<U>) {
        let u2 = unsafe { self.uart.steal() };
        UartInterruptTx::new([self.uart, u2], buf_size, self.baudrate, timeout)
    }
}

impl<U: UartConfig + UartPeriphWithDma> Tx<U> {
    // pub fn into_dma<CH>(self, dma_ch: CH) -> UartDmaTx<U, CH>
    // where
    //     CH: BindDmaTx<U>,
    // {
    //     UartDmaTx::<U, CH>::new(self.uart, dma_ch)
    // }

    pub fn into_dma_ringbuf<CH, OS>(
        self,
        dma_ch: CH,
        buf_size: usize,
        timeout: MicrosDurationU32,
        _os: OS,
    ) -> (UartDmaBufTx<U, CH, OS>, DmaRingbufTxLoader<u8, CH>)
    where
        CH: DmaBindTx<U>,
        OS: OsInterface,
    {
        UartDmaBufTx::new(self.uart, dma_ch, buf_size, self.baudrate, timeout)
    }
}

// ------------------------------------------------------------------------------------------------

/// UART Receiver
pub struct Rx<U> {
    uart: U,
    baudrate: u32,
}

impl<U: UartConfig> Rx<U> {
    pub(crate) fn new(uart: U, baudrate: u32) -> Self {
        Self { uart, baudrate }
    }

    pub fn into_poll<OS: OsInterface>(self, timeout: MicrosDurationU32) -> UartPollRx<U, OS> {
        UartPollRx::new(self.uart, self.baudrate, timeout)
    }

    pub fn into_interrupt<OS: OsInterface>(
        self,
        buf_size: usize,
        timeout: MicrosDurationU32,
    ) -> (UartInterruptRx<U, OS>, UartInterruptRxHandler<U>) {
        let u2 = unsafe { self.uart.steal() };
        UartInterruptRx::new([self.uart, u2], buf_size, timeout)
    }
}

impl<U: UartConfig + UartPeriphWithDma> Rx<U> {
    pub fn into_dma_circle<OS, CH>(
        self,
        dma_ch: CH,
        buf_size: usize,
        timeout: MicrosDurationU32,
        _os: OS,
    ) -> UartDmaRx<U, CH, OS>
    where
        CH: DmaBindRx<U>,
        OS: OsInterface,
    {
        UartDmaRx::new(self.uart, dma_ch, buf_size, timeout)
    }
}
