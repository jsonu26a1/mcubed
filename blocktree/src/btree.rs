use std::mem::size_of;
use std::rc::Rc;

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

/*
hmmm, I wonder how we should handle the Internal/LeafNode APIs; for fetching a key from the BTree,
we mostly just need InternalNode's get_key(), get_edge(), and LeafNode's get(); but with insert()
and remove(), we'll want to manipulate entire lists (keys, edges, values). we could have an
methods on the Nodes that fill a `&mut Vec`, allowing BTree to manipulate, then another method to
write the new lists back. although, we will also want to be changing the length
*/

enum EitherNode {
    Internal(Rc<InternalNode>),
    Leaf(Rc<LeafNode>),
}

impl EitherNode {
    fn internal(self) -> Rc<InternalNode> {
        match self {
            Self::Internal(n) => n,
            Self::Leaf(_) => {
                panic!("EitherNode access failed: expected InternalNode, found LeafNode")
            }
        }
    }

    fn leaf(self) -> Rc<LeafNode> {
        match self {
            Self::Internal(_) => {
                panic!("EitherNode access failed: expected LeafNode, found InternalNode")
            }
            Self::Leaf(n) => n,
        }
    }

    fn block_index(&self) -> BlockIndex {
        match self {
            Self::Internal(n) => n.block.index(),
            Self::Leaf(n) => n.block.index(),
        }
    }
}

// TODO: need From impls for EitherNode

struct InternalNode {
    block: BlockBuffer,
    len: usize,
}

// these fields are encoded in the block buffer:
// (len: u32, keys: [u64; len], edges: [BlockIndex; len])
impl InternalNode {
    fn key_offset(&self, index: usize) -> usize {
        size_of::<u32>() + index * size_of::<u64>()
    }

    fn edge_offset(&self, index: usize) -> usize {
        // NOTE: length == number of edges, and number of keys will be `length - 1`
        self.key_offset(self.len) + index * BLOCK_INDEX_SIZE
    }

    fn get_key(&self, index: usize) -> u64 {
        self.block.reader().at(self.key_offset(index))
    }

    fn get_edge(&self, index: usize) -> BlockIndex {
        self.block.reader().at(self.edge_offset(index))
    }

    fn set_key(&self, index: usize, key: u64) {
        self.block.writer().at(self.edge_offset(index), key);
    }

    // hmmm.. I don't think we actually need this method
    fn set_edge(&self, index: usize, edge: BlockIndex) {
        self.block.writer().at(self.edge_offset(index), edge);
    }

    fn set_keys_edges(&self, keys: &[u64], edges: &[BlockIndex]) {
        assert!(keys.len() == edges.len() - 1);
        let mut offset = 0;
        // TODO we need a Cell<usize> for len
        // self.len = edges.len();
        let len = self.len as u32;
        let mut writer = self.block.writer();
        writer.at(0, (len, keys, edges));
    }
}

struct LeafNode {
    block: BlockBuffer,
    len: usize,
}

// encoding in block:
// (len: u32, keys: [u64; len], values: [u64; len])
impl LeafNode {
    fn key_offset(&self, index: usize) -> usize {
        size_of::<u32>() + index * size_of::<u64>()
    }

    fn value_offset(&self, index: usize) -> usize {
        self.key_offset(self.len + 1) + index * size_of::<u64>()
    }

    // get (key, value) at index
    fn get(&self, index: usize) -> (u64, u64) {
        let reader = self.block.reader();
        (
            reader.at(self.key_offset(index)),
            reader.at(self.value_offset(index))
        )
    }
}
