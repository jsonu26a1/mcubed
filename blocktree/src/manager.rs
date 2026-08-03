use std::collections::btree_map::{BTreeMap, Entry};
use std::cell::{RefCell, Ref, RefMut, Cell};
use std::io::Result as IoResult;
use std::rc::{Rc, Weak};
use std::mem::size_of;

use super::backend::IoBackend;
use super::{ BlockIndex, BLOCK_INDEX_SIZE, BLOCK_SIZE };

pub struct BlockManager {
    inner: Rc<RefCell<BlockManagerInner>>,
}

/*
BlockManager::read_block() returns a Rc<[u8]>, fetching the buffer from the backend (file
system) if it's not in the cache already. write_block() updates the cache by inserting a
new buffer, which invalidates any previous Rc buffers that were returned by the BlockManager.
it's up to the user to ensure the data in an Rc buffer is current. since this is a low level
interface on which higher level features will be built, this shouldn't be an issue.

the design of BlockManager will probably change a lot, but this is the minimum effort design
I'm going with right now.
*/

impl BlockManager {
    pub fn new(backend: Box<dyn IoBackend>) -> IoResult<Self> {
        let manager = Self {
            inner: Rc::new(RefCell::new(BlockManagerInner {
                backend,
                cache: BTreeMap::new(),
                modified: vec![],
                root_header: RootHeader::new(),
            })),
        };
        let buffer = manager.read_block(0)?;
        manager.inner.borrow_mut().root_header.load(&buffer);
        Ok(manager)
    }

    pub fn read_block(&self, index: BlockIndex) -> IoResult<BlockBuffer> {
        let inner = &mut *self.inner.borrow_mut();
        match inner.cache.entry(index) {
            Entry::Vacant(ve) => {
                let mut buffer = vec![];
                buffer.resize(BLOCK_SIZE, 0);
                inner.backend.read(index * BLOCK_SIZE as u64, &mut buffer)?;
                let buffer = Rc::new(BlockBufferInner::new(buffer.into_boxed_slice(), index, Some(Rc::downgrade(&self.inner))));
                ve.insert(buffer.clone());
                Ok(buffer)
            },
            Entry::Occupied(oe) => {
                Ok(oe.get().clone())
            }
        }
    }

    // pub fn write_block(&self, index: BlockIndex, buffer: impl Into<Rc<[u8]>>) {
    //     let inner = &mut *self.inner.borrow_mut();
    //     inner.cache.insert(index, BlockBuffer::new(buffer.into()));
    //     inner.modified.push(index);
    // }

    fn mark_block_as_modified(&self, index: BlockIndex) {
        let inner = &mut *self.inner.borrow_mut();
        inner.modified.push(index);
    }

    pub fn commit_pending_writes(&self) -> IoResult<()> {
        let inner = &mut *self.inner.borrow_mut();
        while let Some(index) = inner.modified.pop() {
            let buffer = inner.cache.get_mut(&index).unwrap();
            buffer.manager.set(Some(Rc::downgrade(&self.inner)));
            inner.backend.write(index * BLOCK_SIZE as u64, &*buffer.borrow())?;
        }
        Ok(())
    }

    pub fn abort_pending_writes(&self) {
        let inner = &mut *self.inner.borrow_mut();
        // inner.cache.clear();
        while let Some(index) = inner.modified.pop() {
            // let _ = inner.cache.remove(index);
            inner.cache.remove(&index);
        }
    }

    // NOTE: this implies an abort of all pending writes
    pub fn clear(&self) {
        let inner = &mut *self.inner.borrow_mut();
        inner.cache.clear();
        inner.modified.clear();
    }

    pub fn alloc_block(&self) -> BlockIndex {
        todo!();
    }

    pub fn free_block(&self, index: BlockIndex) {
        todo!();
    }
}

struct BlockManagerInner {
    backend: Box<dyn IoBackend>,
    cache: BTreeMap<BlockIndex, BlockBuffer>,
    modified: Vec<BlockIndex>,
    root_header: RootHeader,
}

type WeakBlockManager = Weak<RefCell<BlockManagerInner>>;

struct RootHeader {
    blocks_used: BlockIndex,
    blocks_total: BlockIndex,
    write_counter: u128,
    free_blocks_tree: BlockIndex,
}

impl RootHeader {
    fn new() -> Self {
        Self {
            blocks_used: 0,
            blocks_total: 0,
            write_counter: 0,
            free_blocks_tree: 0,
        }
    }

    fn load(&mut self, buffer: &BlockBuffer) {
        let reader = buffer.reader();
        let mut offset = 0;
        self.blocks_used = reader.read_and(&mut offset);
        self.blocks_total = reader.read_and(&mut offset);
        self.write_counter = reader.read_and(&mut offset);
        self.free_blocks_tree = reader.read_and(&mut offset);
    }

    fn store(&mut self, buffer: &BlockBuffer) {
        let mut writer = buffer.writer();
        let mut offset = 0;
        writer.write_and(&mut offset, self.blocks_used);
        writer.write_and(&mut offset, self.blocks_total);
        writer.write_and(&mut offset, self.write_counter);
        writer.write_and(&mut offset, self.free_blocks_tree);
    }
}

/*
a shared pointer that allows reading from and writing to a buffer

this will probably change in the future, I had a few ideas I was messing around with, including
Rc::make_mut() on write; and another that used a `*mut [u8]` ptr and only allowing copying bytes
into and out of the buffer, and forbidding slices of the buffer. but I decided to go with this
design for now just to be safe.

NOTE: calling borrow_mut() marks this block as being modified, meaning it will be written
out to the backend during commit_pending_writes().
*/

pub type BlockBuffer = Rc<BlockBufferInner>;

pub struct BlockBufferInner {
    buffer: RefCell<Box<[u8]>>,
    index: BlockIndex,
    manager: Cell<Option<WeakBlockManager>>,
}

impl BlockBufferInner {
    pub fn new(buffer: Box<[u8]>, index: BlockIndex, manager: Option<WeakBlockManager>) -> Self {
        Self {
            buffer: RefCell::new(buffer),
            index,
            manager: Cell::new(manager),
        }
    }

    pub fn borrow(&self) -> Ref<'_, [u8]> {
        Ref::map(self.buffer.borrow(), |b| &**b)
    }

    pub fn borrow_mut(&self) -> RefMut<'_, [u8]> {
        if let Some(inner) = self.manager.replace(None).map(|w| w.upgrade()).flatten() {
            let manager = BlockManager { inner };
            manager.mark_block_as_modified(self.index);
        }
        RefMut::map(self.buffer.borrow_mut(), |b| &mut **b)
    }

    pub fn reader(&self) -> BufferReader<'_> {
        BufferReader(self.borrow())
    }

    pub fn writer(&self) -> BufferWriter<'_> {
        BufferWriter(self.borrow_mut())
    }
}

pub struct BufferReader<'a>(pub Ref<'a, [u8]>);

impl<'a> BufferReader<'a> {
    pub fn read<T: FromToBytes>(&self, offset: usize) -> T {
        T::from_bytes(&self.0[offset..])
    }

    pub fn read_and<T: FromToBytes>(&self, offset: &mut usize) -> T {
        let value = self.read(*offset);
        *offset += size_of::<T>();
        value
    }
}

pub struct BufferWriter<'a>(pub RefMut<'a, [u8]>);

impl<'a> BufferWriter<'a> {
    pub fn write<T: FromToBytes>(&mut self, offset: usize, value: T) {
        value.to_bytes(&mut self.0[offset..])
    }

    pub fn write_and<T: FromToBytes>(&mut self, offset: &mut usize, value: T) {
        self.write(*offset, value);
        *offset += size_of::<T>();
    }
}

pub trait FromToBytes {
    fn from_bytes(buffer: &[u8]) -> Self;
    fn to_bytes(&self, buffer: &mut [u8]);
}

impl FromToBytes for u8 {
    fn from_bytes(buffer: &[u8]) -> Self {
        buffer[0]
    }
    fn to_bytes(&self, buffer: &mut [u8]) {
        buffer[0] = *self;
    }
}

impl FromToBytes for u16 {
    fn from_bytes(buffer: &[u8]) -> Self {
        Self::from_be_bytes(buffer[0..size_of::<Self>()].try_into().unwrap())
    }
    fn to_bytes(&self, buffer: &mut [u8]) {
        buffer[0..size_of::<Self>()].copy_from_slice(self.to_be_bytes().as_slice());
    }
}

impl FromToBytes for u32 {
    fn from_bytes(buffer: &[u8]) -> Self {
        Self::from_be_bytes(buffer[0..size_of::<Self>()].try_into().unwrap())
    }
    fn to_bytes(&self, buffer: &mut [u8]) {
        buffer[0..size_of::<Self>()].copy_from_slice(self.to_be_bytes().as_slice());
    }
}

impl FromToBytes for u64 {
    fn from_bytes(buffer: &[u8]) -> Self {
        Self::from_be_bytes(buffer[0..size_of::<Self>()].try_into().unwrap())
    }
    fn to_bytes(&self, buffer: &mut [u8]) {
        buffer[0..size_of::<Self>()].copy_from_slice(self.to_be_bytes().as_slice());
    }
}

impl FromToBytes for u128 {
    fn from_bytes(buffer: &[u8]) -> Self {
        Self::from_be_bytes(buffer[0..size_of::<Self>()].try_into().unwrap())
    }
    fn to_bytes(&self, buffer: &mut [u8]) {
        buffer[0..size_of::<Self>()].copy_from_slice(self.to_be_bytes().as_slice());
    }
}

impl FromToBytes for usize {
    fn from_bytes(buffer: &[u8]) -> Self {
        Self::from_be_bytes(buffer[0..size_of::<Self>()].try_into().unwrap())
    }
    fn to_bytes(&self, buffer: &mut [u8]) {
        buffer[0..size_of::<Self>()].copy_from_slice(self.to_be_bytes().as_slice());
    }
}

impl FromToBytes for i8 {
    fn from_bytes(buffer: &[u8]) -> Self {
        buffer[0] as Self
    }
    fn to_bytes(&self, buffer: &mut [u8]) {
        buffer[0] = *self as u8;
    }
}

impl FromToBytes for i16 {
    fn from_bytes(buffer: &[u8]) -> Self {
        Self::from_be_bytes(buffer[0..size_of::<Self>()].try_into().unwrap())
    }
    fn to_bytes(&self, buffer: &mut [u8]) {
        buffer[0..size_of::<Self>()].copy_from_slice(self.to_be_bytes().as_slice());
    }
}

impl FromToBytes for i32 {
    fn from_bytes(buffer: &[u8]) -> Self {
        Self::from_be_bytes(buffer[0..size_of::<Self>()].try_into().unwrap())
    }
    fn to_bytes(&self, buffer: &mut [u8]) {
        buffer[0..size_of::<Self>()].copy_from_slice(self.to_be_bytes().as_slice());
    }
}

impl FromToBytes for i64 {
    fn from_bytes(buffer: &[u8]) -> Self {
        Self::from_be_bytes(buffer[0..size_of::<Self>()].try_into().unwrap())
    }
    fn to_bytes(&self, buffer: &mut [u8]) {
        buffer[0..size_of::<Self>()].copy_from_slice(self.to_be_bytes().as_slice());
    }
}

impl FromToBytes for i128 {
    fn from_bytes(buffer: &[u8]) -> Self {
        Self::from_be_bytes(buffer[0..size_of::<Self>()].try_into().unwrap())
    }
    fn to_bytes(&self, buffer: &mut [u8]) {
        buffer[0..size_of::<Self>()].copy_from_slice(self.to_be_bytes().as_slice());
    }
}

impl FromToBytes for isize {
    fn from_bytes(buffer: &[u8]) -> Self {
        Self::from_be_bytes(buffer[0..size_of::<Self>()].try_into().unwrap())
    }
    fn to_bytes(&self, buffer: &mut [u8]) {
        buffer[0..size_of::<Self>()].copy_from_slice(self.to_be_bytes().as_slice());
    }
}
