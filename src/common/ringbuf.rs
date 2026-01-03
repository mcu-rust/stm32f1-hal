pub use crate::common::rtrb::{
    chunks::{ChunkError, ReadChunk, WriteChunkUninit},
    *,
};

pub trait ProducerExt<T> {
    fn get_write_chunk_uninit(&mut self) -> Option<WriteChunkUninit<'_, T>>;
    fn push_slice(&mut self, buf: &[T]) -> usize;
    fn is_empty(&self) -> bool;
}
impl<T: Copy> ProducerExt<T> for Producer<T> {
    fn get_write_chunk_uninit(&mut self) -> Option<WriteChunkUninit<'_, T>> {
        let n = self.slots();
        if n > 0
            && let Ok(chunk) = self.write_chunk_uninit(n)
        {
            return Some(chunk);
        }
        None
    }

    fn push_slice(&mut self, buf: &[T]) -> usize {
        let mut size = self.slots();
        if size > 0 {
            let buf = if size >= buf.len() {
                size = buf.len();
                buf
            } else {
                &buf[..size]
            };

            let mut chunk = self.write_chunk_uninit(size).unwrap();
            let (c1, c2) = chunk.get_mut_slices();

            if c1.len() == size {
                c1.copy_from_slice(buf);
            } else {
                let (b1, b2) = buf.split_at(c1.len());
                c1.copy_from_slice(b1);
                c2.copy_from_slice(b2);
            };
            unsafe {
                chunk.commit_all();
            }
            size
        } else {
            0
        }
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
    fn pop_slice(&mut self, elems: &mut [T]) -> usize;
    fn is_full(&self) -> bool;
}
impl<T: Copy> ConsumerExt<T> for Consumer<T> {
    fn get_read_chunk(&mut self) -> Option<ReadChunk<'_, T>> {
        let n = self.slots();
        if n > 0
            && let Ok(chunk) = self.read_chunk(n)
        {
            return Some(chunk);
        }
        None
    }

    fn pop_slice(&mut self, buf: &mut [T]) -> usize {
        let mut size = self.slots();
        if size > 0 {
            let buf = if size >= buf.len() {
                size = buf.len();
                buf
            } else {
                &mut buf[..size]
            };

            let chunk = self.read_chunk(size).unwrap();
            let (c1, c2) = chunk.as_slices();

            if c1.len() == size {
                buf.copy_from_slice(c1);
            } else {
                let (b1, b2) = buf.split_at_mut(c1.len());
                b1.copy_from_slice(c1);
                b2.copy_from_slice(c2);
            };
            chunk.commit_all();
            size
        } else {
            0
        }
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
