use core::{marker::PhantomData, sync::atomic::AtomicUsize};

pub use core::sync::atomic::Ordering;

pub struct AtomicCell<M: AtomicCellMember> {
    value: AtomicUsize,
    _m: PhantomData<M>,
}

unsafe impl<M: AtomicCellMember> Send for AtomicCell<M> {}
unsafe impl<M: AtomicCellMember> Sync for AtomicCell<M> {}

impl<M: AtomicCellMember> AtomicCell<M> {
    pub fn new(value: M) -> Self {
        Self {
            value: AtomicUsize::new(value.as_num()),
            _m: PhantomData,
        }
    }

    pub const fn const_new(value: usize) -> Self {
        Self {
            value: AtomicUsize::new(value),
            _m: PhantomData,
        }
    }

    #[inline]
    pub fn load(&self, order: Ordering) -> M {
        unsafe { M::from_num(self.value.load(order)) }
    }

    #[inline]
    pub fn store(&self, value: M, order: Ordering) {
        self.value.store(value.as_num(), order);
    }
}

pub trait AtomicCellMember: Copy {
    fn as_num(self) -> usize;
    unsafe fn from_num(value: usize) -> Self;
}
