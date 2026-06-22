pub enum DataType {
    Inline(u32),
    Pointer(u32),
    DynSize,
}

pub struct BlockTree {
    manager: BlockManager,
    index: BlockIndex,
    // 
    height: u32,
    len: u64,
    key_type: DataType,
    value_type: DataType,
}

impl BlockTree {

}

struct InternalNode {
    index: BlockIndex,
    keys: Vec<()>,
    edges: Vec<BlockIndex>,
}

struct LeafNode {
    index: BlockIndex,
    keys: Vec<
}