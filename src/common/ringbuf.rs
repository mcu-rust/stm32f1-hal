pub use crate::common::rtrb::{
    chunks::{ChunkError, ReadChunk, WriteChunkUninit},
    *,
};

pub trait ProducerExt<T> {
    fn get_write_chunk_uninit(&mut self) -> Option<WriteChunkUninit<'_, T>>;
    fn is_empty(&self) -> bool;
}
impl<T: Copy> ProducerExt<T> for Producer<T> {
    fn get_write_chunk_uninit(&mut self) -> Option<WriteChunkUninit<'_, T>> {
        let n = self.slots();
        if n > 0 {
            return self.write_chunk_uninit(n).ok();
        }
        None
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.slots() == self.buffer().capacity()
    }
}

pub trait WriteChunkExt<T> {
    fn get_mut_slice(&mut self) -> &mut [T];
    fn get_mut_slices(&mut self) -> (&mut [T], &mut [T]);
}
impl<T: Copy> WriteChunkExt<T> for WriteChunkUninit<'_, T> {
    #[inline]
    fn get_mut_slice(&mut self) -> &mut [T] {
        let (buf, _) = self.as_mut_slices();
        unsafe {
            let dst_ptr = buf.as_mut_ptr().cast();
            core::slice::from_raw_parts_mut(dst_ptr, buf.len())
        }
    }

    #[inline]
    fn get_mut_slices(&mut self) -> (&mut [T], &mut [T]) {
        let (a, b) = self.as_mut_slices();
        unsafe {
            (
                core::slice::from_raw_parts_mut(a.as_mut_ptr().cast(), a.len()),
                core::slice::from_raw_parts_mut(b.as_mut_ptr().cast(), b.len()),
            )
        }
    }
}

pub trait ConsumerExt<T> {
    fn get_read_chunk(&mut self) -> Option<ReadChunk<'_, T>>;
    fn is_full(&self) -> bool;
}
impl<T: Copy> ConsumerExt<T> for Consumer<T> {
    fn get_read_chunk(&mut self) -> Option<ReadChunk<'_, T>> {
        let n = self.slots();
        if n > 0 {
            return self.read_chunk(n).ok();
        }
        None
    }

    fn is_full(&self) -> bool {
        self.slots() == self.buffer().capacity()
    }
}

pub trait ReadChunkExt<T> {
    fn get_slice(&self) -> &[T];
}
impl<T: Copy> ReadChunkExt<T> for ReadChunk<'_, T> {
    #[inline]
    fn get_slice(&self) -> &[T] {
        let (buf, _) = self.as_slices();
        buf
    }
}
