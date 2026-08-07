use std::cell::{RefCell, Ref, RefMut, Cell};
use std::mem::size_of;
use std::ops::{Deref, DerefMut, RangeBounds};
use std::rc::Rc;

use super::{BlockIndex, WeakBlockManager, FromBytes, ToBytes};

#[derive(Clone)]
pub struct BlockBuffer(Rc<BlockBufferInner>);

impl BlockBuffer {
    pub fn new(buffer: Box<[u8]>, index: BlockIndex, manager: WeakBlockManager) -> Self {
        Self(Rc::new(BlockBufferInner {
            buffer: RefCell::new(buffer),
            index,
            manager,
            modified: Cell::new(false),
        }))
    }

    pub fn index(&self) -> BlockIndex {
        self.0.index
    }

    pub fn buffer_len(&self) -> usize {
        self.0.buffer.borrow().len()
    }

    pub fn reader(&self) -> BlockReader<'_> {
        BlockReader(Ref::map(self.0.buffer.borrow(), |b| &**b))
    }

    pub fn writer(&self) -> BlockWriter<'_> {
        if !self.0.modified.get() {
            self.0.modified.set(true);
            if let Some(manager) = self.0.manager.upgrade() {
                manager.mark_block_as_modified(self.0.index);
            }
        }
        BlockWriter(RefMut::map(self.0.buffer.borrow_mut(), |b| &mut **b))
    }

    // marked as unsafe because this type's API is designed to avoid slices into the buffer.
    // caller must not access the buffer via reader/writer until slices are dropped.
    pub(crate) unsafe fn as_slice(&self) -> impl Deref<Target=[u8]> {
        Ref::map(self.0.buffer.borrow(), |b| &**b)
    }

    // marked as unsafe because this type's API is designed to avoid slices into the buffer.
    // caller must not access the buffer via reader/writer until slices are dropped.
    pub(crate) unsafe fn as_mut_slice(&self) -> impl DerefMut<Target=[u8]> {
        RefMut::map(self.0.buffer.borrow_mut(), |b| &mut **b)
    }

    pub(crate) fn set_modified(&self, modified: bool) {
        self.0.modified.set(modified);
    }
}

struct BlockBufferInner {
    buffer: RefCell<Box<[u8]>>,
    index: BlockIndex,
    manager: WeakBlockManager,
    modified: Cell<bool>,
}

pub struct BlockReader<'a>(Ref<'a, [u8]>);

impl<'a> BlockReader<'a> {
    pub fn at<T: FromBytes>(&self, offset: usize) -> T {
        T::from_bytes(&self.0[offset..])
    }

    pub fn at_and<T: FromBytes>(&self, offset: &mut usize) -> T {
        let value = self.at(*offset);
        *offset += size_of::<T>();
        value
    }

    pub fn slice(&self, offset: usize, slice: &mut [u8]) {
        slice.copy_from_slice(&self.0[offset..offset + slice.len()]);
    }

    pub fn slice_and(&self, offset: &mut usize, slice: &mut [u8]) {
        self.slice(*offset, slice);
        *offset += slice.len();
    }
}

pub struct BlockWriter<'a>(RefMut<'a, [u8]>);

impl<'a> BlockWriter<'a> {
    pub fn at<T: ToBytes>(&mut self, offset: usize, value: T) {
        value.to_bytes(&mut self.0[offset..]);
    }

    pub fn at_and<T: ToBytes>(&mut self, offset: &mut usize, value: T) {
        self.at(*offset, value);
        *offset += size_of::<T>();
    }

    pub fn slice(&mut self, offset: usize, slice: &[u8]) {
        self.0[offset..offset + slice.len()].copy_from_slice(slice);
    }

    pub fn slice_and(&mut self, offset: &mut usize, slice: &[u8]) {
        self.slice(*offset, slice);
        *offset += slice.len();
    }

    pub fn copy_within(&mut self, src: impl RangeBounds<usize>, dest: usize) {
        self.0.copy_within(src, dest)
    }

    #[allow(private_bounds)]
    pub fn copy_from(&mut self, other: &impl ReaderOrWriter, src: impl RangeBounds<usize>, dest: usize) {
        // the API for RangeBounds is so dumb
        let r = (src.start_bound().map(|i| *i), src.end_bound().map(|i| *i));
        let src_slice = &other.inner()[r];
        self.0[dest..dest+src_slice.len()].copy_from_slice(src_slice);
    }
}

trait ReaderOrWriter {
    fn inner(&self) -> &[u8];
}

impl<'a> ReaderOrWriter for BlockReader<'a> {
    fn inner(&self) -> &[u8] {
        &self.0
    }
}

impl<'a> ReaderOrWriter for BlockWriter<'a> {
    fn inner(&self) -> &[u8] {
        &self.0
    }
}
