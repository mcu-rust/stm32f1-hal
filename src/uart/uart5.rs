use crate::pac::uart4::cr1;
type UartX = pac::UART5;

// $sync begin

use super::*;
use crate::{Mcu, pac};

// Initialization -------------------------------------------------------------

impl UartInit<UartX> for UartX {
    fn init<OS: OsInterface>(self, mcu: &mut Mcu) -> Uart<OS, UartX> {
        mcu.rcc.enable(&self);
        mcu.rcc.reset(&self);
        Uart {
            uart: self,
            _os: PhantomData,
        }
    }
}

impl UartPeriphConfig for UartX {
    fn config(&mut self, config: Config) {
        // Configure baud rate
        let brr = self.get_clock().raw() / config.baudrate;
        assert!(brr >= 16, "impossible baud rate");
        self.brr().write(|w| unsafe { w.bits(brr as u16) });

        // Configure word
        self.cr1().modify(|_, w| {
            w.m().bit(match config.word_length {
                WordLength::Bits8 => false,
                WordLength::Bits9 => true,
            });
            w.ps().variant(match config.parity {
                Parity::ParityOdd => cr1::PS::Odd,
                _ => cr1::PS::Even,
            });
            w.pce().bit(!matches!(config.parity, Parity::ParityNone));
            w
        });

        // Configure stop bits
        self.set_stop_bits(config.stop_bits);
    }

    fn enable_comm(&mut self, tx: bool, rx: bool) {
        // UE: enable USART
        // TE: enable transceiver
        // RE: enable receiver
        self.cr1().modify(|_, w| {
            w.ue().set_bit();
            w.te().bit(tx);
            w.re().bit(rx);
            w
        });
    }

    fn set_stop_bits(&mut self, bits: StopBits) {
        // $sync stop_bits_u4
        use pac::uart4::cr2::STOP;

        // StopBits::STOP0P5 and StopBits::STOP1P5 aren't supported when using UART
        // STOP_A::STOP1 and STOP_A::STOP2 will be used, respectively
        self.cr2().write(|w| {
            w.stop().variant(match bits {
                StopBits::STOP0P5 | StopBits::STOP1 => STOP::Stop1,
                StopBits::STOP1P5 | StopBits::STOP2 => STOP::Stop2,
            })
        });
        // $sync stop_bits_end
    }

    #[inline]
    fn is_tx_empty(&self) -> bool {
        self.sr().read().txe().bit_is_set()
    }

    #[inline]
    fn is_rx_not_empty(&self) -> bool {
        self.sr().read().rxne().bit_is_set()
    }
}

// Implement Peripheral -------------------------------------------------------

impl UartPeriph for UartX {
    #[inline]
    fn is_tx_complete(&self) -> bool {
        let sr = self.sr().read();
        sr.txe().bit_is_set() && sr.tc().bit_is_set()
    }

    #[inline]
    fn write(&mut self, word: u16) -> nb::Result<(), Error> {
        if self.is_tx_empty() {
            self.dr().write(|w| unsafe { w.dr().bits(word) });
            Ok(())
        } else {
            Err(nb::Error::WouldBlock)
        }
    }

    #[inline]
    fn write_with(&mut self, f: impl FnOnce() -> Option<u16>) -> Option<bool> {
        if self.is_tx_empty() {
            if let Some(data) = f() {
                self.dr().write(|w| unsafe { w.dr().bits(data) });
                Some(true)
            } else {
                Some(false)
            }
        } else {
            None
        }
    }

    fn read(&mut self) -> nb::Result<u16, Error> {
        let sr = self.sr().read();

        // Check if a byte is available
        if sr.rxne().bit_is_set() {
            // Read the received byte
            return Ok(self.dr().read().dr().bits());
        }

        // Check for any errors
        let err = if sr.pe().bit_is_set() {
            Some(Error::Parity)
        } else if sr.fe().bit_is_set() {
            Some(Error::FrameFormat)
        } else if sr.ne().bit_is_set() {
            Some(Error::Noise)
        } else if sr.ore().bit_is_set() {
            Some(Error::Overrun)
        } else {
            None
        };

        if let Some(err) = err {
            self.clear_err_flag();
            Err(nb::Error::Other(err))
        } else {
            Err(nb::Error::WouldBlock)
        }
    }

    #[inline]
    fn set_interrupt(&mut self, event: Event, enable: bool) {
        match event {
            Event::Idle => {
                self.cr1().modify(|_, w| w.idleie().bit(enable));
            }
            Event::RxNotEmpty => {
                self.cr1().modify(|_, w| w.rxneie().bit(enable));
            }
            Event::TxEmpty => {
                self.cr1().modify(|_, w| w.txeie().bit(enable));
            }
        }
    }

    #[inline]
    fn is_interrupt_enable(&mut self, event: Event) -> bool {
        let cr1 = self.cr1().read();
        match event {
            Event::Idle => cr1.idleie().bit_is_set(),
            Event::RxNotEmpty => cr1.rxneie().bit_is_set(),
            Event::TxEmpty => cr1.txeie().bit_is_set(),
        }
    }

    #[inline]
    fn is_interrupted(&mut self, event: Event) -> bool {
        let sr = self.sr().read();
        match event {
            Event::Idle => {
                if sr.idle().bit_is_set() && self.cr1().read().idleie().bit_is_set() {
                    self.clear_err_flag();
                    return true;
                }
            }
            Event::RxNotEmpty => {
                if (sr.rxne().bit_is_set() || sr.ore().bit_is_set())
                    && self.cr1().read().rxneie().bit_is_set()
                {
                    return true;
                }
            }
            Event::TxEmpty => {
                if sr.txe().bit_is_set() && self.cr1().read().txeie().bit_is_set() {
                    return true;
                }
            }
        }
        false
    }

    /// In order to clear that error flag, you have to do a read from the sr register
    /// followed by a read from the dr register.
    #[inline]
    fn clear_err_flag(&self) {
        let _ = self.sr().read();
        let _ = self.dr().read();
    }
}

impl UartPeriphWithDma for UartX {
    #[inline]
    fn get_tx_data_reg_addr(&self) -> usize {
        self.dr().as_ptr() as usize
    }

    #[inline]
    fn get_rx_data_reg_addr(&self) -> usize {
        self.dr().as_ptr() as usize
    }

    #[inline]
    fn enable_dma_tx(&mut self, enable: bool) {
        self.cr3().modify(|_, w| w.dmat().bit(enable));
    }

    #[inline]
    fn enable_dma_rx(&mut self, enable: bool) {
        self.cr3().modify(|_, w| w.dmar().bit(enable));
    }
}

// $sync end
