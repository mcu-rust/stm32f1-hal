mod spi1;
#[cfg(feature = "connectivity")]
mod spi3;

pub use crate::common::spi::*;

use crate::{
    Mcu, Steal,
    fugit::HertzU32,
    rcc::{Enable, GetClock, Reset},
};
use core::marker::PhantomData;

pub trait SpiInit<T, WD: FrameSize> {
    fn init(self, mcu: &mut Mcu) -> Spi<T, WD>;
}

pub trait SpiPeriphConfig<WD: FrameSize>:
    SpiPeriph<WD> + GetClock + Enable + Reset + Steal
{
    fn init_config(&mut self, mode: &Mode, freq: HertzU32, master_mode: bool);
}

pub struct Spi<T, WD: Word> {
    spi: T,
    _wd: PhantomData<WD>,
}

impl<T, WD> Spi<T, WD>
where
    T: SpiPeriphConfig<WD>,
    WD: FrameSize,
{
}

fn calculate_baud_rate(clock: HertzU32, freq: HertzU32) -> u8 {
    match clock / freq {
        0 => unreachable!(),
        1..=2 => 0b000,
        3..=5 => 0b001,
        6..=11 => 0b010,
        12..=23 => 0b011,
        24..=47 => 0b100,
        48..=95 => 0b101,
        96..=191 => 0b110,
        _ => 0b111,
    }
}

type SpiRB = crate::pac::spi1::RegisterBlock;

pub trait FrameSize: Word {
    const DFF: bool;
    #[doc(hidden)]
    fn read_data(spi: &SpiRB) -> Self;
    #[doc(hidden)]
    fn write_data(self, spi: &SpiRB);
}

impl FrameSize for u8 {
    const DFF: bool = false;
    fn read_data(spi: &SpiRB) -> Self {
        spi.dr8().read().dr().bits()
    }
    fn write_data(self, spi: &SpiRB) {
        spi.dr8().write(|w| w.dr().set(self));
    }
}

impl FrameSize for u16 {
    const DFF: bool = true;
    fn read_data(spi: &SpiRB) -> Self {
        spi.dr().read().dr().bits()
    }
    fn write_data(self, spi: &SpiRB) {
        spi.dr().write(|w| w.dr().set(self));
    }
}
