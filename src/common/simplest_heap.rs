use core::{
    alloc::{GlobalAlloc, Layout},
    cell::UnsafeCell,
    mem::MaybeUninit,
    ptr,
    sync::atomic::{AtomicUsize, Ordering},
};

/// The simplest possible heap.
///
/// # Safety
///
/// Because it's the simplest implementation, it does **NOT** free memory.
/// Any memory you drop cannot be reused (it's leaked), so avoid dropping anything whenever possible.
///
/// It is recommended that you use [embedded-alloc](https://crates.io/crates/embedded-alloc)
pub struct Heap<const SIZE: usize> {
    arena: UnsafeCell<[MaybeUninit<u8>; SIZE]>,
    remained: AtomicUsize,
}

unsafe impl<const SIZE: usize> Sync for Heap<SIZE> {}

impl<const SIZE: usize> Heap<SIZE> {
    /// Create a new heap allocator
    pub const fn new() -> Self {
        Self {
            arena: UnsafeCell::new([MaybeUninit::uninit(); SIZE]),
            remained: AtomicUsize::new(SIZE),
        }
    }

    /// Returns an estimate of the amount of bytes in use.
    pub fn remained(&self) -> usize {
        self.remained.load(Ordering::Relaxed)
    }
}

unsafe impl<const SIZE: usize> GlobalAlloc for Heap<SIZE> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // `Layout` contract forbids making a `Layout` with align=0, or align not power of 2.
        // So we can safely use a mask to ensure alignment without worrying about UB.
        let align_mask_to_round_down = !(layout.align() - 1);

        let mut old_remained = self.remained.load(Ordering::Relaxed);
        loop {
            if layout.size() > old_remained {
                return ptr::null_mut();
            }

            let remained = (old_remained - layout.size()) & align_mask_to_round_down;
            match self.remained.compare_exchange_weak(
                old_remained,
                remained,
                Ordering::SeqCst,
                Ordering::Relaxed,
            ) {
                Err(x) => old_remained = x,
                Ok(_) => {
                    return unsafe {
                        ((&mut *self.arena.get()).as_mut_ptr() as *mut u8).add(remained)
                    };
                }
            }
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    static HEAP: Heap<100> = Heap::new();

    #[test]
    fn test_heap() {
        assert_eq!(HEAP.remained.load(Ordering::Relaxed), 100);
        let p1 = unsafe { &mut *HEAP.arena.get() }.as_mut_ptr() as *mut u8;
        let p2 = unsafe { HEAP.alloc(Layout::new::<u64>()) };
        assert_eq!(HEAP.remained.load(Ordering::Relaxed), 88);
        assert_eq!(unsafe { p2.offset_from(p1) }, 88);

        unsafe { HEAP.alloc(Layout::new::<u32>()) };
        assert_eq!(HEAP.remained.load(Ordering::Relaxed), 84);
    }
}
