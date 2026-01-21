type SpiX = pac::SPI1;

// $sync begin

use super::*;
use crate::{Mcu, pac};
use core::mem::size_of;

// Initialization -------------------------------------------------------------

impl SpiInit<SpiX> for SpiX {
    fn init<OS: OsInterface>(self, mcu: &mut Mcu) -> Spi<OS, SpiX> {
        mcu.rcc.enable(&self);
        mcu.rcc.reset(&self);

        Spi {
            spi: self,
            _os: PhantomData,
        }
    }
}

impl SpiPeriphConfig for SpiX {
    fn init_config<W: Word>(&mut self, mode: Mode, freq: KilohertzU32, master_mode: bool) {
        let br = calculate_baud_rate(self.get_clock(), freq);

        // disable SS output
        self.cr2().write(|w| w.ssoe().clear_bit());

        self.cr1().write(|w| {
            // clock phase from config
            w.cpha().bit(mode.phase == Phase::CaptureOnSecondTransition);
            // clock polarity from config
            w.cpol().bit(mode.polarity == Polarity::IdleHigh);
            // mstr: slave configuration
            w.mstr().bit(master_mode);
            if master_mode {
                // baudrate value
                w.br().set(br);
            }
            // lsbfirst: MSB first
            w.lsbfirst().clear_bit();
            // ssm: enable software slave management (NSS pin free for other uses)
            w.ssm().set_bit();
            // ssi: set nss low = slave mode
            w.ssi().bit(master_mode);
            // dff: 8 bit frames
            w.dff().bit(size_of::<W>() != 1);
            // bidimode: 2-line unidirectional
            w.bidimode().clear_bit();
            // both TX and RX are used
            w.rxonly().clear_bit();
            // spe: enable the SPI bus
            w.spe().set_bit()
        });
    }
}

// Implement Peripheral -------------------------------------------------------

impl SpiPeriph for SpiX {
    fn config<W: Word>(&mut self, mode: Mode, freq: KilohertzU32) -> bool {
        let dff = size_of::<W>() != 1;
        let cpha = mode.phase == Phase::CaptureOnSecondTransition;
        let cpol = mode.polarity == Polarity::IdleHigh;
        let br = calculate_baud_rate(self.get_clock(), freq);

        let cr1 = self.cr1().read();
        if cr1.dff().bit() == dff
            && cr1.cpha().bit() == cpha
            && cr1.cpol().bit() == cpol
            && cr1.br().bits() == br
        {
            return false;
        }

        self.cr1().modify(|_, w| w.spe().clear_bit());
        self.cr1().modify(|_, w| {
            // dff: 8 bit or 16 bit frames
            w.dff().bit(dff);
            // clock phase from config
            w.cpha().bit(cpha);
            // clock polarity from config
            w.cpol().bit(cpol);
            // baudrate value
            w.br().set(br)
        });
        self.cr1().modify(|_, w| w.spe().set_bit());
        true
    }

    #[inline]
    fn is_tx_empty(&self) -> bool {
        self.sr().read().txe().bit_is_set()
    }

    #[inline]
    fn is_busy(&self) -> bool {
        self.sr().read().bsy().bit_is_set()
    }

    fn get_and_clean_error(&mut self) -> Option<Error> {
        let sr = self.sr().read();
        Some(if sr.ovr().bit_is_set() {
            let _ = self.dr().read();
            let _ = self.sr().read();
            Error::Overrun
        } else if sr.crcerr().bit_is_set() {
            self.sr().modify(|_, w| w.crcerr().clear_bit());
            Error::Crc
        } else if sr.modf().bit_is_set() {
            Error::ModeFault
        } else {
            return None;
        })
    }

    #[inline]
    fn set_interrupt(&mut self, event: Event, enable: bool) {
        match event {
            Event::TxEmpty => self.cr2().modify(|_, w| w.txeie().bit(enable)),
            Event::RxNotEmpty => self.cr2().modify(|_, w| w.rxneie().bit(enable)),
            Event::Error => self.cr2().modify(|_, w| w.errie().bit(enable)),
        };
    }

    fn disable_all_interrupt(&mut self) {
        self.cr2().modify(|_, w| {
            w.txeie().clear_bit();
            w.rxneie().clear_bit();
            w.errie().clear_bit()
        });
    }

    #[inline]
    fn read<W: Word>(&mut self) -> Option<W> {
        if self.sr().read().rxne().bit_is_set() {
            Some(W::from_u32(if size_of::<W>() == 1 {
                self.dr8().read().dr().bits() as u32
            } else {
                self.dr().read().dr().bits() as u32
            }))
        } else {
            None
        }
    }

    #[inline]
    fn write_unchecked<W: Word>(&mut self, data: W) {
        if size_of::<W>() == 1 {
            self.dr8().write(|w| w.dr().set(data.into_u32() as u8));
        } else {
            self.dr().write(|w| w.dr().set(data.into_u32() as u16));
        }
    }
}

// $sync end
