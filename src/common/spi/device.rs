use super::*;
use core::marker::PhantomData;
use embedded_hal::{digital::OutputPin, spi::SpiDevice};
use fugit::NanosDurationU32;
use os_trait::{Mutex, OsInterface};

// Mutex device -----------------------------------------------------

pub struct SpiMutexDevice<OS: OsInterface, CS, BUS, W> {
    bus: Arc<Mutex<OS, BUS>>,
    cs: CS,
    cs_delay: NanosDurationU32,
    mode: Mode,
    freq: KilohertzU32,
    id: u8,
    _w: PhantomData<W>,
}

impl<OS, CS, BUS, W> SpiMutexDevice<OS, CS, BUS, W>
where
    OS: OsInterface,
    BUS: SpiBusInterface,
    CS: OutputPin,
    W: Word,
{
    pub fn new(
        bus: Arc<Mutex<OS, BUS>>,
        cs: CS,
        cs_delay: NanosDurationU32,
        mode: Mode,
        freq: KilohertzU32,
        id: u8,
    ) -> Self {
        Self {
            bus,
            cs,
            cs_delay,
            mode,
            freq,
            id,
            _w: PhantomData,
        }
    }
}

impl<OS, CS, BUS, W> SpiDevice<W> for SpiMutexDevice<OS, CS, BUS, W>
where
    OS: OsInterface,
    BUS: SpiBusInterface,
    CS: OutputPin,
    W: Word,
{
    fn transaction(&mut self, operations: &mut [Operation<'_, W>]) -> Result<(), Self::Error> {
        let mut bus = self.bus.lock();
        bus.config::<W>(self.mode, self.freq, self.id);
        self.cs.set_low().map_err(|_| Error::ChipSelectFault)?;
        let ns = self.cs_delay.ticks();
        if ns > 0 {
            OS::delay().delay_ns(ns);
        }
        let result = bus.transaction(operations);
        if ns > 0 {
            OS::delay().delay_ns(ns);
        }
        self.cs.set_high().map_err(|_| Error::ChipSelectFault)?;
        result
    }
}

impl<OS, CS, BUS, W> ErrorType for SpiMutexDevice<OS, CS, BUS, W>
where
    OS: OsInterface,
    BUS: SpiBusInterface,
    CS: OutputPin,
    W: Word,
{
    type Error = Error;
}

// Sole device ------------------------------------------------------

pub struct SpiSoleDevice<OS: OsInterface, CS, BUS, W> {
    bus: BUS,
    cs: CS,
    cs_delay: NanosDurationU32,
    _os: PhantomData<OS>,
    _w: PhantomData<W>,
}

impl<OS, CS, BUS, W> SpiSoleDevice<OS, CS, BUS, W>
where
    OS: OsInterface,
    BUS: SpiBusInterface,
    CS: OutputPin,
    W: Word,
{
    pub fn new(bus: BUS, cs: CS, cs_delay: NanosDurationU32) -> Self {
        Self {
            bus,
            cs,
            cs_delay,
            _os: PhantomData,
            _w: PhantomData,
        }
    }
}

impl<OS, CS, BUS, W> SpiDevice<W> for SpiSoleDevice<OS, CS, BUS, W>
where
    OS: OsInterface,
    BUS: SpiBusInterface,
    CS: OutputPin,
    W: Word,
{
    fn transaction(&mut self, operations: &mut [Operation<'_, W>]) -> Result<(), Self::Error> {
        self.cs.set_low().map_err(|_| Error::ChipSelectFault)?;
        let ns = self.cs_delay.ticks();
        if ns > 0 {
            OS::delay().delay_ns(ns);
        }
        let result = self.bus.transaction(operations);
        if ns > 0 {
            OS::delay().delay_ns(ns);
        }
        self.cs.set_high().map_err(|_| Error::ChipSelectFault)?;
        result
    }
}

impl<OS, CS, BUS, W> ErrorType for SpiSoleDevice<OS, CS, BUS, W>
where
    OS: OsInterface,
    BUS: SpiBusInterface,
    CS: OutputPin,
    W: Word,
{
    type Error = Error;
}
