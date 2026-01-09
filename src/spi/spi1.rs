type SpiX = pac::SPI1;

// $sync begin

use super::*;
use crate::{Mcu, pac};

// Initialization -------------------------------------------------------------

impl<WD: FrameSize> SpiInit<SpiX, WD> for SpiX {
    fn init(self, mcu: &mut Mcu) -> Spi<SpiX, WD> {
        mcu.rcc.enable(&self);
        mcu.rcc.reset(&self);

        Spi {
            spi: self,
            _wd: PhantomData,
        }
    }
}

impl<WD: FrameSize> SpiPeriphConfig<WD> for SpiX {
    fn init_config(&mut self, mode: &Mode, freq: HertzU32, master_mode: bool) {
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
            w.dff().bit(WD::DFF);
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

impl<WD: FrameSize> SpiPeriph<WD> for SpiX {
    fn config(&mut self, mode: &Mode, freq: HertzU32) {
        let br = calculate_baud_rate(self.get_clock(), freq);
        self.cr1().modify(|_, w| {
            // clock phase from config
            w.cpha().bit(mode.phase == Phase::CaptureOnSecondTransition);
            // clock polarity from config
            w.cpol().bit(mode.polarity == Polarity::IdleHigh);
            // baudrate value
            w.br().set(br)
        });
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
        } else if sr.udr().bit_is_set() {
            Error::Underrun
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
    fn read(&mut self) -> Option<WD> {
        if self.sr().read().rxne().bit_is_set() {
            Some(WD::read_data(&self))
        } else {
            None
        }
    }

    #[inline]
    fn uncheck_write(&mut self, data: WD) {
        data.write_data(&self);
    }
}

// $sync end
