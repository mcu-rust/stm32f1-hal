type I2cX = pac::I2C1;

// $sync begin

use super::*;
use crate::{Mcu, pac};

// Initialization -------------------------------------------------------------

impl I2cInit<I2cX> for I2cX {
    fn init(self, mcu: &mut Mcu) -> I2c<I2cX> {
        mcu.rcc.enable(&self);
        mcu.rcc.reset(&self);
        I2c { i2c: self }
    }
}

impl I2cConfig for I2cX {
    fn config(&mut self, mode: Mode, mcu: &mut Mcu) {
        assert!(mode.get_frequency() <= kHz(400));

        // Calculate settings for I2C speed modes
        let clock = mcu.rcc.get_clock(self).raw();
        let clc_mhz = clock / 1_000_000;

        // Configure bus frequency into I2C peripheral
        self.cr2()
            .write(|w| unsafe { w.freq().bits(clc_mhz as u8) });

        let trise = match mode {
            Mode::Standard { .. } => clc_mhz + 1,
            Mode::Fast { .. } => clc_mhz * 300 / 1000 + 1,
        };

        // Configure correct rise times
        self.trise().write(|w| w.trise().set(trise as u8));

        match mode {
            // I2C clock control calculation
            Mode::Standard { frequency } => {
                let ccr = (clock / (frequency.raw() * 2)).max(4);

                // Set clock to standard mode with appropriate parameters for selected speed
                self.ccr().write(|w| unsafe {
                    w.f_s().clear_bit();
                    w.duty().clear_bit();
                    w.ccr().bits(ccr as u16)
                });
            }
            Mode::Fast {
                frequency,
                duty_cycle,
            } => match duty_cycle {
                DutyCycle::Ratio2to1 => {
                    let ccr = (clock / (frequency.raw() * 3)).max(1);

                    // Set clock to fast mode with appropriate parameters for selected speed (2:1 duty cycle)
                    self.ccr().write(|w| unsafe {
                        w.f_s().set_bit().duty().clear_bit().ccr().bits(ccr as u16)
                    });
                }
                DutyCycle::Ratio16to9 => {
                    let ccr = (clock / (frequency.raw() * 25)).max(1);

                    // Set clock to fast mode with appropriate parameters for selected speed (16:9 duty cycle)
                    self.ccr().write(|w| unsafe {
                        w.f_s().set_bit().duty().set_bit().ccr().bits(ccr as u16)
                    });
                }
            },
        }

        // Enable the I2C processing
        // Disable acknowledge at next position
        self.cr1().modify(|_, w| w.pe().set_bit().pos().clear_bit());
    }

    #[inline]
    fn set_ack(&mut self, en: bool) {
        self.cr1().modify(|_, w| w.ack().bit(en));
    }

    #[inline]
    fn continue_after_addr(&mut self) {
        let _ = self.sr1().read();
        let _ = self.sr2().read();
    }

    #[inline]
    fn send_addr(&mut self, addr: u8, read: bool) {
        self.dr()
            .write(|w| unsafe { w.dr().bits(addr | u8::from(read)) });
    }

    #[inline]
    fn set_interrupt(&mut self, event: Interrupt, en: bool) {
        match event {
            Interrupt::Buffer => self.cr2().modify(|_, w| w.itbufen().bit(en)),
            Interrupt::Error => self.cr2().modify(|_, w| w.iterren().bit(en)),
            Interrupt::Event => self.cr2().modify(|_, w| w.itevten().bit(en)),
        };
    }

    #[inline]
    fn disable_all_interrupt(&mut self) {
        self.cr2().modify(|_, w| {
            w.itbufen()
                .clear_bit()
                .iterren()
                .clear_bit()
                .itevten()
                .clear_bit()
        });
    }
}

// Implement Peripheral -------------------------------------------------------

impl I2cPeriph for I2cX {
    #[inline]
    fn it_reset(&mut self) {
        self.disable_all_interrupt();
        self.set_ack(false);

        // Clean interrupt flag
        if self.get_flag(Flag::ByteTransferFinished) {
            let _ = self.dr().read().bits() as u8;
        }
    }

    #[inline]
    fn it_send_start(&mut self) {
        self.set_interrupt(Interrupt::Buffer, false);
        self.set_interrupt(Interrupt::Event, true);
        // Clear all pending error bits
        // NOTE(unsafe): Writing 0 clears the r/w bits and has no effect on the r bits
        self.sr1().write(|w| unsafe { w.bits(0) });
        self.cr1().modify(|_, w| w.start().set_bit());
        self.set_interrupt(Interrupt::Error, true);
    }

    #[inline]
    fn it_send_slave_addr(&mut self, slave_addr: u8, read: bool) -> bool {
        if self.get_flag(Flag::Started) {
            self.send_addr(slave_addr, read);
            true
        } else {
            false
        }
    }

    #[inline]
    fn it_start_write_data(&mut self) -> bool {
        if self.get_flag(Flag::AddressSent) {
            self.continue_after_addr();
            self.set_interrupt(Interrupt::Buffer, true);
            true
        } else {
            false
        }
    }

    #[inline]
    fn it_start_read_data(&mut self, total_len: usize) -> bool {
        if self.get_flag(Flag::AddressSent) {
            self.set_ack(total_len > 1);
            self.continue_after_addr();
            self.set_interrupt(Interrupt::Buffer, true);
            true
        } else {
            false
        }
    }

    #[inline]
    fn it_read(&mut self, left_len: usize) -> Option<u8> {
        if self.sr1().read().rx_ne().bit_is_set() {
            let data = self.dr().read().bits() as u8;
            if left_len == 2 {
                self.set_ack(false);
            }
            Some(data)
        } else {
            None
        }
    }

    #[inline]
    fn it_write(&mut self, data: u8) -> bool {
        if self.get_flag(Flag::TxEmpty) {
            self.dr().write(|w| unsafe { w.dr().bits(data) });
            true
        } else {
            false
        }
    }

    #[inline]
    fn it_write_with(&mut self, f: impl FnOnce() -> Option<u8>) -> Option<bool> {
        if self.get_flag(Flag::TxEmpty) {
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

    #[inline]
    fn is_busy(&self) -> bool {
        self.sr2().read().busy().bit_is_set()
    }

    fn send_stop(&mut self) {
        self.cr1().modify(|_, w| w.stop().set_bit());
    }

    #[inline]
    fn get_flag(&mut self, flag: Flag) -> bool {
        match flag {
            Flag::Started => self.sr1().read().sb().bit_is_set(),
            Flag::AddressSent => self.sr1().read().addr().bit_is_set(),
            Flag::TxEmpty => self.sr1().read().tx_e().bit_is_set(),
            Flag::RxNotEmpty => self.sr1().read().rx_ne().bit_is_set(),
            Flag::ByteTransferFinished => self.sr1().read().btf().bit_is_set(),
            _ => false,
        }
    }

    #[inline]
    fn get_and_clean_error(&mut self) -> Option<Error> {
        let sr1 = self.sr1().read();
        if sr1.arlo().bit_is_set() {
            self.sr1().write(|w| w.arlo().clear_bit());
            Some(Error::ArbitrationLoss)
        } else if sr1.af().bit_is_set() {
            self.sr1().write(|w| w.af().clear_bit());
            Some(Error::NoAcknowledge(NoAcknowledgeSource::Unknown))
        } else if sr1.ovr().bit_is_set() {
            self.sr1().write(|w| w.ovr().clear_bit());
            Some(Error::Overrun)
        } else if sr1.timeout().bit_is_set() {
            self.sr1().write(|w| w.timeout().clear_bit());
            Some(Error::SMBusTimeout)
        } else if sr1.smbalert().bit_is_set() {
            self.sr1().write(|w| w.smbalert().clear_bit());
            Some(Error::SMBusAlert)
        } else if sr1.pecerr().bit_is_set() {
            self.sr1().write(|w| w.pecerr().clear_bit());
            Some(Error::Pec)
        } else {
            // The errata indicates that BERR may be incorrectly detected. It recommends ignoring and
            // clearing the BERR bit instead.
            if sr1.berr().bit_is_set() {
                self.sr1().write(|w| w.berr().clear_bit());
            }
            None
        }
    }

    // #[inline]
    // fn wait_for_flag(&self, flag: Flag) -> nb::Result<(), Error> {
    //     let sr1 = self.sr1().read();
    //     if let Err(err) = read_error(&sr1, NoAcknowledgeSource::Unknown) {
    //         return Err(nb::Error::Other(err));
    //     } else if flag == Flag::Stopped {
    //         if self.cr1().read().stop().is_no_stop() {
    //             return Ok(());
    //         }
    //     } else {
    //         if read_flag(&sr1, flag) {
    //             return Ok(());
    //         }
    //     }
    //     Err(nb::Error::WouldBlock)
    // }
}

// $sync end
