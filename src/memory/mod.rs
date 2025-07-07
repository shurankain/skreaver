mod r#trait;
mod in_memory;
mod file_memory;

pub use r#trait::{Memory, MemoryUpdate};
pub use in_memory::InMemoryMemory;
pub use file_memory::FileMemory;
