mod spi1;
mod spi2;
#[cfg(feature = "connectivity")]
mod spi3;

use embedded_hal::digital::OutputPin;
use os_trait::{Mutex, OsInterface, prelude::*};

pub use crate::common::spi::*;
pub use embedded_hal::spi::{MODE_0, MODE_1, MODE_2, MODE_3};

use crate::{
    Mcu, Steal,
    afio::{RemapMode, spi_remap::*},
    fugit::NanosDurationU32,
    l,
    rcc::{Enable, GetClock, Reset},
};
use core::marker::PhantomData;

pub trait SpiInit<T> {
    fn init<OS: OsInterface>(self, mcu: &mut Mcu) -> Spi<OS, T>;
}

pub trait SpiPeriphConfig: SpiPeriph + GetClock + Enable + Reset + Steal {
    fn init_config<W: Word>(&mut self, mode: Mode, freq: KilohertzU32, master_mode: bool);
}

pub struct Spi<OS: OsInterface, T> {
    spi: T,
    _os: PhantomData<OS>,
}

impl<OS, T> Spi<OS, T>
where
    OS: OsInterface,
    T: SpiPeriphConfig,
{
    #[allow(clippy::too_many_arguments)]
    pub fn into_interrupt_sole<W: Word, REMAP: RemapMode<T>, CS: OutputPin>(
        mut self,
        pins: (
            impl SpiSckPin<REMAP>,
            impl SpiMisoPin<REMAP>,
            impl SpiMosiPin<REMAP>,
        ),
        mode: Mode,
        freq: KilohertzU32,
        cs: impl SpiCsPin<CS>,
        cs_delay: NanosDurationU32,
        max_operation: usize,
        mcu: &mut Mcu,
    ) -> (
        SpiSoleDevice<OS, CS, bus_it::SpiBus<OS, T>, W>,
        bus_it::InterruptHandler<OS, T>,
        bus_it::ErrorInterruptHandler<OS, T>,
    ) {
        let cs = cs.into_cs_pin();
        let _ = (pins.0.into_alternate(), pins.2.into_alternate());
        REMAP::remap(&mut mcu.afio);
        self.spi.init_config::<W>(mode, freq, true);
        let (bus, it, err_it) = bus_it::SpiBus::new(self.spi, freq, max_operation);
        (SpiSoleDevice::new(bus, cs, cs_delay), it, err_it)
    }

    pub fn into_interrupt_bus<REMAP: RemapMode<T>>(
        mut self,
        pins: (
            impl SpiSckPin<REMAP>,
            impl SpiMisoPin<REMAP>,
            impl SpiMosiPin<REMAP>,
        ),
        max_operation: usize,
        mcu: &mut Mcu,
    ) -> (
        SpiDeviceBuilder<OS, bus_it::SpiBus<OS, T>>,
        bus_it::InterruptHandler<OS, T>,
        bus_it::ErrorInterruptHandler<OS, T>,
    ) {
        let _ = (pins.0.into_alternate(), pins.2.into_alternate());
        REMAP::remap(&mut mcu.afio);
        let freq = 100.kHz();
        self.spi.init_config::<u8>(MODE_0, freq, true);
        let (bus, it, err_it) = bus_it::SpiBus::new(self.spi, freq, max_operation);
        (
            SpiDeviceBuilder {
                bus: Arc::new(OS::mutex(bus)),
                id: 0,
            },
            it,
            err_it,
        )
    }
}

pub struct SpiDeviceBuilder<OS: OsInterface, BUS> {
    bus: Arc<Mutex<OS, BUS>>,
    id: u8,
}

impl<OS, BUS> SpiDeviceBuilder<OS, BUS>
where
    OS: OsInterface,
    BUS: SpiBusInterface,
{
    pub fn new_device<W: Word, CS: OutputPin>(
        &mut self,
        mode: Mode,
        freq: KilohertzU32,
        cs: impl SpiCsPin<CS>,
        cs_delay: NanosDurationU32,
    ) -> SpiMutexDevice<OS, CS, BUS, W> {
        let cs = cs.into_cs_pin();
        self.id = self.id.strict_add(1);
        SpiMutexDevice::new(Arc::clone(&self.bus), cs, cs_delay, mode, freq, self.id)
    }
}

fn calculate_baud_rate(clock: HertzU32, freq: KilohertzU32) -> u8 {
    match clock / freq {
        0 => l::unreachable!(),
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
