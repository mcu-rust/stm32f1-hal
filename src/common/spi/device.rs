use super::*;
use core::marker::PhantomData;
use embedded_hal::{digital::OutputPin, spi::SpiDevice};
use fugit::NanosDurationU32;
use os_trait::OsInterface;

// Mutex device -----------------------------------------------------

// Sole device ------------------------------------------------------

pub struct SpiSoleDevice<OS: OsInterface, CS, BUS, WD> {
    bus: BUS,
    cs: CS,
    cs_delay: NanosDurationU32,
    _os: PhantomData<OS>,
    _wd: PhantomData<WD>,
}

impl<OS, CS, BUS, WD> SpiSoleDevice<OS, CS, BUS, WD>
where
    OS: OsInterface,
    BUS: SpiBusInterface<WD>,
    CS: OutputPin,
    WD: Word,
{
    pub fn new(bus: BUS, cs: CS, cs_delay: NanosDurationU32) -> Self {
        Self {
            bus,
            cs,
            cs_delay,
            _os: PhantomData,
            _wd: PhantomData,
        }
    }
}

impl<OS, CS, BUS, WD> SpiDevice<WD> for SpiSoleDevice<OS, CS, BUS, WD>
where
    OS: OsInterface,
    BUS: SpiBusInterface<WD>,
    CS: OutputPin,
    WD: Word,
{
    #[inline]
    fn transaction(&mut self, operations: &mut [Operation<'_, WD>]) -> Result<(), Self::Error> {
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

impl<OS, CS, BUS, WD> ErrorType for SpiSoleDevice<OS, CS, BUS, WD>
where
    OS: OsInterface,
    BUS: SpiBusInterface<WD>,
    CS: OutputPin,
    WD: Word,
{
    type Error = Error;
}
