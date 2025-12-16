use super::{
    atomic_cell::AtomicCellMember,
    fugit::{Duration, Rate},
};
use core::cell::UnsafeCell;

/// Don't use it if the value changes frequently.
/// And mind that you may get member data from different version.
/// You'd better use something like atomic.
pub struct StaticHolder<T: Copy> {
    inner: UnsafeCell<T>,
}

unsafe impl<T: Copy> Sync for StaticHolder<T> {}

impl<T: Copy> StaticHolder<T> {
    pub const fn new(v: T) -> Self {
        Self {
            inner: UnsafeCell::new(v),
        }
    }

    #[inline]
    pub fn set(&self, v: T) {
        critical_section::with(|_| {
            *self.get_inner() = v;
        });
    }

    /// # Safety
    ///
    /// You may get member data from different version.
    #[inline]
    pub unsafe fn get(&self) -> &T {
        self.get_inner()
    }

    #[inline]
    #[allow(clippy::mut_from_ref)]
    fn get_inner(&self) -> &mut T {
        unsafe { &mut *self.inner.get() }
    }
}

impl<const NOM: u32, const DENOM: u32> AtomicCellMember for Duration<u32, NOM, DENOM> {
    #[inline(always)]
    fn to_num(self) -> usize {
        self.ticks() as usize
    }

    #[inline(always)]
    unsafe fn from_num(value: usize) -> Self {
        Self::from_ticks(value as u32)
    }
}

impl<const NOM: u32, const DENOM: u32> AtomicCellMember for Rate<u32, NOM, DENOM> {
    #[inline(always)]
    fn to_num(self) -> usize {
        self.raw() as usize
    }

    #[inline(always)]
    unsafe fn from_num(value: usize) -> Self {
        Self::from_raw(value as u32)
    }
}
