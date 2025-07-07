mod file_memory;
mod in_memory;
mod r#trait;

pub use file_memory::FileMemory;
pub use in_memory::InMemoryMemory;
pub use r#trait::{Memory, MemoryUpdate};
