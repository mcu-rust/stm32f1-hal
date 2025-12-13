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
    fn write_data(&mut self, data: u8) {
        self.dr().write(|w| unsafe { w.dr().bits(data) });
    }

    #[inline]
    fn read_data(&self) -> u8 {
        self.dr().read().bits() as u8
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

    #[inline]
    fn it_routine(&self) {
        // Clean useless interrupt flag
        if self.sr1().read().btf().bit_is_set() {
            let _ = self.read_data();
        }
    }
}

// Implement Peripheral -------------------------------------------------------

impl I2cPeriph for I2cX {
    #[inline]
    fn it_reset(&mut self) {
        self.disable_all_interrupt();
        self.set_ack(false);
        self.it_routine();
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

    fn it_prepare_write(&mut self, addr: Address, step: &mut u8) -> Result<(), bool> {
        match *step {
            0 => {
                if !self.get_flag(Flag::Started) {
                    return Err(false);
                }
                match addr {
                    Address::Seven(addr) => {
                        self.write_data(addr);
                        *step = 2;
                    }
                    Address::Ten(addr) => {
                        let [msb, _] = addr.to_be_bytes();
                        self.write_data(msb);
                        next(step);
                    }
                }
            }
            1 => {
                if !self.get_flag(Flag::Address10Sent) {
                    return Err(false);
                }
                if let Address::Ten(addr) = addr {
                    let [_, lsb] = addr.to_be_bytes();
                    self.write_data(lsb);
                    next(step);
                } else {
                    panic!();
                }
            }
            2 => {
                if !self.get_flag(Flag::AddressSent) {
                    return Err(false);
                }
                self.continue_after_addr();
                self.set_interrupt(Interrupt::Buffer, true);
                next(step);
                return Ok(());
            }
            _ => return Ok(()),
        }
        Err(true)
    }

    fn it_prepare_read(
        &mut self,
        addr: Address,
        total_len: usize,
        step: &mut u8,
    ) -> Result<(), bool> {
        match *step {
            0 => {
                if !self.get_flag(Flag::Started) {
                    return Err(false);
                }
                match addr {
                    Address::Seven(addr) => {
                        self.write_data(addr | 1);
                        *step = 4;
                    }
                    Address::Ten(addr) => {
                        let [msb, _] = addr.to_be_bytes();
                        self.write_data(msb);
                        next(step);
                    }
                }
            }
            1 => {
                if !self.get_flag(Flag::Address10Sent) {
                    return Err(false);
                }
                if let Address::Ten(addr) = addr {
                    let [_, lsb] = addr.to_be_bytes();
                    self.write_data(lsb);
                    next(step);
                } else {
                    panic!();
                }
            }
            2 => {
                if !self.get_flag(Flag::AddressSent) {
                    return Err(false);
                }
                self.it_send_start();
                next(step);
            }
            3 => {
                if !self.get_flag(Flag::Started) {
                    return Err(false);
                }
                if let Address::Ten(addr) = addr {
                    let [msb, _] = addr.to_be_bytes();
                    self.write_data(msb | 1);
                    next(step);
                } else {
                    panic!();
                }
            }
            4 => {
                if !self.get_flag(Flag::AddressSent) {
                    return Err(false);
                }
                self.set_ack(total_len > 1);
                self.continue_after_addr();
                self.set_interrupt(Interrupt::Buffer, true);
                next(step);
                return Ok(());
            }
            _ => return Ok(()),
        }
        Err(true)
    }

    #[inline]
    fn it_read(&mut self, left_len: usize) -> Option<u8> {
        if self.sr1().read().rx_ne().bit_is_set() {
            let data = self.read_data();
            if left_len == 2 {
                self.set_ack(false);
            }
            Some(data)
        } else {
            None
        }
    }

    #[inline]
    fn it_write_with(&mut self, f: impl FnOnce() -> Option<u8>) -> Result<(), bool> {
        if self.get_flag(Flag::TxEmpty) {
            if let Some(data) = f() {
                self.write_data(data);
                Err(true)
            } else {
                Ok(())
            }
        } else {
            Err(false)
        }
    }

    #[inline]
    fn send_stop(&mut self) {
        self.cr1().modify(|_, w| w.stop().set_bit());
    }

    #[inline]
    fn is_stopped(&mut self, master_mode: bool) -> bool {
        if master_mode {
            self.cr1().read().stop().bit_is_clear() && !self.get_flag(Flag::Busy)
        } else {
            self.sr1().read().stopf().bit_is_set()
        }
    }

    #[inline]
    fn get_flag(&mut self, flag: Flag) -> bool {
        match flag {
            Flag::Started => self.sr1().read().sb().bit_is_set(),
            Flag::AddressSent => self.sr1().read().addr().bit_is_set(),
            Flag::Address10Sent => self.sr1().read().add10().bit_is_set(),
            Flag::TxEmpty => self.sr1().read().tx_e().bit_is_set(),
            Flag::RxNotEmpty => self.sr1().read().rx_ne().bit_is_set(),
            Flag::ByteTransferFinished => self.sr1().read().btf().bit_is_set(),
            Flag::MasterSlave => self.sr2().read().msl().bit_is_set(),
            Flag::Busy => self.sr2().read().busy().bit_is_set(),
            _ => false,
        }
    }

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
}

fn next(step: &mut u8) {
    *step += 1;
}

// $sync end
