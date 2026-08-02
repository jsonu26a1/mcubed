pub mod backend;
pub mod sampletree;
pub mod manager;
pub mod btree;

// use backend::IoBackend;

pub type BlockIndex = u64;
pub const BLOCK_INDEX_SIZE: usize = std::mem::size_of::<BlockIndex>();
pub const BLOCK_SIZE: usize = 1024 * 4;
