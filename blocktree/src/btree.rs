use std::cell::Cell;
use std::io::Result as IoResult;
use std::marker::PhantomData;
use std::mem::size_of;
use std::rc::Rc;
use std::cmp::{Ord, Ordering};

use super::{BlockIndex, BlockManager, BlockBuffer, FromBytes, ToBytes};

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
    root_block: BlockBuffer,
    manager: BlockManager,
    height: u32,
    len: u64,
    // key_type: DataType,
    // value_type: DataType,
}

// encoding in block:
// (len: u64, height: u32, root_node: [..])
impl BTree {
    pub fn new(manager: BlockManager, root_index: BlockIndex) -> IoResult<Self> {
        let root_block = manager.read_block(root_index)?;
        let (len, height) = root_block.reader().at(0);
        Ok(Self {
            root_block,
            manager,
            height,
            len,
        })
    }

    fn root_node_offset(&self) -> usize {
        size_of::<(u64, u32)>()
    }

    pub fn get(&self, search_key: u64) -> IoResult<Option<u64>> {
        if self.height == 0 {
            let leaf = LeafNode::<u64, u64>::new(self.root_block.clone(), self.root_node_offset());
            // binary search: start
            let mut upper = leaf.len;
            let mut lower = 0;
            let mut index;
            let mut key;
            loop {
                index = lower + (upper - lower) / 2;
                key = leaf.get_key(index);
                if lower >= index {
                    break;
                }
                match search_key.cmp(&key) {
                    Ordering::Less => upper = index,
                    Ordering::Equal => break,
                    Ordering::Greater => lower = index,
                }
            }
            // binary search: end
            return Ok(if key != search_key {
                None
            } else {
                Some(leaf.get_value(index))
            })
        }
        let internal = InternalNode::<u64, u64>::new(self.root_block.clone(), self.root_node_offset());
        todo!();
    }

    pub fn insert(&mut self, key: u64, value: u64) -> IoResult<Option<u64>> {
        todo!();
    }

    pub fn remove(&mut self, key: u64) -> IoResult<Option<u64>> {
        todo!();
    }
}

struct InternalNode<K> {
    block: BlockBuffer,
    offset: usize,
    len: usize,
    _p: PhantomData<K>
}

// these fields are encoded in the block buffer:
// (len: u32, keys: [u64; len], edges: [BlockIndex; len])
impl<K: FromBytes + ToBytes> InternalNode<K> {
    fn new(block: BlockBuffer, offset: usize) -> Self {
        let len = block.reader().at::<u32>(offset) as usize;
        Self {
            block,
            offset,
            len,
            _p: PhantomData,
        }
    }

    fn key_offset(&self, index: usize) -> usize {
        self.offset + size_of::<u32>() + index * size_of::<K>()
    }

    fn edge_offset(&self, index: usize) -> usize {
        // NOTE: length == number of edges, and number of keys will be `length - 1`
        self.key_offset(self.len) + index * size_of::<BlockIndex>()
    }

    fn max_len(&self) -> usize {
        let available = self.block.buffer_len() - self.key_offset(0);
        // how many bytes B would L items take up? where size_of<K> = K, size_of<V> = V
        // B = (L - 1) * K + L * V
        // B =  L * K - K + L * V
        // B = (K + V) * L - K
        // B + K = (K + V) * L
        // (B + K) / (K + V) = L
        // so we use that formula to compute the maximum number of items that fit into B bytes
        // math is fun even tho I'm bad at it.
        (available + size_of::<K>()) / (size_of::<K>() + size_of::<V>())
    }

    fn get_key(&self, index: usize) -> K {
        self.block.reader().at(self.key_offset(index))
    }

    fn get_edge(&self, index: usize) -> BlockIndex {
        self.block.reader().at(self.edge_offset(index))
    }

    fn get_keys_edges(&self, keys: &mut Vec<K>, edges: &mut Vec<BlockIndex>) {
        let reader = self.block.reader();
        let mut key_offset = self.key_offset(0);
        let mut edge_offset = self.edge_offset(0);
        for _ in 0..self.len - 1 {
            keys.push(reader.at(key_offset));
            key_offset += size_of::<K>();
            edges.push(reader.at(edge_offset));
            edge_offset += size_of::<BlockIndex>();
        }
        edges.push(reader.at(edge_offset));
    }

    fn set_key(&self, index: usize, key: K) {
        self.block.writer().at(self.edge_offset(index), key);
    }

    // hmmm.. I don't think we actually need this method
    // fn set_edge(&self, index: usize, edge: BlockIndex) {
    //     self.block.writer().at(self.edge_offset(index), edge);
    // }

    fn set_keys_edges(&mut self, keys: &[K], edges: &[BlockIndex]) {
        assert!(keys.len() == edges.len() - 1);
        self.len = edges.len();
        self.block.writer().at(self.offset, (self.len as u32, keys, edges));
    }
}

struct LeafNode<K, V> {
    block: BlockBuffer,
    offset: usize,
    len: usize,
    _p: PhantomData<(K, V)>,
}

// encoding in block:
// (len: u32, keys: [K; len], values: [V; len])
impl<K: FromBytes + ToBytes, V: FromBytes + ToBytes> LeafNode<K, V> {
    fn new(block: BlockBuffer, offset: usize) -> Self {
        let len = block.reader().at::<u32>(offset) as usize;
        Self {
            block,
            offset,
            len,
            _p: PhantomData,
        }
    }

    fn key_offset(&self, index: usize) -> usize {
        self.offset + size_of::<u32>() + index * size_of::<K>()
    }

    fn value_offset(&self, index: usize) -> usize {
        self.key_offset(self.len + 1) + index * size_of::<V>()
    }

    fn get_key(&self, index: usize) -> K {
        self.block.reader().at(self.key_offset(index))
    }

    fn get_value(&self, index: usize) -> V {
        self.block.reader().at(self.value_offset(index))
    }

    fn get(&self, index: usize) -> (K, V) {
        let reader = self.block.reader();
        (
            reader.at(self.key_offset(index)),
            reader.at(self.value_offset(index))
        )
    }

    fn get_keys_values(&self, keys: &mut Vec<K>, values: &mut Vec<V>) {
        let reader = self.block.reader();
        let mut key_offset = self.key_offset(0);
        let mut value_offset = self.value_offset(0);
        for index in 0..self.len {
            keys.push(reader.at(key_offset));
            key_offset += size_of::<K>();
            values.push(reader.at(value_offset));
            value_offset += size_of::<V>();
        }
    }

    fn set_value(&self, index: usize, value: V) {
        self.block.writer().at(self.value_offset(index), value);
    }

    fn set_keys_values(&mut self, keys: &[K], values: &[V]) {
        assert!(keys.len() == values.len());
        self.len = values.len();
        self.block.writer().at(self.offset, (self.len as u32, keys, values));
    }
}
