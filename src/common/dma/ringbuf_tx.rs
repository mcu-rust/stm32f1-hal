use super::*;
use crate::common::{critical_section::Mutex, ringbuf::*};
use core::cell::RefCell;

pub struct DmaRingbufTx {}

impl DmaRingbufTx {
    #[allow(clippy::new_ret_no_self)]
    pub fn new<T, CH, OS: OsInterface>(
        mut ch: CH,
        peripheral_addr: usize,
        buf_size: usize,
        notifier: OS::Notifier,
    ) -> (DmaRingbufTxWriter<T, CH>, DmaRingbufTxLoader<T, CH, OS>)
    where
        T: Sized + Copy,
        CH: DmaChannel,
    {
        let (w, r) = RingBuffer::<T>::new(buf_size);
        ch.set_peripheral_address::<T>(peripheral_addr, true, false, false);
        ch.set_interrupt(DmaEvent::TransferComplete, true);
        let dma = Arc::new(Mutex::new(RefCell::new(DmaHolder::new(ch, r))));
        (
            DmaRingbufTxWriter {
                w,
                dma: Arc::clone(&dma),
            },
            DmaRingbufTxLoader { dma, notifier },
        )
    }
}

// ------------------------------------------------------------------------------------------------

pub struct DmaRingbufTxWriter<T, CH> {
    w: Producer<T>,
    dma: Arc<Mutex<RefCell<DmaHolder<T, CH>>>>,
}

impl<T, CH> DmaRingbufTxWriter<T, CH>
where
    T: Sized + Copy,
    CH: DmaChannel,
{
    #[inline]
    pub fn write(&mut self, data: &[T]) -> usize {
        let ret = self.w.push_slice(data);
        self.reload();
        ret
    }

    #[inline]
    pub fn is_full(&mut self) -> bool {
        self.w.is_full()
    }

    #[inline]
    pub fn in_progress(&self) -> bool {
        critical_section::with(|cs| self.dma.borrow_ref(cs).in_progress())
    }

    #[inline]
    pub fn cancel(&mut self) {
        critical_section::with(|cs| {
            let mut dma = self.dma.borrow_ref_mut(cs);
            dma.work = false;
            dma.ch.stop();
        });
    }

    #[inline]
    fn reload(&mut self) {
        critical_section::with(|cs| {
            let mut dma = self.dma.borrow_ref_mut(cs);
            if !dma.work {
                dma.work = true;
            }
            dma.reload();
        });
    }
}

// ------------------------------------------------------------------------------------------------

/// Can be used in a separate thread or interrupt callback.
pub struct DmaRingbufTxLoader<T, CH, OS: OsInterface> {
    dma: Arc<Mutex<RefCell<DmaHolder<T, CH>>>>,
    notifier: OS::Notifier,
}

impl<T, CH, OS> DmaRingbufTxLoader<T, CH, OS>
where
    T: Sized + Copy,
    CH: DmaChannel,
    OS: OsInterface,
{
    pub fn reload(&mut self) {
        critical_section::with(|cs| {
            self.dma.borrow_ref_mut(cs).reload();
        });
    }

    pub fn interrupt_reload(&mut self) {
        let reloaded = critical_section::with(|cs| {
            let mut dma = self.dma.borrow_ref_mut(cs);
            if dma.ch.is_interrupted(DmaEvent::TransferComplete) {
                dma.reload();
                true
            } else {
                false
            }
        });
        if reloaded {
            self.notifier.notify();
        }
    }
}

// ------------------------------------------------------------------------------------------------

struct DmaHolder<T, CH> {
    ch: CH,
    r: Consumer<T>,
    busy_len: usize,
    work: bool,
}

impl<T, CH> DmaHolder<T, CH>
where
    T: Sized + Copy,
    CH: DmaChannel,
{
    fn new(ch: CH, r: Consumer<T>) -> Self {
        Self {
            ch,
            r,
            busy_len: 0,
            work: false,
        }
    }

    fn in_progress(&self) -> bool {
        if self.work { !self.r.is_empty() } else { false }
    }

    fn reload(&mut self) {
        if self.work && !self.ch.in_progress() {
            self.ch.stop();

            if self.busy_len > 0 {
                let chunk = self.r.read_chunk(self.busy_len).unwrap();
                chunk.commit_all();
            }

            let n = self.r.buffer().capacity() / 2;
            let chunk = match self.r.read_chunk(n) {
                Ok(chunk) => chunk,
                Err(ChunkError::TooFewSlots(n)) => self.r.read_chunk(n).unwrap(),
            };

            let data = chunk.get_slice();
            self.busy_len = data.len();
            if self.busy_len > 0 {
                self.ch.set_memory_buf_for_peripheral(data);
                self.ch.start();
            }
        }
    }
}
