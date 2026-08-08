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
    modified: bool,
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
            modified: false,
            height,
            len,
        })
    }

    fn root_node_offset(&self) -> usize {
        size_of::<(u64, u32)>()
    }

    fn find_leaf(&self, key: u64) -> IoResult<(LeafNode<u64, u64>, Result<usize, usize>)> {
        let mut height = 0;
        let mut cursor = self.root_block.clone();
        let mut offset = self.root_node_offset();
        while height < self.height {
            let internal = InternalNode::<u64>::new(cursor, offset);
            if height == 0 {
                offset = 0;
            }
            height += 1;
            let i = binary_search(key, internal.len, |i| internal.get_key(i)).map_or_else(|i| i, |i| i);
            cursor = self.manager.read_block(internal.get_edge(i))?;
        }
        let leaf = LeafNode::<u64, u64>::new(cursor, offset);
        let index = binary_search(key, leaf.len, |i| leaf.get_key(i));
        Ok((leaf, index))
    }

    pub fn get(&self, key: u64) -> IoResult<Option<u64>> {
        let mut height = 0;
        let mut cursor = self.root_block.clone();
        let mut offset = self.root_node_offset();
        while height < self.height {
            let internal = InternalNode::<u64>::new(cursor, offset);
            if height == 0 {
                offset = 0;
            }
            height += 1;
            let i = binary_search(key, internal.len, |i| internal.get_key(i)).map_or_else(|i| i, |i| i);
            cursor = self.manager.read_block(internal.get_edge(i))?;
        }
        let leaf = LeafNode::<u64, u64>::new(cursor, offset);
        Ok(binary_search(key, leaf.len, |i| leaf.get_key(i)).ok().map(|i| leaf.get_value(i)))
    }

    pub fn insert(&mut self, key: u64, value: u64) -> IoResult<Option<u64>> {
        let mut height = 0;
        let mut cursor = self.root_block.clone();
        let mut parents = vec![];
        let mut offset = self.root_node_offset();
        while height < self.height {
            let internal = InternalNode::<u64>::new(cursor, offset);
            if height == 0 {
                offset = 0;
            }
            height += 1;
            let i = binary_search(key, internal.len, |i| internal.get_key(i)).map_or_else(|i| i, |i| i);
            cursor = self.manager.read_block(internal.get_edge(i))?;
            parents.push((internal, i));
        }
        self.modified = true;
        let mut leaf = LeafNode::<u64, u64>::new(cursor, offset);
        let i = match binary_search(key, leaf.len, |i| leaf.get_key(i)) {
            Ok(i) => {
                let old_value = leaf.get_value(i);
                leaf.set_value(i, value);
                return Ok(Some(old_value));
            },
            Err(i) => i,
        };
        self.len += 1;
        let mut parents = parents.into_iter().rev();

        let inserted_at_child_end = i == leaf.len;
        if leaf.len < leaf.max_len() {
            // leaf doesn't need to be split
            // since keys precedes values, we need to shift all values to make room
            // we also want to avoid wastefully double-copying when possible
            let mut writer = leaf.block.writer();
            // i is in the index at which we're inserting a new key/value
            // shift all of value's elements after `i` over by 2 slots; 1 for new value + 1 for new key
            writer.copy_within(leaf.value_offset(i)..leaf.value_offset(leaf.len), leaf.value_offset(i + 2));
            if i > 0 {
                // we can shift over keys and values with the same copy_within call
                writer.copy_within(leaf.key_offset(i)..leaf.value_offset(i - 1), leaf.key_offset(i + 1));
            } else {
                // i == 0, no values to shift, but we still need to shift over keys
                writer.copy_within(leaf.key_offset(i)..leaf.key_offset(leaf.len), leaf.key_offset(i + 1));
            }
            // update len, so leaf.value_offset() is correct.
            leaf.len += 1;
            writer.at(leaf.len_offset(), leaf.len as u32);
            // insert new key/value
            writer.at(leaf.key_offset(i), key);
            writer.at(leaf.value_offset(i), value);
            return Ok(None);
        }


        // NOTE: we're at max_len, there's no room to insert-then-split, we must split first
        let mut left = leaf;
        let mut l_writer = left.block.writer();
        let mut right = LeafNode::<u64, u64>::new(self.manager.alloc_block()?, 0);
        let mut r_writer = right.block.writer();

        let mid = (left.len + 1) / 2;
        right.len = left.len - mid;
        let left_new_len = mid;
        r_writer.at(right.len_offset(), right.len as u32);

        // we don't need unsafe block.as_slice() now that we have writer.copy_from
        /*
        if i < mid {
            // insert i in left
            {
                // safety: we don't access left until after the slice is dropped
                let l_slice = unsafe { left.block.as_slice() };
                r_writer.slice(
                    right.key_offset(0)..right.key_offset(right.len),
                    &l_slice[left.key_offset(mid)..left.key_offset(left.len)]
                );
                // make room in left to insert key/value
                // since we already have a direct slice, just use that for copy_within()
                l_slice.copy_within(left.key_offset(i)..left.key_offset(mid), left.key_offset(i + 1));
            }
        } else {
            // insert i in right
            // p: the position in right where we're inserting new key/value
            let p = i - mid;
            {
                // safety: we don't access left until after the slice is dropped
                let l_slice = unsafe { left.block.as_slice() };
                if p > 0 {
                    r_writer.slice(
                        right.key_offset(0)..right.key_offset(p),
                        &l_slice[left.key_offset(mid)..left.key_offset(i)]
                    );
                }
                r_writer.slice(
                    right.key_offset(p + 1)..right.key_offset(right.len),
                    &l_slice[left.key_offset(i)..left.key_offset(left.len)]
                );
            }
        }
        */

        // ...

        if height == 0 {
            // edge case: if height == 0, leaf is inside root_block, so we must move it out into
            // *another* new leaf, and the space within root_block will become an InternalNode
            todo!();
            return Ok(None);
        }

        // ... next, we need to update parent(s)
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
    // parent: BlockIndex,
    _p: PhantomData<K>
}

// these fields are encoded in the block buffer:
// (len: u32, keys: [u64; len], edges: [BlockIndex; len])
impl<K: FromBytes + ToBytes> InternalNode<K> {
    fn new(block: BlockBuffer, offset: usize) -> Self {
        let reader = block.reader();
        let mut ro = offset;
        let len = reader.at_and::<u32>(&mut ro) as usize;
        // let parent = reader.at(ro);
        drop(reader);
        Self {
            block,
            offset,
            len,
            // parent,
            _p: PhantomData,
        }
    }

    fn len_offset(&self) -> usize {
        self.offset
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
        (available + size_of::<K>()) / (size_of::<K>() + size_of::<BlockIndex>())
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
    // parent: BlockIndex,
    _p: PhantomData<(K, V)>,
}

// encoding in block:
// (len: u32, keys: [K; len], values: [V; len])
impl<K: FromBytes + ToBytes, V: FromBytes + ToBytes> LeafNode<K, V> {
    fn new(block: BlockBuffer, offset: usize) -> Self {
        let reader = block.reader();
        let mut ro = offset;
        let len = reader.at_and::<u32>(&mut ro) as usize;
        // let parent = reader.at(ro);
        drop(reader);
        Self {
            block,
            offset,
            len,
            // parent,
            _p: PhantomData,
        }
    }

    fn len_offset(&self) -> usize {
        self.offset
    }

    fn key_offset(&self, index: usize) -> usize {
        // self.offset + size_of::<u32>() + size_of::<BlockIndex>() + index * size_of::<K>()
        self.offset + size_of::<u32>() + index * size_of::<K>()
    }

    fn value_offset(&self, index: usize) -> usize {
        self.key_offset(self.len + 1) + index * size_of::<V>()
    }

    fn max_len(&self) -> usize {
        let available = self.block.buffer_len() - self.key_offset(0);
        available / (size_of::<K>() + size_of::<V>())
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

fn binary_search<K: Ord>(search_key: K, mut upper: usize, f: impl Fn(usize) -> K) -> Result<usize, usize> {
    let mut lower = 0;
    let mut index;
    let mut key;
    if loop {
        index = lower + (upper - lower) / 2;
        key = f(index);
        if lower >= index {
            break false;
        }
        match search_key.cmp(&key) {
            Ordering::Less => upper = index,
            Ordering::Equal => break true,
            Ordering::Greater => lower = index,
        }
    } {
        Err(index)
    } else {
        Ok(index)
    }
}
