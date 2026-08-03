use super::manager::{BlockBuffer, BlockManager};
use super::{BlockIndex, BLOCK_INDEX_SIZE, BLOCK_SIZE};

// pub enum DataType {
//     Inline(u32),
//     Pointer(u32),
//     DynSize,
// }

/*
for now, BTree will only support u64 keys and values, just like SampleTree.

the structs for Nodes will store a pointer to the buffer, and will have the logic for
retrieving keys, edges, and values from the buffer.

with BlockBuffer, we can modify it, and BlockManager will automatically be aware that it's
been modified. I also have an interface to easily read/write integers. I think this API
will be good enough for implementing BTree, which serves as the core data structure within
this file format.
*/

pub struct BTree {
    root_block: BlockIndex,
    manager: BlockManager,
    height: u32,
    len: u64,
    // key_type: DataType,
    // value_type: DataType,
}

impl BTree {}

struct InternalNode {
    block: BlockIndex,
    buffer: BlockBuffer,
    offset: usize,
    len: usize,
}

// these fields are encoded in the block buffer:
// (len: u32, keys: [u64; len], edges: [BlockIndex; len])
impl InternalNode {
    fn get_key(&self, index: usize) -> u64 {
        let offset = 4 + index * 8;
        self.buffer.reader().read(offset)
    }

    fn get_edge(&self, index: usize) -> BlockIndex {
        let offset = 4 + self.len * 8 + index * BLOCK_INDEX_SIZE;
        self.buffer.reader().read(offset)
    }
}

struct LeafNode {
    inblockdex: BlockIndex,
    buffer: BlockBuffer,
    offset: usize,
    len: usize,
}

impl LeafNode {
    // get (key, value) at index
    fn get(&self, index: usize) -> (u64, u64) {
        todo!();
    }
}
