use std::cell::{RefCell, Ref, RefMut, Cell};
use std::mem::size_of;
use std::ops::Deref;
use std::rc::Rc;

use super::{BlockIndex, WeakBlockManager, FromBytes, ToBytes};

#[derive(Clone)]
pub struct BlockBuffer(Rc<BlockBufferInner>);

impl BlockBuffer {
    pub fn new(buffer: Box<[u8]>, index: BlockIndex, manager: Option<WeakBlockManager>) -> Self {
        Self(Rc::new(BlockBufferInner {
            buffer: RefCell::new(buffer),
            index,
            manager: Cell::new(manager),
        }))
    }

    fn borrow(&self) -> Ref<'_, [u8]> {
        Ref::map(self.0.buffer.borrow(), |b| &**b)
    }

    fn borrow_mut(&self) -> RefMut<'_, [u8]> {
        if let Some(manager) = self.0.manager.replace(None).map(|w| w.upgrade()).flatten() {
            manager.mark_block_as_modified(self.0.index);
        }
        RefMut::map(self.0.buffer.borrow_mut(), |b| &mut **b)
    }

    pub fn index(&self) -> BlockIndex {
        self.0.index
    }

    pub fn reader(&self) -> BlockReader<'_> {
        BlockReader(self.borrow())
    }

    pub fn writer(&self) -> BlockWriter<'_> {
        BlockWriter(self.borrow_mut())
    }

    // marked as unsafe because this type's API is designed to avoid slices into the buffer.
    // calls to BlockBuffer::write() must not occur while the slice exists.
    pub(crate) unsafe fn as_slice(&self) -> impl Deref<Target=[u8]> {
        self.borrow()
    }

    pub(crate) fn set_manager(&self, manager: Option<WeakBlockManager>) {
        self.0.manager.set(manager);
    }
}

struct BlockBufferInner {
    buffer: RefCell<Box<[u8]>>,
    index: BlockIndex,
    manager: Cell<Option<WeakBlockManager>>,
}

pub struct BlockReader<'a>(pub Ref<'a, [u8]>);

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

pub struct BlockWriter<'a>(pub RefMut<'a, [u8]>);

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
}
