use super::*;
use core::{cell::Cell, marker::PhantomData};

pub struct TickTimeout<T> {
    frequency: Cell<u32>,
    _t: PhantomData<T>,
}

unsafe impl<T: TickInstant> Sync for TickTimeout<T> {}

impl<T> TickTimeout<T>
where
    T: TickInstant,
{
    pub const fn empty() -> Self {
        Self {
            frequency: Cell::new(1_000_000),
            _t: PhantomData,
        }
    }

    pub fn new(frequency: u32) -> Self {
        assert_eq!(frequency % 1_000_000, 0);
        Self {
            frequency: Cell::new(frequency),
            _t: PhantomData,
        }
    }

    pub fn set(&self, frequency: u32) {
        assert_eq!(frequency % 1_000_000, 0);
        critical_section::with(|_| {
            self.frequency.set(frequency);
        })
    }
}

impl<T> Timeout for TickTimeout<T>
where
    T: TickInstant,
{
    fn start(&self, timeout: MicrosDurationU32) -> impl TimeoutStatus {
        TickTimeoutStatus::<T> {
            tick: T::now(),
            timeout_tick: timeout
                .ticks()
                .checked_mul(self.frequency.get() / 1_000_000)
                .unwrap(),
            elapsed_tick: 0,
        }
    }
}

pub struct TickTimeoutStatus<T: TickInstant> {
    tick: T,
    timeout_tick: u32,
    elapsed_tick: u32,
}

impl<T> TimeoutStatus for TickTimeoutStatus<T>
where
    T: TickInstant,
{
    /// Can be reused without calling `restart()`.
    #[inline]
    fn timeout(&mut self) -> bool {
        let now = T::now();
        self.elapsed_tick = self.elapsed_tick.add_u32(now.tick_since(self.tick));
        self.tick = now;

        if self.elapsed_tick >= self.timeout_tick {
            self.elapsed_tick -= self.timeout_tick;
            true
        } else {
            false
        }
    }

    #[inline(always)]
    fn restart(&mut self) {
        self.tick = T::now();
        self.elapsed_tick = 0;
    }
}
pub trait Num: Sized + Copy + core::cmp::Ord + core::ops::SubAssign {
    const ZERO: Self;
    fn add_u32(self, v: u32) -> Self;
}

impl Num for u32 {
    const ZERO: Self = 0;
    fn add_u32(self, v: u32) -> Self {
        self.saturating_add(v)
    }
}

impl Num for u64 {
    const ZERO: Self = 0u64;
    fn add_u32(self, v: u32) -> Self {
        self.saturating_add(v as u64)
    }
}
