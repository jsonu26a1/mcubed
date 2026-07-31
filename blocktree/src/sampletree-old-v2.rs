use std::cell::{RefCell, RefMut};
use std::rc::Rc;
use std::ops::{RangeBounds, Bound};

// a sample B+Tree, implemented only in memory to experiment with algorithms and help with debugging.

pub struct SampleTree {
    pub order: u32,
    // pub minimum: u32,
    pub height: u32,
    pub len: u64,
    pub first_leaf: Rc<LeafNode>,
    pub root: EitherNode,
    pub next_id: i32,
}

/*
SampleTree is technically a "B+ Tree"; internal nodes do not store values, the keys between edges
correspond to the last value of the left-edge. however, I plan on updating it to be a "B*+ Tree",
which means the minimum number of items per node is configurable (maybe 2/3 of `order`), to reduce
the amount of free slots in nodes, which is "being wasted".

hmmm, I'm not sure if we want to use `B*+ Tree` or a regular `B+ Tree`. the trade off is that
insertions might require updating more blocks. maybe it turns out that that isn't desirable? I
guess for now, I will juse assume `minimum == order / 2`, like how insert() is currently
implemented. I'll have to write a new implementation later to compare the two.
*/

// `order` is the maximum number of items per node (edges per internal node, values per leaf node)
// `minimum` is the minimum number of items per node
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
    // TODO: `minimum` isn't supported at the moment; insert() assumes it will be `order / 2`
    // pub fn new(order: u32) -> Self {
    //     Self::new_ext(order, order / 2)
    // }
    // pub fn new_ext(order: u32, minimum: u32) -> Self {
    //     let leaf: Rc<LeafNode> = LeafNode::new(0).into();
    //     Self {
    //         order,
    //         minimum,
    //         height: 0,
    //         len: 0,
    //         first_leaf: leaf.clone(),
    //         root: leaf.into(),
    //         next_id: 1,
    //     }
    // }
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
        // TODO: this doesn't work; we currently don't correctly set leaf.next()/.prev()
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
    // TODO: insert() assumes `minimum` is `order / 2`; this needs to be updated for `minimum > order / 2`,
    // where we spill over excess keys into an adjacent sibling...
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

    /*
    we need to fix the algorithm from `remove_old1`; it has a bug: 
        Error: "removal index (is 0) should be < len (is 0)"
        in case 2, (first one, for leaf nodes)
        `let key = leaf.keys().remove(0);`
    the algorithm move a single item 
    */
    pub fn remove(&mut self, key: u64) -> Option<u64> {

    // }

    // removes a value with the given key, returning Some(value) if existed and was removed, otherwise None
    // pub fn remove_old1(&mut self, key: u64) -> Option<u64> {
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
            Err(_) => return None,
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
            // note: if leaf.keys() is empty, then it must be the root of the now empty tree
            leaf.keys().last().map(|k| *k)
        } else {
            None
        };

        let min_items = (self.order / 2) as usize;

        if leaf.keys().len() >= min_items {
            if let Some(new_key) = leaf_new_last_key {
                for (parent, i) in parents {
                    if i < parent.keys().len() {
                        parent.keys()[i] = new_key;
                        // we can stop once we've found the key to update
                        break;
                    }
                }
            }
            return Some(value);
        }

        // algorithm mentioned on wikipedia has 3 cases:
        //   1) rotate left (right sibling into current node)
        //   2) rotate right (left sibling into current node)
        //   3) merge current node with a sibling (parent must be re-balanced)
        // in all cases, the parent must be updated: fix keys between any modified edges

        // cases 1 and 2 do not require walking up the tree to re-balance any internal nodes.
        // in addition, we only ever need to update the immediate parent's key;

        // TODO: I think we could do a "combined" implementation here? where we share some
        // some of the logic for deciding which case? maybe I'll look into this later...

        // pop parent node from vec;
        let (mut parent, mut i) = match parents.next() {
            Some(t) => t,
            None => {
                // no parent node, means this leaf node is the tree root; nothing to merge

                // TODO: we still need to remove `leaf` from the parent if it is empty; we could
                // also still attempt to merge with a sibling, but a separate merge algorithm
                // would need to be used, since a leaf may be left with fewer than the minimum of
                // `order / 2` items.
                todo!();

                return Some(value);
            }
        };

        // at this point: `leaf` (the current node) has fewer than `minimum` items, so we need
        // to take an item from one of it's siblings
        let left_sibling = parent.edges().get(i.wrapping_sub(1)).map(|either| either.clone().leaf());
        let right_sibling = parent.edges().get(i + 1).map(|either| either.clone().leaf());

        // hmmm... we check if right_sibling has MORE than min_items... but we should be checking
        // if it has LESS than max items, right?
        if let Some(ref right_sibling) = right_sibling && right_sibling.keys().len() > min_items {
            // case 1)
            let key = right_sibling.keys().remove(0);
            leaf.keys().push(key);
            leaf.values().push(right_sibling.values().remove(0));
            // now we need to correct the parent's key at index `i`;
            parent.keys()[i] = key;
            return Some(value);
        }
        else if let Some(ref left_sibling) = left_sibling && left_sibling.keys().len() > min_items {
            // case 2)
            let key = left_sibling.keys().pop().unwrap();
            leaf.keys().insert(0, key);
            leaf.values().insert(0, left_sibling.values().pop().unwrap());
            /*
            // this is wrong; we were supposed to move from left to leaf;
            // but this moves from leaf to left;
            let key = leaf.keys().remove(0);
            left_sibling.keys().push(key);
            left_sibling.values().push(leaf.values().remove(0));
            */
            // now we need to correct the parent's key at index `i - 1`;
            parent.keys()[i - 1] = key;
            return Some(value);
        }
        else {
            // case 3)
            // either:
            //   3.1) one of left and right are None
            //   OR
            //   3.2) each left's and right's length == `order / 2`
            // if case 3.2, we simply place half of `leaf`'s items in left and half in right.
            //
            // if case 3.1, we will need to redefine left/right sibling, to look at the next
            // adjacent sibling of the left/right (which ever is non-None).
            if parent.edges().len() < 3 {
                // 
                return Some(value);
            }
            let (left, mid, right, mid_index);
            if left_sibling.is_none() {
                left = leaf;
                mid = right_sibling.unwrap();
                // unwrap() might fail; we return early above if the parent is the root node.
                // however, if `order / 2 < 3`, then an internal node might only have 2 leaf
                // nodes. so only `order >= 6` is "valid" for this algorithm.
                right = parent.edges().get(i + 2).unwrap().clone().leaf();
                mid_index = i + 1;
            } else if right_sibling.is_none() {
                // see above note about when unwrap() might fail
                left = parent.edges().get(i - 2).unwrap().clone().leaf();
                mid = left_sibling.unwrap();
                right = leaf;
                mid_index = i - 1;
            } else {
                left = left_sibling.unwrap();
                mid = leaf;
                right = right_sibling.unwrap();
                mid_index = i;
            }
            let split_at = mid.keys().len() / 2;
            let mut rem_keys = mid.keys().split_off(split_at);
            let mut rem_values = mid.values().split_off(split_at);
            left.keys().extend(mid.keys().drain(..));
            left.values().extend(mid.values().drain(..));
            rem_keys.extend(right.keys().drain(..));
            rem_values.extend(right.values().drain(..));
            *right.keys() = rem_keys;
            *right.values() = rem_values;
            parent.keys()[mid_index - 1] = *left.keys().last().unwrap();
            parent.keys().remove(mid_index);
            parent.edges().remove(mid_index);
            *left.next() = Some(right.clone());
            *right.prev() = Some(left.clone());
        }

        // TODO: remove this, this check moved into the loop below
        /*
        // since we've removed an item from the parent (internal) node, we now need to
        // check if we need to re-balance the parent with it's siblings, etc.
        if parent.keys().len() >= min_items {
            // TODO: double check if this is right; I think we just return here; parent's keys
            // are already updated at the end of case 3
            return Some(value);
        }
        */

        let mut current;
        loop {
            current = parent;
            // check if `current` needs to be re-balanced
            if current.keys().len() >= min_items {
                // TODO: double check if this break/return is correct...
                break;
            }
            (parent, i) = match parents.next() {
                Some(t) => t,
                None => {
                    // `current` is the root node, which is allowed to be un-balanced
                    break;
                }
            };
            // we break on cases 1 and 2, because we didn't remove a node from the parent
            // but on case 3, we continue walking up the tree
            let left_sibling = parent.edges().get(i.wrapping_sub(1)).map(|either| either.clone().internal());
            let right_sibling = parent.edges().get(i + 1).map(|either| either.clone().internal());
            if let Some(ref right_sibling) = right_sibling && right_sibling.keys().len() > min_items {
                // case 1)
                let key = right_sibling.keys().remove(0);
                current.keys().push(key);
                current.edges().push(right_sibling.edges().remove(0));
                parent.keys()[i] = key;
                break;
            }
            else if let Some(ref left_sibling) = left_sibling && left_sibling.keys().len() > min_items {
                // case 2)
                let key = left_sibling.keys().pop().unwrap();
                current.keys().insert(0, key);
                current.edges().insert(0, left_sibling.edges().pop().unwrap());
                // let key = current.keys().remove(0);
                // left_sibling.keys().push(key);
                // left_sibling.edges().push(current.edges().remove(0));
                parent.keys()[i - 1] = key;
                break;
            } else {
                // case 3)
                // ...
                let (left, mid, right, mid_index);
                if left_sibling.is_none() {
                    left = current;
                    mid = right_sibling.unwrap();
                    right = parent.edges().get(i + 2).unwrap().clone().internal();
                    mid_index = i + 1;
                } else if right_sibling.is_none() {
                    left = parent.edges().get(i - 2).unwrap().clone().internal();
                    mid = left_sibling.unwrap();
                    right = current;
                    mid_index = i - 1;
                } else {
                    left = left_sibling.unwrap();
                    mid = current;
                    right = right_sibling.unwrap();
                    mid_index = i;
                }
                let split_at = mid.keys().len() / 2;
                let mut rem_keys = mid.keys().split_off(split_at);
                let mut rem_edges = mid.edges().split_off(split_at);
                left.keys().extend(mid.keys().drain(..));
                left.edges().extend(mid.edges().drain(..));
                rem_keys.extend(right.keys().drain(..));
                rem_edges.extend(right.edges().drain(..));
                *right.keys() = rem_keys;
                *right.edges() = rem_edges;
                parent.keys()[mid_index - 1] = *left.keys().last().unwrap();
                parent.keys().remove(mid_index);
                parent.edges().remove(mid_index);
            }
        }
        return Some(value);

        // this implementation below isn't used (and it might be wrong or bad)
        /*
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
        */
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