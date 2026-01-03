mod circular_buffer_rx;
mod ringbuf_tx;

pub use circular_buffer_rx::*;
pub use ringbuf_tx::*;

use crate::common::prelude::*;

pub trait DmaChannel {
    fn start(&mut self);
    fn stop(&mut self);

    fn set_peripheral_address<T: Sized + Copy>(
        &mut self,
        address: usize,
        mem_to_periph: bool,
        increase: bool,
        circular: bool,
    );
    fn set_memory_address(&mut self, address: usize, increase: bool);
    fn set_transfer_length(&mut self, len: usize);
    fn set_memory_buf_for_peripheral<T: Sized + Copy>(&mut self, buf: &[T]) {
        self.set_memory_address(buf.as_ptr() as usize, true);
        self.set_transfer_length(buf.len());
    }

    fn set_memory_to_memory<T: Sized + Copy>(
        &mut self,
        src_addr: usize,
        dst_addr: usize,
        len: usize,
    );

    fn get_unprocessed_len(&self) -> usize;
    fn in_progress(&self) -> bool;

    fn set_interrupt(&mut self, event: DmaEvent, enable: bool);
    /// check and clear interrupt flag
    fn check_and_clear_interrupt(&mut self, event: DmaEvent) -> bool;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DmaEvent {
    TransferComplete,
    HalfTransfer,
}
