use std::cell::RefCell;
use std::io::{Error as IoError, Result as IoResult, Read, Write, Seek, SeekFrom};
use std::fs::File;
use std::mem;
use std::rc::Rc;

pub type BlockIndex = u64;
pub const BLOCK_INDEX_SIZE: usize = mem::size_of::<BlockIndex>();

// fn read_block_index(buffer: &[u8], offset: usize) -> BlockIndex {

// }

// fn write_block_index()

pub trait FileBackend {
    fn len(&self) -> IoResult<u64>;
    // return std::io::ErrorKind::UnexpectedEof if buffer cannot be filled
    fn read(&mut self, offset: u64, buffer: &mut [u8]) -> IoResult<()>;
    fn write(&mut self, offset: u64, buffer: &[u8]) -> IoResult<()>;
    fn set_len(&mut self, len: u64) -> IoResult<()>;
    fn sync_data(&self) -> IoResult<()>;
}

impl FileBackend for Box<dyn FileBackend> {
    fn len(&self) -> IoResult<u64> {
        (**self).len()
    }
    fn read(&mut self, offset: u64, buffer: &mut [u8]) -> IoResult<()> {
        (**self).read(offset, buffer)
    }
    fn write(&mut self, offset: u64, buffer: &[u8]) -> IoResult<()> {
        (**self).write(offset, buffer)
    }
    fn set_len(&mut self, len: u64) -> IoResult<()> {
        (**self).set_len(len)
    }
    fn sync_data(&self) -> IoResult<()> {
        (**self).sync_data()
    }
}

impl FileBackend for File {
    fn len(&self) -> IoResult<u64> {
        println!("file.len()");
        Ok(self.metadata()?.len())
    }
    fn read(&mut self, offset: u64, buffer: &mut [u8]) -> IoResult<()> {
        // println!("file.read({}, {})", offset, buffer.len());
        self.seek(SeekFrom::Start(offset))?;
        self.read_exact(buffer)
    }
    fn write(&mut self, offset: u64, buffer: &[u8]) -> IoResult<()> {
        // println!("file.write({}, {})", offset, buffer.len());
        self.seek(SeekFrom::Start(offset))?;
        self.write_all(buffer)
    }
    fn set_len(&mut self, len: u64) -> IoResult<()> {
        println!("file.set_len({})", len);
        self.set_len(len)
    }
    fn sync_data(&self) -> IoResult<()> {
        println!("file.sync_data()");
        self.sync_data()
    }
}

impl FileBackend for Vec<u8> {
    fn len(&self) -> IoResult<u64> {
        println!("vec.len()");
        Ok(self.len() as u64)
    }
    fn read(&mut self, offset: u64, mut buffer: &mut [u8]) -> IoResult<()> {
        // println!("vec.read({}, {})", offset, buffer.len());
        let offset = offset as usize;
        let end = offset + buffer.len();
        buffer.write(&self[offset..end]);
        Ok(())
    }
    fn write(&mut self, offset: u64, buffer: &[u8]) -> IoResult<()> {
        // println!("vec.write({}, {})", offset, buffer.len());
        let offset = offset as usize;
        (&mut self[offset..offset + buffer.len()]).write(buffer);
        Ok(())
    }
    fn set_len(&mut self, len: u64) -> IoResult<()> {
        println!("vec.set_len({})", len);
        self.resize(len as usize, 0);
        Ok(())
    }
    fn sync_data(&self) -> IoResult<()> {
        println!("vec.sync_data()");
        // no op
        Ok(())
    }
}

struct RootHeader {
    block_size: u64,
    blocks_used: BlockIndex,
    blocks_total: BlockIndex,
    empty_list_index: BlockIndex,
}

impl RootHeader {
    const OFFSET_BLOCK_SIZE: usize = 0;
    // const OFFSET_BLOCK_COUNT: usize = RootHeader::OFFSET_BLOCK_SIZE + mem::size_of::<u64>();
    const OFFSET_BLOCKS_USED: usize = RootHeader::OFFSET_BLOCK_SIZE + mem::size_of::<u64>();
    const OFFSET_BLOCKS_TOTAL: usize = RootHeader::OFFSET_BLOCKS_USED + BLOCK_INDEX_SIZE;
    const OFFSET_EMPTY_LIST_INDEX: usize = RootHeader::OFFSET_BLOCKS_TOTAL + BLOCK_INDEX_SIZE;
    const OFFSET_END: usize = RootHeader::OFFSET_EMPTY_LIST_INDEX + BLOCK_INDEX_SIZE;

    fn create<F: FileBackend>(backend: &mut F, block_size: u64) -> IoResult<Self> {
        let mut root_header = RootHeader {
            block_size,
            blocks_used: 0,
            blocks_total: 0,
            empty_list_index: 0,
        };
        backend.set_len(block_size)?;
        root_header.set_blocks_used(backend, 1)?;
        root_header.set_blocks_total(backend, 1)?;
        root_header.set_empty_list_index(backend, 0)?;
        Ok(root_header)
    }
    fn open<F: FileBackend>(backend: &mut F) -> IoResult<Self> {
        let mut buffer = vec![]; // [0u8; 4096];
        // buffer.resize(4096, 0);
        buffer.resize(Self::OFFSET_END, 0);
        backend.read(0, &mut buffer)?;
        Ok(Self {
            block_size: u64::from_be_bytes(
                buffer[RootHeader::OFFSET_BLOCK_SIZE..RootHeader::OFFSET_BLOCKS_USED]
                    .try_into()
                    .unwrap(),
            ),
            blocks_used: BlockIndex::from_be_bytes(
                buffer[RootHeader::OFFSET_BLOCKS_USED..RootHeader::OFFSET_BLOCKS_TOTAL]
                    .try_into()
                    .unwrap(),
            ),
            blocks_total: BlockIndex::from_be_bytes(
                buffer[RootHeader::OFFSET_BLOCKS_TOTAL..RootHeader::OFFSET_EMPTY_LIST_INDEX]
                    .try_into()
                    .unwrap(),
            ),
            empty_list_index: BlockIndex::from_be_bytes(
                buffer[RootHeader::OFFSET_EMPTY_LIST_INDEX..RootHeader::OFFSET_END]
                    .try_into()
                    .unwrap(),
            ),
        })
    }

    fn block_size(&self) -> u64 {
        self.block_size
    }

    fn blocks_used(&self) -> BlockIndex {
        self.blocks_used
    }

    fn set_blocks_used<F: FileBackend>(
        &mut self,
        backend: &mut F,
        blocks_used: BlockIndex,
    ) -> IoResult<()> {
        println!("set_blocks_used({blocks_used})");
        backend.write(
            RootHeader::OFFSET_BLOCKS_USED as u64,
            &blocks_used.to_be_bytes(),
        )?;
        self.blocks_used = blocks_used;
        Ok(())
    }

    fn blocks_total(&self) -> BlockIndex {
        self.blocks_total
    }

    fn set_blocks_total<F: FileBackend>(
        &mut self,
        backend: &mut F,
        blocks_total: BlockIndex,
    ) -> IoResult<()> {
        println!("set_blocks_total({blocks_total})");
        backend.write(
            RootHeader::OFFSET_BLOCKS_TOTAL as u64,
            &blocks_total.to_be_bytes(),
        )?;
        self.blocks_total = blocks_total;
        Ok(())
    }

    fn empty_list_index(&self) -> BlockIndex {
        self.empty_list_index
    }

    fn set_empty_list_index<F: FileBackend>(
        &mut self,
        backend: &mut F,
        index: BlockIndex,
    ) -> IoResult<()> {
        println!("set_empty_list_index({index})");
        backend.write(
            RootHeader::OFFSET_EMPTY_LIST_INDEX as u64,
            &index.to_be_bytes(),
        )?;
        self.empty_list_index = index;
        Ok(())
    }
}

struct EmptyList {
    current_slot: usize,
    data: Vec<u8>,
}

/*
when we mark a block index as "free", we add it to the empty list.
when we allocate a block, we remove an index from the empty list.

the EmptyList stores the "next list" pointer in the last slot.
empty slots in the list start low and grow to a higher offset (towards the "next list" ptr)
*/

impl EmptyList {
    fn new(data: Vec<u8>) -> Self {
        let mut list = Self {
            current_slot: 0,
            data,
        };
        list.find_current_slot();
        list
    }
    fn new_from_root_header<F: FileBackend>(
        backend: &mut F,
        root_header: &RootHeader,
    ) -> IoResult<Self> {
        let index = root_header.empty_list_index();
        let mut data = vec![];
        if index != 0 {
            data.resize(root_header.block_size() as usize, 0);
            backend.read(index * root_header.block_size(), &mut data)?;
        }
        Ok(Self::new(data))
    }
    fn find_current_slot(&mut self) {
        if self.data.len() == 0 {
            return;
        }
        let mut lower = 0;
        let mut upper = self.data.len() / BLOCK_INDEX_SIZE - 1;
        while lower != upper {
            let mid = lower + (upper - lower) / 2;
            if self.get(mid) == 0 {
                upper = mid;
            } else {
                lower = mid + 1;
            }
        }
        self.current_slot = lower;
    }
    fn get(&self, slot: usize) -> BlockIndex {
        let offset = slot * BLOCK_INDEX_SIZE;
        BlockIndex::from_be_bytes(
            self.data[offset..offset + BLOCK_INDEX_SIZE]
                .try_into()
                .unwrap(),
        )
    }
    fn set<F: FileBackend>(&mut self, backend: &mut F, slot: usize, index: BlockIndex) -> IoResult<()> {
        println!("EmptyList::set(slot: {slot}, index: {index})");
        let offset = slot as u64 * BLOCK_INDEX_SIZE as u64;
        backend.write(offset, &index.to_be_bytes())?;
        // let offset = offset as usize;
        let offset = slot * BLOCK_INDEX_SIZE;
        (&mut self.data[offset..offset + BLOCK_INDEX_SIZE])
            .write(&index.to_be_bytes())
            .unwrap();
        Ok(())
    }
    fn alloc<F: FileBackend>(
        &mut self,
        backend: &mut F,
        root_header: &mut RootHeader,
    ) -> IoResult<BlockIndex> {
        println!("... EmptyList::alloc(); current_slot: {}, data.len(): {}", self.current_slot, self.data.len());
        if self.data.len() == 0 {
            // there is no empty list, so we must extend the file backend
            debug_assert!(root_header.blocks_used() == root_header.blocks_total());
            // TODO should we support batching; attempting to allocate N blocks, so we don't have to do these
            // updates for each intermediate block?
            backend.set_len(backend.len()? + root_header.block_size())?;
            let new_block = root_header.blocks_used() + 1;
            root_header.set_blocks_used(backend, new_block)?;
            root_header.set_blocks_total(backend, new_block)?;
            // root_header.set_blocks_total(backend, root_header.blocks_total() + 1)?;
            return Ok(new_block);
        } else if self.current_slot == 0 {
            // current empty list has no slots to remove from (so we use current empty list as new block)
            // blocks_used does not change, since we're "recycling" this empty list block
            let new_block = root_header.empty_list_index;
            let next_empty_list_index = self.get(self.data.len() / BLOCK_INDEX_SIZE - 1);
            // let org_len = self.data.len();
            // self.data.truncate(0);
            // self.data.resize(org_len, 0);
            root_header.set_empty_list_index(backend, next_empty_list_index)?;
            if next_empty_list_index == 0 {
                self.data.truncate(0);
            } else {
                backend.read(
                    next_empty_list_index * root_header.block_size(),
                    &mut self.data,
                )?;
                self.find_current_slot();
                // new_block should be zeroed, except the last slot (the next_empty_list_index)
                backend.write(
                    (new_block + 1) * root_header.block_size() - BLOCK_INDEX_SIZE as u64,
                    &(0 as BlockIndex).to_be_bytes(),
                )?;
            }
            return Ok(new_block);
        } else {
            // remove a slot from the list
            let index = self.get(self.current_slot);
            self.set(backend, self.current_slot, 0)?;
            self.current_slot -= 1;
            root_header.set_blocks_used(backend, root_header.blocks_used + 1)?;
            return Ok(index);
        }
    }
    fn free<F: FileBackend>(
        &mut self,
        backend: &mut F,
        root_header: &mut RootHeader,
        index: BlockIndex,
    ) -> IoResult<()> {
        println!("... EmptyList::free({index}); current_slot: {}, data.len(): {}", self.current_slot, self.data.len());
        if self.data.len() == 0 {
            // there is no current empty list; this block becomes a new empty list
            // blocks_used does not change, since we're "recycling" this block as the new empty list
            debug_assert!(root_header.blocks_used() == root_header.blocks_total());
            self.data.resize(root_header.block_size() as usize, 0);
            backend.write(index * root_header.block_size(), &self.data)?;
            // self.current_slot is already 0, which is correct in this new empty list block
            root_header.set_empty_list_index(backend, index)?;
            return Ok(());
        } else if self.current_slot == self.data.len() / BLOCK_INDEX_SIZE {
            // current empty list block is full; this block becomes a new empty list
            // blocks_used does not change, since we're "recycling" this block as the new empty list
            self.data.truncate(0);
            self.data.resize(root_header.block_size() as usize, 0);
            // update next_empty_list_index slot
            self.set(backend, self.current_slot, root_header.empty_list_index())?;
            self.current_slot = 0;
            root_header.set_empty_list_index(backend, index)?;
            backend.write(index * root_header.block_size(), &self.data)?;
            return Ok(());
        } else {
            // add a slot to the list
            self.current_slot += 1;
            self.set(backend, self.current_slot, index)?;
            root_header.set_blocks_used(backend, root_header.blocks_used - 1)?;
            Ok(())
        }
    }
}

pub enum Error {
    Io(IoError),
    BlockSizeMismatch,
}

impl From<IoError> for Error {
    fn from(ioerr: IoError) -> Self {
        Self::Io(ioerr)
    }
}

/*

I'm not sure how to organize this; or I'm over complicating things.
accessing the "empty_list" is complicated; we need to parse slots using BlockIndex::from_be_bytes()
(like how RootHeader does), so I want to isolate it to it's own type... but it also needs to be
able to update the root header... idk, maybe it should just be part of the BlockManager type?


*/

struct BlockManagerInner<F> {
    backend: F,
    root_header: RootHeader,
    empty_list: EmptyList,
}

impl<F: FileBackend> BlockManagerInner<F> {
    fn create(mut backend: F, block_size: u64) -> IoResult<Self> {
        let root_header = RootHeader::create(&mut backend, block_size)?;
        Self::from_root_header(backend, root_header)
    }
    fn open(mut backend: F) -> IoResult<Self> {
        let root_header = RootHeader::open(&mut backend)?;
        Self::from_root_header(backend, root_header)
    }
    fn from_root_header(mut backend: F, root_header: RootHeader) -> IoResult<Self> {
        let empty_list = EmptyList::new_from_root_header(&mut backend, &root_header)?;
        Ok(Self {
            backend,
            root_header,
            empty_list,
        })
    }
    fn index_to_offset(&self, index: BlockIndex) -> u64 {
        self.root_header.block_size as u64 * index as u64
    }
    fn get(&mut self, index: BlockIndex) -> IoResult<Vec<u8>> {
        let offset = self.index_to_offset(index);
        let mut buffer = vec![];
        buffer.resize(self.root_header.block_size as usize, 0);
        self.backend.read(offset, &mut buffer)?;
        Ok(buffer)
    }
    fn set(&mut self, index: BlockIndex, buffer: &[u8]) -> Result<(), Error> {
        if buffer.len() as u64 != self.root_header.block_size as u64 {
            return Err(Error::BlockSizeMismatch);
        }
        let offset = self.index_to_offset(index);
        self.backend.write(offset, buffer)?;
        Ok(())
    }
    fn alloc(&mut self) -> IoResult<BlockIndex> {
        self.empty_list.alloc(&mut self.backend, &mut self.root_header)
    }
    fn free(&mut self, index: BlockIndex) -> IoResult<()> {
        self.empty_list
            .free(&mut self.backend, &mut self.root_header, index)
    }
}

// external API
pub struct BlockManager {
    inner: Rc<RefCell<BlockManagerInner<Box<dyn FileBackend>>>>,
}

impl BlockManager {
    pub fn create<F: FileBackend + 'static>(backend: F, block_size: u64) -> IoResult<Self> {
        Ok(Self::new(BlockManagerInner::<Box<dyn FileBackend>>::create(Box::new(backend), block_size)?))
    }
    pub fn open<F: FileBackend + 'static>(backend: F) -> IoResult<Self> {
        Ok(Self::new(BlockManagerInner::<Box<dyn FileBackend>>::open(Box::new(backend))?))
    }
    fn new(inner: BlockManagerInner<Box<dyn FileBackend>>) -> Self {
        Self {
            inner: Rc::new(RefCell::new(inner)),
        }
    }
    pub fn get(&self, index: BlockIndex) -> IoResult<Vec<u8>> {
        self.inner.borrow_mut().get(index)
    }
    pub fn set(&self, index: BlockIndex, buffer: &[u8]) -> Result<(), Error> {
        self.inner.borrow_mut().set(index, buffer)
    }
    pub fn alloc(&self) -> IoResult<BlockIndex> {
        self.inner.borrow_mut().alloc()
    }
    pub fn free(&self, index: BlockIndex) -> IoResult<()> {
        self.inner.borrow_mut().free(index)
    }
}



pub struct BlockTree {
    manager: BlockManager,
    index: BlockIndex,
    len: u64,
    
}

impl BlockTree {
    create_empty
}

pub struct RootNode {
    tree_height: u8,
    // packed into 2 bits

}

/*
for DataType::Inline(n), n is the fixed size of the inline data in bytes, restricted to being a
power of two. it is represented within the file as an exponent `x` encoded in 3 bits, so that
`n = 2^(x + 3)`; with 3 bits, the value x can range from 0 to 7; the minimum size of inline data
is 1 byte, so we add 3 to x, meaning the value n can range from 8 to 1024.

3 bits...
    000 - x=0, n=   8,
    001 - x=1, n=  16,
    010 - x=2, n=  32,
    011 - x=3, n=  64,
    100 - x=4, n= 128,
    101 - x=5, n= 256,
    110 - x=6, n= 512,
    111 - x=7, n=1024,

oh wait, I need to redo this table; `n is the fixed size of the inline data in bytes`

---------------

`n = 2^x`; with 3 bits, the value x can range from 0 to 7; so the value n can range from 1 to 128.

3 bits...
    000 - x=0, n=  1,
    001 - x=1, n=  2,
    010 - x=2, n=  4,
    011 - x=3, n=  8,
    100 - x=4, n= 16,
    101 - x=5, n= 32,
    110 - x=6, n= 64,
    111 - x=7, n=128,

hmm, what if we made the minimum be 8 bytes...

no, enough of this. we shouldn't care about maximally packing this right now. we should make it
be as flexible as possible. here's the new format, which is subject to change later:

---------------

the DataType for keys and values follows this format:
the size of inline data is: `n = 2^x`; x ranges from 0 to 255 (the u8 in DataType::Inline),
but in practice, n is restricted to at most 1024 bytes (or so?), so x > 10 is invalid.

another byte is used as a tag for the enum; 0: Inline, 1: Pointer, 2: TaggedPointer
actually, we could make x == 0xFF mean "TaggedPointer"; a pointer is 8 bytes (u64), with the
most sig bit specifying (0) if the pointer is an offset into the tree's managed address space, or (1) a
BlockIndex of a header block to resolve dyn-sized data spanning multiple blocks.


---------------

so, "fixed-width data type stored inline with the node" are a simple case; we just store the width
in the root node's metadata.

now, how do we want to support "fixed-width data type stored externally, accessed via a pointer"?

method 1) the pointer is an offset into an address space managed by the BlockTree; the address
space would span multiple blocks, and these BlockIndex's would be stored in another BlockTree,
which maps address offset to BlockIndex. to track free slots, we would use an EmptyList like data
structure, which would have an entry for each free slot within the address space (this EmptyList
is a simple linked list).

method 2) ...

*/
enum DataType {
    Inline(u8),
    Pointer,
}

