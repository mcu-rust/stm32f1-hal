use crate::common::critical_section::Mutex;
use core::{
    alloc::{GlobalAlloc, Layout},
    cell::UnsafeCell,
    mem::MaybeUninit,
    ptr,
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
    heap: Mutex<UnsafeCell<SimplestHeap<SIZE>>>,
}

impl<const SIZE: usize> Heap<SIZE> {
    /// Create a new heap allocator
    pub const fn new() -> Self {
        Self {
            heap: Mutex::new(UnsafeCell::new(SimplestHeap::new())),
        }
    }

    /// Returns an estimate of the amount of bytes in use.
    pub fn used(&self) -> usize {
        critical_section::with(|cs| unsafe { &*self.heap.borrow(cs).get() }.used())
    }
}

unsafe impl<const SIZE: usize> GlobalAlloc for Heap<SIZE> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        critical_section::with(|cs| unsafe { &mut *self.heap.borrow(cs).get() }.alloc(layout))
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}

struct SimplestHeap<const SIZE: usize> {
    arena: [MaybeUninit<u8>; SIZE],
    remaining: usize,
}

unsafe impl<const SIZE: usize> Send for SimplestHeap<SIZE> {}

impl<const SIZE: usize> SimplestHeap<SIZE> {
    const fn new() -> Self {
        Self {
            arena: [MaybeUninit::uninit(); SIZE],
            remaining: SIZE,
        }
    }

    fn used(&self) -> usize {
        SIZE - self.remaining
    }

    fn alloc(&mut self, layout: Layout) -> *mut u8 {
        if layout.size() > self.remaining {
            return ptr::null_mut();
        }

        // `Layout` contract forbids making a `Layout` with align=0, or align not power of 2.
        // So we can safely use a mask to ensure alignment without worrying about UB.
        let align_mask_to_round_down = !(layout.align() - 1);

        self.remaining -= layout.size();
        self.remaining &= align_mask_to_round_down;
        (self.arena.as_mut_ptr() as *mut u8).wrapping_add(self.remaining)
    }
}
