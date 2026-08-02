use std::collections::btree_map::{BTreeMap, Entry};
use std::cell::RefCell;
use std::io::Result as IoResult;
use std::rc::Rc;

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

    pub fn read_block(&self, index: BlockIndex) -> IoResult<Rc<[u8]>> {
        let inner = &mut *self.inner.borrow_mut();
        match inner.cache.entry(index) {
            Entry::Vacant(ve) => {
                let mut buffer = vec![];
                buffer.resize(BLOCK_SIZE, 0);
                inner.backend.read(index * BLOCK_SIZE as u64, &mut buffer)?;
                let buffer: Rc<[u8]> = buffer.into();
                ve.insert(buffer.clone());
                Ok(buffer)
            },
            Entry::Occupied(oe) => {
                Ok(oe.get().clone())
            }
        }
    }

    pub fn write_block(&self, index: BlockIndex, buffer: impl Into<Rc<[u8]>>) {
        let inner = &mut *self.inner.borrow_mut();
        inner.cache.insert(index, buffer.into());
        inner.modified.push(index);
    }

    pub fn commit_pending_writes(&self) -> IoResult<()> {
        let inner = &mut *self.inner.borrow_mut();
        while let Some(index) = inner.modified.pop() {
            let buffer = inner.cache.get_mut(&index).unwrap();
            inner.backend.write(index * BLOCK_SIZE as u64, &buffer)?;
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
    cache: BTreeMap<BlockIndex, Rc<[u8]>>,
    modified: Vec<BlockIndex>,
    root_header: RootHeader,
}

pub struct BlockBuffer {
    ptr: *mut u8,
    len: usize,
}

impl BlockBuffer {
    fn new(buffer: Vec<u8>) -> Self {
        todo!();
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

    fn load(&mut self, buffer: &[u8]) {
        let mut offset = 0;
        self.blocks_used = BlockIndex::from_be_bytes(buffer[offset..offset+BLOCK_INDEX_SIZE].try_into().unwrap());
        offset += BLOCK_INDEX_SIZE;
        self.blocks_total = BlockIndex::from_be_bytes(buffer[offset..offset+BLOCK_INDEX_SIZE].try_into().unwrap());
        offset += BLOCK_INDEX_SIZE;
        self.write_counter = u128::from_be_bytes(buffer[offset..offset+16].try_into().unwrap());
        offset += 16;
        self.free_blocks_tree = BlockIndex::from_be_bytes(buffer[offset..offset+BLOCK_INDEX_SIZE].try_into().unwrap());
        offset += BLOCK_INDEX_SIZE;
    }

    fn store(&mut self, buffer: &mut [u8]) {
        let mut offset = 0;
        buffer[offset..offset+BLOCK_INDEX_SIZE].copy_from_slice(&self.blocks_used.to_be_bytes()[..]);
        offset += BLOCK_INDEX_SIZE;
        buffer[offset..offset+BLOCK_INDEX_SIZE].copy_from_slice(&self.blocks_total.to_be_bytes()[..]);
        offset += BLOCK_INDEX_SIZE;
        buffer[offset..offset+16].copy_from_slice(&self.write_counter.to_be_bytes()[..]);
        offset += 16;
        buffer[offset..offset+BLOCK_INDEX_SIZE].copy_from_slice(&self.free_blocks_tree.to_be_bytes()[..]);
        offset += BLOCK_INDEX_SIZE;
    }
}
