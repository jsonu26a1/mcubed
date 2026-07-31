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
    // for this demo impl, construct a vec instead of a true iterator
    pub fn iter(&self) -> Vec<(u64, u64)> {
        let mut out = vec![];
        let mut some_leaf = Some(self.first_leaf.clone());
        while let Some(leaf) = some_leaf {
            out.extend(leaf.keys().iter().map(|k| *k).zip(leaf.values().iter().map(|v| *v)));
            some_leaf = leaf.next().clone();
        }
        out
    }
    pub fn iter_range(&self, range: impl RangeBounds<u64>) -> Vec<(u64, u64)> {
        let mut out = vec![];
        let mut next_leaf = match range.start_bound() {
            Bound::Unbounded => {
                out.extend(self.first_leaf.keys().iter().map(|k| *k).zip(self.first_leaf.values().iter().map(|v| *v)));
                self.first_leaf.next().clone()
            },
            Bound::Included(key) | Bound::Excluded(key) => {
                let mut height = 0;
                let mut cursor = self.root.clone();
                while height < self.height {
                    height += 1;
                    let internal = cursor.internal();
                    cursor =
                        internal.edges()[internal.keys().binary_search(key).map_or_else(|i| i, |i| i)].clone();
                }
                let leaf = cursor.leaf();
                let i = match leaf.keys().binary_search(key) {
                    Ok(i) => if let Bound::Excluded(_) = range.start_bound() {
                            i + 1
                        } else {
                            i
                        },
                    Err(i) => i,
                };
                if i < leaf.keys().len() {
                    out.extend(leaf.keys()[i..].iter().map(|k| *k).zip(leaf.values()[i..].iter().map(|v| *v)));
                }
                leaf.next().clone()
            },
        };
        let end_key = match range.end_bound() {
            Bound::Unbounded => None,
            Bound::Included(key) | Bound::Excluded(key) => Some(*key),
        };
        loop {
            let leaf = match next_leaf {
                Some(l) => l,
                None => break,
            };
            let leaf_keys = leaf.keys();
            if let Some(key) = end_key && *leaf_keys.last().unwrap() > key {
                let end = match leaf_keys.binary_search(&key) {
                    Ok(i) => if let Bound::Excluded(_) = range.end_bound() && key == leaf_keys[i] {
                            Bound::Excluded(i)
                        } else {
                            Bound::Included(i)
                        },
                    Err(i) => Bound::Excluded(i)
                };
                let r = (Bound::Unbounded, end);
                out.extend(leaf_keys[r].iter().map(|k| *k).zip(leaf.values()[r].iter().map(|v| *v)));
                break;
            } else {
                out.extend(leaf_keys[..].iter().map(|k| *k).zip(leaf.values()[..].iter().map(|v| *v)));
                next_leaf = leaf.next().clone();
            }
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

    // removes and returns Some(value) with the given key, otherwise None if key not found
    // currently doesn't work; I need to debugging this later
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
            Err(_) => return None,
        };
        self.len -= 1;
        let mut parents = parents.into_iter().rev();

        leaf.keys().remove(i);
        let value = leaf.values().remove(i);

        let min_items = (self.order / 2) as usize;
        let max_items = self.order as usize;

        if leaf.keys().len() >= min_items {
            if i == leaf.keys().len() {
                for (parent, i) in parents {
                    if i < parent.keys().len() {
                        parent.keys()[i] = *leaf.keys().last().unwrap();
                        // we can stop once we've found the key to update
                        break;
                    }
                }
            }
            return Some(value);
        }

        let (mut parent, mut i) = match parents.next() {
            Some(t) => t,
            None => {
                // no parent, means this leaf node is the root of the tree; it is permitted
                // to have fewer than min_items, since there are no sibling nodes to merge with
                return Some(value);
            }
        };

        let left_sibling = parent.edges().get(i.wrapping_sub(1)).map(|either| either.clone().leaf());
        let right_sibling = parent.edges().get(i + 1).map(|either| either.clone().leaf());
        enum Case {
            BalanceFromLeft,
            MergeWithLeft,
            BalanceFromRight,
            MergeWithRight,
            MergeThree,
        }
        let case: Case;
        if let Some(ref left_sibling) = left_sibling && let Some(ref right_sibling) = right_sibling {
            let current_len = leaf.keys().len();
            let left_len = left_sibling.keys().len();
            let right_len = right_sibling.keys().len();
            let left_available = max_items - left_len;
            let right_available = max_items - right_len;
            if left_available + right_available < current_len {
                if left_len > right_len {
                    // pop from end of left, insert at 0 in current
                    case = Case::BalanceFromLeft;
                } else {
                    // remove at 0 from right, push to end of current
                    case = Case::BalanceFromRight;
                }
            } else {
                // split current's items between left and right (and remove current from parent)
                case = Case::MergeThree;
            }
        } else if let Some(ref left_sibling) = left_sibling {
            // one of left or right can be None when either:
            // - parent internal node is root (tree height is 1)
            // - OR leaf is first or last node in parent
            case = if left_sibling.keys().len() > min_items {
                // pop from end of left, insert at 0 in current
                Case::BalanceFromLeft
            } else {
                Case::MergeWithLeft
            };
        } else if let Some(ref right_sibling) = right_sibling {
            case = if right_sibling.keys().len() > min_items {
                // remove at 0 from right, push to end of current
                Case::BalanceFromRight
            } else {
                Case::MergeWithRight
            };
        } else {
            // a non-root node should always have at least one sibling (left or right); an internal
            // node will merge with a sibling if it ever has less children than min_items, or if it
            // is the root node, replace itself with it's one remaining child.
            unreachable!();
        }
        match case {
            Case::BalanceFromLeft => {
                // pop from end of left, insert at 0 in current
                let left_sibling = left_sibling.unwrap();
                leaf.keys().insert(0, left_sibling.keys().pop().unwrap());
                leaf.values().insert(0, left_sibling.values().pop().unwrap());
                parent.keys()[i - 1] = *left_sibling.keys().last().unwrap();
                return Some(value);
            },
            Case::MergeWithLeft => {
                let left_sibling = left_sibling.unwrap();
                left_sibling.keys().extend(leaf.keys().drain(..));
                left_sibling.values().extend(leaf.values().drain(..));
                // remove the key associated with left_sibling, since it is now the last node
                parent.keys().remove(i - 1);
                parent.edges().remove(i);
                let next = leaf.next().clone();
                if let Some(ref next) = next {
                    *next.prev() = Some(left_sibling.clone());
                }
                *left_sibling.next() = next;
            },
            Case::BalanceFromRight => {
                // remove at 0 from right, push to end of current
                let right_sibling = right_sibling.unwrap();
                leaf.keys().push(right_sibling.keys().remove(0));
                leaf.values().push(right_sibling.values().remove(0));
                parent.keys()[i] = *leaf.keys().last().unwrap();
                return Some(value);
            },
            Case::MergeWithRight => {
                let right_sibling = right_sibling.unwrap();
                leaf.keys().extend(right_sibling.keys().drain(..));
                leaf.values().extend(right_sibling.values().drain(..));
                std::mem::swap(&mut *leaf.keys(), &mut *right_sibling.keys());
                std::mem::swap(&mut *leaf.values(), &mut *right_sibling.values());
                parent.keys().remove(i);
                parent.edges().remove(i);
                let prev = leaf.prev().clone();
                if let Some(ref prev) = prev {
                    *prev.next() = Some(right_sibling.clone());
                } else {
                    self.first_leaf = right_sibling.clone();
                }
                *right_sibling.prev() = prev;
            },
            Case::MergeThree => {
                // split current's items between left and right (and remove current from parent)
                let left_sibling = left_sibling.unwrap();
                let right_sibling = right_sibling.unwrap();
                let leaf_len = leaf.keys().len();
                let mid = std::cmp::min(max_items - left_sibling.keys().len(), leaf_len / 2);
                let mut rem_keys = leaf.keys().split_off(mid);
                let mut rem_values = leaf.values().split_off(mid);
                left_sibling.keys().extend(leaf.keys().drain(..));
                left_sibling.values().extend(leaf.values().drain(..));
                rem_keys.extend(right_sibling.keys().drain(..));
                rem_values.extend(right_sibling.values().drain(..));
                *right_sibling.keys() = rem_keys;
                *right_sibling.values() = rem_values;
                parent.keys()[i - 1] = *left_sibling.keys().last().unwrap();
                parent.keys().remove(i);
                parent.edges().remove(i);
                *left_sibling.next() = Some(right_sibling.clone());
                *right_sibling.prev() = Some(left_sibling.clone());
            },
        }

        // re-balance parent
        let mut current;
        loop {
            current = parent;
            if current.keys().len() >= min_items {
                break;
            }
            (parent, i) = match parents.next() {
                Some(t) => t,
                None => {
                    // `current` is the root node, which is permitted to be below min_items
                    if current.edges().len() == 1 {
                        self.root = current.edges().pop().unwrap().into();
                        self.height -= 1;
                    }
                    break;
                }
            };
            let left_sibling = parent.edges().get(i.wrapping_sub(1)).map(|either| either.clone().internal());
            let right_sibling = parent.edges().get(i + 1).map(|either| either.clone().internal());
            let case: Case;
            if let Some(ref left_sibling) = left_sibling && let Some(ref right_sibling) = right_sibling {
                let current_len = current.keys().len();
                let left_len = left_sibling.keys().len();
                let right_len = right_sibling.keys().len();
                let left_available = max_items - left_len;
                let right_available = max_items - right_len;
                if left_available + right_available < current_len {
                    if left_len > right_len {
                        case = Case::BalanceFromLeft;
                    } else {
                        case = Case::BalanceFromRight;
                    }
                } else {
                    case = Case::MergeThree;
                }
            } else if let Some(ref left_sibling) = left_sibling {
                case = if left_sibling.keys().len() > min_items {
                    Case::BalanceFromLeft
                } else {
                    Case::MergeWithLeft
                };
            } else if let Some(ref right_sibling) = right_sibling {
                case = if right_sibling.keys().len() > min_items {
                    Case::BalanceFromRight
                } else {
                    Case::MergeWithRight
                };
            } else {
                // a non-root node should always have at least one sibling (left or right); an internal
                // node will merge with a sibling if it ever has less children than min_items, or if it
                // is the root node, replace itself with it's one remaining child.
                unreachable!();
            }
            match case {
                Case::BalanceFromLeft => {
                    // pop from end of left, insert at 0 in current
                    let left_sibling = left_sibling.unwrap();
                    let left_key = left_sibling.keys().pop().unwrap();
                    current.keys().insert(0, left_sibling.largest_key_in_subtree());
                    current.edges().insert(0, left_sibling.edges().pop().unwrap());
                    parent.keys()[i - 1] = left_key;
                    break
                },
                Case::MergeWithLeft => {
                    let left_sibling = left_sibling.unwrap();
                    left_sibling.keys().push(left_sibling.largest_key_in_subtree());
                    left_sibling.keys().extend(current.keys().drain(..));
                    left_sibling.edges().extend(current.edges().drain(..));
                    // remove the key associated with left_sibling, since it is now the last node
                    parent.keys().remove(i - 1);
                    parent.edges().remove(i);
                },
                Case::BalanceFromRight => {
                    // remove at 0 from right, push to end of current
                    let right_sibling = right_sibling.unwrap();
                    let right_key = right_sibling.keys().remove(0);
                    current.keys().push(current.largest_key_in_subtree());
                    current.edges().push(right_sibling.edges().remove(0));
                    parent.keys()[i] = right_key;
                    break;
                },
                Case::MergeWithRight => {
                    let right_sibling = right_sibling.unwrap();
                    current.keys().push(current.largest_key_in_subtree());
                    current.keys().extend(right_sibling.keys().drain(..));
                    current.edges().extend(right_sibling.edges().drain(..));
                    std::mem::swap(&mut *current.keys(), &mut *right_sibling.keys());
                    std::mem::swap(&mut *current.edges(), &mut *right_sibling.edges());
                    parent.keys().remove(i);
                    parent.edges().remove(i);
                },
                Case::MergeThree => {
                    // split current's items between left and right (and remove current from parent)
                    let left_sibling = left_sibling.unwrap();
                    left_sibling.keys().push(left_sibling.largest_key_in_subtree());
                    let right_sibling = right_sibling.unwrap();
                    current.keys().push(current.largest_key_in_subtree());
                    let current_len = current.keys().len();
                    let mid = std::cmp::min(max_items - left_sibling.keys().len(), current_len / 2);
                    let mut rem_keys = current.keys().split_off(mid);
                    let mut rem_edges = current.edges().split_off(mid);
                    left_sibling.keys().extend(current.keys().drain(..));
                    let left_key = left_sibling.keys().pop().unwrap();
                    left_sibling.edges().extend(current.edges().drain(..));
                    std::mem::swap(&mut rem_keys, &mut *right_sibling.keys());
                    std::mem::swap(&mut rem_edges, &mut *right_sibling.edges());
                    right_sibling.keys().extend(rem_keys);
                    right_sibling.edges().extend(rem_edges);
                    parent.keys()[i - 1] = left_key;
                    parent.keys().remove(i);
                    parent.edges().remove(i);
                },
            }
        }
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

    pub fn largest_key_in_subtree(&self) -> u64 {
        let mut last_edge = self.edges().last().unwrap().clone();
        loop {
            match last_edge {
                EitherNode::Internal(internal) => {
                    last_edge = internal.edges().last().unwrap().clone();
                },
                EitherNode::Leaf(leaf) => return *leaf.keys().last().unwrap(),
            }
        }
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