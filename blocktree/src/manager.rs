use std::cell::RefCell;
use std::io::Result as IoResult;
use std::mem;
use std::rc::Rc;

use super::backend::IoBackend;

struct BlockManagerInner<B> {
    backend: B,
    // TODO
}

impl<B: IoBackend> BlockManagerInner<B> {
    // TODO
}

pub struct BlockManager {
    inner: Rc<RefCell<BlockManagerInner<Box<dyn IoBackend>>>>,
}

impl BlockManager {
    // pub fn create<F: IoBackend + 'static>(backend: F, block_size: u64) -> IoResult<Self> {
    //     Ok(Self::new(BlockManagerInner::<Box<dyn IoBackend>>::create(Box::new(backend), block_size)?))
    // }
    // pub fn open<F: IoBackend + 'static>(backend: F) -> IoResult<Self> {
    //     Ok(Self::new(BlockManagerInner::<Box<dyn IoBackend>>::open(Box::new(backend))?))
    // }
    // fn new(inner: BlockManagerInner<Box<dyn IoBackend>>) -> Self {
    //     Self {
    //         inner: Rc::new(RefCell::new(inner)),
    //     }
    // }
    pub fn read_block(&self, index: BlockIndex) -> IoResult<Vec<u8>> {
        todo!();
    }
    pub fn write_block(&self, index: BlockIndex, buffer: &[u8]) -> IoResult<()> {
        todo!();
    }
    pub fn alloc(&self) -> IoResult<BlockIndex> {
        todo!();
    }
    pub fn free(&self, index: BlockIndex) -> IoResult<()> {
        todo!();
    }
    // accessors
    pub fn get_block_size(&self) -> u64 {
        todo!();
    }
}