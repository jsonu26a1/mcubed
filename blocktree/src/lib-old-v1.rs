use std::mem;
use std::ops::{ Deref, DerefMut };
use std::collections::BTreeMap;
use std::rc::Rc;

pub type BlockIndex = u64;
pub const BlockIndexSize: usize = mem::size_of::<BlockIndex>();


type Result<T> = std::result::Result<T, String>;


/*
I'm not sure how to decide which blocks to free up from the cache first. I don't believe we know
when a particular block was last accessed, so we can't remove the least recently used block for
example... hmmm.

do we really need a "cache"? we could just attempt to read whenever we need the data.

alternatively, we could create a CachedBlock object, which contains a Weak<[u8]>; when the data
is accessed, it would update the "last used time" in the BlockCache. if the data was dropped, then
then it would automatically re-fetch the data by calling BlockCache::get() again.

might as well just build my CachedBlock idea, we can always ditch it later.
*/

/*
pub struct BlockCache<F> {
    backend: F,
    block_size: u32,
    limit: usize,
    cache: BTreeMap<BlockIndex, Rc<[u8]>>,
}

impl<F: FileBackend> BlockCache<F> {
    pub fn new(backend: F, block_size: u32, limit: usize) -> Self {
        assert!(block_size.is_power_of_two());
        Self {
            backend,
            block_size,
            limit,
            cache: Default::default(),
        }
    }

    fn get_from_backend(&mut self, index: BlockIndex) -> Result<Rc<[u8]>> {
        if (cache.len() + 1) * self.block_size as usize > self.limit {
            // we need to free up some cached blocks here
        }
        // TODO we could use MaybeUninit here instead
        let mut v = vec![];
        v.resize(self.block_size as usize, 0);
        self.backend.read(index * self.block_size as BlockIndex, &mut v)?;
        Ok(v.into())
    }

    pub fn get(&mut self, index: BlockIndex) -> Result<Rc<[u8]>> {
        if let Some(block) = self.cache.get(&index) {
            return Ok(block.clone())
        }
        panic!();
    }

    // pub fn update(&mut self, index: BlockIndex, buffer: &[u8]) -> Result<Rc<[u8]>> {
    //     let block = match self.cache.get(&index) {
    //         Some(block) => block,
    //         None => {
    //             backend.read()
    //         }
    //     }
    //     panic!()
    // }
}
*/



/*
hmmm, I don't know about this. the idea was when the data in CachedBlock is accessed, it updates
the InnerBlockCache::last_used field; but that means we're doing a lot of updates to that BTreeMap
to this every single time we read from a CachedBlock... well, we could only update it if a certain
amount of time has elapsed;

I think we shouldn't worry about making a cache right now; at this point, we don't know how we'll
be using the block data we fetch from the file, so I think it makes sense to just have an API that
fetches and updates blocks, and has the consumer manage memory.
*/
pub struct BlockCache<F> {
    inner: Rc<InnerBlockCache<F>>
}

struct InnerBlockCache<F> {
    backend: F,
    block_size: u32,
    limit: usize,
    cache: BTreeMap<BlockIndex, Rc<[u8]>>,
    last_used: BTreeMap<u64, ()>,
}



pub trait FileBackend {
    fn len(&self) -> Result<u64>;
    fn read(&self, offset: u64, buffer: &mut [u8]) -> Result<()>;
    fn write(&self, offset: u64, buffer: &[u8]) -> Result<()>;
}

// contains metadata about the entire file
pub struct RootHeader {
    block_size: u32,
    block_count: BlockIndex,
    empty_blocks: BlockIndex,
}

/*

empty_blocks
============
    points to a block containing an array of block indexes that are empty. index 0 is the block of the
    root header, which will never be empty, so 0 is used to designate an empty slot in the array. the
    last slot in the array points to another block with more array slots, forming a linked list.

    when a block is freed, the first list is searched for an empty slot to record the block index into.
    if the list is full, the block to be freed becomes a new empty list; it is zeroed, and the last
    slot points to the previous list that was found to be full.

    when a block is being allocated, the first list is searched for a non-empty slot. if there are
    none, the block containing the list is used, and the next linked list (block index in last slot)
    is shifted into the root header, if there is one.


do we want to make block_size be fixed at 4 KiB (4096)? making it fixed would simplify
things (EmptyList would just use a const), but maybe it would be desirable to configure the
block_size? if it was configurable, we can't make it const; it would need to be a field tied to
the RootHeader, and the value would need to be passed around to where it's needed. while type
parameters could have consts associated with them, we wouldn't want to use that, because we won't
know what block_sizes we need until we open a blocktree file at runtime.

actually, with EmptyList, we can infer block_size from T; `T.len() / BlockIndexSize`.



I think I need to build the "block access backend" before I continue; a way to fetch blocks by
their index, allocate new blocks, and truncate the file (to "free" empty blocks at the end of the
file). I could make this generic, a trait object, or hard code it as one implementation. I'm
tempted to just hard code it with mmap; however, this would complicate supporting encryption;
I don't think we really need trait objects, so I guess I'll do generics.

hmm, the appeal of mmap is we can write "directly" to the file; if we want to support
encryption, the API for that would require managing the memory somewhat differently; we can't just
use `&mut [u8]`, but instead call something like edit() to get `&mut [u8]`, then call commit() to
write the changes out; with the direct mmap backend, commit wouldn't do anything, since mutating
the buffer should update the file. so it would be misleading where not calling commit() might
cause a block to remain unchanged in the backend wasn't mmap, but for changes to be written to
disk if it commit() was called... although we could use some other way of firing that "commit()"
event to the backend, like once the buffer returned from edit() is dropped, for example.

another issue is with mmap; I was wondering how writes actually occur, what about errors, etc, and
on wikipedia found this:
    I/O errors on the underlying file (e.g. its removable drive is unplugged or optical media is
    ejected, disk full when writing, etc.) while accessing its mapped memory are reported to the
    application as the SIGSEGV/SIGBUS signals on POSIX, and the EXECUTE_IN_PAGE_ERROR structured
    exception on Windows. All code accessing mapped memory must be prepared to handle these errors,
    which don't normally occur when accessing memory.

so using mmap might be too complicated in practice... some reading:
- https://github.com/RazrFalcon/memmap2-rs/issues/13
- https://users.rust-lang.org/t/how-unsafe-is-mmap/19635/28

redb used to have mmap, but dropped support because of unsafety and maintaining it alongside regular
file IO wasn't worth the performance benefits it gave.

personally, I wouldn't mind using mmap, but I don't like the idea of trying to handle IO errors (say
the storage device is unplugged while our app is trying to read/write), I would rather just use the
standard file IO with it's mature error handling in the std lib. the discussion on users.rust-lang
was interesting, with the talk of undefined behavior, but I personally am fine with assuming that
the file I map into memory is "locked", and if another process decides to mess with it, they're to
blame.






*/

/*
I don't like this API how it is; the EmptyList(T) type is supposed to point to a single
"EmptyList block", but I'm writing the methods for this type as though they will be used at a high
level; ideally, we want a type that can encapsulate the operations related to the "empty list"
concept as a whole, where it implemented the algorithm described above for "empty_blocks". but to
do that, we would need an API that allows us to look up and fetch blocks by index, and also alloc
new blocks.

struct EmptyList<T>(T);

impl<T: Deref<Target=[u8]>> EmptyList<T> {
    pub fn len(&self) -> usize {
        self.0.len() / BlockIndexSize - 1
    }

    fn get_raw_index(&self, offset: usize) -> Option<BlockIndex> {
        let b_offset = offset * BlockIndexSize;
        let index = BlockIndex::from_be_bytes(self.0[b_offset..b_offset + BlockIndexSize].try_into().unwrap());
        if index == 0 {
            None
        } else {
            Some(index)
        }
    }

    pub fn get_slot(&self, offset: usize) -> Option<BlockIndex> {
        debug_assert!(offset < self.len());
        self.get_raw_index(offset)
    }

    pub fn get_next_list(&self) -> Option<BlockIndex> {
        self.get_raw_index(self.len())
    }

    pub fn iter_slots(&self) -> impl Iterator<Item=(usize, Option<BlockIndex>)> {
        (0..self.len()).map(|n| (n, self.get_raw_index(n)))
    }
}


impl<T: DerefMut<Target=[u8]>> EmptyList<T> {
    fn set_raw_index(&self, offset: usize, index: BlockIndex) -> BlockIndex {

    }

    pub fn set_slot(&self, offset: usize, slot: Option<BlockIndex>) {

    }
}
*/

// struct Block {
//     data: BlackData,
// }

// enum BlockData {
//     Box(Box<)
//     Mmap
// }