use super::{ BlockIndex, BLOCK_SIZE };
use super::manager::BlockManager;

// pub enum DataType {
//     Inline(u32),
//     Pointer(u32),
//     DynSize,
// }

/*
for now, BTree will only support u64 keys and values, just like SampleTree.

the structs for Nodes will store a pointer to the buffer, and will have the logic for
retrieving keys, edges, and values from the buffer.

I'm not quite sure yet how to handle modifications. the BTree will need to track which
nodes/blocks have been modified, and call BlockManager::write_block() after an operation is
completed. although, this would mean that calling "insert()" on the btree several times,
would require cloning the buffers of blocks for each operation (since they become shared
Rc<[u8]> again after write_block(), and cannot be modified in place while in the cache...

hmm, we may need to rethink the design of BlockManager's cache; what about something like
Rc<Cell<[u8]>>? nope, cannot use Cell. I think I have another idea tho...
*/

pub struct BTree {
    root_block: BlockIndex,
    manager: BlockManager,
    height: u32,
    len: u64,
    // key_type: DataType,
    // value_type: DataType,
}

impl BTree {

}

struct InternalNode {
    block: BlockIndex,
    buffer: Rc<[u8]>,
    offset: usize,
    len: u64,
}

impl InternalNode {
    fn get_key(&self, index: usize) -> u64 {
        todo!();
    }

    fn get_edge(&self, index: usize) -> BlockIndex {
        todo!();
    }
}

struct LeafNode {
    inblockdex: BlockIndex,
    buffer: Rc<[u8]>,
    offset: usize,
    len: u64,
}

impl LeafNode {
    // get (key, value) at index
    fn get(&self, index: usize) -> (u64, u64) {
        todo!();
    }
}
