use std::cell::Cell;
use std::marker::PhantomData;
use std::mem::size_of;
use std::ops::Deref;
use std::rc::Rc;
use std::slice;

use super::{BlockIndex, WeakBlockManager, FromBytes, ToBytes};

#[derive(Clone)]
pub struct BlockBuffer {
    ptr: *mut [u8],
    index: BlockIndex,
    manager: Rc<Cell<Option<WeakBlockManager>>>,
}

impl BlockBuffer {
    pub fn new(buffer: Box<[u8]>, index: BlockIndex, manager: Option<WeakBlockManager>) -> Self {
        Self {
            ptr: Box::into_raw(buffer),
            index,
            manager: Rc::new(Cell::new(manager)),
        }
    }

    pub fn index(&self) -> BlockIndex {
        self.index
    }

    pub fn reader(&self) -> BlockReader<'_> {
        BlockReader::new(self.ptr)
    }

    pub fn writer(&self) -> BlockWriter<'_> {
        BlockWriter::new(self.ptr)
    }

    // marked as unsafe because this type's API is designed to avoid slices into the buffer.
    // calls to BlockBuffer::write() must not occur while the slice exists.
    pub(crate) unsafe fn as_slice(&self) -> impl Deref<Target=[u8]> {
        unsafe { slice::from_raw_parts(self.ptr.cast::<u8>(), self.ptr.len()) }
    }

    pub(crate) fn set_manager(&self, manager: Option<WeakBlockManager>) {
        self.manager.set(manager);
    }
}

pub struct BlockReader<'a> {
    ptr: *mut [u8],
    _p: PhantomData<&'a [u8]>,
}

impl<'a> BlockReader<'a> {
    fn new(ptr: *mut [u8]) -> Self {
        Self {
            ptr,
            _p: PhantomData,
        }
    }

    pub fn at<T: FromBytes>(&self, offset: usize) -> T {
        T::from_bytes(&unsafe { slice::from_raw_parts(self.ptr.cast::<u8>(), self.ptr.len()) }[offset..offset + size_of::<T>()])
    }

    pub fn at_and<T: FromBytes>(&self, offset: &mut usize) -> T {
        let value = self.at(*offset);
        *offset += size_of::<T>();
        value
    }

    pub fn slice(&self, offset: usize, slice: &mut [u8]) {
        slice.copy_from_slice(&unsafe { slice::from_raw_parts(self.ptr.cast::<u8>(), self.ptr.len()) }[offset..offset + slice.len()]);
    }

    pub fn slice_and(&self, offset: &mut usize, slice: &mut [u8]) {
        self.slice(*offset, slice);
        *offset += slice.len();
    }
}

pub struct BlockWriter<'a> {
    ptr: *mut [u8],
    _p: PhantomData<&'a [u8]>,
}

impl<'a> BlockWriter<'a> {
    fn new(ptr: *mut [u8]) -> Self {
        Self {
            ptr,
            _p: PhantomData,
        }
    }

    pub fn at<T: ToBytes>(&mut self, offset: usize, value: T) {
        value.to_bytes(&mut unsafe { slice::from_raw_parts_mut(self.ptr.cast::<u8>(), self.ptr.len()) }[offset..offset + size_of::<T>()]);
    }

    pub fn at_and<T: ToBytes>(&mut self, offset: &mut usize, value: T) {
        self.at(*offset, value);
        *offset += size_of::<T>();
    }

    pub fn slice(&mut self, offset: usize, slice: &[u8]) {
        (unsafe { slice::from_raw_parts_mut(self.ptr.cast::<u8>(), self.ptr.len()) }[offset..offset + slice.len()]).copy_from_slice(slice);
    }

    pub fn slice_and(&mut self, offset: &mut usize, slice: &[u8]) {
        self.slice(*offset, slice);
        *offset += slice.len();
    }
}

