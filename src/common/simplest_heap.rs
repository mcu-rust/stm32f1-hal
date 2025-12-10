use crate::common::critical_section::Mutex;
use core::{
    alloc::{GlobalAlloc, Layout},
    cell::{Cell, RefCell},
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
pub struct Heap {
    heap: Mutex<RefCell<SimplestHeap>>,
    once_flag: Mutex<Cell<bool>>,
}

impl Heap {
    /// Create a new UNINITIALIZED heap allocator
    ///
    /// You must initialize this heap using the
    /// [`init`](Self::init) method before using the allocator.
    pub const fn empty() -> Heap {
        Heap {
            heap: Mutex::new(RefCell::new(SimplestHeap::empty())),
            once_flag: Mutex::new(Cell::new(false)),
        }
    }

    /// Initializes the heap
    ///
    /// This function must be called BEFORE you run any code that makes use of the
    /// allocator.
    ///
    /// `start_addr` is the address where the heap will be located.
    ///
    /// `size` is the size of the heap in bytes.
    ///
    /// # Safety
    ///
    /// Obey these or Bad Stuff will happen.
    ///
    /// - This function must be called exactly ONCE.
    /// - `size > 0`
    pub unsafe fn init(&self, start_addr: usize, size: usize) {
        assert!(size > 0);
        critical_section::with(|cs| {
            let once_flag = self.once_flag.borrow(cs);
            assert!(!once_flag.get());
            once_flag.set(true);

            self.heap
                .borrow_ref_mut(cs)
                .init(start_addr as *mut u8, size);
        });
    }

    /// Returns an estimate of the amount of bytes in use.
    pub fn used(&self) -> usize {
        critical_section::with(|cs| self.heap.borrow_ref(cs).used())
    }

    /// Returns an estimate of the amount of bytes available.
    pub fn free(&self) -> usize {
        critical_section::with(|cs| self.heap.borrow_ref(cs).free())
    }
}

unsafe impl GlobalAlloc for Heap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        critical_section::with(|cs| self.heap.borrow_ref_mut(cs).alloc(layout))
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}

struct SimplestHeap {
    arena: *mut u8,
    remaining: usize,
    size: usize,
}

unsafe impl Send for SimplestHeap {}

impl SimplestHeap {
    const fn empty() -> Self {
        Self {
            arena: ptr::null_mut(),
            remaining: 0,
            size: 0,
        }
    }

    fn init(&mut self, start_addr: *mut u8, size: usize) {
        self.arena = start_addr;
        self.remaining = size;
        self.size = size;
    }

    fn free(&self) -> usize {
        self.remaining
    }

    fn used(&self) -> usize {
        self.size - self.remaining
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
        self.arena.wrapping_add(self.remaining)
    }
}
