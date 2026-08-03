use std::collections::btree_map::{BTreeMap, Entry};
use std::cell::{RefCell, Ref, RefMut, Cell};
use std::io::Result as IoResult;
use std::rc::{Rc, Weak};
use std::mem::size_of;
use std::ops::Deref;

use super::backend::IoBackend;
use super::block::{BlockBuffer};
use super::{BlockIndex, BLOCK_INDEX_SIZE, BLOCK_SIZE};

pub struct BlockManager {
    inner: Rc<RefCell<BlockManagerInner>>,
}

/*
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

    pub fn downgrade(&self) -> WeakBlockManager {
        WeakBlockManager(Rc::downgrade(&self.inner))
    }

    pub fn read_block(&self, index: BlockIndex) -> IoResult<BlockBuffer> {
        let inner = &mut *self.inner.borrow_mut();
        match inner.cache.entry(index) {
            Entry::Vacant(ve) => {
                let mut buffer = vec![];
                buffer.resize(BLOCK_SIZE, 0);
                inner.backend.read(index * BLOCK_SIZE as u64, &mut buffer)?;
                let buffer = BlockBuffer::new(buffer.into_boxed_slice(), index, Some(self.downgrade()));
                ve.insert(buffer.clone());
                Ok(buffer)
            },
            Entry::Occupied(oe) => {
                Ok(oe.get().clone())
            }
        }
    }

    pub(crate) fn mark_block_as_modified(&self, index: BlockIndex) {
        let inner = &mut *self.inner.borrow_mut();
        inner.modified.push(index);
    }

    pub fn commit_pending_writes(&self) -> IoResult<()> {
        let inner = &mut *self.inner.borrow_mut();
        while let Some(index) = inner.modified.pop() {
            let buffer = inner.cache.get_mut(&index).unwrap();
            buffer.set_manager(Some(self.downgrade()));
            inner.backend.write(index * BLOCK_SIZE as u64, unsafe { buffer.as_slice() }.deref())?;
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

// pub type WeakBlockManager = Weak<RefCell<BlockManagerInner>>;

#[derive(Clone)]
pub struct WeakBlockManager(Weak<RefCell<BlockManagerInner>>);

impl WeakBlockManager {
    pub fn upgrade(&self) -> Option<BlockManager> {
        self.0.upgrade().map(|inner| BlockManager { inner })
    }
}

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
        self.blocks_used = reader.at_and(&mut offset);
        self.blocks_total = reader.at_and(&mut offset);
        self.write_counter = reader.at_and(&mut offset);
        self.free_blocks_tree = reader.at_and(&mut offset);
    }

    fn store(&mut self, buffer: &BlockBuffer) {
        let mut writer = buffer.writer();
        let mut offset = 0;
        writer.at_and(&mut offset, self.blocks_used);
        writer.at_and(&mut offset, self.blocks_total);
        writer.at_and(&mut offset, self.write_counter);
        writer.at_and(&mut offset, self.free_blocks_tree);
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
