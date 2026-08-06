use std::collections::btree_map::{BTreeMap, Entry};
use std::cell::RefCell;
use std::io::Result as IoResult;
use std::rc::{Rc, Weak};
use std::mem::size_of;
use std::ops::Deref;

use super::backend::IoBackend;
use super::block::{BlockBuffer};
use super::{BlockIndex, BLOCK_SIZE};

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
                let buffer = BlockBuffer::new(buffer.into_boxed_slice(), index, self.downgrade());
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
            buffer.set_modified(false);
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

pub trait FromBytes {
    fn from_bytes(buffer: &[u8]) -> Self;
}

pub trait ToBytes {
    fn to_bytes(&self, buffer: &mut [u8]);
}

impl<T: ToBytes> ToBytes for &T {
    fn to_bytes(&self, buffer: &mut [u8]) {
        self.to_bytes(buffer);
    }
}

// this is just for convenience; this doesn't writ out the count or length, it must be
// done explicitly else where.
impl<T: ToBytes> ToBytes for &[T] {
    fn to_bytes(&self, buffer: &mut [u8]) {
        let mut offset = 0;
        for t in *self {
            t.to_bytes(&mut buffer[offset..]);
            offset += size_of::<T>();
        }
    }
}

macro_rules! impl_numeric_ftb {
    ($($n:ident),+) => {
        $(
            // little-endian makes the most sense here, to be honest.
            impl FromBytes for $n {
                fn from_bytes(buffer: &[u8]) -> Self {
                    Self::from_le_bytes(buffer[0..size_of::<Self>()].try_into().unwrap())
                }
            }
            impl ToBytes for $n {
                fn to_bytes(&self, buffer: &mut [u8]) {
                    buffer[0..size_of::<Self>()].copy_from_slice(self.to_le_bytes().as_slice());
                }
            }
        )+
    };
}

// don't impl for non-portable usize or isize

impl_numeric_ftb!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128);

macro_rules! impl_tuple_ftb {
    ($start:ident, $($rem:ident),+) => {
        impl_tuple_ftb!(do_impl $start, $($rem),+);
        impl_tuple_ftb!($($rem),+);
    };
    (do_impl $($n:ident),+) => {
        #[allow(non_snake_case, unused_assignments)]
        impl<$($n: FromBytes),+> FromBytes for ($($n),+ ,) {
            fn from_bytes(buffer: &[u8]) -> Self {
                let mut offset = 0;
                $(
                    let $n = $n::from_bytes(&buffer[offset..]);
                    offset += size_of::<$n>();
                )+
                ($($n),+ ,)
            }
        }

        #[allow(non_snake_case, unused_assignments)]
        impl<$($n: ToBytes),+> ToBytes for ($($n),+ ,) {
            fn to_bytes(&self, buffer: &mut [u8]) {
                let ($($n),+ ,) = self;
                let mut offset = 0;
                $(
                    $n.to_bytes(&mut buffer[offset..]);
                    offset += size_of::<$n>();
                )+
            }
        }

    };
    ($start:ident) => {
        impl_tuple_ftb!(do_impl $start);
    };
}

impl_tuple_ftb!(T7, T6, T5, T4, T3, T2, T1, T0);
