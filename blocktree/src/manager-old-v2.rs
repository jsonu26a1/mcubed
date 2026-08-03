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
                let buffer = BlockBuffer::new(buffer.into());
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
        inner.cache.insert(index, BlockBuffer::new(buffer.into()));
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
    cache: BTreeMap<BlockIndex, BlockBuffer>,
    modified: Vec<BlockIndex>,
    root_header: RootHeader,
}




/*
this is another version of BlockBuffer, which clones the shared buffer so an owned copy can be
modified. less efficient than the below version that reads/writes a pointer, it's a less since,
since users need to call write_block() to have the changes cached and marked as modified.

however, this can also get a bit messy, since Rc::make_mut() is used, if it's used improperly
by a user, a bunch of clones of the original might diverge and branch in complex ways. we could
customize the impl of Clone, so it panics if modified == true; but I don't really anticipate
this being an issue.
*/

#[derive(Clone)]
pub struct BlockBuffer {
    buffer: Rc<[u8]>,
    modified: bool,
}

use std::ops::{ Deref, DerefMut };

impl BlockBuffer {
    pub fn new(buffer: Rc<[u8]>) -> Self {
        Self {
            buffer,
            modified: false,
        }
    }

    pub fn is_modified(&self) -> bool {
        self.modified
    }
}

impl Deref for BlockBuffer {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.buffer
    }
}

impl DerefMut for BlockBuffer {
    fn deref_mut(&mut self) -> &mut [u8] {
        Rc::make_mut(&mut self.buffer)
    }
}



/*
/*
enables reading from and writing to a buffer from behind a shared pointer like Rc

however, what will we do when BlockBuffer might be access by multiple threads? that's
something to worry about later I guess, we will probably be completely re-designing
the BlockManager at that point.

I believe these unsafe operations are sound, because `self.ptr` points to a valid buffer
(obtained from Box<u8>). although we modify the buffer in write() using a immutable reference,
there are never any references into the buffer at those times. we only allow copying bytes
into or out of the buffer.

it might be possible to accomplish this without unsafe{} using Cell, but I couldn't figure
out an easy way to do that.
*/

pub type BlockBuffer = Rc<BlockBufferInner>;

pub struct BlockBufferInner {
    ptr: *mut [u8],
    index: BlockIndex,
    manager: Cell<Option<WeakBlockManager>>,
}

impl BlockBufferInner {
    pub fn new(buffer: Box<[u8]>, index: BlockIndex, manager: WeakBlockManager) -> Self {
        assert!(buffer.len() == BLOCK_SIZE);
        Self {
            ptr: Box::into_raw(buffer),
            index,
            manager,
        }
    }

    pub fn read_to_array<const T: usize>(&self, offset: usize) -> [u8; T] {
        assert!(T + offset <= BLOCK_SIZE);
        unsafe { slice::from_raw_parts(self.ptr.cast::<u8>().add(offset), T) }.try_into().unwrap()
    }

    pub fn read(&self, offset: usize, buffer: &mut [u8]) {
        assert!(buffer.len() + offset <= BLOCK_SIZE);
        unsafe { ptr::copy_nonoverlapping(self.ptr.cast::<u8>().add(offset), buffer.as_mut_ptr(), buffer.len()); }
    }

    // this is unsafe, because write() must not be called while this slice exists.
    pub unsafe fn as_slice(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.ptr.cast::<u8>().add(offset), BLOCK_SIZE) }
    }

    // this is unsafe, because write() must not be called while this slice exists.
    pub unsafe fn as_mut_slice(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.ptr.cast::<u8>().add(offset), BLOCK_SIZE) }
    }

    pub fn write(&self, offset: usize, buffer: &[u8]) {
        assert!(buffer.len() + offset <= BLOCK_SIZE);
        if let Some(manager) = self.manager.replace(None).map(|w| w.upgrade()) {
            manager.mark_block_as_modified(self.index);
        }
        unsafe { ptr::copy_nonoverlapping(buffer.as_ptr(), self.ptr.cast::<u8>().add(offset), buffer.len()); }
    }
}

impl Drop for BlockBufferInner {
    fn drop(&mut self) {
        drop(unsafe { Box::from_raw(self.ptr) });
    }
}
*/

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
