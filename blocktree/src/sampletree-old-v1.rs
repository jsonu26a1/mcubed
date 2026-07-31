use std::cell::{RefCell, RefMut};
use std::rc::Rc;
use std::ops::{RangeBounds, Bound};

// a sample B+Tree, implemented only in memory to experiment with algorithms and help with debugging.

pub struct SampleTree {
    pub order: u32,
    pub height: u32,
    pub len: u64,
    pub first_leaf: Rc<LeafNode>,
    pub root: EitherNode,
    pub next_id: i32,
}

impl SampleTree {
    pub fn new(order: u32) -> Self {
        let leaf: Rc<LeafNode> = LeafNode::new(0).into();
        Self {
            order,
            height: 0,
            len: 0,
            first_leaf: leaf.clone(),
            root: leaf.into(),
            next_id: 1,
        }
    }
    fn new_internal(&mut self) -> InternalNode {
        let node = InternalNode::new(self.next_id);
        self.next_id += 1;
        node
    }
    fn new_leaf(&mut self) -> LeafNode {
        let node = LeafNode::new(self.next_id);
        self.next_id += 1;
        node
    }
    pub fn iter(&self) -> Vec<(u64, u64)> {
        let mut out = vec![];
        let mut some_leaf = Some(self.first_leaf.clone());
        while let Some(leaf) = some_leaf {
            out.extend(leaf.keys().iter().map(|k| *k).zip(leaf.values().iter().map(|v| *v)));
            some_leaf = leaf.next().clone();
        }
        out
    }
    pub fn get(&self, key: u64) -> Option<u64> {
        let mut value = None;
        self.get_mut(key, |v| value = v.map(|r| *r));
        value
    }
    pub fn get_mut(&self, key: u64, f: impl FnOnce(Option<&mut u64>)) {
        let mut height = 0;
        let mut cursor = self.root.clone();
        while height < self.height {
            height += 1;
            let internal = cursor.internal();
            cursor =
                internal.edges()[internal.keys().binary_search(&key).map_or_else(|i| i, |i| i)].clone();
        }
        let leaf = cursor.leaf();
        return match leaf.keys().binary_search(&key) {
            // Ok(i) => Ok(&mut leaf.values()[i]),
            // Err(i) => Err((leaf, i)),
            Ok(i) => f(Some(&mut leaf.values()[i])),
            Err(_) => f(None),
        };
    }
    pub fn update_or_insert(&mut self, key: u64, update: impl FnOnce(&mut u64), insert: impl FnOnce() -> u64) {
        let mut was_updated = true;
        self.get_mut(key, |v| match v {
            Some(v) => update(v),
            None => was_updated = false,
        });
        if !was_updated {
            self.insert(key, insert());
        }
    }
    // inserts key-value, and returns Some(n) if the key already existed, otherwise None
    pub fn insert(&mut self, key: u64, value: u64) -> Option<u64> {
        let mut height = 0;
        let mut cursor = self.root.clone();
        let mut parents = vec![];
        while height < self.height {
            height += 1;
            let internal = cursor.internal();
            let i = internal.keys().binary_search(&key).map_or_else(|i| i, |i| i);
            cursor = internal.edges()[i].clone();
            parents.push((internal, i));
        }
        let leaf = cursor.leaf();
        let i = match leaf.keys().binary_search(&key) {
            Ok(i) => {
                let prev = leaf.values()[i];
                leaf.values()[i] = value;
                // replace existing value
                return Some(prev);
            },
            Err(i) => i,
        };
        self.len += 1;
        let mut parents = parents.into_iter().rev();

        // `key` must propagate upwards, updating 
        let inserted_at_child_end = i == leaf.keys().len();
        leaf.keys().insert(i, key);
        leaf.values().insert(i, value);
        // if leaf doesn't need to be split
        if leaf.values().len() < self.order as usize {
            if inserted_at_child_end {
                // we need to update parent's key
                for (parent, i) in parents {
                    if i < parent.keys().len() {
                        // update key and exit loop
                        parent.keys()[i] = key;
                        break;
                    }
                }
            }
            return None;
        }

        // split leaf node into (left, right)
        let new_leaf = Rc::new(self.new_leaf());
        *new_leaf.prev() = Some(leaf.clone());
        *new_leaf.next() = leaf.next().clone();
        *leaf.next() = Some(new_leaf.clone());
        let mid = leaf.values().len() / 2;
        *new_leaf.keys() = leaf.keys().split_off(mid);
        *new_leaf.values() = leaf.values().split_off(mid);
        // right-most key of new left-edge
        let mut left_key = *leaf.keys().last().unwrap();
        // right-most key of new right-edge
        let mut right_key = Some(*new_leaf.keys().last().unwrap());
        let mut edges: Option<(EitherNode, EitherNode)> = Some((leaf.into(), new_leaf.into()));

        // can't use `for` loop syntax; if no more parent nodes, we update root node slot
        loop {
            let (parent, i) = match parents.next() {
                Some(t) => t,
                None => {
                    if let Some((left_edge, right_edge)) = edges {
                        self.height += 1;
                        let parent = self.new_internal();
                        parent.keys().push(left_key);
                        parent.edges().extend([left_edge, right_edge]);
                        self.root = parent.into();
                    }
                    break;
                }
            };
            if let Some((_, right_edge)) = edges {
                let inserted_at_parent_end = i == parent.keys().len();
                parent.edges().insert(i + 1, right_edge);
                if !inserted_at_parent_end && let Some(some_key) = right_key {
                    parent.keys()[i] = left_key;
                    parent.keys().insert(i + 1, some_key);
                    right_key = None;
                } else {
                    parent.keys().insert(i, left_key);
                }
                if parent.edges().len() >= self.order as usize {
                    // parent needs to be split
                    let new_internal = self.new_internal();
                    let mid = parent.edges().len() / 2;
                    *new_internal.keys() = parent.keys().split_off(mid);
                    *new_internal.edges() = parent.edges().split_off(mid);
                    left_key = parent.keys().pop().unwrap();
                    edges = Some((parent.into(), new_internal.into()));
                } else {
                    edges = None;
                }
            } else if inserted_at_child_end && let Some(right_key) = right_key && i < parent.keys().len() {
                parent.keys()[i] = right_key;
                break;
            }
        }
        return None;
    }

    // removes a value with the given key, returning Some(value) if existed and was removed, otherwise None
    pub fn remove(&mut self, key: u64) -> Option<u64> {
        let mut height = 0;
        let mut cursor = self.root.clone();
        let mut parents = vec![];
        while height < self.height {
            height += 1;
            let internal = cursor.internal();
            let i = internal.keys().binary_search(&key).map_or_else(|i| i, |i| i);
            cursor = internal.edges()[i].clone();
            parents.push((internal, i));
        }
        let leaf = cursor.leaf();
        let i = match leaf.keys().binary_search(&key) {
            Ok(i) => i,
            Err(_) => return None;
        };
        self.len -= 1;
        let mut parents = parents.into_iter().rev();

        // some edge cases; if i == leaf.keys.len() - 1, then we need to update parent's key (if
        // right most, then bubble up)
        // if leaf.keys.len() < lower (order / 2 - 1?), then we need to merge with adjacent node
        // hmm, picturing this gets complicated... but there's a lot of states which appear
        // problematic at first, but are actually unreachable. a node will never have less than 
        // `lower`, unless the tree is almost empty, but then, that would be a leaf root node;
        // internal nodes will never only have a single edge on them...


        leaf.keys().remove(i);
        let value = leaf.values().remove(i);

        let mut leaf_new_last_key = if i == leaf.keys().len() {
            // TODO: edge case, what if we removed last key, and leaf is now empty?
            // leaf_new_last_key will be None, how will this be handled below?
            // oops, this state should be unreachable; leaf would be merged before it's len
            // ever reaches 0 (unless it is the root node, in which case this being None doesn't
            // matter)

            // note: if leaf.keys() is empty, then it must be the root of the now empty tree
            leaf.keys().last()
        } else {
            None
        };

        if leaf.keys().len() >= self.order / 2 as usize {
            if let Some(new_key) = leaf_new_last_key {
                for (parent, i) in parents {
                    if i < parent.keys().len() {
                        parent.keys()[i] = new_key;
                    }
                }
            }
            return Some(value);
        }

        // merge leaf with adjacent leaf
        let (parent, i) = match parents.next() {
            Some(t) => t,
            None => {
                // no parent node, means this leaf node is the tree root; nothing to merge
                return Some(value);
            }
        };

        let (left_leaf, right_leaf, right_into_left) = if i == parent.keys().len() && i > 0 {
            // right most edge; must merge with leaf node to the left
            (parent.edges()[i - 1].clone().leaf(), leaf, true)
        } else {
            // merge with leaf node to the right
            (leaf, parent.edges()[i + 1].clone().leaf(), false)
        };


        let left_len = left_leaf.keys().len();
        let right_len = right_leaf.keys().len();
        let total_len = left_len + right_len;
        if total_len > self.order as usize {
            // no room to merge, so balance elements between both nodes
            let lk = left_leaf.keys();
            let lv = left_leaf.values();
            let rk = right_leaf.keys();
            let rv = right_leaf.values();
            let mid = total_len / 2;
            // let mut left_leaf_last_key = None;
            let mut right_leaf_last_key = None;
            // index of left_leaf within `parent`;
            let mut j = i;
            if mid > left_len {
                // move items from front of right_leaf to end of left_leaf
                let end = mid - left_len;
                for i in 0..end {
                    lk.push(rk[i]);
                    rk[i] = rk[i + end];
                    lv.push(rv[i]);
                    rv[i] = rv[i + end];
                }
                rk.truncate(right_len - end);
                rv.truncate(right_len - end);
                // left_leaf had new elements pushed onto end, so this will be Some() regardless
                // of what `leaf_new_last_key` was.
                // left_leaf_last_key = lk.last();
                // TODO right_leaf last key should remain unchanged here, right?
                // right_leaf_last_key = rk.last();
            } else {
                // move items from end of left_leaf to front of right_leaf
                j -= 1;
                let new_len = total_len - mid;
                rk.resize(new_len, 0);
                rv.resize(new_len, 0);
                for j in 0..right_len {
                    let i = right_len - j;
                    rk[new_len - j - 1] = rk[i];
                    rv[new_len - j - 1] = rv[i];
                }
                for i in 0..new_len - right_len {
                    rk[i] = lk[mid + i];
                    rv[i] = lv[mid + i];
                }
                lk.truncate(mid);
                lv.truncate(mid);
                // leaf_leaf had elements popped from end, so last key has changed
                // left_leaf_last_key = lk.last().unwrap();
                // end of right_leaf didn't change here, so we propagate `leaf_new_last_key`
                right_leaf_last_key = leaf_new_last_key;
            }
            // the last key of left_leaf will always need to be updated.
            let left_leaf_last_key = lk.last().unwrap();
            let pk = parent.keys();
            pk[j] = left_leaf_last_key;
            if let Some(rlk) = right_leaf_last_key {
                if pk.len() > j + 1 {
                    pk[j + 1] = rlk;
                } else {
                    loop {
                        let (parent, i) = match parents.next() {
                            Some(t) => t,
                            None => {
                                // we've reached the root node, nothing to update
                                break;
                            }
                        };
                        if i < parent.keys().len() {
                            // update edge key in parent node and exit loop
                            parent.keys()[i] = rlk;
                            break;
                        }
                        // node is the last edge, no key to update, continue to next parent node
                    }
                }
            }
            // I think the below TODO is resolved now?

            // TODO before we return, we still need to update parent (if leaf_new_last_key == true)
            // so, `leaf_new_last_key` could refer to either the left_leaf or right_leaf.
            // also, we will need to iterate over `parents` to resolve this.
            // in addition, balancing elements in this branch means we must fix keys for BOTH
            // left_leaf and right_leaf, in `parents`.
            return Some(value);
        }

        left_leaf.keys().extend(right_leaf.keys());
        left_leaf.values().extend(right_leaf.values());
        parent.keys().remove(i);
        parent.edges().remove(i);
        if !right_into_left {
            // swap vecs between left and right leafs, since we're removing left_leaf
            std::mem::swap(&mut *left_leaf.keys(), &mut *right_leaf.keys());
            std::mem::swap(&mut *left_leaf.values(), &mut *right_leaf.values());
        }
        // hmmm, now that the parent (internal node) has had a removal, we need to check if
        // balancing or merging is needed on that node.

        // TODO what's next?

        if right_into_left {
            // ...
            // right node must now be removed.
        } else {
            // ...
            // left node must be removed.
        }

        todo!();
        return Some(value);
    }
}

pub enum EitherNode {
    Internal(Rc<InternalNode>),
    Leaf(Rc<LeafNode>),
}

impl EitherNode {
    pub fn internal(self) -> Rc<InternalNode> {
        match self {
            Self::Internal(n) => n,
            Self::Leaf(_) => {
                panic!("EitherNode access failed: expected InternalNode, found LeafNode")
            }
        }
    }
    pub fn leaf(self) -> Rc<LeafNode> {
        match self {
            Self::Internal(_) => {
                panic!("EitherNode access failed: expected LeafNode, found InternalNode")
            }
            Self::Leaf(n) => n,
        }
    }
    pub fn last_key(&self) -> u64 {
        match self {
            Self::Internal(n) => *n.keys().last().unwrap(),
            Self::Leaf(n) => *n.keys().last().unwrap(),
        }
    }
    pub fn id(&self) -> i32 {
        match self {
            Self::Internal(n) => n.id,
            Self::Leaf(n) => n.id,
        }
    }
}

impl From<InternalNode> for EitherNode {
    fn from(n: InternalNode) -> Self {
        Self::Internal(Rc::new(n))
    }
}

impl From<Rc<InternalNode>> for EitherNode {
    fn from(n: Rc<InternalNode>) -> Self {
        Self::Internal(n)
    }
}

impl From<LeafNode> for EitherNode {
    fn from(n: LeafNode) -> Self {
        Self::Leaf(Rc::new(n))
    }
}

impl From<Rc<LeafNode>> for EitherNode {
    fn from(n: Rc<LeafNode>) -> Self {
        Self::Leaf(n)
    }
}

impl Clone for EitherNode {
    fn clone(&self) -> Self {
        match self {
            Self::Internal(n) => Self::Internal(n.clone()),
            Self::Leaf(n) => Self::Leaf(n.clone()),
        }
    }
}

/*
suppose a node looks like:
    keys: [5, 10, 15]
    edges: [_0, _1, _2, _3]
interlaced:
    [_, 5, _, 10, _, 15, _]
where "_" is an edge.
the keys correspond to the last key in each child edge
so the key "5" will be contained in edge _0

*/



pub struct InternalNode {
    pub id: i32,
    pub keys: RefCell<Vec<u64>>,
    pub edges: RefCell<Vec<EitherNode>>,
}

impl InternalNode {
    pub fn new(id: i32) -> Self {
        Self {
            id,
            keys: RefCell::new(vec![]),
            edges: RefCell::new(vec![]),
        }
    }

    pub fn keys(&self) -> RefMut<'_, Vec<u64>> {
        self.keys.borrow_mut()
    }

    pub fn edges(&self) -> RefMut<'_, Vec<EitherNode>> {
        self.edges.borrow_mut()
    }
}

pub struct LeafNode {
    pub id: i32,
    pub keys: RefCell<Vec<u64>>,
    pub values: RefCell<Vec<u64>>,
    pub prev: RefCell<Option<Rc<LeafNode>>>,
    pub next: RefCell<Option<Rc<LeafNode>>>,
}

impl LeafNode {
    pub fn new(id: i32) -> Self {
        Self {
            id,
            keys: vec![].into(),
            values: vec![].into(),
            prev: None.into(),
            next: None.into(),
        }
    }

    pub fn keys(&self) -> RefMut<'_, Vec<u64>> {
        self.keys.borrow_mut()
    }

    pub fn values(&self) -> RefMut<'_, Vec<u64>> {
        self.values.borrow_mut()
    }

    pub fn prev(&self) -> RefMut<'_, Option<Rc<LeafNode>>> {
        self.prev.borrow_mut()
    }

    pub fn next(&self) -> RefMut<'_, Option<Rc<LeafNode>>> {
        self.next.borrow_mut()
    }
}

pub fn debug_print_tree(sample: &SampleTree, height_range: impl RangeBounds<u32>, only_id: Option<i32>) {
    let mut lower = match height_range.start_bound() {
        Bound::Included(n) => *n,
        _ => 0,
    };
    let upper = match height_range.end_bound() {
        Bound::Included(n) => n + 1,
        Bound::Excluded(n) => *n,
        Bound::Unbounded => u32::MAX,
    };
    let mut tree: Vec<Vec<EitherNode>> = vec![];
    let mut height = 0;
    tree.push(vec![sample.root.clone()]);
    loop {
        if height == upper {
            break;
        }
        let h = height as usize;
        height += 1;
        let mut i = 0;
        let mut next_nodes = vec![];
        while i < tree[h].len() {
            let node = tree[h][i].clone();
            i += 1;
            if let Some(only_id) = only_id && node.id() == only_id {
                tree[h] = vec![node.clone()];
                next_nodes = vec![];
                lower = h as u32;
            }
            if let EitherNode::Internal(internal) = node {
                for edge in &*internal.edges() {
                    next_nodes.push(edge.clone());
                }
            }
        }
        if next_nodes.is_empty() {
            break;
        } else {
            tree.push(next_nodes);
        }
    }
    let mut height = lower;
    let sp = "    ";
    println!("tree: order {}, height {}, len {}", sample.order, sample.height, sample.len);
    for layer in &tree[lower as usize..(upper as usize).min(tree.len())] {
        if height == upper {
            break;
        }
        println!("(height {height})");
        height += 1;
        for node in layer {
            match node {
                EitherNode::Internal(n) => {
                    println!("{sp}#{} internal: keys {}, edges {}", node.id(), n.keys().len(), n.edges().len());
                    println!("{sp}{sp}keys{:?}", n.keys());
                    println!("{sp}{sp}edges{:?}", n.edges().iter().map(|e| e.id()).collect::<Vec<i32>>());
                },
                EitherNode::Leaf(n) => {
                    println!("{sp}#{} leaf: keys {}, values {}", node.id(), n.keys().len(), n.values().len());
                    println!("{sp}{sp}keys{:?}", n.keys());
                    println!("{sp}{sp}values{:?}", n.values());
                },
            };
        }
    }
}