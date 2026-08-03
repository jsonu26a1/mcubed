pub mod backend;
pub mod sampletree;
pub mod manager;
pub mod btree;
// #[path = "block-alt.rs"]
pub mod block;

pub use block::{BlockBuffer, BlockReader, BlockWriter};
pub use manager::{BlockManager, WeakBlockManager, FromToBytes};
pub use backend::IoBackend;

pub type BlockIndex = u64;
pub const BLOCK_INDEX_SIZE: usize = std::mem::size_of::<BlockIndex>();
pub const BLOCK_SIZE: usize = 1024 * 4;
