use std::mem::size_of;

use super::{BlockIndex, BLOCK_INDEX_SIZE, BLOCK_SIZE, BlockManager, BlockBuffer};

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
    fn keys_start(&self) -> usize {
        size_of::<u32>()
    }

    fn edges_start(&self) -> usize {
        self.keys_start() + self.len * size_of::<u64>()
    }

    fn get_key(&self, index: usize) -> u64 {
        let offset = self.keys_start();
        self.buffer.reader().at(offset)
    }

    fn get_edge(&self, index: usize) -> BlockIndex {
        let offset = self.edges_start() + index * BLOCK_INDEX_SIZE;
        self.buffer.reader().at(offset)
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
